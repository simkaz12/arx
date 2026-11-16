use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use std::sync::Mutex;

mod common;
use common::*;

static DOCKER_STATE_SERIAL: Mutex<()> = Mutex::new(());

fn serialize_on_docker_state() -> std::sync::MutexGuard<'static, ()> {
    DOCKER_STATE_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn ledger_rows(workspace_run: &Path) -> Vec<serde_json::Value> {
    let contents = fs::read_to_string(workspace_run.join("threads/main.ledger.jsonl")).unwrap_or_default();
    contents.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

fn session_argv(run_dir: &Path, hand: &str) -> String {
    let path = run_dir.join(format!("threads/main.{hand}/session.jsonl"));
    let contents = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    let first_line = contents.lines().next().expect("session.jsonl has a first line");
    let value: serde_json::Value = serde_json::from_str(first_line).unwrap();
    value["argv"].as_str().expect("argv field is a string").to_string()
}

fn stub_hand_echo(rows: &[serde_json::Value]) -> Option<String> {
    rows.iter()
        .rev()
        .find(|row| {
            row["kind"] == "turn"
                && row["role"] == "main"
                && row["text"].as_str().map(|t| t.starts_with("stub-hand:")).unwrap_or(false)
        })
        .and_then(|row| row["text"].as_str())
        .map(|s| s.to_string())
}

#[test]
fn the_credential_is_present_in_the_container_and_absent_from_the_trace() {
    let _guard = serialize_on_docker_state();
    ensure_claude_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let claude_config_dir = plant_stub_credential(workspace.path());

    run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cc:read the file".to_string()],
        &[
            ("CLAUDE_CONFIG_DIR", claude_config_dir.display().to_string()),
            ("ARX_CANARY", "1".to_string()),
        ],
    );

    let trace_dir = run_dir.join("threads/main.h1");
    let session_contents = fs::read_to_string(trace_dir.join("session.jsonl")).unwrap();
    let lines: Vec<&str> = session_contents.lines().collect();
    assert_eq!(lines.get(1), Some(&r#"{"type":"cred","present":true}"#), "session.jsonl: {session_contents}");

    let grep = StdCommand::new("grep")
        .args(["-r", r"stub-token\|accessToken\|claudeAiOauth", trace_dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&grep.stdout).trim().is_empty(),
        "credential leaked into the trace: {}",
        String::from_utf8_lossy(&grep.stdout)
    );

    let trace_md = fs::read_to_string(trace_dir.join("trace.md")).unwrap();
    assert!(trace_md.contains("path: session.jsonl"), "trace.md: {trace_md}");
    assert!(trace_md.contains("path: out/stdout.log"), "trace.md: {trace_md}");
    assert!(trace_md.contains("path: out/stderr.log"), "trace.md: {trace_md}");
    assert!(!trace_md.contains("path: home"), "trace.md: {trace_md}");

    assert!(!session_contents.contains("ARX_CANARY"), "session.jsonl: {session_contents}");
    let stdout_log = fs::read_to_string(trace_dir.join("out/stdout.log")).unwrap_or_default();
    assert!(!stdout_log.contains("ARX_CANARY"), "out/stdout.log: {stdout_log}");
}

#[test]
fn the_clamp_reaches_the_child_and_strikes_write_tools_for_a_read_only_hand() {
    let _guard = serialize_on_docker_state();
    ensure_claude_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let claude_config_dir = plant_stub_credential(workspace.path());

    run_session(
        &run_dir,
        "arx.toml",
        &[
            "hand:dispatch:hand-cc:read the file".to_string(),
            "hand:dispatch:hand-cw:read the file".to_string(),
        ],
        &[("CLAUDE_CONFIG_DIR", claude_config_dir.display().to_string())],
    );

    let cc_argv = session_argv(&run_dir, "h1");
    assert!(cc_argv.contains("--permission-mode default"), "argv: {cc_argv}");
    assert!(cc_argv.contains("--allowedTools Read,Glob,Grep,Bash"), "argv: {cc_argv}");
    let cc_disallowed = cc_argv.split("--disallowedTools ").nth(1).unwrap().split(' ').next().unwrap();
    for tool in ["Write", "Edit", "MultiEdit", "NotebookEdit", "WebFetch", "WebSearch", "Task", "Agent", "TodoWrite"] {
        assert!(cc_disallowed.split(',').any(|t| t == tool), "disallowedTools missing {tool}: {cc_disallowed}");
    }
    assert!(!cc_disallowed.split(',').any(|t| t == "Read"), "disallowedTools carries Read: {cc_disallowed}");

    let cw_argv = session_argv(&run_dir, "h2");
    let cw_allowed = cw_argv.split("--allowedTools ").nth(1).unwrap().split(' ').next().unwrap();
    assert!(cw_allowed.split(',').any(|t| t == "Write"), "hand-cw allowedTools missing Write: {cw_allowed}");
    let cw_disallowed = cw_argv.split("--disallowedTools ").nth(1).unwrap().split(' ').next().unwrap();
    assert!(!cw_disallowed.split(',').any(|t| t == "Write"), "hand-cw disallowedTools carries Write: {cw_disallowed}");
}

#[test]
fn a_write_enabled_claude_hand_leaves_an_applicable_diff() {
    let _guard = serialize_on_docker_state();
    ensure_claude_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let claude_config_dir = plant_stub_credential(workspace.path());

    let head = StdCommand::new("git").arg("-C").arg(&run_dir).arg("rev-parse").arg("HEAD").output().unwrap();
    let head_sha = String::from_utf8_lossy(&head.stdout).trim().to_string();

    assert!(!run_dir.join("claude-write-probe.txt").exists(), "claude-write-probe.txt already present before dispatch");

    run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cw:[write:claude-write-probe.txt=from-claude] make the file".to_string()],
        &[("CLAUDE_CONFIG_DIR", claude_config_dir.display().to_string())],
    );

    let trace_dir = run_dir.join("threads/main.h1");
    let trace_md = fs::read_to_string(trace_dir.join("trace.md")).unwrap();
    assert!(trace_md.contains(&format!("base: {head_sha}")), "trace.md: {trace_md}");

    let diff_path = trace_dir.join("worktree.diff");
    let apply = StdCommand::new("git").arg("-C").arg(&run_dir).arg("apply").arg(&diff_path).output().unwrap();
    assert!(apply.status.success(), "git apply failed: {}", String::from_utf8_lossy(&apply.stderr));
    let applied = fs::read_to_string(run_dir.join("claude-write-probe.txt")).unwrap();
    assert_eq!(applied, "from-claude\n");
}

#[test]
fn denials_in_the_result_become_wall_events_and_refusals_name_their_cause() {
    let _guard = serialize_on_docker_state();
    ensure_claude_stub_image();

    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let claude_config_dir = plant_stub_credential(workspace.path());

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cc:[deny:Write] do it".to_string()],
        &[("CLAUDE_CONFIG_DIR", claude_config_dir.display().to_string())],
    );

    let wall_index = rows.iter().position(|row| row["kind"] == "wall" && row["hand"] == "h1" && row["tool"] == "Write");
    let delivered_index = rows.iter().position(|row| row["kind"] == "delivered" && row["hand"] == "h1");
    assert!(wall_index.is_some(), "rows: {rows:?}");
    assert!(delivered_index.is_some(), "rows: {rows:?}");
    assert!(wall_index.unwrap() < delivered_index.unwrap(), "wall must precede delivered: {rows:?}");
    assert_eq!(rows.iter().filter(|row| row["kind"] == "wall").count(), 1, "rows: {rows:?}");

    let no_cred_workspace = fresh_run_fixture();
    let no_cred_run_dir = no_cred_workspace.path().join("run");
    let no_cred_config_dir = no_cred_workspace.path().join("claude-config");

    let echo_rows = run_session(
        &no_cred_run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cc:read the file".to_string()],
        &[("CLAUDE_CONFIG_DIR", no_cred_config_dir.display().to_string())],
    );
    let echo = stub_hand_echo(&echo_rows).expect("stub-hand echo row");
    assert!(echo.contains("no Claude Code credential at"), "echo: {echo}");

    let refused_ledger = ledger_rows(&no_cred_run_dir);
    assert!(
        refused_ledger.iter().any(|row| row["kind"] == "dispatch" && row["refused"] == true),
        "ledger: {refused_ledger:?}"
    );
    assert!(!refused_ledger.iter().any(|row| row["kind"] == "spawn"), "ledger: {refused_ledger:?}");

    plant_stub_credential(no_cred_workspace.path());
    let config_path = no_cred_run_dir.join("arx.toml");
    let contents = fs::read_to_string(&config_path).unwrap();
    let updated = contents
        .replacen("hand-cc = \"claude-sonnet-5\"\n", "hand-cc = \"bad-image-claude\"\n", 1)
        + "\n[[models]]\nid = \"bad-image-claude\"\nbody = \"claude-code\"\nimage = \"arx-no-such-image:0\"\n";
    fs::write(&config_path, updated).unwrap();

    let bad_image_rows = run_session(
        &no_cred_run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cc:read the file".to_string()],
        &[("CLAUDE_CONFIG_DIR", no_cred_config_dir.display().to_string())],
    );
    let bad_image_echo = stub_hand_echo(&bad_image_rows).expect("stub-hand echo row");
    assert!(
        bad_image_echo.contains("image arx-no-such-image:0 is not present on this host"),
        "echo: {bad_image_echo}"
    );
}
