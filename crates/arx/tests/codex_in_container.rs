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

fn arx_verdict_text(rows: &[serde_json::Value], hand: &str) -> String {
    let needle = format!("[hand {hand} verdict]");
    rows.iter()
        .find(|row| {
            row["kind"] == "turn" && row["role"] == "arx" && row["text"].as_str().map(|t| t.contains(&needle)).unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("no arx row contains {needle}: {rows:?}"))["text"]
        .as_str()
        .unwrap()
        .to_string()
}

fn git_worktree_count(run_dir: &Path) -> usize {
    let output = StdCommand::new("git").arg("-C").arg(run_dir).arg("worktree").arg("list").arg("--porcelain").output().unwrap();
    String::from_utf8_lossy(&output.stdout).lines().filter(|line| line.starts_with("worktree ")).count()
}

fn arx_hand_root_stray(hand_prefix: &str) -> bool {
    let arx_hands = std::env::temp_dir().join("arx-hands");
    fs::read_dir(&arx_hands)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .any(|entry| entry.file_name().to_string_lossy().starts_with(hand_prefix))
        })
        .unwrap_or(false)
}

#[test]
fn credential_lands_in_the_hand_home_and_never_in_the_trace() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let codex_home = plant_stub_codex_credential(workspace.path());
    let credential_path = codex_home.join("auth.json");
    let before = fs::read(&credential_path).unwrap();

    run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cx:read the file".to_string()],
        &[("CODEX_HOME", codex_home.display().to_string())],
    );

    let trace_dir = run_dir.join("threads/main.h1");
    let session_contents = fs::read_to_string(trace_dir.join("session.jsonl")).unwrap();
    let lines: Vec<&str> = session_contents.lines().collect();
    assert_eq!(lines.get(1), Some(&r#"{"stub":"auth","present":true,"mode":"600"}"#), "session.jsonl: {session_contents}");

    let grep = StdCommand::new("grep")
        .args(["-r", r"refresh_token\|access_token\|id_token", trace_dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&grep.stdout).trim().is_empty(),
        "credential leaked into the trace: {}",
        String::from_utf8_lossy(&grep.stdout)
    );

    let after = fs::read(&credential_path).unwrap();
    assert_eq!(before, after, "the host's own auth.json must not be modified by a dispatch");
}

#[test]
fn missing_credential_refuses_dispatch_before_any_container() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let empty_codex_home = workspace.path().join("codex-home-empty");
    fs::create_dir_all(&empty_codex_home).unwrap();

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cx:read the file".to_string()],
        &[("CODEX_HOME", empty_codex_home.display().to_string())],
    );

    let echo = stub_hand_echo(&rows).expect("stub-hand echo row");
    assert!(echo.contains("no Codex credential at"), "echo: {echo}");
    assert!(echo.contains("log in on the host first"), "echo: {echo}");

    let refused_ledger = ledger_rows(&run_dir);
    assert!(
        refused_ledger.iter().any(|row| row["kind"] == "dispatch" && row["refused"] == true),
        "ledger: {refused_ledger:?}"
    );
    assert!(!refused_ledger.iter().any(|row| row["kind"] == "spawn"), "ledger: {refused_ledger:?}");

    let containers = StdCommand::new("docker")
        .args(["ps", "-a", "--filter", "label=arx.hand=h1", "--filter", "name=arx-h1-", "--format", "{{.Names}}"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&containers.stdout).trim().is_empty(),
        "a container was created despite the missing credential"
    );

    assert!(!arx_hand_root_stray("hand-cx-"), "a hand root for hand-cx was left under /tmp/arx-hands");

    let worktree_count_before = git_worktree_count(&run_dir);

    let cxw_rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cxw:read the file".to_string()],
        &[("CODEX_HOME", empty_codex_home.display().to_string())],
    );
    let cxw_echo = stub_hand_echo(&cxw_rows).expect("stub-hand echo row");
    assert!(cxw_echo.contains("no Codex credential at"), "echo: {cxw_echo}");
    assert!(cxw_echo.contains("log in on the host first"), "echo: {cxw_echo}");

    let cxw_refused_ledger = ledger_rows(&run_dir);
    assert!(!cxw_refused_ledger.iter().any(|row| row["kind"] == "spawn"), "ledger: {cxw_refused_ledger:?}");

    let worktree_count_after = git_worktree_count(&run_dir);
    assert_eq!(
        worktree_count_before, worktree_count_after,
        "a write-enabled dispatch created a worktree despite the missing credential"
    );

    assert!(!arx_hand_root_stray("hand-cxw-"), "a hand root for hand-cxw was left under /tmp/arx-hands");
}

#[test]
fn read_only_hand_cannot_write_the_repo_and_says_so() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let codex_home = plant_stub_codex_credential(workspace.path());

    assert!(!run_dir.join("PROBE.txt").exists(), "PROBE.txt already present before dispatch");

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cx:probe-write: PROBE.txt".to_string()],
        &[("CODEX_HOME", codex_home.display().to_string())],
    );

    let text = arx_verdict_text(&rows, "h1");
    assert!(text.contains("write refused"), "text: {text}");

    let trace_dir = run_dir.join("threads/main.h1");
    assert!(!trace_dir.join("worktree.diff").exists(), "a read-only hand must not leave a worktree.diff");
    assert!(!run_dir.join("PROBE.txt").exists(), "the scratch repo must be unchanged for a read-only hand");
}

#[test]
fn write_hand_writes_the_worktree_and_the_diff_applies() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let codex_home = plant_stub_codex_credential(workspace.path());

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cxw:probe-write: PROBE.txt".to_string()],
        &[("CODEX_HOME", codex_home.display().to_string())],
    );

    let text = arx_verdict_text(&rows, "h1");
    assert!(text.contains("wrote /hand/repo/PROBE.txt"), "text: {text}");

    let trace_dir = run_dir.join("threads/main.h1");
    let diff_path = trace_dir.join("worktree.diff");
    assert!(diff_path.exists(), "worktree.diff missing");
    let apply = StdCommand::new("git")
        .arg("-C")
        .arg(&run_dir)
        .arg("apply")
        .arg("--check")
        .arg(&diff_path)
        .output()
        .unwrap();
    assert!(apply.status.success(), "git apply --check failed: {}", String::from_utf8_lossy(&apply.stderr));
}
