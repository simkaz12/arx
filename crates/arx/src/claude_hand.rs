//! The claude-code adapter's shared shape: argv clamp and verdict parsing, L2-08.
//! The body itself runs in the container; see `container_hand.rs`.

use std::io::Read;
use std::process::ExitStatus;

use crate::codex_hand::composed_instructions;
use crate::ledger::HandSpec;

pub(crate) const STDERR_TAIL_BYTES: usize = 2000;
pub(crate) const MAX_STDOUT_BYTES: usize = 4 * 1024 * 1024;

pub const CLAUDE_TOOLS: [&str; 13] = [
    "Agent",
    "Bash",
    "Edit",
    "Glob",
    "Grep",
    "MultiEdit",
    "NotebookEdit",
    "Read",
    "Task",
    "TodoWrite",
    "WebFetch",
    "WebSearch",
    "Write",
];

pub const WRITE_TOOLS: [&str; 4] = ["Write", "Edit", "MultiEdit", "NotebookEdit"];

pub(crate) fn generate_uuid_v4() -> String {
    let mut bytes = [0u8; 16];
    let read_urandom = std::fs::File::open("/dev/urandom").and_then(|mut file| file.read_exact(&mut bytes));
    if read_urandom.is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = ((nanos >> ((i % 16) * 8)) & 0xff) as u8;
        }
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

pub fn effective_tools(spec: &HandSpec) -> Vec<String> {
    let mut tools: Vec<String> = if spec.tools.is_empty() {
        let mut default = vec!["Read".to_string(), "Glob".to_string(), "Grep".to_string(), "Bash".to_string()];
        if spec.write {
            default.push("Write".to_string());
            default.push("Edit".to_string());
            default.push("MultiEdit".to_string());
        }
        default
    } else {
        spec.tools.clone()
    };
    if !spec.write {
        tools.retain(|tool| !WRITE_TOOLS.contains(&tool.as_str()));
    }
    tools
}

pub fn build_argv(spec: &HandSpec, session_id: &str) -> Vec<String> {
    let effective = effective_tools(spec);
    let effective_set: std::collections::HashSet<&str> = effective.iter().map(|s| s.as_str()).collect();
    let disallowed: Vec<&str> = CLAUDE_TOOLS.iter().filter(|tool| !effective_set.contains(*tool)).copied().collect();
    vec![
        "-p".to_string(),
        "--model".to_string(),
        spec.model.clone(),
        "--output-format".to_string(),
        "json".to_string(),
        "--session-id".to_string(),
        session_id.to_string(),
        "--permission-mode".to_string(),
        "default".to_string(),
        "--allowedTools".to_string(),
        effective.join(","),
        "--disallowedTools".to_string(),
        disallowed.join(","),
        "--tools".to_string(),
        effective.join(","),
        "--append-system-prompt".to_string(),
        composed_instructions(spec),
    ]
}

pub fn permission_denials(result: &serde_json::Value) -> Vec<String> {
    result
        .get("permission_denials")
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("tool_name").and_then(|tool| tool.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn build_verdict_text(status: &std::io::Result<ExitStatus>, stdout: &[u8], stderr_tail: &[u8]) -> String {
    let exited_ok = matches!(status, Ok(status) if status.success());
    if exited_ok {
        if let Ok(text) = std::str::from_utf8(stdout) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(text.trim()) {
                if let Some(result) = value.get("result").and_then(|v| v.as_str()) {
                    let is_error = value.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
                    if is_error {
                        return format!("{result} (is_error: true)");
                    }
                    return result.to_string();
                }
            }
        }
    }
    let status_text = match status {
        Ok(status) => format!("exit status: {status}"),
        Err(err) => format!("wait error: {err}"),
    };
    format!(
        "claude -p failed — {status_text}; stderr tail: {}",
        String::from_utf8_lossy(stderr_tail)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{BodyKind, HandRole};
    use std::path::PathBuf;

    fn sample_spec() -> HandSpec {
        HandSpec {
            agent: "hand-ex".to_string(),
            kind: BodyKind::ClaudeCode,
            role: HandRole::Hand,
            model: "claude-sonnet-5".to_string(),
            cwd: PathBuf::from("/tmp/arx-claude-hand-test"),
            threads_dir: PathBuf::from("/tmp/arx-claude-hand-test/threads"),
            thread: "main".to_string(),
            provider: None,
            write: false,
            image: crate::ledger::DEFAULT_HAND_IMAGE.to_string(),
            env: Vec::new(),
            store_root: PathBuf::from("/tmp/arx-claude-hand-test/store"),
            config_dir: PathBuf::from("/tmp/arx-claude-hand-test"),
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

    fn is_uuid_v4(candidate: &str) -> bool {
        let bytes = candidate.as_bytes();
        bytes.len() == 36
            && bytes[8] == b'-'
            && bytes[13] == b'-'
            && bytes[18] == b'-'
            && bytes[23] == b'-'
            && bytes[14] == b'4'
            && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
            && candidate.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
    }

    #[test]
    fn two_fresh_uuids_differ() {
        let a = generate_uuid_v4();
        let b = generate_uuid_v4();
        assert_ne!(a, b);
    }

    #[test]
    fn argv_carries_the_clamp_and_the_session() {
        let spec = sample_spec();
        let session_id = generate_uuid_v4();
        assert!(is_uuid_v4(&session_id), "not a v4 uuid: {session_id}");
        let argv = build_argv(&spec, &session_id);
        assert_eq!(
            argv,
            vec![
                "-p".to_string(),
                "--model".to_string(),
                "claude-sonnet-5".to_string(),
                "--output-format".to_string(),
                "json".to_string(),
                "--session-id".to_string(),
                session_id.clone(),
                "--permission-mode".to_string(),
                "default".to_string(),
                "--allowedTools".to_string(),
                "Read,Glob,Grep,Bash".to_string(),
                "--disallowedTools".to_string(),
                "Agent,Edit,MultiEdit,NotebookEdit,Task,TodoWrite,WebFetch,WebSearch,Write".to_string(),
                "--tools".to_string(),
                "Read,Glob,Grep,Bash".to_string(),
                "--append-system-prompt".to_string(),
                crate::codex_hand::ARX_HAND_INSTRUCTIONS.to_string(),
            ]
        );
    }

    #[test]
    fn build_argv_appends_a_non_empty_role_line_as_the_final_line_of_the_system_prompt() {
        let mut spec = sample_spec();
        spec.role_line = Some("Implements a bounded brief and leaves the result as a diff in its trace.".to_string());
        let argv = build_argv(&spec, &generate_uuid_v4());
        let prompt_pos = argv.iter().position(|arg| arg == "--append-system-prompt").unwrap();
        assert_eq!(
            argv[prompt_pos + 1],
            format!(
                "{}\nImplements a bounded brief and leaves the result as a diff in its trace.",
                crate::codex_hand::ARX_HAND_INSTRUCTIONS
            )
        );
    }

    #[test]
    fn build_argv_leaves_the_system_prompt_unchanged_when_the_role_line_is_empty() {
        let spec = sample_spec();
        assert!(spec.role_line.is_none());
        let argv = build_argv(&spec, &generate_uuid_v4());
        let prompt_pos = argv.iter().position(|arg| arg == "--append-system-prompt").unwrap();
        assert_eq!(argv[prompt_pos + 1], crate::codex_hand::ARX_HAND_INSTRUCTIONS);
    }

    #[test]
    fn a_read_only_hand_never_lists_a_write_tool_even_if_the_definition_does() {
        let mut spec = sample_spec();
        spec.tools = vec!["Read".to_string(), "Write".to_string(), "Edit".to_string(), "Bash".to_string()];
        let effective = effective_tools(&spec);
        assert_eq!(effective, vec!["Read".to_string(), "Bash".to_string()]);
        for tool in WRITE_TOOLS {
            assert!(!effective.contains(&tool.to_string()), "effective: {effective:?}");
        }
    }

    #[test]
    fn an_empty_tools_key_gets_the_default_set() {
        let spec = sample_spec();
        assert_eq!(
            effective_tools(&spec),
            vec!["Read".to_string(), "Glob".to_string(), "Grep".to_string(), "Bash".to_string()]
        );

        let mut write_spec = sample_spec();
        write_spec.write = true;
        assert_eq!(
            effective_tools(&write_spec),
            vec![
                "Read".to_string(),
                "Glob".to_string(),
                "Grep".to_string(),
                "Bash".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "MultiEdit".to_string(),
            ]
        );
    }

    #[test]
    fn permission_denials_are_read_by_tool_name() {
        let result = serde_json::json!({
            "permission_denials": [
                {"tool_name": "Write"},
                {"tool_name": "Bash"},
            ]
        });
        assert_eq!(permission_denials(&result), vec!["Write".to_string(), "Bash".to_string()]);
        assert_eq!(permission_denials(&serde_json::json!({})), Vec::<String>::new());
    }

    fn exit_status(code: i32) -> std::io::Result<ExitStatus> {
        use std::os::unix::process::ExitStatusExt;
        Ok(ExitStatus::from_raw(code << 8))
    }

    #[test]
    fn successful_json_result_becomes_the_verdict_text() {
        let stdout = br#"{"type":"result","is_error":false,"result":"ok"}"#;
        let text = build_verdict_text(&exit_status(0), stdout, b"");
        assert_eq!(text, "ok");
    }

    #[test]
    fn is_error_flag_is_noted_in_the_verdict_text() {
        let stdout = br#"{"type":"result","is_error":true,"result":"boom"}"#;
        let text = build_verdict_text(&exit_status(0), stdout, b"");
        assert_eq!(text, "boom (is_error: true)");
    }

    #[test]
    fn non_zero_exit_falls_back_to_status_and_stderr_tail() {
        let text = build_verdict_text(&exit_status(1), b"", b"segfault");
        assert!(text.contains("claude -p failed"), "text: {text}");
        assert!(text.contains("segfault"), "text: {text}");
    }

    #[test]
    fn invalid_json_on_a_clean_exit_falls_back_to_the_failure_path() {
        let text = build_verdict_text(&exit_status(0), b"not json", b"");
        assert!(text.contains("claude -p failed"), "text: {text}");
    }

    #[test]
    fn missing_result_field_does_not_produce_a_silent_empty_verdict() {
        let stdout = br#"{"type":"result","is_error":false}"#;
        let text = build_verdict_text(&exit_status(0), stdout, b"");
        assert!(text.contains("claude -p failed"), "text: {text}");
        assert!(!text.is_empty());
    }

    #[test]
    fn non_string_result_field_does_not_produce_a_silent_empty_verdict() {
        let stdout = br#"{"type":"result","is_error":false,"result":42}"#;
        let text = build_verdict_text(&exit_status(0), stdout, b"");
        assert!(text.contains("claude -p failed"), "text: {text}");
    }
}
