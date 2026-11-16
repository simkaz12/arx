use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tempfile::tempdir;

use arx_engine::{ClaimHandle, ClaimKind, Engine, SaveIntent, Store};

mod common;
use common::*;

static DOCKER_STATE_SERIAL: Mutex<()> = Mutex::new(());

fn serialize_on_docker_state() -> std::sync::MutexGuard<'static, ()> {
    DOCKER_STATE_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

const DERIVABLE_ANSWER: &str = "the dispatcher retries three times before giving up";

fn store_workspace() -> (tempfile::TempDir, PlantedScope) {
    let workspace = tempdir().unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    copy_dir_recursive(&fixtures.join("run/agents"), &workspace.path().join("agents"));

    fs::write(workspace.path().join("arx.toml"), run_fixture_toml(&stub_dir().display().to_string(), "testscope", None)).unwrap();

    fs::write(workspace.path().join("README.md"), format!("{DERIVABLE_ANSWER}\n")).unwrap();

    let git = |args: &[&str]| {
        let status = Command::new("git").arg("-C").arg(workspace.path()).args(args).status().unwrap();
        assert!(status.success(), "git {args:?} failed in {}", workspace.path().display());
    };
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["-c", "user.name=arx", "-c", "user.email=arx@localhost", "commit", "-q", "-m", "init"]);

    let planted = plant_engine_scope(&workspace.path().join("store"), "testscope");
    (workspace, planted)
}

fn build_workspace_with_root(root: &Path) -> tempfile::TempDir {
    let workspace = tempdir().unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    copy_dir_recursive(&fixtures.join("run/agents"), &workspace.path().join("agents"));

    fs::write(workspace.path().join("arx.toml"), run_fixture_toml(&stub_dir().display().to_string(), "testscope", Some(root))).unwrap();

    fs::write(workspace.path().join("README.md"), format!("{DERIVABLE_ANSWER}\n")).unwrap();

    let git = |args: &[&str]| {
        let status = Command::new("git").arg("-C").arg(workspace.path()).args(args).status().unwrap();
        assert!(status.success(), "git {args:?} failed in {}", workspace.path().display());
    };
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["-c", "user.name=arx", "-c", "user.email=arx@localhost", "commit", "-q", "-m", "init"]);

    workspace
}

fn store_workspace_with_root(root: &Path) -> (tempfile::TempDir, PlantedScope) {
    let workspace = build_workspace_with_root(root);
    let planted = plant_engine_scope(root, "testscope");
    (workspace, planted)
}

#[test]
fn a_root_outside_the_workspace_is_the_store_the_session_uses() {
    let root = tempdir().unwrap();
    let (workspace, _planted) = store_workspace_with_root(root.path());
    let run_dir = workspace.path().to_path_buf();
    let before_head = head_count(root.path());

    let save_args = serde_json::json!({
        "scope": "testscope",
        "kind": "fact",
        "question": "what's the codename for the launch",
        "answer": "not confirmed yet",
        "why": "checking before we commit to a name"
    });
    let line = format!("store:save:{save_args}");
    let rows = run_session(&run_dir, "arx.toml", &[line], &[]);

    let system_row = rows.iter().find(|row| row["kind"] == "system").unwrap();
    let text = system_row["text"].as_str().unwrap();
    assert!(text.contains("skylark"), "system row: {text}");

    let quarantine_dir = root.path().join("quarantine/main");
    let entries: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(entries.len(), 1);

    assert!(!workspace.path().join("store").exists(), "workspace must not gain a store/ directory");

    assert_eq!(head_count(root.path()), before_head, "save must not commit");
}

#[test]
fn an_unregistered_scope_is_refused_naming_the_directory_to_create() {
    let root = tempdir().unwrap();
    let workspace = build_workspace_with_root(root.path());
    let run_dir = workspace.path().to_path_buf();

    let (code, stderr) = run_session_expect_failure(&run_dir, "arx.toml", &[], &[]);
    assert_ne!(code, 0, "expected a non-zero exit, stderr: {stderr}");
    let expected = format!(
        "scope 'testscope' has no directory under the store root {root}; mkdir {root}/testscope to register it",
        root = root.path().display()
    );
    assert!(stderr.contains(&expected), "stderr: {stderr}");
    assert!(root.path().join(".git").exists(), "expected the store root to be git-initialized already");
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

fn commit_subject(root: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("log")
        .arg("-1")
        .arg("--format=%s")
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn grounding_is_hot_and_ember_only() {
    let (workspace, _planted) = store_workspace();
    let run_dir = workspace.path().to_path_buf();

    let rows = run_session(&run_dir, "arx.toml", &["hello".to_string()], &[]);

    let system_row = rows.iter().find(|row| row["kind"] == "system").unwrap();
    let text = system_row["text"].as_str().unwrap();

    assert!(text.contains("what is the launch codename"), "system row: {text}");
    assert!(text.contains("keep replies short"), "system row: {text}");
    assert!(!text.contains("orion desk"), "system row: {text}");
}

#[test]
fn save_returns_landscape_and_commits_nothing() {
    let (workspace, planted) = store_workspace();
    let run_dir = workspace.path().to_path_buf();
    let store_root = run_dir.join("store");
    let before_head = head_count(&store_root);

    let save_args = serde_json::json!({
        "scope": "testscope",
        "kind": "fact",
        "question": "what's the codename for the launch",
        "answer": "not confirmed yet",
        "why": "checking before we commit to a name"
    });
    let line = format!("store:save:{save_args}");
    let (rows, _stdout) = run_session_with_stdout(&run_dir, "arx.toml", &[line], &[]);

    let main_row = rows.iter().find(|row| row["kind"] == "turn" && row["role"] == "main").unwrap();
    let text = main_row["text"].as_str().unwrap();
    assert!(text.contains("stub-store:"), "main turn text: {text}");
    let echoed = text.split("stub-store:").nth(1).unwrap();
    let claims: Vec<serde_json::Value> = serde_json::from_str(echoed).unwrap();
    assert!(!claims.is_empty(), "expected a non-empty landscape: {echoed}");
    assert!(
        claims.iter().any(|claim| claim["id"] == planted.hot_id),
        "expected the hot claim {} among {echoed}",
        planted.hot_id
    );

    let quarantine_dir = store_root.join("quarantine/main");
    let entries: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(entries.len(), 1);
    let candidate_path = entries.into_iter().next().unwrap().unwrap().path();
    let candidate: serde_json::Value = serde_json::from_str(&fs::read_to_string(&candidate_path).unwrap()).unwrap();
    assert_eq!(candidate["handle"]["thread"], "main");
    let turn = candidate["handle"]["turn"].as_i64().expect("handle.turn is an integer");
    assert!(turn >= 1, "handle.turn: {turn}");

    assert_eq!(head_count(&store_root), before_head, "save must not commit");
}

fn seed_two_drain_candidates(store_root: &Path) -> (String, String) {
    let mut engine = Engine::open(store_root, "main").unwrap();
    let handle = ClaimHandle {
        thread: "main".to_string(),
        turn: 1,
    };
    engine
        .save(
            &SaveIntent {
                scope: "testscope".to_string(),
                kind: ClaimKind::Fact,
                question: "how many retries does the dispatcher attempt".to_string(),
                answer: DERIVABLE_ANSWER.to_string(),
                why: "seeded for the drain test".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();
    engine
        .save(
            &SaveIntent {
                scope: "testscope".to_string(),
                kind: ClaimKind::Fact,
                question: "what queue backs the dispatcher".to_string(),
                answer: "redis".to_string(),
                why: "seeded for the drain test".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();
    drop(engine);

    let quarantine_dir = store_root.join("quarantine/main");
    let derivable_id = quarantine_id_by_question(&quarantine_dir, "how many retries does the dispatcher attempt");
    let other_id = quarantine_id_by_question(&quarantine_dir, "what queue backs the dispatcher");
    (derivable_id, other_id)
}

#[test]
fn drain_apply_with_an_override_is_one_commit_with_the_handle() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let (workspace, _planted) = store_workspace();
    let run_dir = workspace.path().to_path_buf();
    let store_root = run_dir.join("store");
    let quarantine_dir = store_root.join("quarantine/main");
    let codex_home = plant_stub_codex_credential(workspace.path());
    let env = [("CODEX_HOME".to_string(), codex_home.display().to_string())];
    let env_refs: Vec<(&str, String)> = env.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();

    let (derivable_id, other_id) = seed_two_drain_candidates(&store_root);

    let before_head = head_count(&store_root);
    let before_quarantine: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(before_quarantine.len(), 2);

    let mut session = LiveSession::spawn(&run_dir, "arx.toml", &env_refs);
    session.drain_output();
    session.write_line("store:drain:{}");

    let spawn_deadline = Instant::now() + Duration::from_secs(60);
    let spawn_row = wait_for_row(&mut session, &run_dir, spawn_deadline, |row| row["kind"] == "spawn" && row["role"] == "drain")
        .expect("drain hand spawned within 60s");
    let hand = spawn_row["hand"].as_str().unwrap().to_string();

    let deadline = Instant::now() + Duration::from_secs(60);
    wait_for_row(&mut session, &run_dir, deadline, |row| row["kind"] == "delivered" && row["hand"] == hand)
        .expect("drain hand delivered within 60s");

    assert_eq!(head_count(&store_root), before_head, "the trigger must not commit");
    let after_trigger_quarantine: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(after_trigger_quarantine.len(), 2, "the trigger must not touch the quarantine");

    let rows = transcript_rows(&run_dir);
    let input_path = run_dir.join(format!("threads/main.{hand}/out/drain-input.json"));
    let input_text = fs::read_to_string(&input_path).unwrap();
    let input_value: serde_json::Value = serde_json::from_str(&input_text).unwrap();
    let candidates = input_value["candidates"].as_array().expect("candidates array");
    assert_eq!(candidates.len(), 2, "drain-input.json candidates: {input_text}");
    for candidate in candidates {
        assert!(candidate["landscape"].is_array(), "candidate missing landscape array: {candidate}");
    }

    let schema_row = rows
        .iter()
        .find(|row| row["kind"] == "schema" && row["hand"] == hand)
        .unwrap_or_else(|| panic!("no schema row for {hand}: {rows:?}"));
    assert_eq!(schema_row["outcome"], "ok", "schema row: {schema_row:?}");

    let proposal_needle = format!("[hand {hand} drain proposal] 2 candidates");
    assert!(
        rows.iter().any(|row| {
            row["kind"] == "turn" && row["role"] == "arx" && row["text"].as_str().map(|t| t.starts_with(&proposal_needle)).unwrap_or(false)
        }),
        "no proposal line starting with {proposal_needle:?} in: {rows:?}"
    );

    let apply_args = serde_json::json!({
        "proposal": hand,
        "overrides": [{"id": derivable_id, "action": "drop", "reason": "README says it"}]
    });
    session.write_line(&format!("store:drain:{apply_args}"));
    session.close_stdin();

    let exit_deadline = Instant::now() + Duration::from_secs(30);
    assert!(session.wait_success(exit_deadline), "arx run did not exit successfully");

    assert_eq!(head_count(&store_root), before_head + 1, "expected exactly one new commit");
    let subject = commit_subject(&store_root);
    assert_drain_subject(&subject, "main", 1, 1);

    let remaining: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert!(remaining.is_empty(), "expected the quarantine to be empty after drain");

    let promoted_path = store_root.join("testscope").join(format!("{other_id}.md"));
    assert!(promoted_path.exists(), "promoted claim missing under the store: {}", promoted_path.display());
    let dropped_path = store_root.join("testscope").join(format!("{derivable_id}.md"));
    assert!(!dropped_path.exists(), "dropped candidate should not have been promoted");

    let final_rows = transcript_rows(&run_dir);
    let drain_row = final_rows
        .iter()
        .find(|row| row["kind"] == "drain")
        .unwrap_or_else(|| panic!("no drain event row: {final_rows:?}"));
    assert_eq!(drain_row["hand"], hand);
    assert_eq!(drain_row["promoted"], 1);
    assert_eq!(drain_row["dropped"], 1);
    assert_eq!(drain_row["overrides"], 1);
}

#[test]
fn drain_without_a_drain_agent_refuses() {
    let _guard = serialize_on_docker_state();
    let (workspace, _planted) = store_workspace();
    let run_dir = workspace.path().to_path_buf();
    let store_root = run_dir.join("store");
    fs::remove_file(workspace.path().join("agents/drain.md")).unwrap();
    let config_path = workspace.path().join("arx.toml");
    let contents = fs::read_to_string(&config_path).unwrap();
    fs::write(&config_path, contents.replace("drain = \"gpt-5.6-sol\"\n", "")).unwrap();

    let before_head = head_count(&store_root);

    let (rows, _) = run_session_with_stdout(&run_dir, "arx.toml", &["store:drain:{}".to_string()], &[]);
    let main_row = rows.iter().filter(|row| row["kind"] == "turn" && row["role"] == "main").last().unwrap();
    let text = main_row["text"].as_str().unwrap();
    assert!(text.contains("drain: no drain agent — add agents/drain.md"), "main turn text: {text}");
    assert!(!rows.iter().any(|row| row["kind"] == "spawn"), "no spawn expected: {rows:?}");
    assert_eq!(head_count(&store_root), before_head, "commit count must not move");
}

#[test]
fn drain_trigger_on_an_empty_quarantine_refuses() {
    let _guard = serialize_on_docker_state();
    let (workspace, _planted) = store_workspace();
    let run_dir = workspace.path().to_path_buf();

    let (rows, _) = run_session_with_stdout(&run_dir, "arx.toml", &["store:drain:{}".to_string()], &[]);
    let main_row = rows.iter().filter(|row| row["kind"] == "turn" && row["role"] == "main").last().unwrap();
    let text = main_row["text"].as_str().unwrap();
    assert!(text.contains("drain: quarantine is empty"), "main turn text: {text}");
    assert!(!rows.iter().any(|row| row["kind"] == "spawn"), "no spawn expected: {rows:?}");
}

#[test]
fn drain_apply_without_a_proposal_is_refused() {
    let _guard = serialize_on_docker_state();
    let (workspace, _planted) = store_workspace();
    let run_dir = workspace.path().to_path_buf();
    let store_root = run_dir.join("store");
    let before_head = head_count(&store_root);

    let overrides_args = serde_json::json!({"overrides": [{"id": "whatever", "action": "drop", "reason": "no proposal"}]});
    let (rows_a, _) = run_session_with_stdout(&run_dir, "arx.toml", &[format!("store:drain:{overrides_args}")], &[]);
    let text_a = rows_a
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .last()
        .unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text_a.contains("drain: overrides need a proposal"), "main turn text: {text_a}");

    let unknown_proposal_args = serde_json::json!({"proposal": "h9"});
    let (rows_b, _) = run_session_with_stdout(&run_dir, "arx.toml", &[format!("store:drain:{unknown_proposal_args}")], &[]);
    let text_b = rows_b
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "main")
        .last()
        .unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        text_b.contains("drain: no proposal from h9 in this session — run drain again"),
        "main turn text: {text_b}"
    );

    assert_eq!(head_count(&store_root), before_head, "nothing should have been written");
}

#[test]
fn drain_malformed_proposal_is_marked_and_cannot_be_applied() {
    let _guard = serialize_on_docker_state();
    ensure_codex_stub_image();
    let (workspace, _planted) = store_workspace();
    let run_dir = workspace.path().to_path_buf();
    let store_root = run_dir.join("store");
    let codex_home = plant_stub_codex_credential(workspace.path());
    let env = [("CODEX_HOME".to_string(), codex_home.display().to_string())];
    let env_refs: Vec<(&str, String)> = env.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();

    seed_two_drain_candidates(&store_root);

    let mut session = LiveSession::spawn(&run_dir, "arx.toml", &env_refs);
    session.drain_output();
    session.write_line(&format!(
        "store:drain:{}",
        serde_json::json!({"note": "probe-malformed:"})
    ));

    let spawn_deadline = Instant::now() + Duration::from_secs(60);
    let spawn_row = wait_for_row(&mut session, &run_dir, spawn_deadline, |row| row["kind"] == "spawn" && row["role"] == "drain")
        .expect("drain hand spawned within 60s");
    let hand = spawn_row["hand"].as_str().unwrap().to_string();

    let deadline = Instant::now() + Duration::from_secs(60);
    let schema_row = wait_for_row(&mut session, &run_dir, deadline, |row| row["kind"] == "schema" && row["hand"] == hand)
        .expect("schema row within 60s");
    assert_eq!(schema_row["outcome"], "malformed", "schema row: {schema_row:?}");

    let rows = transcript_rows(&run_dir);
    let malformed_needle = format!("[hand {hand} drain proposal malformed]");
    assert!(
        rows.iter().any(|row| {
            row["kind"] == "turn" && row["role"] == "arx" && row["text"].as_str().map(|t| t.starts_with(&malformed_needle)).unwrap_or(false)
        }),
        "no malformed proposal line starting with {malformed_needle:?} in: {rows:?}"
    );

    session.write_line(&format!("store:drain:{}", serde_json::json!({"proposal": hand})));
    session.close_stdin();
    let exit_deadline = Instant::now() + Duration::from_secs(30);
    assert!(session.wait_success(exit_deadline), "arx run did not exit successfully");

    let final_rows = transcript_rows(&run_dir);
    let main_row = final_rows.iter().filter(|row| row["kind"] == "turn" && row["role"] == "main").last().unwrap();
    let text = main_row["text"].as_str().unwrap();
    assert!(
        text.contains(&format!("drain: no proposal from {hand} in this session — run drain again")),
        "main turn text: {text}"
    );
}
