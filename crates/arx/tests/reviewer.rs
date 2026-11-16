use std::fs;
use std::path::Path;
use std::sync::Mutex;

mod common;
use common::*;

static DOCKER_STATE_SERIAL: Mutex<()> = Mutex::new(());

fn serialize_on_docker_state() -> std::sync::MutexGuard<'static, ()> {
    DOCKER_STATE_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

const REVIEWER_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["verdict","summary","must_fix"],
 "properties":{
   "verdict":{"type":"string","enum":["PASS","FAIL"]},
   "summary":{"type":"string"},
   "must_fix":{"type":"array","items":{"type":"string"}}}}"#;

fn ledger_rows(workspace_run: &Path) -> Vec<serde_json::Value> {
    let contents = fs::read_to_string(workspace_run.join("threads/main.ledger.jsonl")).unwrap_or_default();
    contents.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

fn arx_verdict_text(rows: &[serde_json::Value], hand: &str) -> String {
    let needle = format!("[hand {hand} verdict]");
    rows.iter()
        .find(|row| {
            row["kind"] == "turn" && row["role"] == "arx" && row["text"].as_str().map(|t| t.starts_with(&needle)).unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("no arx row starts with {needle}: {rows:?}"))["text"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn a_reviewer_verdict_conforms_to_the_schema_and_is_logged() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let codex_home = plant_stub_codex_credential(workspace.path());

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:reviewer:Reviewer the seeded diff".to_string()],
        &[("CODEX_HOME", codex_home.display().to_string())],
    );

    let verdict_text = arx_verdict_text(&rows, "h1");
    assert_eq!(
        verdict_text,
        r#"[hand h1 verdict] {"verdict":"PASS","summary":"stub verdict: Reviewer the seeded diff","must_fix":[]} (trace: threads/main.h1)"#
    );

    let verdict_idx = rows
        .iter()
        .position(|row| row["kind"] == "turn" && row["role"] == "arx" && row["text"].as_str() == Some(verdict_text.as_str()))
        .expect("verdict turn row");
    let schema_idx = rows
        .iter()
        .position(|row| row["kind"] == "schema" && row["hand"] == "h1")
        .expect("schema event row");
    assert_eq!(rows[schema_idx]["outcome"], "ok");
    assert!(schema_idx < verdict_idx, "rows: {rows:?}");

    let trace_dir = run_dir.join("threads/main.h1");
    let schema_bytes = fs::read(trace_dir.join("out/verdict-schema.json")).unwrap();
    assert_eq!(schema_bytes, REVIEWER_SCHEMA.as_bytes());

    let session_contents = fs::read_to_string(trace_dir.join("session.jsonl")).unwrap();
    let session_lines: Vec<&str> = session_contents.lines().collect();
    assert!(session_lines[0].contains("--output-schema"), "line0: {}", session_lines[0]);
    assert!(session_lines[0].contains("/hand/out/verdict-schema.json"), "line0: {}", session_lines[0]);
    assert!(session_lines[2].contains(r#""stub":"schema""#), "line2: {}", session_lines[2]);
    assert!(session_lines[2].contains(r#""present":true"#), "line2: {}", session_lines[2]);
}

#[test]
fn a_malformed_reviewer_verdict_takes_the_failure_path() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let codex_home = plant_stub_codex_credential(workspace.path());

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:reviewer:probe-malformed:".to_string()],
        &[("CODEX_HOME", codex_home.display().to_string())],
    );

    let verdict_text = arx_verdict_text(&rows, "h1");
    assert!(
        verdict_text.starts_with(r#"[hand h1 verdict] reviewer verdict malformed: missing field summary. Raw: {"verdict":"maybe"}"#),
        "text: {verdict_text}"
    );

    let schema_row = rows
        .iter()
        .find(|row| row["kind"] == "schema" && row["hand"] == "h1")
        .expect("schema row");
    assert_eq!(schema_row["outcome"], "malformed");
    assert_eq!(schema_row["reason"], "missing field summary");

    let verdict_row = ledger_rows(&run_dir)
        .into_iter()
        .find(|row| row["kind"] == "verdict" && row["hand"] == "h1")
        .expect("ledger verdict row");
    assert_eq!(verdict_row["text"], r#"{"verdict":"maybe"}"#);
}

#[test]
fn a_reviewer_write_is_refused_and_the_refusal_is_in_the_log() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let codex_home = plant_stub_codex_credential(workspace.path());

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:reviewer:probe-write: VERDICT.md".to_string()],
        &[("CODEX_HOME", codex_home.display().to_string())],
    );

    let verdict_text = arx_verdict_text(&rows, "h1");
    assert!(verdict_text.contains("write refused"), "text: {verdict_text}");

    let schema_row = rows
        .iter()
        .find(|row| row["kind"] == "schema" && row["hand"] == "h1")
        .expect("schema row");
    assert_eq!(schema_row["outcome"], "ok");

    let trace_dir = run_dir.join("threads/main.h1");
    assert!(!trace_dir.join("worktree.diff").exists(), "a read-only reviewer must not leave a worktree.diff");
    assert!(!run_dir.join("VERDICT.md").exists(), "the scratch repo must be unchanged for a read-only reviewer");

    let verdict_row = ledger_rows(&run_dir)
        .into_iter()
        .find(|row| row["kind"] == "verdict" && row["hand"] == "h1")
        .expect("ledger verdict row");
    assert!(
        verdict_row["text"].as_str().unwrap().contains("write refused"),
        "ledger verdict text: {}",
        verdict_row["text"]
    );
}

#[test]
fn a_reviewer_in_mains_family_warns_and_still_runs() {
    let _guard = serialize_on_docker_state();
    ensure_claude_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let claude_config_dir = plant_stub_credential(workspace.path());
    rewrite_reviewer_definition(workspace.path(), "claude-code", "stub-model", &claude_stub_tag());

    let (rows, stdout) = run_session_with_stdout(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:reviewer:Reviewer the seeded diff".to_string()],
        &[("CLAUDE_CONFIG_DIR", claude_config_dir.display().to_string())],
    );

    assert!(
        stdout.contains("[arx] reviewer 'reviewer' is claude, the same family as main; its verdict is not independent"),
        "stdout: {stdout}"
    );

    let family_idx = rows
        .iter()
        .position(|row| row["kind"] == "family" && row["agent"] == "reviewer" && row["family"] == "claude")
        .expect("family event row");
    let spawn_idx = rows
        .iter()
        .position(|row| row["kind"] == "spawn" && row["agent"] == "reviewer")
        .expect("spawn event row");
    assert!(family_idx < spawn_idx, "rows: {rows:?}");

    let ledger_spawn = ledger_rows(&run_dir)
        .into_iter()
        .find(|row| row["kind"] == "spawn" && row["agent"] == "reviewer")
        .expect("ledger spawn row for reviewer");
    let hand = ledger_spawn["hand"].as_str().unwrap().to_string();

    assert!(
        rows.iter()
            .any(|row| row["kind"] == "turn" && row["role"] == "arx" && row["text"].as_str().map(|t| t.starts_with(&format!("[hand {hand} verdict]"))).unwrap_or(false)),
        "no verdict delivery for {hand}: {rows:?}"
    );

    let schema_row = rows
        .iter()
        .find(|row| row["kind"] == "schema" && row["hand"] == hand.as_str())
        .expect("schema row for the claude reviewer");
    assert_eq!(schema_row["outcome"], "malformed");
    assert!(
        schema_row["reason"].as_str().unwrap().starts_with("not JSON"),
        "reason: {}",
        schema_row["reason"]
    );
}

#[test]
fn a_hand_dispatch_carries_no_schema_flag_and_no_schema_event() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let codex_home = plant_stub_codex_credential(workspace.path());

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-cx:Trace a bug".to_string()],
        &[("CODEX_HOME", codex_home.display().to_string())],
    );

    let text = arx_verdict_text(&rows, "h1");
    assert_eq!(text, "[hand h1 verdict] stub verdict: Trace a bug (trace: threads/main.h1)");

    let trace_dir = run_dir.join("threads/main.h1");
    let session_contents = fs::read_to_string(trace_dir.join("session.jsonl")).unwrap();
    let argv_line = session_contents.lines().next().unwrap();
    assert!(!argv_line.contains("--output-schema"), "argv line: {argv_line}");

    assert!(!rows.iter().any(|row| row["kind"] == "schema"), "rows: {rows:?}");
}

#[test]
fn runner_dispatch_is_write_enabled_and_carries_no_reviewer_schema() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let codex_home = plant_stub_codex_credential(workspace.path());

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:runner:probe-write: RUNNER.md".to_string()],
        &[("CODEX_HOME", codex_home.display().to_string())],
    );

    assert!(rows.iter().any(|row| {
        row["kind"] == "spawn" && row["agent"] == "runner" && row["role"] == "runner"
    }), "{rows:?}");
    assert!(!rows.iter().any(|row| row["kind"] == "schema"), "{rows:?}");
    let trace_dir = run_dir.join("threads/main.h1");
    assert!(trace_dir.join("worktree.diff").exists());
    let session = fs::read_to_string(trace_dir.join("session.jsonl")).unwrap();
    assert!(!session.lines().next().unwrap().contains("--output-schema"));
}

#[test]
fn the_roster_lists_the_runner_and_reviewer_pair() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let rows = run_session(&run_dir, "arx.toml", &["hand:dispatch:fake:noop".to_string()], &[]);

    let system_row = rows.iter().find(|row| row["kind"] == "system").expect("system row");
    let text = system_row["text"].as_str().unwrap();
    assert!(
        text.contains("- runner: gpt-5.6-sol, codex, tools Read, Glob, Grep, Bash"),
        "text: {text}"
    );
    assert!(
        text.contains("- reviewer: gpt-5.6-sol, codex, read-only; returns a JSON verdict under arx's schema"),
        "text: {text}"
    );
    assert!(
        text.contains(
            "Runner and reviewer are a pair. Dispatch runner with an agreed brief; it implements the brief, runs its checks and never commits. Once its result lands, dispatch reviewer with the same brief and the runner's trace. Reviewer checks the result independently and returns PASS or FAIL with must_fix."
        ),
        "text: {text}"
    );
}
