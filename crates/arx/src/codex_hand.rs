//! The codex adapter, driven inside the container by `container_hand.rs`, L2-09.

use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use crate::ledger::HandSpec;

const RAW_TAIL_BYTES: usize = 2000;
const MAX_LINE_BYTES: usize = 1024 * 1024;
const READ_CHUNK_BYTES: usize = 8192;

pub(crate) const ARX_HAND_INSTRUCTIONS: &str = "You are a hand dispatched by the arx harness. Execute the brief you receive as your prompt; your final message is your verdict — state it plainly.";

pub(crate) fn composed_instructions(spec: &HandSpec) -> String {
    match spec.role_line.as_deref().map(str::trim).filter(|line| !line.is_empty()) {
        Some(role_line) => format!("{ARX_HAND_INSTRUCTIONS}\n{role_line}"),
        None => ARX_HAND_INSTRUCTIONS.to_string(),
    }
}

fn escape_toml_basic_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

const CODEX_BOOLEAN_NEGATIONS: [(&str, &str); 1] = [("Bash", "features.shell_tool")];

pub(crate) fn build_argv(spec: &HandSpec) -> Vec<String> {
    let instructions = escape_toml_basic_string(&composed_instructions(spec));
    let sandbox = if spec.write { "danger-full-access" } else { "read-only" };
    let mut argv = vec![
        "exec".to_string(),
        "--json".to_string(),
        "--model".to_string(),
        spec.model.clone(),
        "--skip-git-repo-check".to_string(),
        "--sandbox".to_string(),
        sandbox.to_string(),
        "-c".to_string(),
        format!("instructions=\"{instructions}\""),
        "-c".to_string(),
        "project_doc_max_bytes=0".to_string(),
    ];
    for (tool, flag) in CODEX_BOOLEAN_NEGATIONS {
        if !spec.tools.iter().any(|listed| listed == tool) {
            argv.push("-c".to_string());
            argv.push(format!("{flag}=false"));
        }
    }
    argv.push("-c".to_string());
    argv.push("features.view_image=false".to_string());
    argv.push("-c".to_string());
    argv.push(format!("web_search=\"{}\"", escape_toml_basic_string("disabled")));
    argv
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct StreamState {
    thread_id: Option<String>,
    last_agent_text: Option<String>,
    pub(crate) usage: Option<serde_json::Value>,
    last_error: Option<String>,
    turn_completed: bool,
    read_error: Option<String>,
}

fn push_tail(tail: &mut VecDeque<u8>, bytes: &[u8], cap: usize) {
    for &byte in bytes {
        if tail.len() == cap {
            tail.pop_front();
        }
        tail.push_back(byte);
    }
}

fn apply_event(state: &mut StreamState, value: &serde_json::Value) {
    match value.get("type").and_then(|v| v.as_str()) {
        Some("thread.started") => {
            if let Some(id) = value.get("thread_id").and_then(|v| v.as_str()) {
                state.thread_id = Some(id.to_string());
            }
        }
        Some("item.completed") => {
            if let Some(item) = value.get("item") {
                if item.get("type").and_then(|v| v.as_str()) == Some("agent_message") {
                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                        state.last_agent_text = Some(text.to_string());
                    }
                }
            }
        }
        Some("turn.completed") => {
            state.turn_completed = true;
            if let Some(usage) = value.get("usage") {
                state.usage = Some(usage.clone());
            }
        }
        Some("turn.failed") => {
            let message = value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("turn.failed");
            state.last_error = Some(message.to_string());
        }
        Some("error") => {
            if let Some(message) = value.get("message").and_then(|v| v.as_str()) {
                state.last_error = Some(message.to_string());
            }
        }
        _ => {}
    }
}

fn handle_line(state: &mut StreamState, line_bytes: &[u8]) {
    let Ok(text) = std::str::from_utf8(line_bytes) else {
        return;
    };
    if text.trim().is_empty() {
        return;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        apply_event(state, &value);
    }
}

pub(crate) fn parse_stream(mut pipe: impl Read) -> (StreamState, Vec<u8>) {
    let mut state = StreamState::default();
    let mut tail: VecDeque<u8> = VecDeque::with_capacity(RAW_TAIL_BYTES);
    let mut line_buf: Vec<u8> = Vec::new();
    let mut overflowed = false;
    let mut chunk = [0u8; READ_CHUNK_BYTES];

    loop {
        let read = match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) => {
                state.read_error = Some(err.to_string());
                break;
            }
        };

        for &byte in &chunk[..read] {
            if byte == b'\n' {
                if !overflowed {
                    handle_line(&mut state, &line_buf);
                }
                line_buf.clear();
                overflowed = false;
            } else if overflowed {
                continue;
            } else if line_buf.len() < MAX_LINE_BYTES {
                line_buf.push(byte);
            } else {
                overflowed = true;
                line_buf.clear();
            }
        }

        push_tail(&mut tail, &chunk[..read], RAW_TAIL_BYTES);
    }

    if !line_buf.is_empty() && !overflowed {
        handle_line(&mut state, &line_buf);
    }

    (state, tail.into_iter().collect())
}

pub(crate) fn build_verdict_text(state: &StreamState, status: &std::io::Result<ExitStatus>, stderr_tail: &[u8], stdout_tail: &[u8]) -> String {
    if state.turn_completed {
        if let Some(text) = &state.last_agent_text {
            if !text.is_empty() {
                return text.clone();
            }
        }
    }
    let status_text = match status {
        Ok(status) => format!("exit status: {status}"),
        Err(err) => format!("wait error: {err}"),
    };
    let reason = if let Some(error) = &state.last_error {
        error.clone()
    } else if let Some(read_error) = &state.read_error {
        format!("{status_text} (stdout read error: {read_error})")
    } else if !stdout_tail.is_empty() {
        format!("{status_text}; unrecognized stdout tail: {}", String::from_utf8_lossy(stdout_tail))
    } else {
        status_text
    };
    format!(
        "codex exec failed — {reason}; stderr tail: {}",
        String::from_utf8_lossy(stderr_tail)
    )
}

pub(crate) fn find_rollout_file(codex_home: &Path) -> Option<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth == 0 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, depth - 1, out);
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                    out.push(path);
                }
            }
        }
    }
    let sessions_dir = codex_home.join("sessions");
    let mut candidates = Vec::new();
    walk(&sessions_dir, 4, &mut candidates);
    candidates.into_iter().max_by_key(|path| std::fs::metadata(path).and_then(|meta| meta.modified()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{BodyKind, HandRole};

    fn sample_spec() -> HandSpec {
        HandSpec {
            agent: "hand-cx".to_string(),
            kind: BodyKind::Codex,
            role: HandRole::Hand,
            model: "gpt-5.4-codex".to_string(),
            cwd: PathBuf::from("/tmp/arx-codex-hand-test"),
            threads_dir: PathBuf::from("/tmp/arx-codex-hand-test/threads"),
            thread: "main".to_string(),
            provider: None,
            write: false,
            image: crate::ledger::DEFAULT_HAND_IMAGE.to_string(),
            env: Vec::new(),
            store_root: PathBuf::from("/tmp/arx-codex-hand-test/store"),
            config_dir: PathBuf::from("/tmp/arx-codex-hand-test"),
            tools: Vec::new(),
            steps: crate::config::DEFAULT_STEPS,
            window: None,
            gate: crate::gate::GateConfig::default(),
            user_gate: crate::gate::GateConfig::default(),
            store_scope: None,
            drain_input: None,
            admission: crate::admission::AdmissionPolicy::default(),
            role_line: None,
        }
    }

    fn exit_status(code: i32) -> std::io::Result<ExitStatus> {
        use std::os::unix::process::ExitStatusExt;
        Ok(ExitStatus::from_raw(code << 8))
    }

    #[test]
    fn argv_carries_both_c_overrides_model_flag_and_no_store_path() {
        let spec = sample_spec();
        let argv = build_argv(&spec);

        assert_eq!(argv[0], "exec");
        assert!(argv.contains(&"--json".to_string()));
        assert!(argv.contains(&"--model".to_string()));
        assert!(argv.contains(&spec.model));
        assert!(argv.contains(&"--skip-git-repo-check".to_string()));
        assert!(argv.contains(&"--sandbox".to_string()));

        let skip_pos = argv.iter().position(|arg| arg == "--skip-git-repo-check").unwrap();
        let sandbox_pos = argv.iter().position(|arg| arg == "--sandbox").unwrap();
        assert_eq!(sandbox_pos, skip_pos + 1, "argv: {argv:?}");

        let c_count = argv.iter().filter(|arg| arg.as_str() == "-c").count();
        assert_eq!(c_count, 5, "argv: {argv:?}");
        assert!(argv.iter().any(|arg| arg.starts_with("instructions=") && arg.contains(ARX_HAND_INSTRUCTIONS)));
        assert!(argv.contains(&"project_doc_max_bytes=0".to_string()));

        for arg in &argv {
            assert!(!arg.contains("store/"), "argv leaked a store path: {arg}");
        }
        assert!(
            !spec.cwd.display().to_string().contains("store/"),
            "cwd leaked a store path: {}",
            spec.cwd.display()
        );
    }

    #[test]
    fn a_position_listing_nothing_negates_shell_and_the_unconditional_pair() {
        let spec = sample_spec();
        assert!(spec.tools.is_empty());
        let argv = build_argv(&spec);
        assert!(argv.contains(&"features.shell_tool=false".to_string()), "argv: {argv:?}");
        assert!(argv.contains(&"features.view_image=false".to_string()), "argv: {argv:?}");
        assert!(argv.contains(&"web_search=\"disabled\"".to_string()), "argv: {argv:?}");
    }

    #[test]
    fn a_position_listing_bash_keeps_shell_but_still_loses_image_and_web() {
        let mut spec = sample_spec();
        spec.tools = vec!["Bash".to_string()];
        let argv = build_argv(&spec);
        assert!(!argv.contains(&"features.shell_tool=false".to_string()), "argv: {argv:?}");
        assert!(argv.contains(&"features.view_image=false".to_string()), "argv: {argv:?}");
        assert!(argv.contains(&"web_search=\"disabled\"".to_string()), "argv: {argv:?}");
    }

    #[test]
    fn build_argv_sandboxes_a_read_only_hand() {
        let spec = sample_spec();
        assert!(!spec.write);
        let argv = build_argv(&spec);
        let skip_pos = argv.iter().position(|arg| arg == "--skip-git-repo-check").unwrap();
        assert_eq!(argv[skip_pos + 1], "--sandbox", "argv: {argv:?}");
        assert_eq!(argv[skip_pos + 2], "read-only", "argv: {argv:?}");
    }

    #[test]
    fn build_argv_grants_danger_full_access_to_a_write_hand() {
        let mut spec = sample_spec();
        spec.write = true;
        let argv = build_argv(&spec);
        let skip_pos = argv.iter().position(|arg| arg == "--skip-git-repo-check").unwrap();
        assert_eq!(argv[skip_pos + 1], "--sandbox", "argv: {argv:?}");
        assert_eq!(argv[skip_pos + 2], "danger-full-access", "argv: {argv:?}");
    }

    #[test]
    fn build_argv_appends_a_non_empty_role_line_as_the_final_line_of_the_instructions() {
        let mut spec = sample_spec();
        spec.role_line = Some("A mechanical scout for one-hop lookups.".to_string());
        let argv = build_argv(&spec);
        let instructions_arg = argv.iter().find(|arg| arg.starts_with("instructions=")).unwrap();
        assert_eq!(
            instructions_arg,
            &format!("instructions=\"{ARX_HAND_INSTRUCTIONS}\\nA mechanical scout for one-hop lookups.\"")
        );
    }

    #[test]
    fn build_argv_leaves_the_instructions_unchanged_when_the_role_line_is_empty() {
        let spec = sample_spec();
        assert!(spec.role_line.is_none());
        let argv = build_argv(&spec);
        let instructions_arg = argv.iter().find(|arg| arg.starts_with("instructions=")).unwrap();
        assert_eq!(instructions_arg, &format!("instructions=\"{ARX_HAND_INSTRUCTIONS}\""));
    }

    #[derive(serde::Deserialize)]
    struct InstructionsOverride {
        instructions: String,
    }

    fn decoded_instructions(spec: &HandSpec) -> String {
        let argv = build_argv(spec);
        let instructions_arg = argv.iter().find(|arg| arg.starts_with("instructions=")).unwrap();
        let toml_value = instructions_arg.strip_prefix("instructions=").unwrap();
        let toml_doc = format!("instructions = {toml_value}");
        let decoded: InstructionsOverride = toml::from_str(&toml_doc).unwrap();
        decoded.instructions
    }

    #[test]
    fn build_argv_round_trips_the_shipped_bolt_prose_through_toml() {
        let mut spec = sample_spec();
        spec.role_line = Some(
            "A mechanical scout for one-hop lookups — a file, a symbol, a caller, a git fact — that returns \"needs a deeper hand\" rather than guessing."
                .to_string(),
        );
        assert_eq!(decoded_instructions(&spec), composed_instructions(&spec));
    }

    #[test]
    fn build_argv_round_trips_a_literal_backslash_sequence_in_the_role_line() {
        let mut spec = sample_spec();
        spec.role_line = Some(r"matches \bTODO\b in the brief".to_string());
        assert_eq!(decoded_instructions(&spec), composed_instructions(&spec));
    }

    #[test]
    fn happy_stream_yields_the_agent_message_as_verdict_text() {
        let stream = concat!(
            "{\"type\":\"thread.started\",\"thread_id\":\"abc-123\"}\n",
            "{\"type\":\"turn.started\"}\n",
            "{\"type\":\"item.completed\",\"item\":{\"id\":\"item_0\",\"type\":\"agent_message\",\"text\":\"OK\"}}\n",
            "{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":10,\"cached_input_tokens\":2,\"cache_write_input_tokens\":0,\"output_tokens\":5,\"reasoning_output_tokens\":0}}\n",
        );
        let (state, tail) = parse_stream(stream.as_bytes());
        assert_eq!(state.thread_id.as_deref(), Some("abc-123"));
        let text = build_verdict_text(&state, &exit_status(0), b"", &tail);
        assert_eq!(text, "OK");
    }

    #[test]
    fn clean_exit_with_no_agent_message_is_a_failure_verdict() {
        let stream = concat!(
            "{\"type\":\"thread.started\",\"thread_id\":\"abc-123\"}\n",
            "{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":10,\"cached_input_tokens\":2,\"cache_write_input_tokens\":0,\"output_tokens\":5,\"reasoning_output_tokens\":0}}\n",
        );
        let (state, tail) = parse_stream(stream.as_bytes());
        let text = build_verdict_text(&state, &exit_status(0), b"", &tail);
        assert!(text.contains("codex exec failed"), "text: {text}");
        assert!(!text.is_empty());
    }

    #[test]
    fn turn_failed_event_is_a_failure_verdict_carrying_the_message() {
        let stream = concat!(
            "{\"type\":\"thread.started\",\"thread_id\":\"abc-123\"}\n",
            "{\"type\":\"turn.failed\",\"error\":{\"message\":\"boom\"}}\n",
        );
        let (state, tail) = parse_stream(stream.as_bytes());
        let text = build_verdict_text(&state, &exit_status(1), b"", &tail);
        assert!(text.contains("codex exec failed"), "text: {text}");
        assert!(text.contains("boom"), "text: {text}");
    }

    #[test]
    fn unrecognized_stdout_shape_surfaces_in_the_failure_reason_when_nothing_else_explains_it() {
        let stream = "{\"type\":\"some.new.event\",\"foo\":\"bar\"}\n";
        let (state, tail) = parse_stream(stream.as_bytes());
        assert!(state.last_error.is_none());
        let text = build_verdict_text(&state, &exit_status(0), b"", &tail);
        assert!(text.contains("codex exec failed"), "text: {text}");
        assert!(text.contains("some.new.event"), "text: {text}");
    }

    struct FailingReader {
        emitted: bool,
    }

    impl Read for FailingReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if !self.emitted {
                self.emitted = true;
                let data = b"{\"type\":\"thread.started\",\"thread_id\":\"abc-123\"}\n";
                let n = data.len().min(buf.len());
                buf[..n].copy_from_slice(&data[..n]);
                Ok(n)
            } else {
                Err(std::io::Error::other("boom"))
            }
        }
    }

    #[test]
    fn stdout_read_errors_are_preserved_in_parser_state_and_the_failure_reason() {
        let (state, tail) = parse_stream(FailingReader { emitted: false });
        assert_eq!(state.thread_id.as_deref(), Some("abc-123"));
        assert!(state.read_error.as_deref().unwrap_or("").contains("boom"));
        let text = build_verdict_text(&state, &exit_status(0), b"", &tail);
        assert!(text.contains("codex exec failed"), "text: {text}");
        assert!(text.contains("stdout read error"), "text: {text}");
        assert!(text.contains("boom"), "text: {text}");
    }

    #[test]
    fn an_oversized_single_line_does_not_grow_the_line_buffer_unbounded() {
        let huge = "x".repeat(MAX_LINE_BYTES + 1000);
        let stream = format!(
            "{huge}\n{{\"type\":\"turn.completed\",\"usage\":{{\"input_tokens\":1,\"cached_input_tokens\":0,\"cache_write_input_tokens\":0,\"output_tokens\":1,\"reasoning_output_tokens\":0}}}}\n"
        );
        let (state, tail) = parse_stream(stream.as_bytes());
        assert!(state.turn_completed);
        assert_eq!(tail.len(), RAW_TAIL_BYTES, "the raw tail must stay bounded even past an oversized line");
    }

    #[test]
    fn usage_is_captured_verbatim_from_turn_completed() {
        let stream = "{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":10,\"cached_input_tokens\":2,\"cache_write_input_tokens\":0,\"output_tokens\":5,\"reasoning_output_tokens\":1}}\n";
        let (state, _tail) = parse_stream(stream.as_bytes());
        assert_eq!(
            state.usage,
            Some(serde_json::json!({
                "input_tokens": 10,
                "cached_input_tokens": 2,
                "cache_write_input_tokens": 0,
                "output_tokens": 5,
                "reasoning_output_tokens": 1
            }))
        );
    }

    #[test]
    fn unparseable_lines_are_bounded_in_the_raw_tail() {
        let garbage_line = "x".repeat(RAW_TAIL_BYTES * 3);
        let stream = format!("{garbage_line}\n");
        let (_state, tail) = parse_stream(stream.as_bytes());
        assert_eq!(tail.len(), RAW_TAIL_BYTES);
    }
}
