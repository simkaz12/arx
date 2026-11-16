use std::fs;
use std::path::Path;

use arx_engine::{ClaimHandle, ClaimKind, Engine, SaveIntent, Store};

mod common;
use common::*;

fn setup_testscope(run_dir: &Path) -> String {
    let config_path = run_dir.join("arx.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[store]\nscope = \"testscope\"\n");
    fs::write(&config_path, config).unwrap();
    let store_root = run_dir.join("store");
    plant_engine_scope(&store_root, "testscope");
    plant_decision(&store_root, "testscope")
}

fn plant_decision(store_root: &Path, scope: &str) -> String {
    let mut engine = Engine::open(store_root, "seed").unwrap();
    let handle = ClaimHandle {
        thread: "seed".to_string(),
        turn: 1,
    };
    engine
        .save(
            &SaveIntent {
                scope: scope.to_string(),
                kind: ClaimKind::Decision,
                question: "what does the checkpoint ritual cite".to_string(),
                answer: "this fixture decision".to_string(),
                why: "checkpoint test fixture".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();
    engine.drain().unwrap();
    engine
        .decisions_query(scope, "checkpoint ritual cite", 1)
        .unwrap()
        .into_iter()
        .next()
        .expect("planted decision resolves via decisions_query")
        .id
}

fn checkpoint_rows(rows: &[serde_json::Value]) -> Vec<&serde_json::Value> {
    rows.iter().filter(|row| row["kind"] == "checkpoint").collect()
}

fn arx_prompt_rows(rows: &[serde_json::Value]) -> Vec<&serde_json::Value> {
    rows.iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "arx")
        .collect()
}

fn simple_body(file_entry: &str, decision_id: Option<&str>) -> String {
    let decision_line = match decision_id {
        Some(id) => format!("- decision: {id}\n"),
        None => String::new(),
    };
    format!(
        "## Position\nnothing new since the last object.\n## Target\nland the checkpoint ritual.\n## Drift\nnone observed.\n## Hands in flight\n- none\n{file_entry}\n{decision_line}"
    )
}

#[test]
fn two_crossings_produce_two_objects_that_pass() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_checkpoint(&config_path, 1000);
    let decision_id = setup_testscope(&run_dir);

    let hash = hash_of(&run_dir.join("arx.toml"));
    let author = simple_body(&format!("- file: arx.toml @ {hash}"), Some(&decision_id));
    plant_checkpoint_replies(&run_dir, &author, None, Some("ok"));

    let env = [("ARX_STUB_TOKENS_PER_TURN", "600".to_string())];
    let lines = vec![
        "first".to_string(),
        "second".to_string(),
        "third".to_string(),
        "fourth".to_string(),
        "fifth".to_string(),
    ];
    let rows = run_session(&run_dir, "arx.toml", &lines, &env);

    let one = run_dir.join("threads/main.checkpoints/1.md");
    let two = run_dir.join("threads/main.checkpoints/2.md");
    assert!(one.exists(), "1.md missing; rows: {rows:?}");
    assert!(two.exists(), "2.md missing; rows: {rows:?}");

    let obj_one = state_object_parse(&fs::read_to_string(&one).unwrap());
    let obj_two = state_object_parse(&fs::read_to_string(&two).unwrap());
    assert_eq!(obj_one.0, 1, "header: {:?}", obj_one);
    assert_eq!(obj_two.0, 2, "header: {:?}", obj_two);

    let events = checkpoint_rows(&rows);
    assert_eq!(events.len(), 2, "rows: {rows:?}");
    assert!(events.iter().all(|row| row["check"] == "pass"), "rows: {rows:?}");

    let threads_dir = run_dir.join("threads");
    for (turn, _) in [obj_one, obj_two] {
        let resolved = transcript_turn_text(&threads_dir, turn);
        assert_eq!(resolved, author, "resolved turn {turn} did not match the authoring reply");
    }
}

#[test]
fn a_false_position_is_replaced_by_the_truth_check_reply() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let _decision_id = setup_testscope(&run_dir);

    let author = simple_body("- file: missing.md @ 0123456789abcdef0123456789abcdef01234567", None);
    let hash = hash_of(&run_dir.join("arx.toml"));
    let corrected = simple_body(&format!("- file: arx.toml @ {hash}"), None);
    plant_checkpoint_replies(&run_dir, &author, None, Some(&corrected));

    let rows = run_session(&run_dir, "arx.toml", &["/checkpoint".to_string()], &[]);

    let written = fs::read_to_string(run_dir.join("threads/main.checkpoints/1.md")).unwrap();
    assert!(written.contains("arx.toml"), "written: {written}");
    assert!(!written.contains("missing.md"), "written: {written}");

    let events = checkpoint_rows(&rows);
    assert_eq!(events.len(), 1, "rows: {rows:?}");
    assert_eq!(events[0]["check"], "pass", "rows: {rows:?}");
}

#[test]
fn a_failing_check_gets_one_more_round_then_passes() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let decision_id = setup_testscope(&run_dir);

    let wrong_hash = "1111111111111111111111111111111111111111";
    let author = simple_body(&format!("- file: arx.toml @ {wrong_hash}"), Some(&decision_id));
    let true_hash = hash_of(&run_dir.join("arx.toml"));
    let author2 = simple_body(&format!("- file: arx.toml @ {true_hash}"), Some(&decision_id));
    plant_checkpoint_replies(&run_dir, &author, Some(&author2), Some("ok"));

    let rows = run_session(&run_dir, "arx.toml", &["/checkpoint".to_string()], &[]);

    let prompts = arx_prompt_rows(&rows);
    let authoring_prompts: Vec<&&serde_json::Value> = prompts
        .iter()
        .filter(|row| row["text"].as_str().unwrap_or("").contains("author the state object"))
        .collect();
    assert_eq!(authoring_prompts.len(), 2, "rows: {rows:?}");
    let second_prompt = authoring_prompts[1]["text"].as_str().unwrap();
    assert!(second_prompt.contains("FileHash"), "second prompt: {second_prompt}");
    assert!(second_prompt.contains("arx.toml"), "second prompt: {second_prompt}");

    let events = checkpoint_rows(&rows);
    assert_eq!(events.len(), 2, "rows: {rows:?}");
    assert_eq!(events[0]["check"], "fail", "rows: {rows:?}");
    assert_eq!(events[1]["check"], "pass", "rows: {rows:?}");

    let written = fs::read_to_string(run_dir.join("threads/main.checkpoints/1.md")).unwrap();
    assert!(written.contains(&true_hash), "written: {written}");
}

#[test]
fn two_failed_rounds_abandon_the_checkpoint() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let _decision_id = setup_testscope(&run_dir);

    let author = simple_body("- file: arx.toml @ 1111111111111111111111111111111111111111", None);
    let author2 = simple_body("- file: arx.toml @ 2222222222222222222222222222222222222222", None);
    plant_checkpoint_replies(&run_dir, &author, Some(&author2), Some("ok"));

    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &["/checkpoint".to_string()], &[]);

    let events = checkpoint_rows(&rows);
    assert_eq!(events.len(), 2, "rows: {rows:?}");
    assert!(events.iter().all(|row| row["check"] == "fail"), "rows: {rows:?}");

    assert!(!run_dir.join("threads/main.checkpoints").exists(), "no checkpoint file should exist");
    assert!(stdout.contains("abandoned after two rounds"), "stdout: {stdout}");
}

#[test]
fn the_operator_command_fires_without_a_crossing() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let decision_id = setup_testscope(&run_dir);

    let hash = hash_of(&run_dir.join("arx.toml"));
    let author = simple_body(&format!("- file: arx.toml @ {hash}"), Some(&decision_id));
    plant_checkpoint_replies(&run_dir, &author, None, Some("ok"));

    let rows = run_session(&run_dir, "arx.toml", &["/checkpoint".to_string()], &[]);

    assert!(run_dir.join("threads/main.checkpoints/1.md").exists(), "rows: {rows:?}");
    let operator_checkpoint_turns = rows
        .iter()
        .filter(|row| row["kind"] == "turn" && row["role"] == "operator" && row["text"] == "/checkpoint")
        .count();
    assert_eq!(operator_checkpoint_turns, 0, "rows: {rows:?}");
}

#[test]
fn a_hand_in_flight_is_listed_and_its_verdict_lands_after() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let decision_id = setup_testscope(&run_dir);

    let hash = hash_of(&run_dir.join("arx.toml"));
    let author = simple_body(&format!("- file: arx.toml @ {hash}\n- h1 fake"), Some(&decision_id));
    plant_checkpoint_replies(&run_dir, &author, None, Some("ok"));

    let lines = vec!["hand:dispatch:fake:[delay:1200] slow".to_string(), "/checkpoint".to_string()];
    let rows = run_session(&run_dir, "arx.toml", &lines, &[]);

    let prompts = arx_prompt_rows(&rows);
    let authoring_prompt = prompts
        .iter()
        .find(|row| row["text"].as_str().unwrap_or("").contains("author the state object"))
        .expect("authoring prompt row");
    assert!(authoring_prompt["text"].as_str().unwrap().contains("- h1 fake"), "prompt: {authoring_prompt:?}");

    let written = fs::read_to_string(run_dir.join("threads/main.checkpoints/1.md")).unwrap();
    assert!(written.contains("- h1 fake"), "written: {written}");

    let pass_idx = rows
        .iter()
        .position(|row| row["kind"] == "checkpoint" && row["check"] == "pass")
        .expect("pass event row");
    let verdict_idx = rows
        .iter()
        .position(|row| {
            row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.starts_with("[hand h1 verdict]")).unwrap_or(false)
        })
        .expect("hand verdict row");
    assert!(pass_idx < verdict_idx, "rows: {rows:?}");
}

fn state_object_parse(text: &str) -> (u64, String) {
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("---"), "text: {text}");
    let mut checkpoint = None;
    for line in lines.by_ref() {
        if line == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("checkpoint: ") {
            checkpoint = Some(rest.parse::<u64>().unwrap());
        }
    }
    (checkpoint.expect("checkpoint field in header"), text.to_string())
}

fn transcript_turn_text(threads_dir: &Path, checkpoint_ordinal: u64) -> String {
    let checkpoints_dir = threads_dir.join("main.checkpoints");
    let text = fs::read_to_string(checkpoints_dir.join(format!("{checkpoint_ordinal}.md"))).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("---"));
    let mut turn = None;
    for line in lines.by_ref() {
        if line == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("turn: ") {
            turn = Some(rest.parse::<u64>().unwrap());
        }
    }
    let turn = turn.expect("turn field in header");
    let contents = fs::read_to_string(threads_dir.join("main.jsonl")).unwrap();
    let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    rows.iter()
        .find(|row| row["kind"] == "turn" && row["turn"].as_u64() == Some(turn))
        .and_then(|row| row["text"].as_str())
        .expect("turn text resolves")
        .to_string()
}
