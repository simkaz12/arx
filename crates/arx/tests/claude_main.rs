use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::tempdir;

#[path = "../src/claude_code.rs"]
mod claude_code;
#[path = "../src/codex_auth.rs"]
mod codex_auth;
#[path = "../src/codex_main.rs"]
mod codex_main;
mod common;
#[path = "../src/gate.rs"]
mod gate;
#[path = "../src/main_body.rs"]
mod main_body;
#[path = "../src/main_tools.rs"]
mod main_tools;

use arx_engine::StoreCall;
use common::*;
use gate::GateVerdict;
use main_body::{MainBody as ClaudeCodeMain, MainError as ClaudeCodeError, SpawnOpts, SystemPrompt};

fn allow(_call: &gate::GateCall) -> GateVerdict {
    GateVerdict::Allow
}

fn no_model_fallback(_from: &str, _to: &str) {}

fn no_store(_call: &StoreCall) -> serde_json::Value {
    serde_json::json!({"error": "no store"})
}

fn no_hand(_call: &claude_code::HandCall) -> serde_json::Value {
    serde_json::json!({"error": "no hands"})
}

fn no_file(_call: &claude_code::FileCall) -> serde_json::Value {
    serde_json::json!({"error": "no files"})
}

fn parse_hand_prefix(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("[hand ")?;
    let space_idx = rest.find(' ')?;
    let hand = &rest[..space_idx];
    let after_hand = &rest[space_idx + 1..];
    let bracket_idx = after_hand.find(']')?;
    let kind = &after_hand[..bracket_idx];
    Some((hand, kind))
}

fn spawn_stub() -> ClaudeCodeMain {
    let workspace = tempdir().unwrap();
    ClaudeCodeMain::spawn(
        &stub_dir(),
        SpawnOpts {
            model: "stub-model".to_string(),
            system_prompt: SystemPrompt {
                static_block: "stub prompt".to_string(),
                dynamic: String::new(),
            },
            cwd: workspace.keep(),
        },
    )
    .unwrap()
}

#[test]
fn kill_then_respawn_does_not_double_signal_and_the_new_child_answers() {
    let mut adapter = spawn_stub();
    adapter.kill();
    adapter.kill();

    let opts = SpawnOpts {
        model: "stub-model".to_string(),
        system_prompt: SystemPrompt {
            static_block: "stub prompt".to_string(),
            dynamic: String::new(),
        },
        cwd: tempdir().unwrap().keep(),
    };
    adapter.respawn(&stub_dir(), &opts).unwrap();

    let outcome = adapter.run_turn("hello", |_| {}, no_model_fallback, no_store, no_hand, no_file).unwrap();
    assert_eq!(outcome.text, "stub:hello");
}

#[test]
fn one_turn_returns_stub_reply_with_usage_and_ordered_deltas() {
    let mut adapter = spawn_stub();
    let mut deltas = Vec::new();
    let outcome = adapter
        .run_turn("hello", |delta| deltas.push(delta.to_string()), no_model_fallback,
            no_store, no_hand, no_file)
        .unwrap();

    assert_eq!(outcome.text, "stub:hello");
    assert_eq!(deltas, vec!["stub:".to_string(), "hello".to_string()]);
    assert_eq!(
        outcome.usage,
        Some(serde_json::json!({
            "input_tokens": 7,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 40,
            "output_tokens": 5
        }))
    );
}

#[test]
fn two_prompts_on_one_adapter_both_complete() {
    let mut adapter = spawn_stub();

    let first = adapter.run_turn("alpha", |_| {}, no_model_fallback, no_store, no_hand, no_file).unwrap();
    assert_eq!(first.text, "stub:alpha");

    let second = adapter.run_turn("beta", |_| {}, no_model_fallback, no_store, no_hand, no_file).unwrap();
    assert_eq!(second.text, "stub:beta");
}

#[test]
fn hang_then_kill_returns_and_child_is_gone() {
    let mut adapter = spawn_stub();
    let mut hung = adapter.clone();

    let (delta_seen_tx, delta_seen_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        hung.run_turn(
            "hang",
            |_| {
                let _ = delta_seen_tx.send(());
            },
            no_model_fallback,
            no_store,
            no_hand,
            no_file,
        )
    });

    delta_seen_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("stub emitted the hang delta");

    adapter.kill();

    let result = handle.join().expect("hang thread panicked");
    assert!(matches!(result, Err(ClaudeCodeError::ChildDiedMidTurn)));
}

#[test]
fn die_mid_turn_surfaces_child_died_error() {
    let mut adapter = spawn_stub();
    let err = adapter.run_turn("die", |_| {}, no_model_fallback, no_store, no_hand, no_file).unwrap_err();
    assert!(matches!(err, ClaudeCodeError::ChildDiedMidTurn));
}

fn store_test_workspace() -> tempfile::TempDir {
    let workspace = tempdir().unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    copy_dir_recursive(&fixtures.join("run/agents"), &workspace.path().join("agents"));

    fs::write(workspace.path().join("arx.toml"), run_fixture_toml(&stub_dir().display().to_string(), "testscope", None)).unwrap();

    fs::write(workspace.path().join("README.md"), "drain candidate answer is already documented here\n").unwrap();
    let git = |args: &[&str]| {
        let status = StdCommand::new("git").arg("-C").arg(workspace.path()).args(args).status().unwrap();
        assert!(status.success(), "git {args:?} failed in {}", workspace.path().display());
    };
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["-c", "user.name=arx", "-c", "user.email=arx@localhost", "commit", "-q", "-m", "init"]);

    plant_engine_scope(&workspace.path().join("store"), "testscope");

    workspace
}

#[test]
fn run_cli_streams_stub_reply_and_writes_transcript_rows() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("hello\n")
        .assert()
        .success()
        .stdout(contains("stub:"));

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0]["kind"], "system");
    assert_eq!(rows[1]["kind"], "turn");
    assert_eq!(rows[1]["role"], "operator");
    assert_eq!(rows[2]["kind"], "turn");
    assert_eq!(rows[2]["role"], "main");
    assert_eq!(rows[3]["kind"], "usage");
}

#[test]
fn n_tool_calls_produce_n_gate_lines() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("file:read_file:{\"path\":\"a\"}\nfile:read_file:{\"path\":\"b\"}\nfile:read_file:{\"path\":\"c\"}\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    let gate_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "gate").collect();
    assert_eq!(gate_rows.len(), 3);
    assert_eq!(gate_rows[0]["tool"], "mcp__arx__read_file");
    assert_eq!(gate_rows[1]["tool"], "mcp__arx__read_file");
    assert_eq!(gate_rows[2]["tool"], "mcp__arx__read_file");
    for row in &gate_rows {
        assert_eq!(row["verdict"], "allow");
    }

    let last_gate_index = rows.iter().rposition(|row| row["kind"] == "gate").unwrap();
    let turn_index = rows
        .iter()
        .rposition(|row| row["kind"] == "turn" && row["role"] == "main")
        .unwrap();
    assert!(
        turn_index > last_gate_index,
        "the turn row must land after every gate row: {rows:?}"
    );
}

#[test]
fn terminal_shows_what_the_transcript_logs() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let prompts = ["alpha", "beta"];

    let output = Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("alpha\nbeta\n")
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8(output.stdout).unwrap();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    let turn_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "turn").collect();
    assert_eq!(turn_rows.len(), 4);

    for row in &turn_rows {
        let text = row["text"].as_str().unwrap();
        match row["role"].as_str().unwrap() {
            "main" => assert!(
                stdout.contains(text),
                "main text {text:?} not found in captured stdout: {stdout:?}"
            ),
            "operator" => assert!(
                prompts.contains(&text),
                "operator text {text:?} does not match a piped prompt line"
            ),
            other => panic!("unexpected turn role: {other}"),
        }
    }
}

#[test]
fn deltas_that_match_the_turn_text_render_and_log_it_exactly_once() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &["delta-then-turn-matches".to_string()], &[]);

    assert_eq!(stdout.matches("AB").count(), 1, "stdout: {stdout}");
    let main_turn = rows
        .iter()
        .find(|row| row["kind"] == "turn" && row["role"] == "main")
        .expect("main turn row");
    assert_eq!(main_turn["text"], "AB", "rows: {rows:?}");
}

#[test]
fn invalid_utf8_stdin_fails_cleanly_instead_of_silent_eof() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let invalid_line: Vec<u8> = vec![0xFF, 0xFE, 0xFD, b'\n'];

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin(invalid_line)
        .assert()
        .failure()
        .stderr(contains("error:"));
}

#[test]
fn broken_stdout_during_streaming_kills_the_body_and_fails_fast() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let log_path = workspace.path().join("run/threads/main.jsonl");

    let mut run = StdCommand::new(cargo_bin("arx"))
        .args(["run", config_path.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = run.stdout.take().unwrap();
    drop(stdout);

    let mut stderr = run.stderr.take().unwrap();
    let stderr_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });

    let mut stdin = run.stdin.take().unwrap();
    stdin.write_all(b"hang\n").unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = run.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = run.kill();
            panic!("arx run did not exit after its stdout pipe broke");
        }
        thread::sleep(Duration::from_millis(20));
    };

    assert!(!status.success(), "expected failure once stdout could not be written to");

    let stderr_bytes = stderr_handle.join().unwrap();
    let stderr_text = String::from_utf8_lossy(&stderr_bytes);
    assert!(
        stderr_text.contains("error:"),
        "unexpected stderr: {stderr_text}"
    );

    let contents = fs::read_to_string(&log_path).unwrap();
    let rows: Vec<serde_json::Value> = contents
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(
        !rows.iter().any(|row| row["kind"] == "turn" && row["role"] == "main"),
        "the failed turn must not be logged as a completed main reply: {rows:?}"
    );
}

#[test]
fn gauge_cli_prints_occupancy_from_last_usage_row() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("hello\n")
        .assert()
        .success();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["gauge", config_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("occupancy 47"));
}

#[test]
fn gauge_cli_prints_arx_configured_main_window_not_the_body_compaction_constant() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("hello\n")
        .assert()
        .success();

    let output = Command::cargo_bin("arx")
        .unwrap()
        .args(["gauge", config_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("window 800000"), "expected the default configured main window: {text}");
    assert!(!text.contains("window 1000000"), "must not print the body's auto-compact constant as arx's window: {text}");
}

#[test]
fn gauge_cli_reflects_a_non_default_configured_window_and_reserve() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    let contents = fs::read_to_string(&config_path).unwrap();
    let updated = contents.replacen(
        "instructions = \"prompts/main.md\"\n",
        "instructions = \"prompts/main.md\"\nwindow = 250000\nreserve = 5000\n",
        1,
    );
    assert_ne!(contents, updated, "config: {contents}");
    fs::write(&config_path, updated).unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("hello\n")
        .assert()
        .success();

    let output = Command::cargo_bin("arx")
        .unwrap()
        .args(["gauge", config_path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();

    assert!(text.contains("window 250000"), "expected the fixture's configured window: {text}");
    assert!(text.contains("reserve 5000"), "expected the fixture's configured reserve: {text}");
    assert!(!text.contains("window 800000"), "must not fall back to the default window: {text}");
    assert!(!text.contains("window 1000000"), "must not print the body's auto-compact constant as arx's window: {text}");
}

#[test]
fn gauge_cli_fails_without_a_threads_dir() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["gauge", config_path.to_str().unwrap()])
        .assert()
        .failure()
        .code(1);
}

fn wait_for_nonempty_file(path: &Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if fs::metadata(path).map(|meta| meta.len() > 0).unwrap_or(false) {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn two_concurrent_run_processes_one_wins_the_lock_and_the_transcript_stays_parseable() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let log_path = workspace.path().join("run/threads/main.jsonl");

    let mut first = StdCommand::new(cargo_bin("arx"))
        .args(["run", config_path.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    assert!(
        wait_for_nonempty_file(&log_path, Duration::from_secs(10)),
        "first process never opened and locked the transcript log"
    );

    let second = Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .assert()
        .failure();
    let second_stderr = String::from_utf8_lossy(&second.get_output().stderr).into_owned();
    assert!(
        second_stderr.contains("locked by another writer"),
        "unexpected stderr: {second_stderr}"
    );
    assert!(
        second_stderr.contains("main.jsonl"),
        "unexpected stderr: {second_stderr}"
    );

    {
        let mut stdin = first.stdin.take().unwrap();
        stdin.write_all(b"hello\n").unwrap();
    }
    let first_status = first.wait().unwrap();
    assert!(first_status.success());

    let contents = fs::read_to_string(&log_path).unwrap();
    for (idx, line) in contents.lines().enumerate() {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|err| panic!("line {}: {err}\n{line}", idx + 1));
    }
}

fn wait_for_usage_row(path: &Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(contents) = fs::read_to_string(path) {
            if contents.contains("\"kind\":\"usage\"") {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn hammered_gauge_survives_a_live_append_stream() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let log_path = workspace.path().join("run/threads/main.jsonl");

    let mut run = StdCommand::new(cargo_bin("arx"))
        .args(["run", config_path.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = run.stdin.take().unwrap();
    let mut stdout = run.stdout.take().unwrap();
    let mut stderr = run.stderr.take().unwrap();
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stdout.read_to_end(&mut sink);
    });
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stderr.read_to_end(&mut sink);
    });

    const TURNS: usize = 3000;
    let big_prompt = "x".repeat(4096);
    let writer = thread::spawn(move || {
        for _ in 0..TURNS {
            if stdin.write_all(big_prompt.as_bytes()).is_err() {
                break;
            }
            if stdin.write_all(b"\n").is_err() {
                break;
            }
        }
        drop(stdin);
    });

    assert!(
        wait_for_usage_row(&log_path, Duration::from_secs(10)),
        "no usage row landed before the hammer window opened"
    );

    let stop = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(AtomicUsize::new(0));
    let failure_messages = Arc::new(Mutex::new(Vec::new()));

    let hammer_handles: Vec<_> = (0..8)
        .map(|_| {
            let config_path = config_path.clone();
            let stop = Arc::clone(&stop);
            let calls = Arc::clone(&calls);
            let failures = Arc::clone(&failures);
            let failure_messages = Arc::clone(&failure_messages);
            thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let output = StdCommand::new(cargo_bin("arx"))
                        .args(["gauge", config_path.to_str().unwrap()])
                        .output()
                        .unwrap();
                    calls.fetch_add(1, Ordering::Relaxed);
                    if !output.status.success() {
                        failures.fetch_add(1, Ordering::Relaxed);
                        failure_messages
                            .lock()
                            .unwrap()
                            .push(String::from_utf8_lossy(&output.stderr).into_owned());
                    }
                }
            })
        })
        .collect();

    writer.join().unwrap();
    stop.store(true, Ordering::Relaxed);
    for handle in hammer_handles {
        handle.join().unwrap();
    }

    let run_status = run.wait().unwrap();
    assert!(run_status.success(), "arx run did not exit cleanly");

    let observed_calls = calls.load(Ordering::Relaxed);
    let observed_failures = failures.load(Ordering::Relaxed);
    assert!(observed_calls > 0, "the hammer loop never ran a single gauge call");
    assert_eq!(
        observed_failures,
        0,
        "{observed_failures} of {observed_calls} gauge calls failed against a live append: {:?}",
        failure_messages.lock().unwrap()
    );

    let contents = fs::read_to_string(&log_path).unwrap();
    for (idx, line) in contents.lines().enumerate() {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|err| panic!("line {}: {err}\n{line}", idx + 1));
    }
}

#[test]
fn root_and_subcommand_help_are_documented() {
    Command::cargo_bin("arx")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("check-config").and(contains("run")).and(contains("gauge")));

    for command in ["check-config", "run", "gauge"] {
        Command::cargo_bin("arx")
            .unwrap()
            .args([command, "--help"])
            .assert()
            .success()
            .stdout(contains("Path to arx.toml"));
    }
}

#[test]
fn model_command_with_no_argument_prints_the_roster_and_sends_no_op() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &["/model".to_string()], &[]);

    assert!(stdout.contains("[arx] model: roster"), "stdout: {stdout}");
    assert!(stdout.contains("main: stub-model (claude-code) — from file"), "stdout: {stdout}");
    assert!(!run_dir.join("stub-set-model.json").exists());
    assert!(!rows.iter().any(|row| row["kind"] == "model"), "rows: {rows:?}");
}

#[test]
fn model_command_with_an_unlisted_id_refuses() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &["/model nope".to_string()], &[]);

    assert!(stdout.contains("[arx] model: nope is not in [[models]]; available:"), "stdout: {stdout}");
    assert!(stdout.contains("stub-model-2"), "stdout: {stdout}");
    assert!(!run_dir.join("stub-set-model.json").exists());
    assert!(!rows.iter().any(|row| row["kind"] == "model"), "rows: {rows:?}");
}

#[test]
fn model_command_switches_to_a_listed_model() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &["/model stub-model-2".to_string()], &[]);

    assert!(stdout.contains("[arx] model: stub-model -> stub-model-2"), "stdout: {stdout}");
    let sent: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(run_dir.join("stub-set-model.json")).unwrap()).unwrap();
    assert_eq!(sent["model"], "stub-model-2");

    let model_events: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "model").collect();
    assert_eq!(model_events.len(), 1, "rows: {rows:?}");
    assert_eq!(model_events[0]["position"], "main");
    assert_eq!(model_events[0]["from"], "stub-model");
    assert_eq!(model_events[0]["to"], "stub-model-2");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["events", "--path", config_path.to_str().unwrap(), "main"])
        .assert()
        .success()
        .stdout(contains("## model (1)"));
}

#[test]
fn model_command_pins_a_non_main_position_and_clear_reports_the_fallback() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let lines = vec![
        "/model hand-ex gpt-5.6-sol".to_string(),
        "/model".to_string(),
        "/model clear hand-ex".to_string(),
    ];
    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &[]);

    assert!(stdout.contains("[arx] model: hand-ex pinned to gpt-5.6-sol"), "stdout: {stdout}");
    assert!(stdout.contains("hand-ex: gpt-5.6-sol (codex) — pinned"), "stdout: {stdout}");
    assert!(
        stdout.contains("[arx] model: hand-ex cleared; falling back to claude-sonnet-5 from arx.toml"),
        "stdout: {stdout}"
    );

    let model_events: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "model").collect();
    assert_eq!(model_events.len(), 2, "rows: {rows:?}");
    assert_eq!(model_events[0]["position"], "hand-ex");
    assert_eq!(model_events[0]["to"], "gpt-5.6-sol");
    assert_eq!(model_events[1]["position"], "hand-ex");
    assert_eq!(model_events[1]["from"], "gpt-5.6-sol");
    assert_eq!(model_events[1]["to"], "claude-sonnet-5");

    let roster_dir = run_dir.join("roster");
    let roster_files: Vec<_> = fs::read_dir(&roster_dir).unwrap().collect();
    assert_eq!(roster_files.len(), 1, "exactly one roster file must have been created");
}

#[test]
fn model_command_unknown_position_and_unknown_id_both_print_a_listing_error() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &["/model nonesuch x".to_string()], &[]);
    assert!(stdout.contains("[arx] model: nonesuch is not a known position; available:"), "stdout: {stdout}");
    assert!(!rows.iter().any(|row| row["kind"] == "model"), "rows: {rows:?}");

    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &["/model hand-ex nonesuch".to_string()], &[]);
    assert!(stdout.contains("[arx] model: nonesuch is not in [[models]]; available:"), "stdout: {stdout}");
    assert!(!rows.iter().any(|row| row["kind"] == "model"), "rows: {rows:?}");
}

#[test]
fn model_command_refuses_a_registry_id_whose_body_does_not_fit_main() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &["/model fake-model".to_string()], &[]);

    assert!(
        stdout.contains("main supports claude-code and codex; provider main is not wired"),
        "stdout: {stdout}"
    );
    assert!(!run_dir.join("stub-set-model.json").exists());
    assert!(!rows.iter().any(|row| row["kind"] == "model"), "rows: {rows:?}");
}

#[test]
fn model_command_clear_main_switches_the_running_model_back_to_the_fallback() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let lines = vec!["/model stub-model-2".to_string(), "/model clear main".to_string()];
    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &[]);

    assert!(
        stdout.contains("[arx] model: main cleared; falling back to stub-model from arx.toml"),
        "stdout: {stdout}"
    );
    let sent: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(run_dir.join("stub-set-model.json")).unwrap()).unwrap();
    assert_eq!(sent["model"], "stub-model", "the clear must issue a live switch back, not just drop the pin");

    let model_events: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "model").collect();
    assert_eq!(model_events.len(), 2, "rows: {rows:?}");
    assert_eq!(model_events[1]["position"], "main");
    assert_eq!(model_events[1]["from"], "stub-model-2");
    assert_eq!(model_events[1]["to"], "stub-model");
}

#[test]
fn model_command_pins_main_even_when_the_chosen_id_already_matches_the_running_model() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &["/model stub-model".to_string()], &[]);

    assert!(stdout.contains("[arx] model: main pinned to stub-model"), "stdout: {stdout}");
    assert!(
        !run_dir.join("stub-set-model.json").exists(),
        "the id already matches the running model, so no live switch call is needed"
    );

    let roster_dir = run_dir.join("roster");
    let roster_file = fs::read_dir(&roster_dir).unwrap().next().unwrap().unwrap().path();
    let roster_contents = fs::read_to_string(&roster_file).unwrap();
    assert!(roster_contents.contains("main = \"stub-model\""), "roster: {roster_contents}");

    let model_events: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "model").collect();
    assert_eq!(model_events.len(), 1, "rows: {rows:?}");
    assert_eq!(model_events[0]["from"], "stub-model");
    assert_eq!(model_events[0]["to"], "stub-model");

    let (rows_again, stdout_again) = run_session_with_stdout(&run_dir, "arx.toml", &["/model stub-model".to_string()], &[]);
    assert!(stdout_again.contains("[arx] model: already stub-model"), "stdout: {stdout_again}");
    let model_events_again = rows_again.iter().filter(|row| row["kind"] == "model").count();
    assert_eq!(
        model_events_again,
        model_events.len(),
        "the repeated, truly-already call must not add another model row: rows: {rows_again:?}"
    );
}

#[test]
fn an_sdk_refusal_fallback_updates_current_model_and_logs_an_event() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let rows = run_session(&run_dir, "arx.toml", &["model-fallback".to_string()], &[]);

    let model_events: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "model").collect();
    assert_eq!(model_events.len(), 1, "rows: {rows:?}");
    assert_eq!(model_events[0]["from"], "stub-model");
    assert_eq!(model_events[0]["to"], "stub-model-2");

    let usage_row = rows.iter().find(|row| row["kind"] == "usage").expect("usage row");
    assert_eq!(usage_row["usage"]["model"], "stub-model-2", "usage row: {usage_row:?}");
}

#[test]
fn a_usage_row_with_one_sdk_reported_model_carries_it_as_sdk_sourced() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let rows = run_session(&run_dir, "arx.toml", &["usage-models:stub-model".to_string()], &[]);

    let usage_row = rows.iter().find(|row| row["kind"] == "usage").expect("usage row");
    assert_eq!(usage_row["usage"]["model"], "stub-model", "usage row: {usage_row:?}");
    assert_eq!(usage_row["usage"]["model_source"], "sdk", "usage row: {usage_row:?}");
}

#[test]
fn a_usage_row_without_sdk_models_falls_back_to_current_model_with_arx_source() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let rows = run_session(&run_dir, "arx.toml", &["hello".to_string()], &[]);

    let usage_row = rows.iter().find(|row| row["kind"] == "usage").expect("usage row");
    assert_eq!(usage_row["usage"]["model"], "stub-model", "usage row: {usage_row:?}");
    assert_eq!(usage_row["usage"]["model_source"], "arx", "usage row: {usage_row:?}");
    assert!(usage_row["usage"].get("models").is_none(), "usage row: {usage_row:?}");
}

#[test]
fn a_usage_row_with_an_empty_models_list_also_falls_back_to_current_model_with_arx_source() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let rows = run_session(&run_dir, "arx.toml", &["usage-models:".to_string()], &[]);

    let usage_row = rows.iter().find(|row| row["kind"] == "usage").expect("usage row");
    assert_eq!(usage_row["usage"]["model"], "stub-model", "usage row: {usage_row:?}");
    assert_eq!(usage_row["usage"]["model_source"], "arx", "usage row: {usage_row:?}");
}

#[test]
fn a_usage_row_with_several_grown_models_joins_them_and_omits_model() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let rows = run_session(&run_dir, "arx.toml", &["usage-models:stub-model,stub-model-2".to_string()], &[]);

    let usage_row = rows.iter().find(|row| row["kind"] == "usage").expect("usage row");
    assert!(usage_row["usage"].get("model").is_none(), "usage row: {usage_row:?}");
    assert_eq!(usage_row["usage"]["models"], "stub-model+stub-model-2", "usage row: {usage_row:?}");
    assert_eq!(usage_row["usage"]["model_source"], "sdk", "usage row: {usage_row:?}");
}

#[test]
fn a_usage_row_whose_sdk_model_disagrees_with_arx_prints_a_notice_and_leaves_current_model_alone() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let lines = vec!["usage-models:stub-model-2".to_string(), "/model".to_string()];
    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &[]);

    assert!(
        stdout.contains("[arx] model: sidecar reports stub-model-2, arx expected stub-model"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("main: stub-model (claude-code) — from file"), "stdout: {stdout}");

    let usage_row = rows.iter().find(|row| row["kind"] == "usage").expect("usage row");
    assert_eq!(usage_row["usage"]["model"], "stub-model-2", "usage row: {usage_row:?}");
    assert_eq!(usage_row["usage"]["model_source"], "sdk", "usage row: {usage_row:?}");
    assert!(!rows.iter().any(|row| row["kind"] == "model"), "rows: {rows:?}");
}

#[test]
fn grounding_lands_in_the_system_row() {
    let workspace = store_test_workspace();
    let config_path = workspace.path().join("arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("hello\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let system_row = rows.iter().find(|row| row["kind"] == "system").unwrap();
    let text = system_row["text"].as_str().unwrap();

    assert!(text.contains("what is the launch codename"), "system row: {text}");
    assert!(text.contains("skylark"), "system row: {text}");
    assert!(!text.contains("what is the fallback contact"), "system row: {text}");
    assert!(!text.contains("orion desk"), "system row: {text}");
}

#[test]
fn recall_returns_file_hits_as_tool_result() {
    let workspace = store_test_workspace();
    let config_path = workspace.path().join("arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("store:recall:{\"scope\":\"testscope\",\"query\":\"skylark\",\"limit\":5}\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_row = rows
        .iter()
        .find(|row| row["kind"] == "turn" && row["role"] == "main")
        .unwrap();
    let text = main_row["text"].as_str().unwrap();

    assert!(text.contains("skylark"), "main turn text: {text}");
}

#[test]
fn save_quarantines_and_leaves_the_store_file_untouched() {
    let workspace = store_test_workspace();
    let config_path = workspace.path().join("arx.toml");
    let store_root = workspace.path().join("store");
    let scope_dir = store_root.join("testscope");
    let before_files = scope_md_files(&scope_dir);
    let before_head = head_count(&store_root);

    let save_args = "{\"scope\":\"testscope\",\"kind\":\"fact\",\"question\":\"who used the skylark codename\",\"answer\":\"ops team\",\"why\":\"handoff record\"}";
    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin(format!("store:save:{save_args}\n"))
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_row = rows
        .iter()
        .find(|row| row["kind"] == "turn" && row["role"] == "main")
        .unwrap();
    let text = main_row["text"].as_str().unwrap();
    assert!(text.contains("skylark"), "main turn text: {text}");

    let quarantine_dir = workspace.path().join("store/quarantine/main");
    let entries: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(entries.len(), 1);
    let candidate_path = entries.into_iter().next().unwrap().unwrap().path();
    let candidate: serde_json::Value = serde_json::from_str(&fs::read_to_string(&candidate_path).unwrap()).unwrap();
    assert_eq!(candidate["handle"]["thread"], "main");
    assert!(candidate["handle"]["turn"].is_number());

    let after_files = scope_md_files(&scope_dir);
    assert_eq!(before_files, after_files);
    assert_eq!(head_count(&store_root), before_head);
}

#[test]
fn drain_promotes_into_the_store_file() {
    ensure_codex_stub_image();
    let workspace = store_test_workspace();
    let run_dir = workspace.path().to_path_buf();
    let store_root = run_dir.join("store");
    let scope_dir = store_root.join("testscope");
    let before_files = scope_md_files(&scope_dir);
    let before_head = head_count(&store_root);
    let codex_home = plant_stub_codex_credential(workspace.path());
    let env = [("CODEX_HOME".to_string(), codex_home.display().to_string())];
    let env_refs: Vec<(&str, String)> = env.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();

    let save_args = "{\"scope\":\"testscope\",\"kind\":\"fact\",\"question\":\"drain candidate question\",\"answer\":\"drain candidate answer\",\"why\":\"drain test\"}";

    let mut session = LiveSession::spawn(&run_dir, "arx.toml", &env_refs);
    session.drain_output();
    session.write_line(&format!("store:save:{save_args}"));
    session.write_line("store:drain:{}");

    let spawn_deadline = Instant::now() + Duration::from_secs(60);
    let spawn_row = wait_for_row(&mut session, &run_dir, spawn_deadline, |row| {
        row["kind"] == "spawn" && row["role"] == "drain"
    })
        .expect("drain hand spawned within 60s");
    let hand = spawn_row["hand"].as_str().unwrap().to_string();

    let delivered_deadline = Instant::now() + Duration::from_secs(60);
    wait_for_row(&mut session, &run_dir, delivered_deadline, |row| {
        row["kind"] == "delivered" && row["hand"] == hand
    })
        .expect("drain hand delivered within 60s");

    session.write_line(&format!("store:drain:{}", serde_json::json!({"proposal": hand})));
    session.close_stdin();
    let exit_deadline = Instant::now() + Duration::from_secs(30);
    assert!(session.wait_success(exit_deadline), "arx run did not exit successfully");

    let contents = fs::read_to_string(run_dir.join("threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let drain_event = rows
        .iter()
        .find(|row| row["kind"] == "drain")
        .unwrap_or_else(|| panic!("no drain event row: {rows:?}"));
    assert_eq!(drain_event["promoted"], 1, "drain event: {drain_event:?}");
    assert_eq!(drain_event["dropped"], 0, "drain event: {drain_event:?}");

    let after_files = scope_md_files(&scope_dir);
    assert_eq!(after_files.len(), before_files.len() + 1, "expected exactly one new claim file");
    let new_file = after_files.iter().find(|name| !before_files.contains(name)).unwrap();
    let new_content = fs::read_to_string(scope_dir.join(new_file)).unwrap();
    assert!(new_content.contains("## Answer\n\ndrain candidate answer"), "new claim file: {new_content}");
    assert_eq!(head_count(&store_root), before_head + 1);

    let quarantine_dir = run_dir.join("store/quarantine/main");
    let remaining: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert!(remaining.is_empty());
}

#[test]
fn malformed_quarantine_candidate_fails_startup_loudly() {
    let workspace = store_test_workspace();
    let config_path = workspace.path().join("arx.toml");
    let quarantine_dir = workspace.path().join("store/quarantine/main");
    fs::create_dir_all(&quarantine_dir).unwrap();
    fs::write(
        quarantine_dir.join("1-broken.json"),
        "{\"kind\":\"fact\",\"question\":\"missing fields\"}",
    )
    .unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("hello\n")
        .assert()
        .failure()
        .stderr(contains("broken.json"));
}

#[test]
fn null_store_compiles_against_the_trait_at_the_same_call_site() {
    struct NullStore;

    impl arx_engine::Store for NullStore {
        fn save(
            &mut self,
            _intent: &arx_engine::SaveIntent,
            _handle: &arx_engine::ClaimHandle,
        ) -> Result<Vec<arx_engine::Claim>, arx_engine::StoreError> {
            Ok(Vec::new())
        }

        fn recall(&mut self, _scope: &str, _query: &str, _limit: usize) -> Result<Vec<arx_engine::Claim>, arx_engine::StoreError> {
            Ok(Vec::new())
        }

        fn decisions_query(
            &mut self,
            _scope: &str,
            _query: &str,
            _limit: usize,
        ) -> Result<Vec<arx_engine::Claim>, arx_engine::StoreError> {
            Ok(Vec::new())
        }

        fn decision(&mut self, _scope: &str, _id: &str) -> Result<Option<arx_engine::Claim>, arx_engine::StoreError> {
            Ok(None)
        }

        fn drain(&mut self) -> Result<arx_engine::DrainReport, arx_engine::StoreError> {
            Ok(arx_engine::DrainReport { promoted: 0, dropped: 0 })
        }

        fn grounding(&mut self, _scope: &str) -> Result<Vec<arx_engine::Claim>, arx_engine::StoreError> {
            Ok(Vec::new())
        }
    }

    let mut store = NullStore;
    let handle = arx_engine::ClaimHandle {
        thread: "main".to_string(),
        turn: 1,
    };

    let save_result = arx_engine::serve(
        &mut store,
        "save",
        &serde_json::json!({"scope": "arx", "kind": "fact", "question": "q", "answer": "a", "why": "w"}),
        &handle,
    );
    assert_eq!(save_result, serde_json::json!([]));

    let recall_result = arx_engine::serve(
        &mut store,
        "recall",
        &serde_json::json!({"scope": "arx", "query": "q", "limit": 5}),
        &handle,
    );
    assert_eq!(recall_result, serde_json::json!([]));

    let decisions_query_result = arx_engine::serve(
        &mut store,
        "decisions_query",
        &serde_json::json!({"scope": "arx", "query": "q", "limit": 5}),
        &handle,
    );
    assert_eq!(decisions_query_result, serde_json::json!([]));

    let decision_result = arx_engine::serve(
        &mut store,
        "decision",
        &serde_json::json!({"scope": "arx", "id": "x"}),
        &handle,
    );
    assert_eq!(decision_result, serde_json::Value::Null);

    let drain_result = arx_engine::serve(&mut store, "drain", &serde_json::json!({}), &handle);
    assert_eq!(drain_result, serde_json::json!({"promoted": 0}));
}

#[test]
fn three_dispatches_keep_main_turning_and_verdicts_land_at_boundaries() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    let stdin = "hand:dispatch:fake:[delay:50] job 1\nhand:dispatch:fake:[delay:50] job 2\nhand:dispatch:fake:[delay:50] job 3\nplain check\n";

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin(stdin)
        .assert()
        .success();

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let spawn_indices: Vec<usize> = ledger_rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row["kind"] == "spawn")
        .map(|(i, _)| i)
        .collect();
    let verdict_indices: Vec<usize> = ledger_rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row["kind"] == "verdict")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(spawn_indices.len(), 3, "ledger rows: {ledger_rows:?}");
    assert_eq!(verdict_indices.len(), 3, "ledger rows: {ledger_rows:?}");
    let last_spawn = *spawn_indices.iter().max().unwrap();
    let first_verdict = *verdict_indices.iter().min().unwrap();
    assert!(last_spawn < first_verdict, "spawns must all precede any verdict: {ledger_rows:?}");

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let plain_index = rows
        .iter()
        .position(|row| row["kind"] == "turn" && row["role"] == "operator" && row["text"] == "plain check")
        .expect("plain prompt row present");

    let arx_indices: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row["kind"] == "turn" && row["role"] == "arx")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(arx_indices.len(), 3, "rows: {rows:?}");

    for &idx in &arx_indices {
        let text = rows[idx]["text"].as_str().unwrap();
        let (hand, kind) = parse_hand_prefix(text).expect("hand-prefixed injection");
        assert_eq!(kind, "verdict", "text: {text}");
        assert!(["h1", "h2", "h3"].contains(&hand), "text: {text}");
        assert_eq!(rows[idx + 1]["kind"], "turn");
        assert_eq!(rows[idx + 1]["role"], "main");
    }

    assert!(
        arx_indices.iter().all(|&idx| plain_index < idx),
        "plain operator turn should land before any verdict injection while hands are in flight: rows={rows:?}"
    );
}

#[test]
fn steer_reaches_a_running_fake() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    let stdin = "hand:dispatch:fake:[delay:800] steerable\nhand:tap:h1\nhand:steer:h1:go faster\n";

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin(stdin)
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .collect();
    let tap_row_text = main_rows[1]["text"].as_str().unwrap();
    assert!(tap_row_text.contains("running"), "tap reply: {tap_row_text}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let steer_index = ledger_rows.iter().position(|row| row["kind"] == "steer").expect("steer row");
    let verdict_index = ledger_rows.iter().position(|row| row["kind"] == "verdict").expect("verdict row");
    assert!(steer_index < verdict_index, "ledger rows: {ledger_rows:?}");

    let verdict_text = ledger_rows[verdict_index]["text"].as_str().unwrap();
    assert!(verdict_text.contains("go faster"), "verdict text: {verdict_text}");
}

#[test]
fn kill_removes_the_hand_and_injects_the_stub() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let log_path = workspace.path().join("run/threads/main.jsonl");
    let ledger_path = workspace.path().join("run/threads/main.ledger.jsonl");

    let mut run = StdCommand::new(cargo_bin("arx"))
        .args(["run", config_path.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let mut stdin = run.stdin.take().unwrap();
        stdin.write_all(b"hand:dispatch:fake:[delay:60000] doomed\nhand:kill:h1\n").unwrap();
    }

    let mut stdout = run.stdout.take().unwrap();
    let mut stderr = run.stderr.take().unwrap();
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stdout.read_to_end(&mut sink);
    });
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stderr.read_to_end(&mut sink);
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = run.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = run.kill();
            panic!("arx run waited for the killed hand instead of exiting promptly");
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success());

    let ledger_contents = fs::read_to_string(&ledger_path).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert!(
        ledger_rows.iter().any(|row| row["kind"] == "killed" && row["hand"] == "h1"),
        "ledger rows: {ledger_rows:?}"
    );

    let contents = fs::read_to_string(&log_path).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let killed_row = rows.iter().find(|row| {
        row["kind"] == "turn"
            && row["role"] == "arx"
            && row["text"]
                .as_str()
                .map(|t| parse_hand_prefix(t) == Some(("h1", "killed")))
                .unwrap_or(false)
    });
    assert!(killed_row.is_some(), "rows: {rows:?}");
}

#[test]
fn stop_lands_as_a_mechanical_request_and_the_answer_releases_it() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("hand:dispatch:fake:[delay:50] [stop:which db?] needs an answer\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let stop_index = rows
        .iter()
        .position(|row| {
            row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"]
                    .as_str()
                    .map(|t| t == "[hand h1 stop] which db? — answer with the steer tool")
                    .unwrap_or(false)
        })
        .expect("stop injection row");

    let steer_gate_index = stop_index + 1;
    assert_eq!(rows[steer_gate_index]["kind"], "gate");
    assert_eq!(rows[steer_gate_index]["tool"], "mcp__arx__steer");
    assert_eq!(rows[steer_gate_index]["verdict"], "allow");

    let steer_event_index = steer_gate_index + 1;
    assert_eq!(rows[steer_event_index]["kind"], "steer");
    assert_eq!(rows[steer_event_index]["hand"], "h1");

    let steer_admission_index = steer_event_index + 1;
    assert_eq!(rows[steer_admission_index]["kind"], "admission");
    assert_eq!(rows[steer_admission_index]["outcome"], "inline");

    let reply_index = steer_admission_index + 1;
    assert_eq!(rows[reply_index]["kind"], "turn");
    assert_eq!(rows[reply_index]["role"], "main");

    let verdict_index = rows
        .iter()
        .enumerate()
        .find(|(i, row)| {
            *i > reply_index
                && row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"]
                    .as_str()
                    .map(|t| parse_hand_prefix(t) == Some(("h1", "verdict")))
                    .unwrap_or(false)
        })
        .map(|(i, _)| i);
    assert!(verdict_index.is_some(), "rows: {rows:?}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let stop_row_index = ledger_rows.iter().position(|row| row["kind"] == "stop").expect("stop row");
    let steer_row_index = ledger_rows.iter().position(|row| row["kind"] == "steer").expect("steer row");
    let verdict_row_index = ledger_rows.iter().position(|row| row["kind"] == "verdict").expect("verdict row");
    assert!(stop_row_index < steer_row_index, "ledger rows: {ledger_rows:?}");
    assert!(steer_row_index < verdict_row_index, "ledger rows: {ledger_rows:?}");
}

static CLAUDE_CODE_DOCKER_SERIAL: Mutex<()> = Mutex::new(());

fn serialize_on_claude_code_docker_state() -> std::sync::MutexGuard<'static, ()> {
    CLAUDE_CODE_DOCKER_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn claude_code_hand_dispatch_delivers_stub_verdict_with_trace() {
    let _guard = serialize_on_claude_code_docker_state();
    ensure_claude_stub_image();
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let claude_config_dir = plant_stub_credential(workspace.path());

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("CLAUDE_CONFIG_DIR", &claude_config_dir)
        .write_stdin("hand:dispatch:hand-cc:read the file\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_row = rows
        .iter()
        .find(|row| {
            row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("stub verdict:")).unwrap_or(false)
        })
        .expect("verdict row present");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(text.contains("stub verdict: read the file"), "text: {text}");
    assert!(text.contains("(trace: threads/main.h1)"), "text: {text}");

    let trace_dir = workspace.path().join("run/threads/main.h1");
    let session_contents = fs::read_to_string(trace_dir.join("session.jsonl")).unwrap();
    let session_lines: Vec<&str> = session_contents.lines().collect();
    assert_eq!(session_lines.len(), 4, "session.jsonl: {session_contents}");
    assert!(session_lines[0].contains(r#""type":"argv""#), "line0: {}", session_lines[0]);
    assert_eq!(session_lines[1], r#"{"type":"cred","present":true}"#);
    assert_eq!(session_lines[2], r#"{"type":"line2"}"#);
    assert_eq!(session_lines[3], r#"{"type":"env","canary":false}"#);

    let trace_md = fs::read_to_string(trace_dir.join("trace.md")).unwrap();
    assert!(trace_md.contains("path: session.jsonl"), "trace.md: {trace_md}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let spawn_row = ledger_rows.iter().find(|row| row["kind"] == "spawn").expect("spawn row");
    assert_eq!(spawn_row["agent"], "hand-cc");
    assert!(ledger_rows.iter().any(|row| row["kind"] == "verdict"), "ledger rows: {ledger_rows:?}");
}

fn jsonl_files_recursive(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(jsonl_files_recursive(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            found.push(path);
        }
    }
    found
}

fn wait_for_path(path: &Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

fn wait_for_ledger_spawn(ledger_path: &Path, hand: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(contents) = fs::read_to_string(ledger_path) {
            let spawned = contents.lines().any(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .map(|row| row["kind"] == "spawn" && row["hand"] == hand)
                    .unwrap_or(false)
            });
            if spawned {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

fn spawn_row_root(ledger_path: &Path, hand: &str) -> Option<String> {
    let contents = fs::read_to_string(ledger_path).ok()?;
    contents
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|row| row["kind"] == "spawn" && row["hand"] == hand)
        .and_then(|row| row["root"].as_str().map(|s| s.to_string()))
}

#[test]
fn claude_code_hand_kill_exits_promptly_and_leaves_no_stray_process() {
    let _guard = serialize_on_claude_code_docker_state();
    ensure_claude_stub_image();
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let claude_config_dir = plant_stub_credential(workspace.path());
    let ledger_path = workspace.path().join("run/threads/main.ledger.jsonl");
    let log_path = workspace.path().join("run/threads/main.jsonl");

    let mut run = StdCommand::new(cargo_bin("arx"))
        .args(["run", config_path.to_str().unwrap()])
        .env("CLAUDE_CONFIG_DIR", &claude_config_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = run.stdin.take().unwrap();
    stdin.write_all(b"hand:dispatch:hand-cc:[sleep:60] doomed\n").unwrap();

    let root_deadline = Instant::now() + Duration::from_secs(5);
    let mut root: Option<String> = None;
    while root.is_none() {
        root = spawn_row_root(&ledger_path, "h1");
        if root.is_some() {
            break;
        }
        assert!(Instant::now() < root_deadline, "spawn row with root not seen within 5s");
        thread::sleep(Duration::from_millis(20));
    }
    let root = root.unwrap();
    let pid_file = Path::new(&root).join("home/.claude/claude-stub.pid");

    assert!(
        wait_for_path(&pid_file, Duration::from_secs(5)),
        "stub never wrote its pid file"
    );

    stdin.write_all(b"hand:kill:h1\n").unwrap();
    drop(stdin);

    let mut stdout = run.stdout.take().unwrap();
    let mut stderr = run.stderr.take().unwrap();
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stdout.read_to_end(&mut sink);
    });
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stderr.read_to_end(&mut sink);
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = run.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = run.kill();
            panic!("arx run waited for the killed claude-code hand instead of exiting promptly");
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success());

    let ledger_contents = fs::read_to_string(&ledger_path).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert!(
        ledger_rows.iter().any(|row| row["kind"] == "killed" && row["hand"] == "h1"),
        "ledger rows: {ledger_rows:?}"
    );

    let contents = fs::read_to_string(&log_path).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let killed_row = rows.iter().find(|row| {
        row["kind"] == "turn"
            && row["role"] == "arx"
            && row["text"]
                .as_str()
                .map(|t| parse_hand_prefix(t) == Some(("h1", "killed")))
                .unwrap_or(false)
    });
    assert!(killed_row.is_some(), "rows: {rows:?}");

    let poll_deadline = Instant::now() + Duration::from_secs(5);
    let mut listed = String::new();
    loop {
        let containers = StdCommand::new("docker")
            .args(["ps", "-a", "--filter", "label=arx.hand=h1", "--filter", "name=arx-h1-", "--format", "{{.Names}}"])
            .output()
            .unwrap();
        listed = String::from_utf8_lossy(&containers.stdout).trim().to_string();
        if listed.is_empty() || Instant::now() >= poll_deadline {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
    assert!(listed.is_empty(), "container still listed: {listed}");
}

#[test]
fn claude_code_hand_steer_returns_a_mechanical_refusal() {
    let _guard = serialize_on_claude_code_docker_state();
    ensure_claude_stub_image();
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let claude_config_dir = plant_stub_credential(workspace.path());

    let stdin = "hand:dispatch:hand-cc:[sleep:60] doomed\nhand:steer:h1:go faster\nhand:kill:h1\n";

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("CLAUDE_CONFIG_DIR", &claude_config_dir)
        .write_stdin(stdin)
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .collect();
    let steer_reply = main_rows[1]["text"].as_str().unwrap();
    assert!(steer_reply.contains("steer is not deliverable"), "steer reply: {steer_reply}");
    assert!(steer_reply.contains("kill and re-dispatch"), "steer reply: {steer_reply}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let refusal_row = ledger_rows.iter().find(|row| row["kind"] == "steer" && row["refused"] == true);
    assert!(refusal_row.is_some(), "ledger rows: {ledger_rows:?}");
    assert!(
        ledger_rows.iter().any(|row| row["kind"] == "killed" && row["hand"] == "h1"),
        "ledger rows: {ledger_rows:?}"
    );
}

#[test]
fn dispatch_on_unknown_agent_is_refused_before_spawn() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    let stdin = "hand:dispatch:ghost-agent:do a thing\n";

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin(stdin)
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .collect();
    assert_eq!(main_rows.len(), 1, "rows: {rows:?}");

    let unknown_reply = main_rows[0]["text"].as_str().unwrap();
    assert!(unknown_reply.contains("unknown agent"), "unknown reply: {unknown_reply}");
    assert!(unknown_reply.contains("hand-cc"), "unknown reply should list known definitions: {unknown_reply}");
    assert!(unknown_reply.contains("reviewer"), "unknown reply should list reviewer among known definitions: {unknown_reply}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    assert!(
        !ledger_contents.lines().any(|line| line.contains("\"kind\":\"spawn\"")),
        "ledger rows: {ledger_contents}"
    );
}

#[test]
fn dispatch_of_drain_is_refused_before_spawn() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    let stdin = "hand:dispatch:drain:do a thing\n";

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin(stdin)
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .collect();
    assert_eq!(main_rows.len(), 1, "rows: {rows:?}");

    let refusal_reply = main_rows[0]["text"].as_str().unwrap();
    assert!(
        refusal_reply.contains("dispatch: drain runs through the drain tool"),
        "refusal reply: {refusal_reply}"
    );

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    assert!(
        !ledger_contents.lines().any(|line| line.contains("\"kind\":\"spawn\"")),
        "ledger rows: {ledger_contents}"
    );
}

#[test]
fn reviewer_dispatch_over_a_seeded_diff_delivers_a_verdict_through_the_codex_body() {
    let _guard = serialize_on_claude_code_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let codex_home = plant_stub_codex_credential(workspace.path());

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("CODEX_HOME", &codex_home)
        .write_stdin("hand:dispatch:reviewer:Review seeded.diff and return a verdict\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_row = rows
        .iter()
        .find(|row| {
            row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("stub verdict:")).unwrap_or(false)
        })
        .expect("verdict row present");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(
        text.contains(
            r#"[hand h1 verdict] {"verdict":"PASS","summary":"stub verdict: Review seeded.diff and return a verdict","must_fix":[]} (trace: threads/main.h1)"#
        ),
        "text: {text}"
    );
    assert!(text.contains("(trace: threads/main.h1)"), "text: {text}");

    let trace_dir = workspace.path().join("run/threads/main.h1");
    assert!(trace_dir.join("trace.md").exists(), "trace.md missing");
    let session_contents = fs::read_to_string(trace_dir.join("session.jsonl")).unwrap();
    let session_lines: Vec<&str> = session_contents.lines().collect();
    assert!(session_lines[0].contains(r#""stub":"argv""#), "line0: {}", session_lines[0]);

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let spawn_row = ledger_rows.iter().find(|row| row["kind"] == "spawn").expect("spawn row");
    assert_eq!(spawn_row["agent"], "reviewer");
    assert_eq!(spawn_row["role"], "reviewer");
    let verdict_ledger_row = ledger_rows.iter().find(|row| row["kind"] == "verdict").expect("verdict row");
    assert_eq!(
        verdict_ledger_row["usage"],
        serde_json::json!({
            "input_tokens": 11,
            "cached_input_tokens": 3,
            "cache_write_input_tokens": 0,
            "output_tokens": 7,
            "reasoning_output_tokens": 1
        })
    );

    assert!(
        workspace.path().join("run/seeded.diff").exists(),
        "seeded.diff missing from the workspace the child ran in"
    );
}

#[test]
fn dispatch_naming_a_body_other_than_the_definition_is_refused_at_the_ledger() {
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let codex_home = plant_stub_codex_credential(workspace.path());
    let path_var = claude_hand_path();

    let stdin = "hand:dispatch:reviewer@claude-code:first\nhand:dispatch:hand-cc@codex:second\nhand:dispatch:reviewer@codex:third\n";

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("PATH", &path_var)
        .env("CODEX_HOME", &codex_home)
        .write_stdin(stdin)
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .collect();
    assert!(main_rows.len() >= 2, "rows: {rows:?}");

    let first_reply = main_rows[0]["text"].as_str().unwrap();
    assert!(first_reply.contains("names body 'claude-code'"), "first reply: {first_reply}");
    assert!(first_reply.contains("definition says 'codex'"), "first reply: {first_reply}");
    assert!(first_reply.contains("never per dispatch"), "first reply: {first_reply}");

    let second_reply = main_rows[1]["text"].as_str().unwrap();
    assert!(second_reply.contains("names body 'codex'"), "second reply: {second_reply}");
    assert!(second_reply.contains("definition says 'claude-code'"), "second reply: {second_reply}");
    assert!(second_reply.contains("never per dispatch"), "second reply: {second_reply}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let refused_rows: Vec<&serde_json::Value> = ledger_rows
        .iter()
        .filter(|row| row["kind"] == "dispatch" && row["refused"] == true)
        .collect();
    assert_eq!(refused_rows.len(), 2, "ledger rows: {ledger_rows:?}");
    assert_eq!(refused_rows[0]["agent"], "reviewer");
    assert_eq!(refused_rows[1]["agent"], "hand-cc");

    let spawn_idx = ledger_rows.iter().position(|row| row["kind"] == "spawn").expect("spawn row");
    let last_refused_idx = ledger_rows
        .iter()
        .rposition(|row| row["kind"] == "dispatch" && row["refused"] == true)
        .expect("a refused dispatch row");
    assert!(last_refused_idx < spawn_idx, "ledger rows: {ledger_rows:?}");

    let spawn_rows: Vec<&serde_json::Value> = ledger_rows.iter().filter(|row| row["kind"] == "spawn").collect();
    assert_eq!(spawn_rows.len(), 1, "ledger rows: {ledger_rows:?}");
    assert_eq!(spawn_rows[0]["agent"], "reviewer");
    assert_eq!(spawn_rows[0]["hand"], "h1");
    assert_eq!(spawn_rows[0]["role"], "reviewer");

    let verdict_row = ledger_rows
        .iter()
        .find(|row| row["kind"] == "verdict" && row["hand"] == "h1")
        .expect("verdict row for h1");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(
        text.starts_with(r#"{"verdict":"PASS","summary":"stub verdict: third"#),
        "text: {text}"
    );
}

#[test]
fn dispatch_naming_an_unknown_body_is_refused() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("hand:dispatch:reviewer@gpt:x\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .collect();
    assert_eq!(main_rows.len(), 1, "rows: {rows:?}");
    let reply = main_rows[0]["text"].as_str().unwrap();
    assert!(reply.contains("names body 'gpt'"), "reply: {reply}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let refused_row = ledger_rows
        .iter()
        .find(|row| row["kind"] == "dispatch" && row["refused"] == true)
        .expect("refused dispatch row");
    assert_eq!(refused_row["requested_body"], "gpt");
    assert!(!ledger_rows.iter().any(|row| row["kind"] == "spawn"), "ledger rows: {ledger_rows:?}");
}

#[test]
fn dispatch_naming_a_body_other_than_a_native_definitions_is_refused_before_the_provider_credential_check() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    let stdin = "hand:dispatch:hand-ds@codex:first\nhand:dispatch:fake:second\n";

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env_remove("ARX_TEST_FAKE_KEY")
        .write_stdin(stdin)
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .collect();
    assert!(main_rows.len() >= 1, "rows: {rows:?}");
    let first_reply = main_rows[0]["text"].as_str().unwrap();
    assert!(first_reply.contains("names body 'codex'"), "first reply: {first_reply}");
    assert!(first_reply.contains("definition says 'native'"), "first reply: {first_reply}");
    assert!(
        !first_reply.contains("needs env"),
        "the missing provider credential must not surface ahead of the body refusal: {first_reply}"
    );

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let refused_rows: Vec<&serde_json::Value> = ledger_rows
        .iter()
        .filter(|row| row["kind"] == "dispatch" && row["refused"] == true)
        .collect();
    assert_eq!(refused_rows.len(), 1, "ledger rows: {ledger_rows:?}");
    assert_eq!(refused_rows[0]["agent"], "hand-ds");
    assert_eq!(refused_rows[0]["requested_body"], "codex");
    assert_eq!(refused_rows[0]["body"], "native");

    let spawn_row = ledger_rows.iter().find(|row| row["kind"] == "spawn").expect("spawn row");
    assert_eq!(spawn_row["agent"], "fake");
    assert_eq!(spawn_row["hand"], "h1");
}

#[test]
fn a_pin_changes_the_effective_binding_the_body_tripwire_checks() {
    let _guard = serialize_on_claude_code_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let codex_home = plant_stub_codex_credential(workspace.path());

    let stdin = "hand:dispatch:hand-ex@codex:first\n/model hand-ex gpt-5.6-sol\nhand:dispatch:hand-ex@codex:second\n";

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("CODEX_HOME", &codex_home)
        .env("ARX_ROSTER_DIR", workspace.path().join("roster"))
        .write_stdin(stdin)
        .assert()
        .success();

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let refused_row = ledger_rows
        .iter()
        .find(|row| row["kind"] == "dispatch" && row["refused"] == true)
        .expect("the first dispatch, before the pin, is refused");
    assert_eq!(refused_row["agent"], "hand-ex");
    assert_eq!(refused_row["requested_body"], "codex");
    assert_eq!(refused_row["body"], "claude-code");

    let spawn_row = ledger_rows.iter().find(|row| row["kind"] == "spawn").expect("the second dispatch, after the pin, spawns");
    assert_eq!(spawn_row["agent"], "hand-ex");
    assert_eq!(spawn_row["body"], "codex");
    assert_eq!(spawn_row["model"], "gpt-5.6-sol");

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let model_events: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "model").collect();
    assert_eq!(model_events.len(), 1, "rows: {rows:?}");
    assert_eq!(model_events[0]["position"], "hand-ex");
    assert_eq!(model_events[0]["to"], "gpt-5.6-sol");
}

#[test]
fn codex_hand_dispatch_delivers_stub_verdict_with_trace_and_usage() {
    let _guard = serialize_on_claude_code_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let codex_home = plant_stub_codex_credential(workspace.path());

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("CODEX_HOME", &codex_home)
        .write_stdin("hand:dispatch:hand-cx:read the file\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_row = rows
        .iter()
        .find(|row| {
            row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("stub verdict:")).unwrap_or(false)
        })
        .expect("verdict row present");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(text.contains("stub verdict: read the file"), "text: {text}");
    assert!(text.contains("(trace: threads/main.h1)"), "text: {text}");

    let trace_dir = workspace.path().join("run/threads/main.h1");
    assert!(trace_dir.join("trace.md").exists(), "trace.md missing");
    assert!(trace_dir.join("session.jsonl").is_file(), "session.jsonl missing");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let spawn_row = ledger_rows.iter().find(|row| row["kind"] == "spawn").expect("spawn row");
    assert_eq!(spawn_row["agent"], "hand-cx");
    let verdict_ledger_row = ledger_rows.iter().find(|row| row["kind"] == "verdict").expect("verdict row");
    assert_eq!(
        verdict_ledger_row["usage"],
        serde_json::json!({
            "input_tokens": 11,
            "cached_input_tokens": 3,
            "cache_write_input_tokens": 0,
            "output_tokens": 7,
            "reasoning_output_tokens": 1
        })
    );
}

#[test]
fn codex_hand_kill_exits_promptly_and_leaves_no_stray_process() {
    let _guard = serialize_on_claude_code_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let codex_home = plant_stub_codex_credential(workspace.path());
    let ledger_path = workspace.path().join("run/threads/main.ledger.jsonl");
    let log_path = workspace.path().join("run/threads/main.jsonl");

    let mut run = StdCommand::new(cargo_bin("arx"))
        .args(["run", config_path.to_str().unwrap()])
        .env("CODEX_HOME", &codex_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = run.stdin.take().unwrap();
    stdin.write_all(b"hand:dispatch:hand-cx:[sleep:60] doomed\n").unwrap();

    let root_deadline = Instant::now() + Duration::from_secs(5);
    let mut root: Option<String> = None;
    while root.is_none() {
        root = spawn_row_root(&ledger_path, "h1");
        if root.is_some() {
            break;
        }
        assert!(Instant::now() < root_deadline, "spawn row with root not seen within 5s");
        thread::sleep(Duration::from_millis(20));
    }
    let root = root.unwrap();
    let pid_file = Path::new(&root).join("home/.codex/codex-stub.pid");

    assert!(
        wait_for_path(&pid_file, Duration::from_secs(5)),
        "stub never wrote its pid file"
    );

    stdin.write_all(b"hand:kill:h1\n").unwrap();
    drop(stdin);

    let mut stdout = run.stdout.take().unwrap();
    let mut stderr = run.stderr.take().unwrap();
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stdout.read_to_end(&mut sink);
    });
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stderr.read_to_end(&mut sink);
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = run.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = run.kill();
            panic!("arx run waited for the killed codex hand instead of exiting promptly");
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success());

    let ledger_contents = fs::read_to_string(&ledger_path).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert!(
        ledger_rows.iter().any(|row| row["kind"] == "killed" && row["hand"] == "h1"),
        "ledger rows: {ledger_rows:?}"
    );

    let contents = fs::read_to_string(&log_path).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let killed_row = rows.iter().find(|row| {
        row["kind"] == "turn"
            && row["role"] == "arx"
            && row["text"]
                .as_str()
                .map(|t| parse_hand_prefix(t) == Some(("h1", "killed")))
                .unwrap_or(false)
    });
    assert!(killed_row.is_some(), "rows: {rows:?}");

    let poll_deadline = Instant::now() + Duration::from_secs(5);
    let mut listed = String::new();
    loop {
        let containers = StdCommand::new("docker")
            .args(["ps", "-a", "--filter", "label=arx.hand=h1", "--filter", "name=arx-h1-", "--format", "{{.Names}}"])
            .output()
            .unwrap();
        listed = String::from_utf8_lossy(&containers.stdout).trim().to_string();
        if listed.is_empty() || Instant::now() >= poll_deadline {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
    assert!(listed.is_empty(), "container still listed: {listed}");
    assert!(!Path::new(&root).exists(), "hand root still present: {root}");
}

#[test]
fn codex_hand_steer_returns_a_mechanical_refusal() {
    let _guard = serialize_on_claude_code_docker_state();
    ensure_codex_stub_image();
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let codex_home = plant_stub_codex_credential(workspace.path());

    let stdin = "hand:dispatch:hand-cx:[sleep:60] doomed\nhand:steer:h1:go faster\nhand:kill:h1\n";

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("CODEX_HOME", &codex_home)
        .write_stdin(stdin)
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .collect();
    let steer_reply = main_rows[1]["text"].as_str().unwrap();
    assert!(steer_reply.contains("steer is not deliverable"), "steer reply: {steer_reply}");
    assert!(steer_reply.contains("kill and re-dispatch"), "steer reply: {steer_reply}");
    assert!(steer_reply.contains("codex hand mid-turn"), "steer reply: {steer_reply}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let refusal_row = ledger_rows.iter().find(|row| row["kind"] == "steer" && row["refused"] == true);
    assert!(refusal_row.is_some(), "ledger rows: {ledger_rows:?}");
    assert!(
        ledger_rows.iter().any(|row| row["kind"] == "killed" && row["hand"] == "h1"),
        "ledger rows: {ledger_rows:?}"
    );
}

#[test]
fn native_hand_text_turn_delivers_verdict_trace_and_usage() {
    let (base_url, state) = fake_openai_server(FakeScript::Sse(vec![
        r#"{"choices":[{"delta":{"role":"assistant"}}]}"#.to_string(),
        r#"{"choices":[{"delta":{"content":"ARX-"}}]}"#.to_string(),
        r#"{"choices":[{"delta":{"content":"L1-10 OK"},"finish_reason":null}]}"#.to_string(),
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string(),
        r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":5,"total_tokens":17}}"#.to_string(),
    ]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:say the word\n")
        .assert()
        .success();

    let usage = serde_json::json!({"prompt_tokens": 12, "completion_tokens": 5, "total_tokens": 17});

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_row = rows
        .iter()
        .find(|row| row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("verdict")).unwrap_or(false))
        .expect("verdict row present");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(text.contains("[hand h1 verdict] ARX-L1-10 OK"), "text: {text}");
    assert!(text.contains("(trace: threads/main.h1)"), "text: {text}");

    let trace_contents = fs::read_to_string(workspace.path().join("run/threads/main.h1/transcript.jsonl")).unwrap();
    let trace_rows: Vec<serde_json::Value> = trace_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let kinds: Vec<&str> = trace_rows.iter().map(|row| row["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, vec!["request", "assistant", "termination"], "trace rows: {trace_rows:?}");
    let termination_row = trace_rows.iter().find(|row| row["kind"] == "termination").unwrap();
    assert_eq!(termination_row["usage"], usage);

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_ledger_row = ledger_rows.iter().find(|row| row["kind"] == "verdict").expect("verdict row");
    assert_eq!(verdict_ledger_row["usage"], usage);
    let spawn_row = ledger_rows.iter().find(|row| row["kind"] == "spawn").expect("spawn row");
    assert_eq!(spawn_row["agent"], "hand-ds");

    let requests = state.requests.lock().unwrap();
    assert_eq!(requests.len(), 1, "requests: {requests:?}");
    let (path, authorization, body) = &requests[0];
    assert_eq!(path, "/chat/completions");
    assert_eq!(authorization, "Bearer test-key");

    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "fake-model");
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["messages"][0]["role"], "system");
    assert!(
        body["messages"][0]["content"].as_str().unwrap().contains("hand dispatched by the arx harness"),
        "system message: {body}"
    );
    assert_eq!(body["messages"][1], serde_json::json!({"role": "user", "content": "say the word"}));
    assert_eq!(body["tools"][0]["function"]["name"], "read_file");
    assert!(!body.to_string().contains("store/"), "body leaked a store path: {body}");
}

#[test]
fn native_hand_tool_call_executes_once_and_makes_no_second_request() {
    let (base_url, state) = fake_openai_server(FakeScript::Sequence(vec![
        FakeScript::Sse(vec![
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read_file","arguments":""}}]}}]}"#.to_string(),
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"pa"}}]}}]}"#.to_string(),
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"th\": \"pro"}}]}}]}"#.to_string(),
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"be.txt\"}"}}]}}]}"#.to_string(),
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#.to_string(),
            r#"{"choices":[],"usage":{"prompt_tokens":20,"completion_tokens":8,"total_tokens":28}}"#.to_string(),
        ]),
        FakeScript::Sse(vec![
            r#"{"choices":[{"delta":{"content":"done reading"}}]}"#.to_string(),
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string(),
            r#"{"choices":[],"usage":{"prompt_tokens":30,"completion_tokens":4,"total_tokens":34}}"#.to_string(),
        ]),
    ]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:call read_file on probe.txt\n")
        .assert()
        .success();

    let trace_contents = fs::read_to_string(workspace.path().join("run/threads/main.h1/transcript.jsonl")).unwrap();
    let trace_rows: Vec<serde_json::Value> = trace_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let kinds: Vec<&str> = trace_rows.iter().map(|row| row["kind"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        vec![
            "request",
            "assistant",
            "termination",
            "gate",
            "admission",
            "tool_result",
            "request",
            "assistant",
            "termination"
        ],
        "trace rows: {trace_rows:?}"
    );

    let assistant_row = &trace_rows[1];
    let content = assistant_row["content"].as_array().unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["ToolCall"]["arguments"], serde_json::json!({"path": "probe.txt"}));

    assert_eq!(trace_rows[2]["termination"], "ToolUse");

    let gate_row = &trace_rows[3];
    assert_eq!(gate_row["tool"], "read_file");
    assert_eq!(gate_row["verdict"], "allow");

    let admission_row = &trace_rows[4];
    assert_eq!(admission_row["outcome"], "inline");

    let tool_result_row = &trace_rows[5];
    assert!(
        tool_result_row["content"].as_str().unwrap().contains("ARX-L1-10-PROBE"),
        "tool_result row: {tool_result_row:?}"
    );

    assert_eq!(trace_rows[8]["termination"], "EndTurn");

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_row = rows
        .iter()
        .find(|row| row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("verdict")).unwrap_or(false))
        .expect("verdict row present");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(text.contains("done reading"), "text: {text}");

    let requests = state.requests.lock().unwrap();
    assert_eq!(requests.len(), 2, "requests: {requests:?}");
    let second_body: serde_json::Value = serde_json::from_str(&requests[1].2).unwrap();
    let messages = second_body["messages"].as_array().unwrap();
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[2]["role"], "assistant");
    assert_eq!(messages[3]["role"], "tool");
    assert!(
        messages[3]["content"].as_str().unwrap().contains("ARX-L1-10-PROBE"),
        "second request body: {second_body}"
    );
}

#[test]
fn native_hand_runs_a_brief_across_three_steps_and_stops_at_end_turn() {
    let tool_call_step = |call_id: &str| {
        FakeScript::Sse(vec![
            format!(
                r#"{{"choices":[{{"delta":{{"tool_calls":[{{"index":0,"id":"{call_id}","type":"function","function":{{"name":"read_file","arguments":"{{\"path\": \"probe.txt\"}}"}}}}]}}}}]}}"#
            ),
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#.to_string(),
        ])
    };
    let (base_url, state) = fake_openai_server(FakeScript::Sequence(vec![
        tool_call_step("call_1"),
        tool_call_step("call_2"),
        FakeScript::Sse(vec![
            r#"{"choices":[{"delta":{"content":"all done"}}]}"#.to_string(),
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string(),
        ]),
    ]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:call read_file on probe.txt repeatedly\n")
        .assert()
        .success();

    let trace_contents = fs::read_to_string(workspace.path().join("run/threads/main.h1/transcript.jsonl")).unwrap();
    let trace_rows: Vec<serde_json::Value> = trace_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let tool_result_rows: Vec<&serde_json::Value> = trace_rows.iter().filter(|row| row["kind"] == "tool_result").collect();
    assert_eq!(tool_result_rows.len(), 2, "trace rows: {trace_rows:?}");

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("verdict")).unwrap_or(false))
        .collect();
    assert_eq!(verdict_rows.len(), 1, "rows: {rows:?}");
    let text = verdict_rows[0]["text"].as_str().unwrap();
    assert!(text.contains("all done"), "text: {text}");

    let requests = state.requests.lock().unwrap();
    assert_eq!(requests.len(), 3, "requests: {requests:?}");
}

#[test]
fn native_hand_fatal_http_error_surfaces_as_failure_verdict() {
    let (base_url, _state) = fake_openai_server(FakeScript::Status(401, r#"{"error":{"message":"bad key"}}"#.to_string()));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:say the word\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_row = rows
        .iter()
        .find(|row| row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("verdict")).unwrap_or(false))
        .expect("verdict row present");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(text.contains("native hand failed — fatal: HTTP 401"), "text: {text}");
    assert!(text.contains("bad key"), "text: {text}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert!(ledger_rows.iter().any(|row| row["kind"] == "verdict" && row["hand"] == "h1"), "ledger rows: {ledger_rows:?}");

    let trace_contents = fs::read_to_string(workspace.path().join("run/threads/main.h1/transcript.jsonl")).unwrap();
    let trace_rows: Vec<serde_json::Value> = trace_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert!(trace_rows.iter().any(|row| row["kind"] == "error"), "trace rows: {trace_rows:?}");
}

#[test]
fn native_hand_retries_a_429_before_the_first_item() {
    let (base_url, state) = fake_openai_server(FakeScript::Sequence(vec![
        FakeScript::StatusWithRetryAfter(429, "{}".to_string(), 0),
        FakeScript::Sse(vec![
            r#"{"choices":[{"delta":{"content":"ok"}}]}"#.to_string(),
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string(),
        ]),
    ]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:say the word\n")
        .assert()
        .success();

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_rows: Vec<&serde_json::Value> = ledger_rows.iter().filter(|row| row["kind"] == "verdict").collect();
    assert_eq!(verdict_rows.len(), 1, "ledger rows: {ledger_rows:?}");
    assert_eq!(verdict_rows[0]["text"], "ok");

    let requests = state.requests.lock().unwrap();
    assert_eq!(requests.len(), 2, "requests: {requests:?}");
}

#[test]
fn native_hand_reports_a_context_window_miss_as_verdict_event_and_notice() {
    let body = r#"{"error":{"message":"This model's maximum context length is 8192 tokens. However, you requested 10000 tokens","type":"invalid_request_error"}}"#;
    let (base_url, _state) = fake_openai_server(FakeScript::Status(400, body.to_string()));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    let assert = Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:say the word\n")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        stdout.contains("[arx] miss: hand-ds (fake-model) rejected the window; the configured window for that model may need updating"),
        "stdout: {stdout}"
    );

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_row = rows
        .iter()
        .find(|row| row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("verdict")).unwrap_or(false))
        .expect("verdict row present");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(
        text.contains("native hand failed — context window exceeded (requested 10000, maximum 8192)"),
        "text: {text}"
    );

    let miss_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "miss").collect();
    assert_eq!(miss_rows.len(), 1, "rows: {rows:?}");
    assert_eq!(miss_rows[0]["size"], 1808);
}

#[test]
fn native_hand_on_openai_usage_can_trip_the_hand_nudge() {
    let (base_url, _state) = fake_openai_server(FakeScript::Sse(vec![
        r#"{"choices":[{"delta":{"content":"ok"}}]}"#.to_string(),
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string(),
        r#"{"choices":[],"usage":{"prompt_tokens":50,"completion_tokens":5,"total_tokens":55}}"#.to_string(),
    ]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");
    with_main_nudge(&config_path, 40);

    let assert = Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:say the word\n")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(stdout.contains("nudge: hand h1 returned at 50 tokens, past 40"), "stdout: {stdout}");

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let hand_nudge_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "nudge" && row["seat"] == "hand").collect();
    assert_eq!(hand_nudge_rows.len(), 1, "rows: {rows:?}");
    assert_eq!(hand_nudge_rows[0]["hand"], "h1");
    assert_eq!(hand_nudge_rows[0]["occupancy"], 50);
}

#[test]
fn native_hand_kill_aborts_the_stream() {
    let (base_url, state) = fake_openai_server(FakeScript::Stall {
        first: r#"{"choices":[{"delta":{"content":"partial"}}]}"#.to_string(),
    });
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");
    let ledger_path = workspace.path().join("run/threads/main.ledger.jsonl");
    let log_path = workspace.path().join("run/threads/main.jsonl");
    let mut run = StdCommand::new(cargo_bin("arx"))
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = run.stdin.take().unwrap();
    stdin.write_all(b"hand:dispatch:hand-ds:say the word\n").unwrap();

    assert!(wait_for_ledger_spawn(&ledger_path, "h1", Duration::from_secs(5)), "hand h1 never spawned");

    stdin.write_all(b"hand:kill:h1\n").unwrap();
    drop(stdin);

    let mut stdout = run.stdout.take().unwrap();
    let mut stderr = run.stderr.take().unwrap();
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stdout.read_to_end(&mut sink);
    });
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stderr.read_to_end(&mut sink);
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = run.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = run.kill();
            panic!("arx run waited for the killed native hand instead of exiting promptly");
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success());

    let ledger_contents = fs::read_to_string(&ledger_path).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert!(
        ledger_rows.iter().any(|row| row["kind"] == "killed" && row["hand"] == "h1"),
        "ledger rows: {ledger_rows:?}"
    );
    assert!(!ledger_rows.iter().any(|row| row["kind"] == "verdict" && row["hand"] == "h1"), "ledger rows: {ledger_rows:?}");

    let contents = fs::read_to_string(&log_path).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let killed_row = rows.iter().find(|row| {
        row["kind"] == "turn"
            && row["role"] == "arx"
            && row["text"]
                .as_str()
                .map(|t| parse_hand_prefix(t) == Some(("h1", "killed")))
                .unwrap_or(false)
    });
    assert!(killed_row.is_some(), "rows: {rows:?}");

    let deadline = Instant::now() + Duration::from_secs(5);
    while !state.disconnected.load(Ordering::SeqCst) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(state.disconnected.load(Ordering::SeqCst), "fake server never observed a disconnect");
}

#[test]
fn native_hand_steer_returns_a_mechanical_refusal() {
    let (base_url, _state) = fake_openai_server(FakeScript::Stall {
        first: r#"{"choices":[{"delta":{"content":"partial"}}]}"#.to_string(),
    });
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    let stdin = "hand:dispatch:hand-ds:say the word\nhand:steer:h1:go faster\nhand:kill:h1\n";

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin(stdin)
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .collect();
    let steer_reply = main_rows[1]["text"].as_str().unwrap();
    assert!(steer_reply.contains("native hand mid-turn"), "steer reply: {steer_reply}");
    assert!(steer_reply.contains("steer is not deliverable"), "steer reply: {steer_reply}");
    assert!(steer_reply.contains("kill and re-dispatch"), "steer reply: {steer_reply}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let refusal_row = ledger_rows.iter().find(|row| row["kind"] == "steer" && row["refused"] == true);
    assert!(refusal_row.is_some(), "ledger rows: {ledger_rows:?}");
}

#[test]
fn native_dispatch_without_api_key_env_is_refused_before_spawn() {
    let (base_url, state) = fake_openai_server(FakeScript::Sse(vec![]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env_remove("ARX_TEST_FAKE_KEY")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:say the word\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let main_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .collect();
    assert_eq!(main_rows.len(), 1, "rows: {rows:?}");
    let reply = main_rows[0]["text"].as_str().unwrap();
    assert!(reply.contains("ARX_TEST_FAKE_KEY"), "reply: {reply}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    assert!(
        !ledger_contents.lines().any(|line| line.contains("\"kind\":\"spawn\"")),
        "ledger rows: {ledger_contents}"
    );

    assert_eq!(state.requests.lock().unwrap().len(), 0);
}

#[test]
fn native_dispatch_through_anthropic_completes_a_text_turn() {
    let (base_url, state) = fake_anthropic_server(FakeScript::Sse(vec![
        r#"{"type":"message_start","message":{"id":"msg_1","usage":{"input_tokens":12,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#.to_string(),
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#.to_string(),
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"ARX-"}}"#.to_string(),
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"L2-10C OK"}}"#.to_string(),
        r#"{"type":"content_block_stop","index":0}"#.to_string(),
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":5}}"#.to_string(),
        r#"{"type":"message_stop"}"#.to_string(),
    ]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-an:say the word\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_row = rows
        .iter()
        .find(|row| row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("verdict")).unwrap_or(false))
        .expect("verdict row present");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(text.contains("[hand h1 verdict] ARX-L2-10C OK"), "text: {text}");

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_ledger_row = ledger_rows.iter().find(|row| row["kind"] == "verdict").expect("verdict row");
    assert_eq!(
        verdict_ledger_row["usage"],
        serde_json::json!({"input_tokens": 12, "output_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0})
    );

    let requests = state.requests.lock().unwrap();
    assert_eq!(requests.len(), 1, "requests: {requests:?}");
    let (path, _authorization, body) = &requests[0];
    assert_eq!(path, "/v1/messages");

    let headers = state.headers.lock().unwrap();
    assert_eq!(
        headers[0].iter().find(|(name, _)| name == "x-api-key").map(|(_, v)| v.as_str()),
        Some("test-key")
    );
    assert_eq!(
        headers[0].iter().find(|(name, _)| name == "anthropic-version").map(|(_, v)| v.as_str()),
        Some("2023-06-01")
    );

    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], "fake-model-anthropic");
    assert_eq!(body["max_tokens"], 8192);
    assert_eq!(body["stream"], true);
    assert!(body["system"].as_str().unwrap().contains("hand dispatched by the arx harness"), "system: {body}");
    assert_eq!(body["messages"][0], serde_json::json!({"role": "user", "content": [{"type": "text", "text": "say the word"}]}));
    assert_eq!(body["tools"][0]["name"], "read_file");
    assert!(body["tools"][0]["input_schema"].is_object(), "tools: {body}");
    assert!(!body.to_string().contains("store/"), "body leaked a store path: {body}");
}

#[test]
fn native_hand_on_anthropic_runs_a_tool_step_with_tool_result_blocks() {
    let (base_url, state) = fake_anthropic_server(FakeScript::Sequence(vec![
        FakeScript::Sse(vec![
            r#"{"type":"message_start","message":{"usage":{"input_tokens":20}}}"#.to_string(),
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_1","name":"read_file","input":{}}}"#.to_string(),
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"pa"}}"#.to_string(),
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"th\": \"probe.txt\"}"}}"#.to_string(),
            r#"{"type":"content_block_stop","index":0}"#.to_string(),
            r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":8}}"#.to_string(),
            r#"{"type":"message_stop"}"#.to_string(),
        ]),
        FakeScript::Sse(vec![
            r#"{"type":"message_start","message":{"usage":{"input_tokens":30}}}"#.to_string(),
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#.to_string(),
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"done reading"}}"#.to_string(),
            r#"{"type":"content_block_stop","index":0}"#.to_string(),
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":4}}"#.to_string(),
            r#"{"type":"message_stop"}"#.to_string(),
        ]),
    ]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-an:call read_file on probe.txt\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_row = rows
        .iter()
        .find(|row| row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("verdict")).unwrap_or(false))
        .expect("verdict row present");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(text.contains("done reading"), "text: {text}");

    let requests = state.requests.lock().unwrap();
    assert_eq!(requests.len(), 2, "requests: {requests:?}");
    let second_body: serde_json::Value = serde_json::from_str(&requests[1].2).unwrap();
    let messages = second_body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[1]["role"], "assistant");
    assert_eq!(messages[1]["content"][0]["type"], "tool_use");
    assert_eq!(messages[1]["content"][0]["id"], "toolu_1");
    let last = &messages[2];
    assert_eq!(last["role"], "user");
    assert_eq!(last["content"][0]["type"], "tool_result");
    assert_eq!(last["content"][0]["tool_use_id"], "toolu_1");
    assert!(
        last["content"][0]["content"].as_str().unwrap().contains("ARX-L1-10-PROBE"),
        "second request body: {second_body}"
    );
}

#[test]
fn native_hand_on_anthropic_retries_a_429_before_the_first_item() {
    let (base_url, state) = fake_anthropic_server(FakeScript::Sequence(vec![
        FakeScript::StatusWithRetryAfter(429, "{}".to_string(), 0),
        FakeScript::Sse(vec![
            r#"{"type":"message_start","message":{"usage":{"input_tokens":1}}}"#.to_string(),
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#.to_string(),
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"ok"}}"#.to_string(),
            r#"{"type":"content_block_stop","index":0}"#.to_string(),
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":1}}"#.to_string(),
            r#"{"type":"message_stop"}"#.to_string(),
        ]),
    ]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-an:say the word\n")
        .assert()
        .success();

    let ledger_contents = fs::read_to_string(workspace.path().join("run/threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_rows: Vec<&serde_json::Value> = ledger_rows.iter().filter(|row| row["kind"] == "verdict").collect();
    assert_eq!(verdict_rows.len(), 1, "ledger rows: {ledger_rows:?}");
    assert_eq!(verdict_rows[0]["text"], "ok");

    let requests = state.requests.lock().unwrap();
    assert_eq!(requests.len(), 2, "requests: {requests:?}");
}

#[test]
fn native_hand_on_anthropic_reports_a_prompt_too_long_miss() {
    let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"prompt is too long: 10000 tokens > 8192 maximum"}}"#;
    let (base_url, _state) = fake_anthropic_server(FakeScript::Status(400, body.to_string()));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    let assert = Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-an:say the word\n")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        stdout.contains("[arx] miss: hand-an (fake-model-anthropic) rejected the window; the configured window for that model may need updating"),
        "stdout: {stdout}"
    );

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let verdict_row = rows
        .iter()
        .find(|row| row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains("verdict")).unwrap_or(false))
        .expect("verdict row present");
    let text = verdict_row["text"].as_str().unwrap();
    assert!(
        text.contains("native hand failed — context window exceeded (requested 10000, maximum 8192)"),
        "text: {text}"
    );

    let miss_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "miss").collect();
    assert_eq!(miss_rows.len(), 1, "rows: {rows:?}");
    assert_eq!(miss_rows[0]["size"], 1808);
}

#[test]
fn native_hand_on_anthropic_http_error_body_echoing_the_key_is_redacted_everywhere_under_threads() {
    let (base_url, _state) = fake_anthropic_server(FakeScript::Status(
        401,
        r#"{"type":"error","error":{"type":"authentication_error","message":"bad key: Bearer test-key"}}"#.to_string(),
    ));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-an:say the word\n")
        .assert()
        .success();

    let threads_dir = workspace.path().join("run/threads");
    for path in jsonl_files_recursive(&threads_dir) {
        let contents = fs::read_to_string(&path).unwrap();
        assert!(!contents.contains("test-key"), "{}: leaked the api key verbatim: {contents}", path.display());
    }

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    assert!(
        contents.contains("[redacted]"),
        "expected the redaction placeholder in the surfaced verdict: {contents}"
    );
}

#[test]
fn native_hand_http_error_body_echoing_the_key_is_redacted_everywhere_under_threads() {
    let (base_url, _state) = fake_openai_server(FakeScript::Status(
        401,
        r#"{"error":{"message":"bad key: Bearer test-key"}}"#.to_string(),
    ));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:say the word\n")
        .assert()
        .success();

    let threads_dir = workspace.path().join("run/threads");
    for path in jsonl_files_recursive(&threads_dir) {
        let contents = fs::read_to_string(&path).unwrap();
        assert!(!contents.contains("test-key"), "{}: leaked the api key verbatim: {contents}", path.display());
    }

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    assert!(
        contents.contains("[redacted]"),
        "expected the redaction placeholder in the surfaced verdict: {contents}"
    );
}

#[test]
fn native_hand_malformed_sse_data_echoing_the_key_is_redacted_everywhere_under_threads() {
    let (base_url, _state) = fake_openai_server(FakeScript::Sse(vec![
        "not valid json and it echoes Bearer test-key back at you".to_string(),
    ]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:say the word\n")
        .assert()
        .success();

    let threads_dir = workspace.path().join("run/threads");
    for path in jsonl_files_recursive(&threads_dir) {
        let contents = fs::read_to_string(&path).unwrap();
        assert!(!contents.contains("test-key"), "{}: leaked the api key verbatim: {contents}", path.display());
    }

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    assert!(
        contents.contains("[redacted]"),
        "expected the redaction placeholder in the surfaced verdict: {contents}"
    );
}

fn with_main_nudge(config_path: &Path, threshold: u64) {
    let contents = fs::read_to_string(config_path).unwrap();
    let needle = "body = \"claude-code\"\n";
    assert!(contents.contains(needle), "config: {contents}");
    let updated = contents.replacen(needle, &format!("{needle}nudge = {threshold}\n"), 1);
    fs::write(config_path, updated).unwrap();
}

#[test]
fn nudge_fires_once_and_never_kills() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    with_main_nudge(&config_path, 40);

    let assert = Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("first\nsecond\nthird\n")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let notice_count = stdout.matches("nudge: main at 47 tokens, past 40").count();
    assert_eq!(notice_count, 1, "stdout: {stdout}");

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let nudge_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "nudge").collect();
    assert_eq!(nudge_rows.len(), 1, "rows: {rows:?}");
    assert_eq!(nudge_rows[0]["seat"], "main");

    let operator_turns: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "operator")
        .collect();
    assert_eq!(operator_turns.len(), 3, "rows: {rows:?}");
    assert_eq!(operator_turns[0]["text"], "first");
    let second_text = operator_turns[1]["text"].as_str().unwrap();
    assert!(
        second_text.starts_with("[arx] nudge: this window holds 47 tokens, past the 40 line. No action is required."),
        "second operator turn: {second_text}"
    );
    assert_eq!(operator_turns[2]["text"], "third");

    let main_reply_count = rows.iter().filter(|row| row["kind"] == "turn" && row["role"] == "main").count();
    assert_eq!(main_reply_count, 3, "rows: {rows:?}");
}

#[test]
fn nudge_latch_survives_reopen() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    with_main_nudge(&config_path, 40);

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("first\n")
        .assert()
        .success();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("second\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let nudge_count = rows.iter().filter(|row| row["kind"] == "nudge").count();
    assert_eq!(nudge_count, 1, "rows: {rows:?}");

    let second_session_operator_turn = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "operator")
        .nth(1)
        .expect("second session's operator turn");
    assert_eq!(second_session_operator_turn["text"], "second");
}

#[test]
fn main_reads_a_workspace_file_and_the_log_records_it() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let config_text = fs::read_to_string(&config_path).unwrap();
    let first_line = config_text.lines().next().unwrap();
    let size = config_text.len();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("file:read_file:{\"path\":\"arx.toml\"}\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let main_row = rows.iter().find(|row| row["kind"] == "turn" && row["role"] == "main").unwrap();
    let text = main_row["text"].as_str().unwrap();
    let payload: serde_json::Value = serde_json::from_str(text.strip_prefix("stub-file:").unwrap()).unwrap();
    let read_text = payload["text"].as_str().unwrap();
    assert!(read_text.contains(first_line), "read text: {read_text}");

    let file_row = rows.iter().find(|row| row["kind"] == "file").expect("a file row");
    assert_eq!(file_row["text"], format!("arx.toml: read {size} bytes, inline"));
}

#[test]
fn main_read_over_the_threshold_is_externalized() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let content = "x".repeat(40960);
    fs::write(workspace.path().join("run/big.txt"), &content).unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("file:read_file:{\"path\":\"big.txt\"}\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let main_row = rows.iter().find(|row| row["kind"] == "turn" && row["role"] == "main").unwrap();
    let text = main_row["text"].as_str().unwrap();
    let payload: serde_json::Value = serde_json::from_str(text.strip_prefix("stub-file:").unwrap()).unwrap();
    let read_text = payload["text"].as_str().unwrap();
    let handle = "threads/main.traces/1-1.md";
    assert!(
        read_text.starts_with(&format!("[arx] externalized read_file result: {handle} (40960 bytes, 1 lines; body from line 12)")),
        "read text head: {}",
        &read_text[..read_text.len().min(160)]
    );

    let file_row = rows.iter().find(|row| row["kind"] == "file").expect("a file row");
    assert_eq!(file_row["text"], "big.txt: read 40960 bytes, externalized");

    let admission_row = rows.iter().find(|row| row["kind"] == "admission").expect("an admission row");
    assert_eq!(admission_row["outcome"], "externalized");
    assert_eq!(admission_row["handle"], handle);

    assert!(workspace.path().join("run").join(handle).exists());
}

#[test]
fn main_read_outside_the_workspace_is_refused() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("file:read_file:{\"path\":\"/etc/hostname\"}\nfile:read_file:{\"path\":\"../\"}\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let main_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "turn" && row["role"] == "main").collect();
    assert_eq!(main_rows.len(), 2, "rows: {rows:?}");
    for row in &main_rows {
        let text = row["text"].as_str().unwrap();
        assert!(text.contains("read_file error: path escapes the workspace"), "main turn text: {text}");
    }

    let file_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "file").collect();
    assert_eq!(file_rows.len(), 2, "rows: {rows:?}");
    for row in &file_rows {
        let text = row["text"].as_str().unwrap();
        assert!(text.ends_with("read_file error: path escapes the workspace"), "file row text: {text}");
    }
}

#[test]
fn main_edits_a_workspace_file_exactly_once() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    fs::write(workspace.path().join("run/NOTE.md"), "alpha beta alpha").unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin(
            "file:edit_file:{\"path\":\"NOTE.md\",\"old\":\"beta\",\"new\":\"gamma\"}\nfile:edit_file:{\"path\":\"NOTE.md\",\"old\":\"alpha\",\"new\":\"delta\"}\n",
        )
        .assert()
        .success();

    assert_eq!(fs::read_to_string(workspace.path().join("run/NOTE.md")).unwrap(), "alpha gamma alpha");

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let file_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "file").collect();
    assert_eq!(file_rows.len(), 2, "rows: {rows:?}");
    assert_eq!(file_rows[0]["text"], "NOTE.md: edit\n--- old\nbeta\n+++ new\ngamma");
    assert_eq!(file_rows[1]["text"], "NOTE.md: edit_file error: old text matches 2 times");

    let main_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "turn" && row["role"] == "main").collect();
    assert_eq!(main_rows.len(), 2, "rows: {rows:?}");
    let second_text = main_rows[1]["text"].as_str().unwrap();
    assert!(second_text.contains("edit_file error: old text matches 2 times"), "second reply: {second_text}");
}

#[test]
fn main_edit_of_arx_state_is_refused() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    fs::create_dir_all(workspace.path().join("run/store")).unwrap();
    fs::write(workspace.path().join("run/store/x.md"), "keep").unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("file:edit_file:{\"path\":\"store/x.md\",\"old\":\"keep\",\"new\":\"changed\"}\n")
        .assert()
        .success();

    assert_eq!(fs::read_to_string(workspace.path().join("run/store/x.md")).unwrap(), "keep");

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let file_row = rows.iter().find(|row| row["kind"] == "file").expect("a file row");
    assert_eq!(file_row["text"], "store/x.md: edit_file error: store/x.md is arx's own state");
}

#[test]
fn a_gate_rule_denies_edit_file_by_path() {
    let workspace = fresh_run_fixture_with_gate(Some("\n[gate]\ndeny = [\"mcp__arx__edit_file(docs/*)\"]\n"));
    let config_path = workspace.path().join("run/arx.toml");
    fs::create_dir_all(workspace.path().join("run/docs")).unwrap();
    fs::write(workspace.path().join("run/docs/x.md"), "foo").unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("file:edit_file:{\"path\":\"docs/x.md\",\"old\":\"foo\",\"new\":\"bar\"}\n")
        .assert()
        .success();

    assert_eq!(fs::read_to_string(workspace.path().join("run/docs/x.md")).unwrap(), "foo");

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let gate_row = rows
        .iter()
        .find(|row| row["kind"] == "gate" && row["tool"] == "mcp__arx__edit_file")
        .expect("a gate row for edit_file");
    assert_eq!(gate_row["verdict"], "deny");

    assert!(!rows.iter().any(|row| row["kind"] == "file"), "rows: {rows:?}");
}

#[test]
fn the_prompt_reaches_the_sidecar_as_static_and_dynamic_blocks() {
    let workspace = fresh_run_fixture_with_store_scope("arx");
    let run_dir = workspace.path().join("run");

    let rows = run_session(&run_dir, "arx.toml", &["hello".to_string()], &[]);

    let init: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(run_dir.join("stub-init.json")).unwrap()).unwrap();
    let static_block = init["systemPrompt"]["static"].as_str().unwrap().to_string();
    let dynamic = init["systemPrompt"]["dynamic"].as_str().unwrap().to_string();

    let expected_static = fs::read_to_string(run_dir.join("prompts/main.md")).unwrap();
    assert_eq!(static_block, expected_static);

    assert!(dynamic.starts_with("Workspace "), "dynamic: {dynamic}");
    assert!(dynamic.contains("Store scope arx"), "dynamic: {dynamic}");
    assert!(dynamic.contains("Hands available:"), "dynamic: {dynamic}");
    assert!(
        dynamic.contains("- hand-cc: claude-sonnet-5, claude-code, tools none. Read-only doer for deep trace and live repro."),
        "dynamic: {dynamic}"
    );

    let system_row = rows.iter().find(|row| row["kind"] == "system").unwrap();
    let text = system_row["text"].as_str().unwrap();
    assert_eq!(text, format!("{static_block}\n\n{dynamic}"));
    assert_eq!(system_row["boundary"].as_u64().unwrap(), (static_block.len() + 2) as u64);
}

#[test]
fn two_spawns_share_the_static_block_and_a_store_change_moves_only_the_dynamic() {
    let workspace = fresh_run_fixture_with_store_scope("arx");
    let run_dir = workspace.path().join("run");

    run_session(&run_dir, "arx.toml", &["hello".to_string()], &[]);
    let first_init: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(run_dir.join("stub-init.json")).unwrap()).unwrap();
    let first_static = first_init["systemPrompt"]["static"].as_str().unwrap().to_string();
    let first_dynamic = first_init["systemPrompt"]["dynamic"].as_str().unwrap().to_string();

    plant_engine_scope(&run_dir.join("store"), "arx");

    run_session(&run_dir, "arx.toml", &["hello again".to_string()], &[]);
    let second_init: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(run_dir.join("stub-init.json")).unwrap()).unwrap();
    let second_static = second_init["systemPrompt"]["static"].as_str().unwrap().to_string();
    let second_dynamic = second_init["systemPrompt"]["dynamic"].as_str().unwrap().to_string();

    assert_eq!(first_static, second_static);
    assert_ne!(first_dynamic, second_dynamic, "dynamic: {second_dynamic}");
    assert!(second_dynamic.contains("what is the launch codename"), "dynamic: {second_dynamic}");
}

#[test]
fn repo_instructions_render_in_the_dynamic_block_only() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    let contents = fs::read_to_string(&config_path).unwrap();
    let updated = contents.replacen(
        "instructions = \"prompts/main.md\"\n",
        "instructions = \"prompts/main.md\"\nrepo_instructions = \"REPO.md\"\n",
        1,
    );
    assert_ne!(contents, updated, "config: {contents}");
    fs::write(&config_path, updated).unwrap();
    fs::write(run_dir.join("REPO.md"), "Repo instructions for the fixture.\n").unwrap();

    run_session(&run_dir, "arx.toml", &["hello".to_string()], &[]);

    let init: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(run_dir.join("stub-init.json")).unwrap()).unwrap();
    let static_block = init["systemPrompt"]["static"].as_str().unwrap();
    let dynamic = init["systemPrompt"]["dynamic"].as_str().unwrap();

    assert!(dynamic.contains("Repo instructions ("), "dynamic: {dynamic}");
    assert!(dynamic.contains("Repo instructions for the fixture."), "dynamic: {dynamic}");
    assert!(!static_block.contains("Repo instructions ("), "static: {static_block}");
}

#[test]
fn a_gate_rule_denies_read_file_by_path_for_main() {
    let workspace = fresh_run_fixture_with_gate(Some("\n[gate]\ndeny = [\"mcp__arx__read_file(docs/*)\"]\n"));
    let config_path = workspace.path().join("run/arx.toml");
    fs::create_dir_all(workspace.path().join("run/docs")).unwrap();
    fs::write(workspace.path().join("run/docs/x.md"), "foo").unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .write_stdin("file:read_file:{\"path\":\"docs/x.md\"}\n")
        .assert()
        .success();

    let contents = fs::read_to_string(workspace.path().join("run/threads/main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let gate_row = rows
        .iter()
        .find(|row| row["kind"] == "gate" && row["tool"] == "mcp__arx__read_file")
        .expect("a gate row for read_file");
    assert_eq!(gate_row["verdict"], "deny");

    assert!(!rows.iter().any(|row| row["kind"] == "file"), "rows: {rows:?}");
}

#[test]
fn an_arx_call_crosses_the_gate_once_in_the_handler() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["store:recall:{\"scope\":\"arx\",\"query\":\"x\"}".to_string()],
        &[],
    );

    let gate_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "gate").collect();
    assert_eq!(gate_rows.len(), 1, "rows: {rows:?}");
    assert_eq!(gate_rows[0]["tool"], "mcp__arx__recall");
    assert_eq!(gate_rows[0]["verdict"], "allow");
    assert!(gate_rows[0]["by"].is_null(), "gate row: {:?}", gate_rows[0]);

    let gate_index = rows.iter().position(|row| row["kind"] == "gate").unwrap();
    let store_turn_index = rows
        .iter()
        .position(|row| row["kind"] == "turn"
                && row["role"] == "main"
                && row["text"].as_str().unwrap_or("").starts_with("stub-store:"))
        .expect("stub-store reply row");
    assert!(gate_index < store_turn_index, "rows: {rows:?}");

    assert_eq!(rows.len(), 5, "rows: {rows:?}");
}

#[test]
fn a_denied_store_call_returns_the_reason_and_has_no_effect() {
    let workspace = fresh_run_fixture_with_gate(Some("\n[gate]\ndeny = [\"mcp__arx__save\"]\n"));
    let run_dir = workspace.path().join("run");

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["store:save:{\"scope\":\"arx\",\"kind\":\"fact\",\"question\":\"q\",\"answer\":\"a\",\"why\":\"w\"}".to_string()],
        &[],
    );

    let gate_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "gate").collect();
    assert_eq!(gate_rows.len(), 1, "rows: {rows:?}");
    assert_eq!(gate_rows[0]["tool"], "mcp__arx__save");
    assert_eq!(gate_rows[0]["verdict"], "deny");
    assert_eq!(gate_rows[0]["by"], "rule");
    assert_eq!(gate_rows[0]["reason"], "deny \"mcp__arx__save\" in arx.toml");

    let main_row = rows.iter().find(|row| row["kind"] == "turn" && row["role"] == "main").unwrap();
    let text = main_row["text"].as_str().unwrap();
    assert!(
        text.contains("gate denied mcp__arx__save: deny \\\"mcp__arx__save\\\" in arx.toml"),
        "main reply: {text}"
    );

    let quarantine_dir = run_dir.join("store/quarantine/main");
    let quarantine_empty = match fs::read_dir(&quarantine_dir) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => true,
    };
    assert!(quarantine_empty, "expected no quarantined file under {}", quarantine_dir.display());
}

#[test]
fn a_denied_dispatch_writes_no_spawn() {
    let workspace = fresh_run_fixture_with_gate(Some("\n[gate]\ndeny = [\"mcp__arx__dispatch\"]\n"));
    let run_dir = workspace.path().join("run");

    let rows = run_session(&run_dir, "arx.toml", &["hand:dispatch:fake:say ok".to_string()], &[]);

    let gate_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "gate").collect();
    assert_eq!(gate_rows.len(), 1, "rows: {rows:?}");
    assert_eq!(gate_rows[0]["tool"], "mcp__arx__dispatch");
    assert_eq!(gate_rows[0]["verdict"], "deny");

    assert!(!rows.iter().any(|row| row["kind"] == "spawn"), "rows: {rows:?}");

    let main_row = rows.iter().find(|row| row["kind"] == "turn" && row["role"] == "main").unwrap();
    let text = main_row["text"].as_str().unwrap();
    assert!(text.contains("gate denied mcp__arx__dispatch:"), "main reply: {text}");

    let ledger_path = run_dir.join("threads/main.ledger.jsonl");
    let ledger_empty = match fs::read_to_string(&ledger_path) {
        Ok(contents) => contents.trim().is_empty(),
        Err(_) => true,
    };
    assert!(ledger_empty, "expected no ledger rows in {}", ledger_path.display());
}

#[test]
fn a_turn_usage_row_carries_elapsed_ms() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let env = [("ARX_STUB_DELAY_MS", "200".to_string())];
    let rows = run_session(&run_dir, "arx.toml", &["plain check".to_string()], &env);
    let usage_row = rows.iter().find(|row| row["kind"] == "usage").expect("usage row present");
    let elapsed_ms = usage_row["elapsed_ms"].as_u64().expect("elapsed_ms present");
    assert!(elapsed_ms >= 200, "elapsed_ms: {elapsed_ms}");
    assert!(elapsed_ms < 5000, "elapsed_ms: {elapsed_ms}");

    let bare_workspace = fresh_run_fixture();
    let bare_run_dir = bare_workspace.path().join("run");
    let bare_rows = run_session(&bare_run_dir, "arx.toml", &["plain check".to_string()], &[]);
    let bare_usage_row = bare_rows.iter().find(|row| row["kind"] == "usage").expect("usage row present");
    let bare_elapsed_ms = bare_usage_row["elapsed_ms"].as_u64().expect("elapsed_ms present");
    assert!(bare_elapsed_ms < 5000, "elapsed_ms: {bare_elapsed_ms}");
}

#[test]
fn a_fake_dispatch_delivered_event_carries_elapsed_and_lag() {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let log_path = workspace.path().join("run/threads/main.jsonl");

    let mut run = StdCommand::new(cargo_bin("arx"))
        .args(["run", config_path.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let mut stdin = run.stdin.take().unwrap();
        stdin.write_all(b"hand:dispatch:fake:[delay:10] job\n").unwrap();
        thread::sleep(Duration::from_millis(4000));
        stdin.write_all(b"plain check\n").unwrap();
    }

    let mut stdout = run.stdout.take().unwrap();
    let mut stderr = run.stderr.take().unwrap();
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stdout.read_to_end(&mut sink);
    });
    thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = stderr.read_to_end(&mut sink);
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = run.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = run.kill();
            panic!("arx run did not exit within the deadline");
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success());

    let contents = fs::read_to_string(&log_path).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let delivered = rows
        .iter()
        .find(|row| row["kind"] == "delivered" && row["hand"] == "h1")
        .expect("delivered event present");
    let elapsed_ms = delivered["elapsed_ms"].as_u64().expect("elapsed_ms present");
    let lag_ms = delivered["lag_ms"].as_u64().expect("lag_ms present");
    assert!(lag_ms >= 1500, "lag_ms: {lag_ms}, elapsed_ms: {elapsed_ms}");

    let events_assert = Command::cargo_bin("arx")
        .unwrap()
        .args(["events", "--path", config_path.to_str().unwrap(), "main"])
        .assert()
        .success();
    let events_output = events_assert.get_output();
    let events_stdout = String::from_utf8_lossy(&events_output.stdout).into_owned();
    assert!(events_stdout.contains("elapsed_ms="), "events stdout: {events_stdout}");
    assert!(events_stdout.contains("lag_ms="), "events stdout: {events_stdout}");
}
