#![allow(dead_code)]
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
#[path = "../src/claude_code.rs"]
mod claude_code;
#[path = "../src/codex_auth.rs"]
mod codex_auth;
#[path = "../src/codex_main.rs"]
mod codex_main;
mod common;
#[path = "../src/main_body.rs"]
mod main_body;
#[path = "../src/main_tools.rs"]
mod main_tools;
use main_body::{Control, MainBody, MainError, SpawnOpts, SystemPrompt};
use codex_main::STDERR_TAIL_BYTES;
static ENV: Mutex<()> = Mutex::new(());

fn setup() -> (std::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
    let guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let credentials = common::plant_stub_codex_credential(dir.path());
    fs::write(credentials.join("AGENTS.md"), "AMBIENT_GLOBAL_CANARY").unwrap();
    fs::write(credentials.join("config.toml"), "model = 'ambient-canary'").unwrap();
    fs::write(dir.path().join("AGENTS.md"), "AMBIENT_PROJECT_CANARY").unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stub-codex-main");
    unsafe {
        for key in [
            "ARX_CODEX_READY_TIMEOUT_MS",
            "ARX_CODEX_STALL_INITIALIZE",
            "ARX_CODEX_STDERR_TEXT",
        ] {
            std::env::remove_var(key);
        }
        std::env::set_var(
            "PATH",
            format!(
                "{}:{}",
                fixture.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
        std::env::set_var("CODEX_HOME", credentials);
        std::env::set_var("ARX_CODEX_CAPTURE", dir.path().join("capture.jsonl"));
    }
    (guard, dir)
}
fn spawn(dir: &std::path::Path) -> MainBody {
    MainBody::spawn_for(
        "codex",
        dir,
        SpawnOpts {
            model: "stub-codex".into(),
            system_prompt: SystemPrompt {
                static_block: "ARX_ONLY".into(),
                dynamic: "DYNAMIC_ONLY".into(),
            },
            cwd: dir.into(),
        },
    )
    .unwrap()
}
fn turn(body: &mut MainBody, prompt: &str) -> Result<main_body::TurnOutcome, MainError> {
    body.run_turn(
        prompt,
        |_| {},
        |_, _| {},
        |_| json!({"text":"store"}),
        |_| json!({"text":"hand"}),
        |_| json!({"text":"mounted-result"}),
    )
}
fn rows(dir: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(dir.join("capture.jsonl"))
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
#[test]
fn mounted_tool_round_trip_and_final_usage_are_correlated() {
    let (_guard, dir) = setup();
    let mut body = spawn(dir.path());
    let outcome = turn(&mut body, "file:read_file:{\"path\":\"note.md\"}").unwrap();
    assert!(outcome.text.contains("mounted-result"));
    let usage = outcome.usage.unwrap();
    assert_eq!(usage["input_tokens"], 120);
    assert_eq!(usage["codex"]["last"]["cachedInputTokens"], 10);
    let captured = rows(dir.path());
    let init = captured
        .iter()
        .find(|r| r["op"]["method"] == "thread/start")
        .unwrap();
    assert_eq!(
        init["op"]["params"]["baseInstructions"],
        "ARX_ONLY\n\nDYNAMIC_ONLY"
    );
    let result = captured
        .iter()
        .find(|r| r["op"]["id"] == "server-request-1")
        .unwrap();
    assert_eq!(result["op"]["result"]["success"], true);
    let home = PathBuf::from(init["home"].as_str().unwrap());
    body.shutdown();
    assert!(!home.exists());
}
#[test]
fn failed_tool_result_is_returned_as_unsuccessful() {
    let (_guard, dir) = setup();
    let mut body = spawn(dir.path());
    let out = body
        .run_turn(
            "file:read_file:{}",
            |_| {},
            |_, _| {},
            |_| json!({}),
            |_| json!({}),
            |_| json!({"error":"denied"}),
        )
        .unwrap();
    assert!(out.text.contains("false"));
    body.shutdown();
}
#[test]
fn interrupt_keeps_thread_and_model_change_waits_for_next_turn() {
    let (_guard, dir) = setup();
    let mut body = spawn(dir.path());
    let seen = std::cell::Cell::new(false);
    let out = body
        .run_turn_controlled(
            "hang",
            &[],
            |_| seen.set(true),
            |_, _| {},
            |_| json!({}),
            |_| json!({}),
            |_| json!({}),
            &mut || {
                if seen.replace(false) {
                    Some(Control::Stop)
                } else {
                    None
                }
            },
        )
        .unwrap();
    assert!(out.stopped);
    body.set_model("next-model").unwrap();
    assert_eq!(turn(&mut body, "after").unwrap().text, "codex:after");
    let captured = rows(dir.path());
    assert_eq!(
        captured
            .iter()
            .filter(|r| r["op"]["method"] == "thread/start")
            .count(),
        1
    );
    let turns: Vec<_> = captured
        .iter()
        .filter(|r| r["op"]["method"] == "turn/start")
        .collect();
    assert_eq!(turns[1]["op"]["params"]["model"], "next-model");
    body.shutdown();
}
#[test]
fn malformed_and_closed_transport_kill_child_and_remove_home() {
    let (_guard, dir) = setup();
    for prompt in ["malformed", "die"] {
        let mut body = spawn(dir.path());
        assert!(turn(&mut body, prompt).is_err());
    }
    for row in rows(dir.path()) {
        assert!(!PathBuf::from(row["home"].as_str().unwrap()).exists());
    }
}
#[test]
fn fresh_starts_have_distinct_threads_and_homes() {
    let (_guard, dir) = setup();
    for _ in 0..2 {
        let mut body = spawn(dir.path());
        turn(&mut body, "hello").unwrap();
        body.shutdown();
    }
    let captured = rows(dir.path());
    let starts: Vec<_> = captured
        .iter()
        .filter(|r| r["op"]["method"] == "thread/start")
        .collect();
    assert!(!PathBuf::from(starts[0]["home"].as_str().unwrap())
        .starts_with(std::env::temp_dir()));
    assert_ne!(starts[0]["thread"], starts[1]["thread"]);
    assert_ne!(starts[0]["home"], starts[1]["home"]);
}

#[test]
fn compaction_discards_the_session_before_later_tools_and_recovers_fresh() {
    let (_guard, dir) = setup();
    let mut body = spawn(dir.path());
    let err = body
        .run_turn(
            "compact",
            |_| {},
            |_, _| {},
            |_| panic!("abandoned store"),
            |_| panic!("abandoned hand"),
            |_| panic!("abandoned file"),
        )
        .unwrap_err();
    assert!(matches!(err,MainError::VendorCompaction{text} if text=="partial before compaction"));
    body.respawn(
        dir.path(),
        &SpawnOpts {
            model: "stub-codex".into(),
            system_prompt: SystemPrompt {
                static_block: "fresh".into(),
                dynamic: String::new(),
            },
            cwd: dir.path().into(),
        },
    )
    .unwrap();
    assert_eq!(turn(&mut body, "recovery").unwrap().text, "codex:recovery");
    let captured = rows(dir.path());
    let starts: Vec<_> = captured
        .iter()
        .filter(|r| r["op"]["method"] == "thread/start")
        .collect();
    assert_eq!(starts.len(), 2);
    assert_ne!(starts[0]["home"], starts[1]["home"]);
    body.shutdown();
}
#[test]
fn exhaustion_is_a_common_floor_outcome() {
    let (_guard, dir) = setup();
    let mut body = spawn(dir.path());
    assert!(matches!(
        turn(&mut body, "exhaust"),
        Err(MainError::ContextWindowExceeded { .. })
    ));
}

fn product_fixture(codex: bool) -> tempfile::TempDir {
    let root = common::fresh_run_fixture();
    let config = root.path().join("run/arx.toml");
    if codex {
        let text = fs::read_to_string(&config)
            .unwrap()
            .replace("main = \"stub-model\"", "main = \"gpt-5.6-sol\"");
        fs::write(&config, text).unwrap();
    }
    fs::write(root.path().join("run/checkpoint-author.md"), "").unwrap();
    root
}
fn run_product(
    root: &std::path::Path,
    lines: &[&str],
    extra: &[(&str, String)],
) -> (Vec<Value>, String) {
    let mut env = vec![("PATH", std::env::var("PATH").unwrap())];
    env.extend_from_slice(extra);
    common::run_session_with_stdout(
        &root.join("run"),
        "arx.toml",
        &lines.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &env,
    )
}

fn run_product_with_output(
    root: &std::path::Path,
    lines: &[&str],
    extra: &[(&str, String)],
) -> (Vec<Value>, String, String) {
    let mut env = vec![("PATH", std::env::var("PATH").unwrap())];
    env.extend_from_slice(extra);
    common::run_session_with_output(
        &root.join("run"),
        "arx.toml",
        &lines.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &env,
    )
}

#[test]
fn codex_stderr_is_drained_without_reaching_the_terminal() {
    let (_guard, _dir) = setup();
    let root = product_fixture(true);
    let (_, stdout, stderr) = run_product_with_output(
        root.path(),
        &["after"],
        &[
            ("ARX_CODEX_STDERR_BYTES", (1024 * 1024).to_string()),
            ("ARX_CODEX_STDERR_TEXT", "CODEX_STDERR_SENTINEL".into()),
        ],
    );
    assert!(stdout.contains("codex:after"), "{stdout}");
    assert!(!stdout.contains("CODEX_STDERR_SENTINEL"), "{stdout}");
    assert!(!stderr.contains("CODEX_STDERR_SENTINEL"), "{stderr}");
}

#[test]
fn codex_startup_failure_reports_the_bounded_stderr_tail() {
    let (_guard, _dir) = setup();
    let root = product_fixture(true);
    let home_capture = root.path().join("startup-home");
    let (code, stdout, stderr) = common::run_session_expect_failure_with_stdout(
        &root.path().join("run"),
        "arx.toml",
        &["after".into()],
        &[
            ("PATH", std::env::var("PATH").unwrap()),
            ("ARX_CODEX_STDERR_BYTES", (STDERR_TAIL_BYTES * 2).to_string()),
            ("ARX_CODEX_STDERR_TEXT", "CODEX_STARTUP_FAILURE".into()),
            ("ARX_CODEX_FAIL_STARTUP", "1".into()),
            (
                "ARX_CODEX_STARTUP_HOME_CAPTURE",
                home_capture.display().to_string(),
            ),
        ],
    );
    assert_eq!(code, 1, "stdout={stdout} stderr={stderr}");
    assert!(!stdout.contains("CODEX_STARTUP_FAILURE"), "{stdout}");
    assert!(stderr.contains("codex app-server stopped:"), "{stderr}");
    assert!(stderr.contains("CODEX_STARTUP_FAILURE"), "{stderr}");
    assert!(stderr.len() < STDERR_TAIL_BYTES * 2, "stderr was not bounded: {}", stderr.len());
    let home = fs::read_to_string(home_capture).unwrap();
    assert!(!Path::new(&home).exists(), "failed Codex home was not removed: {home}");
}

#[test]
fn codex_write_failure_reports_the_bounded_stderr_tail() {
    let (_guard, dir) = setup();
    unsafe {
        std::env::set_var("ARX_CODEX_STDERR_TEXT", "CODEX_WRITE_FAILURE");
    }
    let mut body = spawn(dir.path());
    assert_eq!(turn(&mut body, "close-stdin").unwrap().text, "closed");
    let err = turn(&mut body, &"x".repeat(1024 * 1024)).unwrap_err();
    assert!(
        matches!(&err, MainError::BodyFatal { message } if message.contains("codex app-server stopped:") && message.contains("CODEX_WRITE_FAILURE")),
        "{err:?}"
    );
}

#[test]
fn codex_write_failure_without_stderr_preserves_spawn_io() {
    let (_guard, dir) = setup();
    let mut body = spawn(dir.path());
    assert_eq!(turn(&mut body, "close-stdin").unwrap().text, "closed");
    let err = turn(&mut body, &"x".repeat(1024 * 1024)).unwrap_err();
    assert!(matches!(err, MainError::SpawnIo { .. }), "{err:?}");
}

#[test]
fn codex_ready_timeout_reports_the_bounded_stderr_tail() {
    let (_guard, dir) = setup();
    unsafe {
        std::env::set_var("ARX_CODEX_STDERR_TEXT", "CODEX_READY_TIMEOUT");
        std::env::set_var("ARX_CODEX_STALL_INITIALIZE", "1");
        std::env::set_var("ARX_CODEX_READY_TIMEOUT_MS", "50");
    }
    let result = MainBody::spawn_for(
        "codex",
        dir.path(),
        SpawnOpts {
            model: "stub-codex".into(),
            system_prompt: SystemPrompt {
                static_block: "ARX_ONLY".into(),
                dynamic: "DYNAMIC_ONLY".into(),
            },
            cwd: dir.path().into(),
        },
    );
    let err = match result {
        Ok(_) => panic!("Codex startup unexpectedly completed"),
        Err(err) => err,
    };
    assert!(
        matches!(&err, MainError::BodyFatal { message } if message.contains("codex app-server stopped:") && message.contains("CODEX_READY_TIMEOUT")),
        "{err:?}"
    );
}

#[test]
fn codex_ready_timeout_without_stderr_preserves_timeout() {
    let (_guard, dir) = setup();
    unsafe {
        std::env::set_var("ARX_CODEX_STALL_INITIALIZE", "1");
        std::env::set_var("ARX_CODEX_READY_TIMEOUT_MS", "50");
    }
    let result = MainBody::spawn_for(
        "codex",
        dir.path(),
        SpawnOpts {
            model: "stub-codex".into(),
            system_prompt: SystemPrompt {
                static_block: "ARX_ONLY".into(),
                dynamic: "DYNAMIC_ONLY".into(),
            },
            cwd: dir.path().into(),
        },
    );
    let err = match result {
        Ok(_) => panic!("Codex startup unexpectedly completed"),
        Err(err) => err,
    };
    assert!(matches!(err, MainError::ReadyTimeout), "{err:?}");
}
#[test]
fn typed_cross_body_and_clear_run_checked_handoffs_before_announcing_models() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let (rows, stdout) = run_product(
        root.path(),
        &[
            "hello",
            "/model main gpt-5.6-sol",
            "after codex",
            "/model clear main",
            "after claude",
        ],
        &[],
    );
    assert!(stdout.contains("codex:after codex"), "{stdout}");
    assert!(stdout.contains("stub:after claude"), "{stdout}");
    assert_eq!(
        rows.iter()
            .filter(|r| r["kind"] == "checkpoint" && r["check"] == "pass")
            .count(),
        2,
        "{rows:?}"
    );
    assert_eq!(
        rows.iter().filter(|r| r["kind"] == "orientation").count(),
        2,
        "{rows:?}"
    );
    let model_indices: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r["kind"] == "model")
        .map(|(i, _)| i)
        .collect();
    for index in model_indices {
        assert_eq!(rows[index - 1]["kind"], "orientation");
    }
}

#[test]
fn outgoing_claude_quota_during_switch_hands_state_to_codex_and_keeps_queued_pins() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let (rows, stdout) = run_product(
        root.path(),
        &[
            "/model main gpt-5.6-sol",
            "/model hand-ex gpt-5.6-sol",
            "after",
        ],
        &[("ARX_STUB_USAGE_LIMIT_AUTHORING_AT", "1".into())],
    );
    assert!(stdout.contains("You've reached your weekly limit"), "{stdout}");
    assert!(stdout.contains("codex:after"), "{stdout}");
    assert!(
        stdout.contains("outgoing main became unavailable during the reset ritual"),
        "{stdout}"
    );
    assert!(!rows.iter().any(|r| r["kind"] == "floor"), "{rows:?}");
    let reset = rows
        .iter()
        .position(|r| {
            r["kind"] == "reset"
                && r["phase"] == "respawned"
                && r["check"] == "outgoing_body_unavailable"
        })
        .unwrap();
    let orientation = rows.iter().position(|r| r["kind"] == "orientation").unwrap();
    let model = rows.iter().position(|r| {
        r["kind"] == "model"
            && r["position"] == "main"
            && r["to"] == "gpt-5.6-sol"
    }).unwrap();
    assert!(reset < orientation && orientation < model, "{rows:?}");
    let pins: toml::Value = toml::from_str(
        &fs::read_to_string(
            fs::read_dir(root.path().join("run/roster"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(pins["pins"]["main"].as_str(), Some("gpt-5.6-sol"));
    assert_eq!(pins["pins"]["hand-ex"].as_str(), Some("gpt-5.6-sol"));
}

#[test]
fn outgoing_claude_quota_during_truth_check_keeps_the_previous_checkpoint() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let author = "## Position\nnothing new.\n## Target\nkeep moving.\n## Drift\nnone.\n## Hands in flight\n- none\n";
    common::plant_checkpoint_replies(&root.path().join("run"), author, None, Some("ok"));
    let (rows, stdout) = run_product(
        root.path(),
        &["/checkpoint", "/model main gpt-5.6-sol", "after"],
        &[("ARX_STUB_USAGE_LIMIT_TRUTH_AT", "2".into())],
    );
    assert!(stdout.contains("You've reached your weekly limit"), "{stdout}");
    assert!(stdout.contains("codex:after"), "{stdout}");
    assert!(root.path().join("run/threads/main.checkpoints/1.md").exists());
    assert!(!root.path().join("run/threads/main.checkpoints/2.md").exists());
    assert!(!rows.iter().any(|r| r["kind"] == "floor"), "{rows:?}");
    assert!(rows.iter().any(|r| {
        r["kind"] == "reset"
            && r["phase"] == "respawned"
            && r["check"] == "outgoing_body_unavailable"
    }), "{rows:?}");
    assert!(rows.iter().any(|r| {
        r["kind"] == "turn"
            && r["role"] == "arx"
            && r["text"]
                .as_str()
                .unwrap_or_default()
                .contains("outgoing body became unavailable before the reset ritual could complete")
    }), "{rows:?}");
}

#[test]
fn ordinary_claude_quota_stops_once_without_reset_or_respawn() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let (code, stdout, stderr) = common::run_session_expect_failure_with_stdout(
        &root.path().join("run"),
        "arx.toml",
        &["usage-limit".into()],
        &[("PATH", std::env::var("PATH").unwrap())],
    );
    assert_eq!(code, 1, "stdout={stdout} stderr={stderr}");
    assert_eq!(stdout.matches("[arx] main unavailable:").count(), 1, "{stdout}");
    assert!(stdout.contains("You've reached your weekly limit"), "{stdout}");
    let rows: Vec<Value> = fs::read_to_string(root.path().join("run/threads/main.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(!rows.iter().any(|r| matches!(r["kind"].as_str(), Some("floor" | "reset" | "orientation"))), "{rows:?}");
}

#[test]
fn generic_checkpoint_failure_recovers_claude_and_does_not_pin_codex() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let (rows, stdout) = run_product(
        root.path(),
        &["/model main gpt-5.6-sol", "after"],
        &[("ARX_STUB_FATAL_AUTHORING_AT", "1".into())],
    );
    assert!(stdout.contains("destination failed; recovered the previous main"), "{stdout}");
    assert!(stdout.contains("stub:after"), "{stdout}");
    assert!(!stdout.contains("codex:after"), "{stdout}");
    assert!(!rows.iter().any(|r| r["kind"] == "model"), "{rows:?}");
    let roster_dir = root.path().join("run/roster");
    if let Some(path) = fs::read_dir(&roster_dir).ok().and_then(|mut entries| entries.next()).and_then(Result::ok).map(|entry| entry.path()) {
        let pins: toml::Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert!(pins.get("pins").and_then(|pins| pins.get("main")).is_none(), "{pins}");
    }
}

#[test]
fn model_command_refuses_a_native_runner_pin() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let (_, stdout) = run_product(
        root.path(),
        &["/model runner fake-model", "after"],
        &[],
    );
    assert!(
        stdout.contains("runner requires claude-code or codex so it can implement the brief"),
        "{stdout}"
    );
    assert!(stdout.contains("stub:after"), "{stdout}");
    let roster_dir = root.path().join("run/roster");
    if let Some(path) = fs::read_dir(&roster_dir)
        .ok()
        .and_then(|mut entries| entries.next())
        .and_then(Result::ok)
        .map(|entry| entry.path())
    {
        let pins: toml::Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert!(
            pins.get("pins").and_then(|pins| pins.get("runner")).is_none(),
            "{pins}"
        );
    }
}

#[test]
fn startup_persists_a_retired_judge_pin_as_reviewer() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    run_product(
        root.path(),
        &["/model hand-ex gpt-5.6-sol"],
        &[],
    );
    let roster_path = fs::read_dir(root.path().join("run/roster"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&roster_path, "[pins]\njudge = \"gpt-5.6-sol\"\n").unwrap();

    let (_, stdout) = run_product(root.path(), &["after"], &[]);
    assert!(
        stdout.contains("moved the retired judge pin to reviewer"),
        "{stdout}"
    );
    let roster: toml::Value = toml::from_str(&fs::read_to_string(&roster_path).unwrap()).unwrap();
    assert!(roster["pins"].get("judge").is_none(), "{roster}");
    assert_eq!(roster["pins"]["reviewer"].as_str(), Some("gpt-5.6-sol"));
}
#[test]
fn pinned_codex_restarts_without_a_claude_sidecar() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    run_product(root.path(), &["/model gpt-5.6-sol"], &[]);
    fs::remove_dir_all(root.path().join("stub-sidecar")).unwrap();
    common::with_main_window_and_reserve(&root.path().join("run/arx.toml"), 950_000, 16_000);
    let (_, stdout) = run_product(root.path(), &["fresh"], &[]);
    assert!(stdout.contains("codex:fresh"), "{stdout}");
}
#[test]
fn codex_normal_checkpoint_reset_and_compaction_use_checked_state() {
    let (_guard, _dir) = setup();
    let root = product_fixture(true);
    let (rows, _) = run_product(
        root.path(),
        &[
            "hello",
            "/checkpoint",
            "compact",
            "after",
            "/reset",
            "ok",
            "last",
        ],
        &[],
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r["kind"] == "reset" && r["phase"] == "recovery")
            .count(),
        1,
        "{rows:?}"
    );
    assert_eq!(
        rows.iter().filter(|r| r["kind"] == "orientation").count(),
        2,
        "{rows:?}"
    );
    assert!(
        rows.iter()
            .any(|r| r["kind"] == "turn" && r["text"] == "partial before compaction")
    );
    assert!(rows.iter().any(|r| r["kind"] == "reset"
        && r["phase"] == "respawned"
        && r["check"] == "checkpoint 1"));
}
#[test]
fn compaction_during_truth_check_never_promotes_the_unfinished_object() {
    let (_guard, _dir) = setup();
    let root = product_fixture(true);
    let (rows, _) = run_product(
        root.path(),
        &["hello", "/reset", "after"],
        &[("ARX_CODEX_COMPACT_TRUTH", "1".into())],
    );
    assert!(
        !root
            .path()
            .join("run/threads/main.checkpoints/1.md")
            .exists()
    );
    assert_eq!(
        rows.iter().filter(|r| r["kind"] == "orientation").count(),
        1
    );
    assert!(
        rows.iter().any(|r| r["kind"] == "reset"
            && r["phase"] == "respawned"
            && r["check"] == "transcript")
    );
}
#[test]
fn roster_prepare_failure_leaves_same_body_model_and_pin_unchanged() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let block = root.path().join("blocked");
    fs::write(&block, "not a directory").unwrap();
    common::with_main_models(&root.path().join("run/arx.toml"), &["stub-model-2"]);
    let (rows, stdout) = run_product(
        root.path(),
        &["/model stub-model-2", "after"],
        &[("ARX_ROSTER_DIR", block.display().to_string())],
    );
    assert!(!rows.iter().any(|r| r["kind"] == "model"), "{rows:?}");
    assert!(!root.path().join("run/stub-set-model.json").exists());
    assert!(stdout.contains("failed"), "{stdout}");
}
#[test]
fn missing_codex_executable_keeps_the_outgoing_main() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let (_, stdout) = run_product(
        root.path(),
        &["/model gpt-5.6-sol", "after"],
        &[("PATH", "/usr/bin:/bin".into())],
    );
    assert!(stdout.contains("stub:after"));
    assert!(!stdout.contains("switched stub-model"));
}
#[test]
fn upstream_model_catalog_refusal_leaves_same_body_selection_unchanged() {
    let (_guard, _dir) = setup();
    let root = product_fixture(true);
    let config = root.path().join("run/arx.toml");
    let mut text = fs::read_to_string(&config).unwrap();
    text.push_str("\n[[models]]\nid = \"unlisted-model\"\nbody = \"codex\"\n");
    fs::write(config, text).unwrap();
    let (rows, stdout) = run_product(root.path(), &["/model unlisted-model", "after"], &[]);
    assert!(!rows.iter().any(|r| r["kind"] == "model"));
    assert!(stdout.contains("selection was not changed"), "{stdout}");
    assert!(stdout.contains("codex:after"));
}

#[test]
#[ignore = "uses installed Codex and paid account"]
fn installed_codex_tool_interrupt_followup_and_model_change() {
    let dir = tempfile::tempdir().unwrap();
    let mut body = MainBody::spawn_for(
        "codex",
        dir.path(),
        SpawnOpts {
            model: "gpt-5.6-sol".into(),
            system_prompt: SystemPrompt {
                static_block: "You are arx main. Use only mounted arx tools. Your canary is ARX_MAIN_LIVE_CANARY.".into(),
                dynamic: String::new(),
            },
            cwd: dir.path().into(),
        },
    )
    .unwrap();
    let called = std::cell::Cell::new(false);
    let reply = body
        .run_turn(
            "Call arx read_file with path probe.txt, then reply with its result and your canary.",
            |_| {},
            |_, _| {},
            |_| json!({}),
            |_| json!({}),
            |call| {
                assert_eq!(call.tool, "read_file");
                called.set(true);
                json!({"text":"REAL_TOOL_RESULT"})
            },
        )
        .unwrap();
    assert!(called.get());
    assert!(reply.text.contains("REAL_TOOL_RESULT"));
    assert!(reply.text.contains("ARX_MAIN_LIVE_CANARY"));
    let streamed = std::cell::Cell::new(false);
    let stopped = body
        .run_turn_controlled(
            "Write the numbers from 1 to 5000, one per line. Do not use tools.",
            &[],
            |_| streamed.set(true),
            |_, _| {},
            |_| json!({}),
            |_| json!({}),
            |_| json!({}),
            &mut || {
                if streamed.replace(false) {
                    Some(Control::Stop)
                } else {
                    None
                }
            },
        )
        .unwrap();
    assert!(stopped.stopped, "{stopped:?}");
    let followup = turn(&mut body, "Reply only AFTER_STOP.").unwrap();
    assert!(followup.text.contains("AFTER_STOP"));
    body.set_model("gpt-6-astra").unwrap();
    let switched = turn(&mut body, "Reply only MODEL_CHANGED.").unwrap();
    assert!(switched.text.contains("MODEL_CHANGED"));
    println!(
        "live mounted-tool: {}; interrupt: {}; followup: {}; model change: {}",
        reply.text, stopped.stopped, followup.text, switched.text
    );
    body.shutdown();
}

#[test]
fn an_in_flight_hand_delivers_after_compaction_recovery() {
    let (_guard, _dir) = setup();
    let mut events = vec![r#"{"choices":[{"delta":{"content":""}}]}"#.to_string(); 30];
    events.push(r#"{"choices":[{"delta":{"content":"HAND_SURVIVED"}}]}"#.to_string());
    events.push(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string());
    let (url, _) = common::fake_openai_server(common::FakeScript::Sse(events));
    let root = product_fixture(true);
    let config = root.path().join("run/arx.toml");
    fs::write(
        &config,
        fs::read_to_string(&config)
            .unwrap()
            .replace("http://127.0.0.1:0", &url),
    )
    .unwrap();
    let (rows, _) = run_product(
        root.path(),
        &[
            "hand:dispatch:{\"agent\":\"hand-ds\",\"brief\":\"Reply HAND_SURVIVED\"}",
            "compact",
        ],
        &[("ARX_TEST_FAKE_KEY", "test-key".into())],
    );
    let orientation = rows
        .iter()
        .position(|r| r["kind"] == "orientation")
        .unwrap();
    let delivered = rows.iter().position(|r| r["kind"] == "delivered").unwrap();
    assert!(orientation < delivered, "{rows:?}");
    assert!(
        rows.iter().any(|r| r["kind"] == "turn"
            && r["role"] == "arx"
            && r["text"]
                .as_str()
                .unwrap_or_default()
                .contains("HAND_SURVIVED")),
        "{rows:?}"
    );
}

#[test]
fn destination_start_failure_preserves_the_previous_binding() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let config = root.path().join("run/arx.toml");
    let mut text = fs::read_to_string(&config).unwrap();
    text.push_str("\n[[models]]\nid = \"fail-start\"\nbody = \"codex\"\n");
    fs::write(config, text).unwrap();
    let (rows, stdout) = run_product(root.path(), &["/model fail-start", "after"], &[]);
    assert!(!rows.iter().any(|r| r["kind"] == "model"));
    assert!(!rows.iter().any(|r| r["kind"] == "checkpoint"));
    assert!(stdout.contains("start refused"));
    assert!(stdout.contains("stub:after"));
}

#[test]
fn destination_model_refusal_recovers_previous_main_from_checked_state() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let config = root.path().join("run/arx.toml");
    let mut text = fs::read_to_string(&config).unwrap();
    text.push_str("\n[[models]]\nid = \"refused-model\"\nbody = \"codex\"\n");
    fs::write(config, text).unwrap();
    let (rows, stdout) = run_product(root.path(), &["/model refused-model", "after"], &[]);
    assert!(!rows.iter().any(|r| r["kind"] == "model"));
    assert!(
        rows.iter()
            .any(|r| r["kind"] == "checkpoint" && r["check"] == "pass")
    );
    assert!(stdout.contains("recovered the previous main"), "{stdout}");
    assert!(stdout.contains("stub:after"));
    assert!(!rows.iter().any(|r| r["kind"] == "floor"), "{rows:?}");
    assert!(rows.iter().any(|r| {
        r["kind"] == "reset"
            && r["phase"] == "respawned"
            && r["check"] == "switch_recovery"
    }), "{rows:?}");
}

#[test]
fn final_roster_save_failure_recovers_previous_main_without_a_model_event() {
    let (_guard, _dir) = setup();
    let root = product_fixture(false);
    let (rows, stdout) = run_product(
        root.path(),
        &["/model gpt-5.6-sol", "after"],
        &[("ARX_CODEX_BLOCK_ROSTER", "1".into())],
    );
    assert!(!rows.iter().any(|r| r["kind"] == "model"), "{rows:?}");
    assert!(stdout.contains("roster save failed"), "{stdout}");
    assert!(stdout.contains("stub:after"), "{stdout}");
    assert!(!rows.iter().any(|r| r["kind"] == "floor"), "{rows:?}");
    assert!(rows.iter().any(|r| {
        r["kind"] == "reset"
            && r["phase"] == "respawned"
            && r["check"] == "switch_recovery"
    }), "{rows:?}");
    assert_eq!(
        rows.iter().filter(|r| r["kind"] == "orientation").count(),
        2
    );
    assert_eq!(
        rows.iter().filter(|r| r["kind"] == "usage").last().unwrap()["usage"]["model"],
        "stub-model"
    );
}

#[test]
fn codex_ordinary_and_ritual_exhaustion_recover_through_the_product_loop() {
    let (_guard, _dir) = setup();
    for (lines, extra) in [
        (vec!["exhaust", "after"], vec![]),
        (
            vec!["/reset", "after"],
            vec![("ARX_CODEX_EXHAUST_AUTHOR", "1".into())],
        ),
        (
            vec!["/reset", "after"],
            vec![("ARX_CODEX_EXHAUST_TRUTH", "1".into())],
        ),
    ] {
        let root = product_fixture(true);
        let (rows, stdout) = run_product(root.path(), &lines, &extra);
        assert_eq!(
            rows.iter().filter(|r| r["kind"] == "orientation").count(),
            1,
            "{rows:?}"
        );
        assert!(rows.iter().any(|r| r["kind"] == "reset"
            && r["phase"] == "respawned"
            && r["check"] == "transcript"));
        assert!(stdout.contains("codex:after"), "{stdout}");
    }
}

#[test]
fn compaction_during_truth_check_keeps_the_previous_checked_object() {
    let (_guard, _dir) = setup();
    let root = product_fixture(true);
    let (rows, _) = run_product(
        root.path(),
        &["/checkpoint", "/reset", "after"],
        &[("ARX_CODEX_COMPACT_TRUTH", "2".into())],
    );
    assert!(
        root.path()
            .join("run/threads/main.checkpoints/1.md")
            .exists()
    );
    assert!(
        !root
            .path()
            .join("run/threads/main.checkpoints/2.md")
            .exists()
    );
    assert!(rows.iter().any(|r| r["kind"] == "reset"
        && r["phase"] == "respawned"
        && r["check"] == "checkpoint 1"));
}

#[test]
fn listed_model_refusal_restores_pin_and_preserves_the_conversation() {
    let (_guard, capture) = setup();
    let root = product_fixture(true);
    let config = root.path().join("run/arx.toml");
    let mut config_text = fs::read_to_string(&config).unwrap();
    config_text.push_str("\n[[models]]\nid = \"refused-model\"\nbody = \"codex\"\n");
    fs::write(config, config_text).unwrap();
    let (events, stdout) = run_product(
        root.path(),
        &[
            "before",
            "/model gpt-5.6-sol",
            "/model refused-model",
            "retry input",
            "after",
        ],
        &[],
    );
    assert!(stdout.contains("activation pending"), "{stdout}");
    assert!(stdout.contains("Codex refused 'refused-model'"), "{stdout}");
    assert!(!stdout.contains("gpt-5.6-sol -> refused-model"), "{stdout}");
    assert!(stdout.contains("codex:retry input"), "{stdout}");
    assert!(stdout.contains("codex:after"), "{stdout}");
    assert!(
        !events
            .iter()
            .any(|r| r["kind"] == "model" && r["to"] == "refused-model")
    );
    assert_eq!(
        events
            .iter()
            .filter(|r| r["role"] == "operator" && r["text"] == "retry input")
            .count(),
        1
    );
    let captured = rows(capture.path());
    assert_eq!(
        captured
            .iter()
            .filter(|r| r["op"]["method"] == "thread/start")
            .count(),
        1
    );
    let models: Vec<_> = captured
        .iter()
        .filter(|r| r["op"]["method"] == "turn/start")
        .map(|r| r["op"]["params"]["model"].as_str().unwrap())
        .collect();
    assert_eq!(
        models,
        ["gpt-5.6-sol", "refused-model", "gpt-5.6-sol", "gpt-5.6-sol"]
    );
    let pins: toml::Value = toml::from_str(
        &fs::read_to_string(
            fs::read_dir(root.path().join("run/roster"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(pins["pins"]["main"].as_str(), Some("gpt-5.6-sol"));
    let (_, restarted) = run_product(root.path(), &["restarted"], &[]);
    assert!(restarted.contains("codex:restarted"), "{restarted}");
    let captured = rows(capture.path());
    assert_eq!(
        captured
            .iter()
            .filter(|r| r["op"]["method"] == "thread/start")
            .last()
            .unwrap()["op"]["params"]["model"],
        "gpt-5.6-sol"
    );
}

#[test]
fn desired_codex_selection_survives_quit_before_the_next_turn() {
    let (_guard, capture) = setup();
    let root = product_fixture(true);
    let config = root.path().join("run/arx.toml");
    let mut config_text = fs::read_to_string(&config).unwrap();
    config_text.push_str("\n[[models]]\nid = \"next-model\"\nbody = \"codex\"\n");
    fs::write(config, config_text).unwrap();
    let (events, stdout) = run_product(root.path(), &["/model next-model"], &[]);
    assert!(
        stdout.contains("next-model saved; activation pending"),
        "{stdout}"
    );
    assert!(!events.iter().any(|r| r["kind"] == "model"));
    assert!(
        !rows(capture.path())
            .iter()
            .any(|r| r["op"]["method"] == "turn/start")
    );
    let (_, stdout) = run_product(root.path(), &["after restart"], &[]);
    assert!(stdout.contains("codex:after restart"), "{stdout}");
    assert_eq!(
        rows(capture.path())
            .iter()
            .filter(|r| r["op"]["method"] == "thread/start")
            .last()
            .unwrap()["op"]["params"]["model"],
        "next-model"
    );
}

#[test]
fn failure_after_turn_start_never_replays_the_operator_request() {
    let (_guard, capture) = setup();
    let mut body = spawn(capture.path());
    body.set_model("next-model").unwrap();
    assert!(matches!(
        turn(&mut body, "fail-after-start"),
        Err(MainError::BodyFatal { .. })
    ));
    assert_eq!(
        body.take_model_selection_result(),
        Some(Ok("next-model".into()))
    );
    assert_eq!(
        rows(capture.path())
            .iter()
            .filter(|r| r["op"]["method"] == "turn/start")
            .count(),
        1
    );
}

#[test]
fn pending_model_activates_during_checkpoint_without_overwriting_other_pins() {
    let (_guard, capture) = setup();
    let root = product_fixture(true);
    let config = root.path().join("run/arx.toml");
    let mut config_text = fs::read_to_string(&config).unwrap();
    config_text.push_str("\n[[models]]\nid = \"next-model\"\nbody = \"codex\"\n");
    fs::write(config, config_text).unwrap();
    let (events, stdout) = run_product(
        root.path(),
        &[
            "/model next-model",
            "/model reviewer gpt-5.6-sol",
            "/checkpoint",
            "/reset",
            "after",
        ],
        &[],
    );
    assert!(stdout.contains("gpt-5.6-sol -> next-model"), "{stdout}");
    assert_eq!(
        events
            .iter()
            .filter(|r| r["kind"] == "model" && r["position"] == "main" && r["to"] == "next-model")
            .count(),
        1
    );
    let captured = rows(capture.path());
    assert!(
        captured
            .iter()
            .filter(|r| r["op"]["method"] == "turn/start")
            .all(|r| r["op"]["params"]["model"] == "next-model")
    );
    assert_eq!(
        captured
            .iter()
            .filter(|r| r["op"]["method"] == "thread/start")
            .last()
            .unwrap()["op"]["params"]["model"],
        "next-model"
    );
    let pins: toml::Value = toml::from_str(
        &fs::read_to_string(
            fs::read_dir(root.path().join("run/roster"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(pins["pins"]["main"].as_str(), Some("next-model"));
    assert_eq!(pins["pins"]["reviewer"].as_str(), Some("gpt-5.6-sol"));
}

#[test]
fn refused_same_body_clear_restores_the_previous_pin() {
    let (_guard, capture) = setup();
    let root = product_fixture(true);
    let config = root.path().join("run/arx.toml");
    let mut config_text = fs::read_to_string(&config)
        .unwrap()
        .replace("main = \"gpt-5.6-sol\"", "main = \"refused-model\"");
    config_text.push_str("\n[[models]]\nid = \"refused-model\"\nbody = \"codex\"\n");
    fs::write(config, config_text).unwrap();
    run_product(root.path(), &["/model gpt-5.6-sol"], &[]);
    let (events, stdout) = run_product(root.path(), &["/model clear main", "after clear"], &[]);
    assert!(stdout.contains("Codex refused 'refused-model'"), "{stdout}");
    assert!(stdout.contains("codex:after clear"), "{stdout}");
    assert!(
        !events
            .iter()
            .any(|r| r["kind"] == "model" && r["to"] == "refused-model")
    );
    let pins: toml::Value = toml::from_str(
        &fs::read_to_string(
            fs::read_dir(root.path().join("run/roster"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(pins["pins"]["main"].as_str(), Some("gpt-5.6-sol"));
    assert_eq!(
        rows(capture.path())
            .iter()
            .filter(|r| r["op"]["method"] == "turn/start")
            .last()
            .unwrap()["op"]["params"]["model"],
        "gpt-5.6-sol"
    );
}

#[test]
fn failed_replacement_keeps_the_pending_selection_and_its_rollback_baseline() {
    for pending in ["next-model", "refused-model"] {
        for launch_failure in [false, true] {
            let (_guard, capture) = setup();
            let root = product_fixture(true);
            let config = root.path().join("run/arx.toml");
            let mut config_text = fs::read_to_string(&config).unwrap();
            for model in ["next-model", "refused-model", "not-in-catalogue"] {
                config_text.push_str(&format!(
                    "\n[[models]]\nid = \"{model}\"\nbody = \"codex\"\n"
                ));
            }
            fs::write(config, config_text).unwrap();
            if launch_failure {
                fs::remove_file(root.path().join("stub-sidecar/claude-main.mjs")).unwrap();
            }
            let selection = format!("/model {pending}");
            let replacement = if launch_failure {
                "/model stub-model"
            } else {
                "/model not-in-catalogue"
            };
            let (events, stdout) = run_product(
                root.path(),
                &[
                    "/model gpt-5.6-sol",
                    &selection,
                    replacement,
                    "after",
                    "followup",
                ],
                &[],
            );
            let expected = if pending == "refused-model" {
                "gpt-5.6-sol"
            } else {
                "next-model"
            };
            assert!(stdout.contains("activation pending"), "{stdout}");
            assert!(stdout.contains("failed"), "{stdout}");
            assert!(stdout.contains("codex:followup"), "{stdout}");
            let captured = rows(capture.path());
            let turns: Vec<_> = captured
                .iter()
                .filter(|r| r["op"]["method"] == "turn/start")
                .collect();
            assert_eq!(turns[0]["op"]["params"]["model"], pending, "{stdout}");
            assert_eq!(
                turns.last().unwrap()["op"]["params"]["model"],
                expected,
                "{stdout}"
            );
            assert!(!events.iter().any(|r| r["kind"] == "model"
                && matches!(
                    r["to"].as_str(),
                    Some("refused-model" | "not-in-catalogue" | "stub-model")
                )));
            let pins: toml::Value = toml::from_str(
                &fs::read_to_string(
                    fs::read_dir(root.path().join("run/roster"))
                        .unwrap()
                        .next()
                        .unwrap()
                        .unwrap()
                        .path(),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(pins["pins"]["main"].as_str(), Some(expected), "{stdout}");
        }
    }
}

#[test]
fn replacing_a_pending_selection_preserves_the_original_refusal_baseline() {
    let (_guard, capture) = setup();
    let root = product_fixture(true);
    let config = root.path().join("run/arx.toml");
    let mut config_text = fs::read_to_string(&config).unwrap();
    for model in ["next-model", "refused-model"] {
        config_text.push_str(&format!(
            "\n[[models]]\nid = \"{model}\"\nbody = \"codex\"\n"
        ));
    }
    fs::write(config, config_text).unwrap();
    let (events, stdout) = run_product(
        root.path(),
        &["/model next-model", "/model refused-model", "after"],
        &[],
    );
    assert!(stdout.contains("Codex refused 'refused-model'"), "{stdout}");
    assert!(!events.iter().any(|r| r["kind"] == "model"));
    let pins: toml::Value = toml::from_str(
        &fs::read_to_string(
            fs::read_dir(root.path().join("run/roster"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(pins["pins"].get("main").is_none());
    let captured = rows(capture.path());
    let turns: Vec<_> = captured
        .iter()
        .filter(|r| r["op"]["method"] == "turn/start")
        .collect();
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0]["op"]["params"]["model"], "refused-model");
    assert_eq!(turns[1]["op"]["params"]["model"], "gpt-5.6-sol");
}
