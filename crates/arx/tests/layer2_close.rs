use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use assert_cmd::cargo::cargo_bin;

mod common;
use common::*;

fn ledger_rows(run_dir: &Path) -> Vec<serde_json::Value> {
    let contents = fs::read_to_string(run_dir.join("threads/main.ledger.jsonl")).unwrap_or_default();
    contents.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

fn arx_text(rows: &[serde_json::Value], needle: &str) -> String {
    rows.iter()
        .find(|row| {
            row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.contains(needle)).unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("no arx row contains {needle}: {rows:?}"))["text"]
        .as_str()
        .unwrap()
        .to_string()
}

fn quarantine_id_by_question(quarantine_dir: &Path, question: &str) -> String {
    for entry in fs::read_dir(quarantine_dir).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value["question"] == question {
            return value["id"].as_str().unwrap().to_string();
        }
    }
    panic!("no quarantined candidate with question '{question}' under {}", quarantine_dir.display());
}

fn commit_subject(root: &Path) -> String {
    let output = StdCommand::new("git").arg("-C").arg(root).arg("log").arg("-1").arg("--format=%s").output().unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn assert_drain_subject(subject: &str, thread: &str, promoted: usize, dropped: usize) {
    let prefix = format!("drain {thread}@");
    let suffix = format!(": promoted {promoted}, dropped {dropped}");
    assert!(
        subject.starts_with(&prefix) && subject.ends_with(&suffix),
        "subject {subject:?} does not match {prefix}<turn>{suffix}"
    );
    let middle = &subject[prefix.len()..subject.len() - suffix.len()];
    assert!(
        !middle.is_empty() && middle.chars().all(|c| c.is_ascii_digit()),
        "turn segment {middle:?} in subject {subject:?} is not all digits"
    );
}

fn poll<F: FnMut() -> bool>(deadline: Instant, mut check: F) -> bool {
    loop {
        if check() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn assert_docker_and_hand_roots_clean() {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut listed = String::new();
    let clean = poll(deadline, || {
        let output = StdCommand::new("docker")
            .args(["ps", "-a", "--filter", "label=arx.hand", "--format", "{{.Names}}"])
            .output()
            .unwrap();
        listed = String::from_utf8_lossy(&output.stdout).trim().to_string();
        listed.is_empty()
    });
    assert!(clean, "containers still listed after the run: {listed}");

    let arx_hands = std::env::temp_dir().join("arx-hands");
    let empty = poll(Instant::now() + Duration::from_secs(2), || {
        fs::read_dir(&arx_hands).map(|entries| entries.count() == 0).unwrap_or(true)
    });
    assert!(empty, "stray entries remain under /tmp/arx-hands after the run");
}

static L1_CLOSE_ONCE: Once = Once::new();
static L1_CLOSE_FAILURE: Mutex<Option<String>> = Mutex::new(None);

fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&str>() {
        message.to_string()
    } else {
        "L1-close script panicked with a non-string payload".to_string()
    }
}

fn run_l1_close_script() {
    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/l1-close.json");
    let local_bytes = fs::read(&script_path).unwrap();
    let show = StdCommand::new("git")
        .args(["-C", env!("CARGO_MANIFEST_DIR"), "show", "main:crates/arx/tests/scripts/l1-close.json"])
        .output()
        .unwrap();
    assert!(show.status.success(), "git show main:l1-close.json failed: {}", String::from_utf8_lossy(&show.stderr));
    assert_eq!(local_bytes, show.stdout, "crates/arx/tests/scripts/l1-close.json differs from main's copy");

    let (base_url, _state) = fake_openai_server(FakeScript::Sse(vec![
        r#"{"choices":[{"delta":{"role":"assistant"}}]}"#.to_string(),
        r#"{"choices":[{"delta":{"content":"native says skylark"}}]}"#.to_string(),
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string(),
        r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":5,"total_tokens":17}}"#.to_string(),
    ]));

    let workspace = native_run_fixture(&base_url);
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");

    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[store]\nscope = \"testscope\"\n");
    fs::write(&config_path, config).unwrap();

    plant_engine_scope(&run_dir.join("store"), "testscope");

    let codex_home = plant_stub_codex_credential(workspace.path());
    ensure_codex_stub_image();
    ensure_claude_stub_image();
    let claude_config = plant_stub_credential(workspace.path());

    let mut script = load_script(&script_path);
    migrate_retired_judge_dispatch(&mut script);

    let env = [
        ("PATH", claude_hand_path()),
        ("CODEX_HOME", codex_home.to_str().unwrap().to_string()),
        ("CLAUDE_CONFIG_DIR", claude_config.to_str().unwrap().to_string()),
        ("ARX_TEST_FAKE_KEY", "test-key".to_string()),
    ];

    let rows = run_session(&run_dir, "arx.toml", &script.operator, &env);

    if let Err(failures) = check_script(&rows, &script) {
        panic!("{}", failures.join("\n"));
    }
}

fn ensure_l1_close_passes_first() {
    L1_CLOSE_ONCE.call_once(|| {
        if let Err(payload) = std::panic::catch_unwind(run_l1_close_script) {
            *L1_CLOSE_FAILURE.lock().unwrap() = Some(panic_payload_to_string(payload));
        }
    });
    if let Some(message) = L1_CLOSE_FAILURE.lock().unwrap().clone() {
        panic!("L1-close script prerequisite failed: {message}");
    }
}

#[test]
fn l1_close_script_passes_unchanged_under_layer_two() {
    ensure_l1_close_passes_first();
}

#[test]
fn l2_close_loop_in_containers() {
    ensure_l1_close_passes_first();

    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");

    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[store]\nscope = \"testscope\"\n");
    fs::write(&config_path, &config).unwrap();

    plant_engine_scope(&run_dir.join("store"), "testscope");

    with_main_checkpoint(&config_path, 1_000_000);

    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[gate]\ndeny = [\"mcp__arx__decisions_query\", \"read_file(secret*)\"]\nallow = [\"mcp__arx__list_files\"]\n");
    fs::write(&config_path, &config).unwrap();

    fs::write(run_dir.join("secret.txt"), "top secret").unwrap();
    fs::write(
        run_dir.join("checkpoint-author.md"),
        "## Position\nnothing new since the last object.\n## Target\nland the layer-two close loop.\n## Drift\nnone observed.\n## Hands in flight\n- h4 fake\n",
    )
    .unwrap();
    fs::write(run_dir.join("checkpoint-truth.md"), "ok").unwrap();

    let (base_url, _state) = fake_openai_server(FakeScript::Sequence(vec![
        FakeScript::Sse(vec![
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read_file","arguments":"{\"path\": \"secret.txt\"}"}}]}}]}"#.to_string(),
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#.to_string(),
        ]),
        FakeScript::Sse(vec![
            r#"{"choices":[{"delta":{"content":"read_file on secret.txt was denied: proceed"}}]}"#.to_string(),
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string(),
        ]),
    ]));
    let mut config = fs::read_to_string(&config_path).unwrap();
    config = config.replace("http://127.0.0.1:0", &base_url);
    fs::write(&config_path, &config).unwrap();

    ensure_claude_stub_image();
    ensure_codex_stub_image();
    let claude_config = plant_stub_credential(workspace.path());
    let codex_home = plant_stub_codex_credential(workspace.path());

    let store_root = run_dir.join("store");
    let before_head = head_count(&store_root);

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/l2-close.json");
    let mut script = load_script(&script_path);
    migrate_retired_judge_dispatch(&mut script);

    let env = [
        ("PATH", claude_hand_path()),
        ("CODEX_HOME", codex_home.to_str().unwrap().to_string()),
        ("CLAUDE_CONFIG_DIR", claude_config.to_str().unwrap().to_string()),
        ("ARX_TEST_FAKE_KEY", "test-key".to_string()),
        ("ARX_CANARY_L2CLOSE", "host-leak".to_string()),
    ];

    let rows = run_session_with_deadline(&run_dir, "arx.toml", &script.operator, &env, Duration::from_secs(30));

    if let Err(failures) = check_script(&rows, &script) {
        panic!("{}", failures.join("\n"));
    }

    let threads_dir = run_dir.join("threads");
    let leak_checks: [(&str, &str); 8] = [
        ("stub-token", "claude stub token"),
        ("accessToken", "claude accessToken field"),
        ("claudeAiOauth", "claude oauth field"),
        ("refresh_token", "codex refresh_token field"),
        ("access_token", "codex access_token field"),
        ("id_token", "codex id_token field"),
        ("host-leak", "host env canary"),
        ("test-key", "the fake provider key"),
    ];
    for (needle, label) in leak_checks {
        let grep = StdCommand::new("grep").args(["-rl", needle, threads_dir.to_str().unwrap()]).output().unwrap();
        match grep.status.code() {
            Some(1) => {}
            Some(0) => {
                let matched_files = String::from_utf8_lossy(&grep.stdout).trim().to_string();
                panic!("{label} leaked under threads/ in: {matched_files}");
            }
            other => panic!("{label} leak scan over threads/ did not complete cleanly (grep exit {other:?})"),
        }
    }

    assert_eq!(head_count(&store_root), before_head, "save must not commit during the main session");

    let quarantine_dir = store_root.join("quarantine/main");
    let entries: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(entries.len(), 1, "quarantine dir entries: {entries:?}");
    let candidate_path = entries.into_iter().next().unwrap().unwrap().path();
    let candidate: serde_json::Value = serde_json::from_str(&fs::read_to_string(&candidate_path).unwrap()).unwrap();
    assert_eq!(candidate["question"], "what closes layer two");
    assert_eq!(candidate["answer"], "the containers");
    assert_eq!(candidate["handle"]["thread"], "main");
    let saved_turn = candidate["handle"]["turn"].as_i64().expect("handle.turn is an integer");
    assert!(saved_turn >= 1, "handle.turn: {saved_turn}");

    let checkpoint_text = fs::read_to_string(run_dir.join("threads/main.checkpoints/1.md")).unwrap();
    let hands_block = checkpoint_text.split("## Hands in flight").nth(1).expect("Hands in flight block present");
    assert!(hands_block.contains("- h4 fake"), "checkpoint file's Hands in flight block did not name h4 fake");

    let ledger = ledger_rows(&run_dir);
    let spawn_rows: Vec<&serde_json::Value> = ledger.iter().filter(|row| row["kind"] == "spawn").collect();
    let expected_agents = [
        ("h1", "hand-cc", "hand"),
        ("h2", "hand-cx", "hand"),
        ("h3", "hand-ds", "hand"),
        ("h4", "fake", "hand"),
        ("h5", "reviewer", "reviewer"),
    ];
    assert_eq!(spawn_rows.len(), 5, "expected five spawn rows in the ledger");
    for (row, (hand, agent, role)) in spawn_rows.iter().zip(expected_agents) {
        assert_eq!(row["hand"], hand, "spawn row hand mismatch");
        assert_eq!(row["agent"], agent, "spawn row agent mismatch for hand {hand}");
        assert_eq!(row["role"], role, "spawn row role mismatch for hand {hand}");
    }

    let h1_session = fs::read_to_string(run_dir.join("threads/main.h1/session.jsonl")).unwrap();
    let h1_lines: Vec<&str> = h1_session.lines().collect();
    assert!(
        h1_lines.get(1) == Some(&r#"{"type":"cred","present":true}"#),
        "h1 session.jsonl line 2 did not match the expected credential-present marker"
    );
    assert!(
        h1_lines.last() == Some(&r#"{"type":"env","canary":false}"#),
        "h1 session.jsonl's last line did not match the expected env-canary marker"
    );

    let h2_session = fs::read_to_string(run_dir.join("threads/main.h2/session.jsonl")).unwrap();
    let h2_lines: Vec<&str> = h2_session.lines().collect();
    assert!(
        h2_lines.get(1) == Some(&r#"{"stub":"auth","present":true,"mode":"600"}"#),
        "h2 session.jsonl line 2 did not match the expected auth-present marker"
    );
    assert!(
        h2_lines.last() == Some(&r#"{"stub":"env","canary":false}"#),
        "h2 session.jsonl's last line did not match the expected env-canary marker"
    );

    let h5_session = fs::read_to_string(run_dir.join("threads/main.h5/session.jsonl")).unwrap();
    let h5_lines: Vec<&str> = h5_session.lines().collect();
    assert!(
        h5_lines.last() == Some(&r#"{"stub":"env","canary":false}"#),
        "h5 session.jsonl's last line did not match the expected env-canary marker"
    );

    let reviewer_text = arx_text(&rows, "[hand h5 verdict]");
    let json_part = reviewer_text
        .strip_prefix("[hand h5 verdict] ")
        .and_then(|rest| rest.rsplit_once(" (trace:"))
        .map(|(json_part, _)| json_part)
        .expect("h5 window text carries a JSON body before the trace suffix");
    let verdict_value: serde_json::Value = serde_json::from_str(json_part).expect("h5 window text is valid JSON");
    assert_eq!(verdict_value["verdict"], "PASS", "h5 verdict JSON: {verdict_value}");
    let schema_row = rows.iter().find(|row| row["kind"] == "schema" && row["hand"] == "h5").expect("schema row for h5");
    assert_eq!(schema_row["outcome"], "ok", "schema row: {schema_row:?}");

    let h3_text = arx_text(&rows, "[hand h3 verdict]");
    assert!(h3_text.contains("denied"), "h3 verdict: {h3_text}");

    let events_output = StdCommand::new(cargo_bin("arx")).args(["events", "main"]).current_dir(&run_dir).output().unwrap();
    assert!(events_output.status.success(), "arx events main failed: {}", String::from_utf8_lossy(&events_output.stderr));
    let events_text = String::from_utf8_lossy(&events_output.stdout).into_owned();

    let gate_row_count = rows.iter().filter(|row| row["kind"] == "gate").count();
    assert!(events_text.contains(&format!("## gate ({gate_row_count})")), "events output: {events_text}");

    let mut last_index: isize = -1;
    for kind in ["gate", "spawn", "delivered", "checkpoint", "schema"] {
        let heading = format!("## {kind} (");
        let index = events_text.find(&heading).unwrap_or_else(|| panic!("missing heading {heading} in: {events_text}"));
        assert!(index as isize > last_index, "heading {heading} out of KINDS order in: {events_text}");
        last_index = index as isize;
    }

    assert_docker_and_hand_roots_clean();

    let quarantine_id = quarantine_id_by_question(&quarantine_dir, "what closes layer two");
    let before_drain_head = head_count(&store_root);

    let mut drain_session = LiveSession::spawn(&run_dir, "arx.toml", &env);
    drain_session.drain_output();
    drain_session.write_line("store:drain:{}");

    let spawn_deadline = Instant::now() + Duration::from_secs(60);
    let spawn_row = wait_for_row(&mut drain_session, &run_dir, spawn_deadline, |row| row["kind"] == "spawn" && row["role"] == "drain")
        .expect("drain hand spawned within 60s");
    let hand = spawn_row["hand"].as_str().unwrap().to_string();

    let trigger_deadline = Instant::now() + Duration::from_secs(60);
    wait_for_row(&mut drain_session, &run_dir, trigger_deadline, |row| row["kind"] == "delivered" && row["hand"] == hand)
        .expect("drain hand delivered within 60s");

    drain_session.write_line(&format!("store:drain:{}", serde_json::json!({"proposal": hand})));
    drain_session.close_stdin();
    let exit_deadline = Instant::now() + Duration::from_secs(30);
    assert!(drain_session.wait_success(exit_deadline), "arx run did not exit successfully");

    assert_eq!(head_count(&store_root), before_drain_head + 1, "expected exactly one new commit from the drain");
    let subject = commit_subject(&store_root);
    assert_drain_subject(&subject, "main", 1, 0);

    let remaining: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert!(remaining.is_empty(), "quarantine not empty after drain: {remaining:?}");

    let promoted_path = store_root.join("testscope").join(format!("{quarantine_id}.md"));
    assert!(promoted_path.exists(), "promoted claim missing under the store: {}", promoted_path.display());

    assert_docker_and_hand_roots_clean();
}
