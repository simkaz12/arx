use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum TraceTurn {
    Text(String),
    ToolCall { name: String, args: String },
    ToolResult(String),
    Diff(String),
}

pub fn read_trace(trace_dir: &Path, body: &str) -> Result<Vec<TraceTurn>, String> {
    let transcript = match body {
        "fake" => read_fake(trace_dir),
        "claude-code" => read_json_lines(trace_dir, "session.jsonl", claude_row_turns),
        "codex" => read_json_lines(trace_dir, "session.jsonl", codex_row_turns),
        "native" => read_json_lines(trace_dir, "transcript.jsonl", native_row_turns),
        other => Err(format!("unknown body kind: {other}")),
    };
    let diff = read_diff(trace_dir);
    match (transcript, diff) {
        (Ok(mut turns), Some(diff)) => {
            turns.push(TraceTurn::Diff(diff));
            Ok(turns)
        }
        (Ok(turns), None) => Ok(turns),
        (Err(err), Some(diff)) => Ok(vec![TraceTurn::Text(format!("trace unreadable: {err}")), TraceTurn::Diff(diff)]),
        (Err(err), None) => Err(err),
    }
}

fn read_diff(trace_dir: &Path) -> Option<String> {
    let path = trace_dir.join("worktree.diff");
    let bytes = fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))
}

fn read_fake(trace_dir: &Path) -> Result<Vec<TraceTurn>, String> {
    let path = trace_dir.join("out/stdout.log");
    let text = read_to_string(&path)?;
    Ok(text.lines().filter(|line| !line.is_empty()).map(|line| TraceTurn::Text(line.to_string())).collect())
}

fn read_json_lines(
    trace_dir: &Path,
    file_name: &str,
    row_turns: impl Fn(&serde_json::Value, &mut Vec<TraceTurn>),
) -> Result<Vec<TraceTurn>, String> {
    let path = trace_dir.join(file_name);
    let text = read_to_string(&path)?;
    let mut turns = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row = serde_json::from_str::<serde_json::Value>(line)
            .map_err(|err| format!("{}:{}: {err}", path.display(), number + 1))?;
        row_turns(&row, &mut turns);
    }
    Ok(turns)
}

fn short_args(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    let parsed = match value {
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text).unwrap_or_else(|_| value.clone()),
        other => other.clone(),
    };
    if let Some(obj) = parsed.as_object() {
        if let Some(command) = obj.get("command").and_then(|v| v.as_str()) {
            return command.to_string();
        }
        if let Some(path) = obj.get("path").or_else(|| obj.get("file_path")).and_then(|v| v.as_str()) {
            return path.to_string();
        }
    }
    parsed.to_string().lines().next().unwrap_or("").to_string()
}

fn text_or_join(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Array(items)) => {
            items.iter().filter_map(|item| item.get("text").and_then(|v| v.as_str())).collect::<Vec<_>>().join("\n")
        }
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn claude_row_turns(row: &serde_json::Value, out: &mut Vec<TraceTurn>) {
    let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if row_type != "user" && row_type != "assistant" {
        return;
    }
    let Some(content) = row.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array()) else {
        return;
    };
    for block in content {
        match block.get("type").and_then(|v| v.as_str()).unwrap_or("") {
            "text" => {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    out.push(TraceTurn::Text(text.to_string()));
                }
            }
            "tool_use" => {
                let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("tool").to_string();
                out.push(TraceTurn::ToolCall { name, args: short_args(block.get("input")) });
            }
            "tool_result" => {
                out.push(TraceTurn::ToolResult(text_or_join(block.get("content"))));
            }
            _ => {}
        }
    }
}

fn codex_row_turns(row: &serde_json::Value, out: &mut Vec<TraceTurn>) {
    let row_type = row.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if row_type != "response_item" {
        return;
    }
    let Some(payload) = row.get("payload") else {
        return;
    };
    match payload.get("type").and_then(|v| v.as_str()).unwrap_or("") {
        "message" => {
            let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role == "system" || role == "developer" {
                return;
            }
            let Some(content) = payload.get("content").and_then(|c| c.as_array()) else {
                return;
            };
            for block in content {
                let text = match block {
                    serde_json::Value::String(text) => Some(text.clone()),
                    serde_json::Value::Object(_) => block.get("text").and_then(|v| v.as_str()).map(str::to_string),
                    _ => None,
                };
                if let Some(text) = text {
                    out.push(TraceTurn::Text(text));
                }
            }
        }
        "function_call" => {
            let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("tool").to_string();
            out.push(TraceTurn::ToolCall { name, args: short_args(payload.get("arguments")) });
        }
        "function_call_output" => {
            let output = match payload.get("output") {
                Some(serde_json::Value::String(text)) => text.clone(),
                Some(other) => other.to_string(),
                None => String::new(),
            };
            out.push(TraceTurn::ToolResult(output));
        }
        _ => {}
    }
}

fn native_row_turns(row: &serde_json::Value, out: &mut Vec<TraceTurn>) {
    match row.get("kind").and_then(|v| v.as_str()).unwrap_or("") {
        "assistant" => {
            let Some(blocks) = row.get("content").and_then(|c| c.as_array()) else {
                return;
            };
            for block in blocks {
                if let Some(text) = block.get("Text").and_then(|v| v.as_str()) {
                    out.push(TraceTurn::Text(text.to_string()));
                } else if let Some(tool_call) = block.get("ToolCall") {
                    let name = tool_call.get("name").and_then(|v| v.as_str()).unwrap_or("tool").to_string();
                    out.push(TraceTurn::ToolCall { name, args: short_args(tool_call.get("arguments")) });
                }
            }
        }
        "tool_result" => {
            let content = row.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            out.push(TraceTurn::ToolResult(content));
        }
        "answer" => {
            if let Some(text) = row.get("text").and_then(|v| v.as_str()) {
                out.push(TraceTurn::Text(text.to_string()));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_session_jsonl_becomes_the_expected_turn_list() {
        let dir = tempfile::tempdir().unwrap();
        let rows = [
            r#"{"type":"summary","summary":"skip me"}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"do the thing"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"a one-line plan"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test -p arx"}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"23 passed"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"done"}]}}"#,
        ];
        fs::write(dir.path().join("session.jsonl"), rows.join("\n")).unwrap();

        let turns = read_trace(dir.path(), "claude-code").unwrap();
        assert_eq!(
            turns,
            vec![
                TraceTurn::Text("do the thing".to_string()),
                TraceTurn::Text("a one-line plan".to_string()),
                TraceTurn::ToolCall { name: "Bash".to_string(), args: "cargo test -p arx".to_string() },
                TraceTurn::ToolResult("23 passed".to_string()),
                TraceTurn::Text("done".to_string()),
            ]
        );
    }

    #[test]
    fn codex_rollout_jsonl_becomes_the_expected_turn_list() {
        let dir = tempfile::tempdir().unwrap();
        let rows = [
            r#"{"type":"session_meta","payload":{"id":"skip"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"review the diff"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call","name":"Bash","arguments":"{\"command\":\"git diff\"}"}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call_output","output":"diff summary"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"two sentences of reasoning"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"no findings"}]}}"#,
        ];
        fs::write(dir.path().join("session.jsonl"), rows.join("\n")).unwrap();

        let turns = read_trace(dir.path(), "codex").unwrap();
        assert_eq!(
            turns,
            vec![
                TraceTurn::Text("review the diff".to_string()),
                TraceTurn::ToolCall { name: "Bash".to_string(), args: "git diff".to_string() },
                TraceTurn::ToolResult("diff summary".to_string()),
                TraceTurn::Text("two sentences of reasoning".to_string()),
                TraceTurn::Text("no findings".to_string()),
            ]
        );
    }

    #[test]
    fn native_transcript_jsonl_becomes_the_expected_turn_list() {
        let dir = tempfile::tempdir().unwrap();
        let rows = [
            r#"{"kind":"request","model":"fake-model"}"#,
            r#"{"kind":"assistant","content":[{"ToolCall":{"id":"c1","name":"read_file","arguments":{"path":"CLAUDE.md"}}}]}"#,
            r#"{"kind":"gate","tool":"read_file","verdict":"allow"}"#,
            r#"{"kind":"tool_result","call_id":"c1","content":"file contents"}"#,
            r#"{"kind":"answer","text":"the hazard line is accurate"}"#,
            r#"{"kind":"termination","termination":"end_turn","usage":null}"#,
        ];
        fs::write(dir.path().join("transcript.jsonl"), rows.join("\n")).unwrap();

        let turns = read_trace(dir.path(), "native").unwrap();
        assert_eq!(
            turns,
            vec![
                TraceTurn::ToolCall { name: "read_file".to_string(), args: "CLAUDE.md".to_string() },
                TraceTurn::ToolResult("file contents".to_string()),
                TraceTurn::Text("the hazard line is accurate".to_string()),
            ]
        );
    }

    #[test]
    fn fake_stdout_log_becomes_one_text_turn_per_line() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("out")).unwrap();
        fs::write(dir.path().join("out/stdout.log"), "stop:which db?\nverdict:done: job 1\n").unwrap();

        let turns = read_trace(dir.path(), "fake").unwrap();
        assert_eq!(
            turns,
            vec![TraceTurn::Text("stop:which db?".to_string()), TraceTurn::Text("verdict:done: job 1".to_string())]
        );
    }

    #[test]
    fn a_missing_trace_file_is_reported_with_the_path_and_the_error() {
        let dir = tempfile::tempdir().unwrap();

        let err = read_trace(dir.path(), "claude-code").unwrap_err();
        assert!(err.contains("session.jsonl"), "err: {err}");

        let err = read_trace(dir.path(), "fake").unwrap_err();
        assert!(err.contains("stdout.log"), "err: {err}");
    }

    #[test]
    fn a_worktree_diff_alongside_the_session_becomes_a_trailing_diff_turn() {
        let dir = tempfile::tempdir().unwrap();
        let rows = [r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"done"}]}}"#];
        fs::write(dir.path().join("session.jsonl"), rows.join("\n")).unwrap();
        fs::write(dir.path().join("worktree.diff"), "--- a/f\n+++ b/f\n").unwrap();

        let turns = read_trace(dir.path(), "claude-code").unwrap();
        assert_eq!(
            turns,
            vec![TraceTurn::Text("done".to_string()), TraceTurn::Diff("--- a/f\n+++ b/f\n".to_string())]
        );
    }

    #[test]
    fn a_captured_diff_still_renders_when_the_transcript_itself_is_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("worktree.diff"), "--- a/f\n+++ b/f\n").unwrap();

        let turns = read_trace(dir.path(), "codex").unwrap();
        assert_eq!(turns.len(), 2, "turns: {turns:?}");
        assert!(
            matches!(&turns[0], TraceTurn::Text(text) if text.starts_with("trace unreadable:") && text.contains("session.jsonl")),
            "turns: {turns:?}"
        );
        assert_eq!(turns[1], TraceTurn::Diff("--- a/f\n+++ b/f\n".to_string()));
    }

    #[test]
    fn a_malformed_transcript_still_shows_a_captured_diff() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("session.jsonl"), "not json\n").unwrap();
        fs::write(dir.path().join("worktree.diff"), "--- a/f\n+++ b/f\n").unwrap();

        let turns = read_trace(dir.path(), "codex").unwrap();
        assert!(matches!(&turns[0], TraceTurn::Text(text) if text.starts_with("trace unreadable:")), "turns: {turns:?}");
        assert_eq!(turns[1], TraceTurn::Diff("--- a/f\n+++ b/f\n".to_string()));
    }

    #[test]
    fn a_non_utf8_diff_renders_lossily_beside_a_readable_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let rows = [r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"done"}]}}"#];
        fs::write(dir.path().join("session.jsonl"), rows.join("\n")).unwrap();
        fs::write(dir.path().join("worktree.diff"), [b"--- a/f\n+++ b/f\n@@\n+".as_slice(), &[0xff, 0xfe]].concat()).unwrap();

        let turns = read_trace(dir.path(), "claude-code").unwrap();
        assert_eq!(turns.len(), 2, "turns: {turns:?}");
        assert_eq!(turns[0], TraceTurn::Text("done".to_string()));
        assert!(matches!(&turns[1], TraceTurn::Diff(text) if text.starts_with("--- a/f\n+++ b/f\n@@\n+")), "turns: {turns:?}");
    }

    #[test]
    fn no_worktree_diff_means_no_diff_turn() {
        let dir = tempfile::tempdir().unwrap();
        let rows = [r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"done"}]}}"#];
        fs::write(dir.path().join("session.jsonl"), rows.join("\n")).unwrap();

        let turns = read_trace(dir.path(), "claude-code").unwrap();
        assert_eq!(turns, vec![TraceTurn::Text("done".to_string())]);
    }

    #[test]
    fn a_valid_prefix_followed_by_a_malformed_row_is_reported_unreadable_not_partially_rendered() {
        let dir = tempfile::tempdir().unwrap();
        let rows = [
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"a one-line plan"}]}}"#,
            r#"{"type":"assistant","message"#,
        ];
        fs::write(dir.path().join("session.jsonl"), rows.join("\n")).unwrap();

        let err = read_trace(dir.path(), "claude-code").unwrap_err();
        assert!(err.contains("session.jsonl:2"), "err: {err}");
    }
}
