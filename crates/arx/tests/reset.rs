use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use predicates::str::contains;

mod common;
use common::*;

fn occupancy_of(usage: &serde_json::Value) -> u64 {
    ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
        .iter()
        .filter_map(|field| usage.get(field).and_then(|value| value.as_u64()))
        .sum()
}

fn header_turn(checkpoint_path: &Path) -> u64 {
    let text = fs::read_to_string(checkpoint_path).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("---"), "text: {text}");
    for line in lines {
        if line == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("turn: ") {
            return rest.parse().unwrap();
        }
    }
    panic!("no turn field in header: {text}");
}

fn find_orientation_turn(rows: &[serde_json::Value]) -> &serde_json::Value {
    rows.iter()
        .find(|row| {
            row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"]
                    .as_str()
                    .unwrap_or("")
                    .starts_with("[arx] this window is a respawn after reset in thread main.")
        })
        .expect("orientation turn row")
}

fn wait_rows(run_dir: &Path, deadline: Instant, pred: impl Fn(&[serde_json::Value]) -> bool) {
    loop {
        let rows = transcript_rows(run_dir);
        if pred(&rows) {
            return;
        }
        if Instant::now() >= deadline {
            panic!("condition not met before deadline; rows: {rows:?}");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn plain_lines(n: u64) -> Vec<String> {
    (1..=n).map(|i| format!("line{i}")).collect()
}

#[test]
fn crossing_the_reserve_arms_once_and_tells_the_operator() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 3000, 600);

    let env = [("ARX_STUB_TOKENS_PER_TURN", "400".to_string())];
    let lines = plain_lines(6);
    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &env);

    let resets: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "reset").collect();
    assert_eq!(resets.len(), 1, "rows: {rows:?}");
    assert_eq!(resets[0]["phase"], "armed", "rows: {rows:?}");
    assert!(resets[0]["check"].is_null(), "rows: {rows:?}");
    assert!(
        stdout.contains("[arx] reset armed: window at 2400 of 3000, reserve 600; /reset to reset now"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("reset fired"), "stdout: {stdout}");
}

#[test]
fn the_operator_fires_and_the_respawn_orients_from_the_state_object() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 3000, 600);

    let env = [("ARX_STUB_TOKENS_PER_TURN", "400".to_string())];
    let mut lines = plain_lines(6);
    lines.push("/reset".to_string());
    lines.push("ok".to_string());
    lines.push("after".to_string());
    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &env);

    assert!(run_dir.join("threads/main.checkpoints/1.md").exists(), "rows: {rows:?}");

    let checkpoint_pass_idx = rows
        .iter()
        .position(|row| row["kind"] == "checkpoint" && row["check"] == "pass")
        .expect("checkpoint pass row");
    let reset_fired_idx = rows
        .iter()
        .position(|row| row["kind"] == "reset" && row["phase"] == "fired" && row["check"] == "operator")
        .expect("reset fired row");
    let system_idxs: Vec<usize> = rows.iter().enumerate().filter(|(_, row)| row["kind"] == "system").map(|(i, _)| i).collect();
    assert_eq!(system_idxs.len(), 2, "rows: {rows:?}");
    let second_system_idx = system_idxs[1];
    let reset_respawned_idx = rows
        .iter()
        .position(|row| row["kind"] == "reset" && row["phase"] == "respawned" && row["check"] == "checkpoint 1")
        .expect("reset respawned row");
    let orientation_turn = find_orientation_turn(&rows);
    let orientation_turn_idx = rows.iter().position(|row| row == orientation_turn).unwrap();
    let orientation_event_idx = rows.iter().position(|row| row["kind"] == "orientation").expect("orientation event row");

    assert!(checkpoint_pass_idx < reset_fired_idx, "rows: {rows:?}");
    assert!(reset_fired_idx < second_system_idx, "rows: {rows:?}");
    assert!(second_system_idx < reset_respawned_idx, "rows: {rows:?}");
    assert!(reset_respawned_idx < orientation_turn_idx, "rows: {rows:?}");
    assert!(orientation_turn_idx < orientation_event_idx, "rows: {rows:?}");
    assert_eq!(rows[orientation_event_idx]["verdict"], "reported", "rows: {rows:?}");

    let t = orientation_turn["turn"].as_u64().unwrap() - 1;
    let text = orientation_turn["text"].as_str().unwrap();
    assert!(
        text.contains(&format!(
            "Your predecessor's state object, checkpoint 1, is below; the transcript is at threads/main.jsonl, turns 1 to {t}, readable with read_file."
        )),
        "text: {text}"
    );

    let after_turn_idx = rows
        .iter()
        .position(|row| row["kind"] == "turn" && row["role"] == "operator" && row["text"] == "after")
        .expect("after turn row");
    let after_usage = rows[after_turn_idx..].iter().find(|row| row["kind"] == "usage").expect("usage row after 'after'");
    assert_eq!(occupancy_of(&after_usage["usage"]), 800, "rows: {rows:?}");

    assert!(stdout.contains("state object 1 above. ok to tear down and respawn"), "stdout: {stdout}");
}

#[test]
fn an_operator_note_rides_into_the_orientation_turn() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 3000, 600);

    let env = [("ARX_STUB_TOKENS_PER_TURN", "400".to_string())];
    let mut session = LiveSession::spawn(&run_dir, "arx.toml", &env);
    session.drain_output();
    let deadline = Instant::now() + Duration::from_secs(5);

    for i in 1..=6 {
        session.write_line(&format!("line{i}"));
    }
    wait_rows(&run_dir, deadline, |rows| rows.iter().any(|row| row["kind"] == "reset" && row["phase"] == "armed"));

    session.write_line("/reset");
    wait_rows(&run_dir, deadline, |rows| rows.iter().any(|row| row["kind"] == "checkpoint" && row["check"] == "pass"));
    let before = fs::read(run_dir.join("threads/main.checkpoints/1.md")).unwrap();

    session.write_line("the branch is l3-02, not main");
    wait_rows(&run_dir, deadline, |rows| rows.iter().any(|row| row["kind"] == "orientation"));

    let after = fs::read(run_dir.join("threads/main.checkpoints/1.md")).unwrap();
    assert_eq!(before, after);

    session.close_stdin();
    assert!(session.wait_success(deadline), "arx run did not exit successfully");

    let rows = transcript_rows(&run_dir);
    let orientation_turn = find_orientation_turn(&rows);
    let text = orientation_turn["text"].as_str().unwrap();
    let last_line = text.lines().last().unwrap();
    assert_eq!(last_line, "Operator note: the branch is l3-02, not main", "text: {text}");
    assert_eq!(text.matches("Operator note:").count(), 1, "text: {text}");
}

#[test]
fn the_harness_fires_alone_at_the_ceiling() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 3000, 600);

    let env = [("ARX_STUB_TOKENS_PER_TURN", "400".to_string())];
    let lines = plain_lines(9);
    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &env);

    let fired = rows.iter().find(|row| row["kind"] == "reset" && row["phase"] == "fired").expect("reset fired row");
    assert_eq!(fired["check"], "harness", "rows: {rows:?}");
    assert!(!stdout.contains("ok to tear down and respawn"), "stdout: {stdout}");
    assert!(
        stdout.contains("[arx] reset fired at the window ceiling: state object 1 written and shown below; respawning."),
        "stdout: {stdout}"
    );
    assert!(rows.iter().any(|row| row["kind"] == "orientation"), "rows: {rows:?}");

    let ninth_idx = rows
        .iter()
        .position(|row| row["kind"] == "turn" && row["role"] == "operator" && row["text"] == "line9")
        .expect("ninth line row");
    let usage_after = rows[ninth_idx..].iter().find(|row| row["kind"] == "usage").expect("usage row after ninth line");
    assert_eq!(occupancy_of(&usage_after["usage"]), 800, "rows: {rows:?}");
}

#[test]
fn a_rejected_ritual_turn_takes_the_floor_from_the_last_checkpoint() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 3000, 600);

    let env = [
        ("ARX_STUB_TOKENS_PER_TURN", "400".to_string()),
        ("ARX_STUB_REJECT_AUTHORING_AT", "2".to_string()),
    ];
    let lines = vec![
        "line1".to_string(),
        "line2".to_string(),
        "/checkpoint".to_string(),
        "line3".to_string(),
        "line4".to_string(),
        "/reset".to_string(),
    ];
    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &env);

    let checkpoint_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "checkpoint").collect();
    assert_eq!(checkpoint_rows.len(), 1, "rows: {rows:?}");
    assert_eq!(checkpoint_rows[0]["check"], "pass", "rows: {rows:?}");
    assert_eq!(checkpoint_rows[0]["delta"], 1600, "rows: {rows:?}");

    let floor_row = rows.iter().find(|row| row["kind"] == "floor").expect("floor row");
    assert_eq!(floor_row["overshoot"], 800, "rows: {rows:?}");

    let respawned = rows.iter().find(|row| row["kind"] == "reset" && row["phase"] == "respawned").expect("respawned row");
    assert_eq!(respawned["check"], "checkpoint 1", "rows: {rows:?}");

    assert!(!rows.iter().any(|row| row["kind"] == "reset" && row["phase"] == "fired"), "rows: {rows:?}");

    let k = header_turn(&run_dir.join("threads/main.checkpoints/1.md")) + 1;
    let orientation_turn = find_orientation_turn(&rows);
    let t = orientation_turn["turn"].as_u64().unwrap() - 1;
    let text = orientation_turn["text"].as_str().unwrap();
    let expected = format!("The reset ritual could not complete: the window had grown 800 tokens past checkpoint 1, so turns {k} to {t} are not in the object.");
    assert!(text.contains(&expected), "text: {text}, expected: {expected}");

    assert!(stdout.contains("respawning from checkpoint 1"), "stdout: {stdout}");
    assert!(!stdout.contains("ok to tear down and respawn"), "stdout: {stdout}");
}

#[test]
fn with_no_checkpoint_the_floor_orients_from_the_transcript_alone() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 3000, 600);

    let env = [
        ("ARX_STUB_TOKENS_PER_TURN", "400".to_string()),
        ("ARX_STUB_REJECT_AUTHORING_AT", "1".to_string()),
    ];
    let mut lines = plain_lines(6);
    lines.push("/reset".to_string());
    let (rows, _stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &env);

    let floor_row = rows.iter().find(|row| row["kind"] == "floor").expect("floor row");
    assert_eq!(floor_row["overshoot"], 2400, "rows: {rows:?}");

    let respawned = rows.iter().find(|row| row["kind"] == "reset" && row["phase"] == "respawned").expect("respawned row");
    assert_eq!(respawned["check"], "transcript", "rows: {rows:?}");

    let orientation_turn = find_orientation_turn(&rows);
    let t = orientation_turn["turn"].as_u64().unwrap() - 1;
    let text = orientation_turn["text"].as_str().unwrap();
    assert!(
        text.contains(&format!(
            "No state object exists for this thread; orient from the transcript at threads/main.jsonl, turns 1 to {t}, readable with read_file."
        )),
        "text: {text}"
    );

    assert!(!run_dir.join("threads/main.checkpoints").exists(), "no checkpoint dir should exist");
}

#[test]
fn an_ordinary_turn_rejected_for_a_full_window_takes_the_floor_too() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 3000, 600);

    let env = [("ARX_STUB_TOKENS_PER_TURN", "400".to_string())];
    let lines = vec!["line1".to_string(), "window-exceeded".to_string(), "after".to_string()];
    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &env);

    let floor_row = rows.iter().find(|row| row["kind"] == "floor").expect("floor row");
    assert_eq!(floor_row["overshoot"], 400, "rows: {rows:?}");

    assert!(!rows.iter().any(|row| row["kind"] == "reset" && row["phase"] == "fired"), "rows: {rows:?}");
    let respawned = rows.iter().find(|row| row["kind"] == "reset" && row["phase"] == "respawned").expect("respawned row");
    assert_eq!(respawned["check"], "transcript", "rows: {rows:?}");
    assert!(rows.iter().any(|row| row["kind"] == "orientation"), "rows: {rows:?}");
    assert!(!stdout.contains("ok to tear down and respawn"), "stdout: {stdout}");

    let after_idx = rows
        .iter()
        .position(|row| row["kind"] == "turn" && row["role"] == "operator" && row["text"] == "after")
        .expect("after row");
    let usage_after = rows[after_idx..].iter().find(|row| row["kind"] == "usage").expect("usage row after 'after'");
    assert_eq!(occupancy_of(&usage_after["usage"]), 800, "rows: {rows:?}");
}

#[test]
fn a_delivered_hand_verdict_that_exceeds_the_window_takes_the_floor_too() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 3000, 600);

    let env = [("ARX_STUB_TOKENS_PER_TURN", "400".to_string())];
    let lines = vec!["hand:dispatch:fake:[delay:200] window-exceeded".to_string()];
    let (rows, stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &env);

    let floor_row = rows.iter().find(|row| row["kind"] == "floor").expect("floor row");
    assert!(floor_row["overshoot"].as_u64().is_some(), "rows: {rows:?}");
    assert!(!rows.iter().any(|row| row["kind"] == "reset" && row["phase"] == "fired"), "rows: {rows:?}");
    let respawned = rows.iter().find(|row| row["kind"] == "reset" && row["phase"] == "respawned").expect("respawned row");
    assert_eq!(respawned["check"], "transcript", "rows: {rows:?}");
    assert!(rows.iter().any(|row| row["kind"] == "orientation"), "rows: {rows:?}");
    assert!(!stdout.contains("ok to tear down and respawn"), "stdout: {stdout}");
}

#[test]
fn a_delivered_hand_verdict_crossing_the_ceiling_fires_the_harness_reset() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 800, 200);

    let env = [("ARX_STUB_TOKENS_PER_TURN", "400".to_string())];
    let lines = vec!["hand:dispatch:fake:[delay:200] slow".to_string()];
    let (rows, _stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &env);

    let dispatch_reply_idx = rows
        .iter()
        .position(|row| row["kind"] == "turn" && row["role"] == "main" && row["text"].as_str().unwrap_or("").starts_with("stub-hand:"))
        .expect("dispatch reply row");
    let fired = rows.iter().find(|row| row["kind"] == "reset" && row["phase"] == "fired" && row["check"] == "harness").expect("reset fired harness row");
    let fired_idx = rows.iter().position(|row| row == fired).unwrap();
    assert!(dispatch_reply_idx < fired_idx, "rows: {rows:?}");
    assert!(rows.iter().any(|row| row["kind"] == "orientation"), "rows: {rows:?}");
}

#[test]
fn an_explicit_checkpoint_crossing_the_ceiling_fires_the_harness_reset() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 800, 200);

    let env = [("ARX_STUB_TOKENS_PER_TURN", "400".to_string())];
    let lines = vec!["/checkpoint".to_string()];
    let (rows, _stdout) = run_session_with_stdout(&run_dir, "arx.toml", &lines, &env);

    let checkpoint_pass_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "checkpoint" && row["check"] == "pass").collect();
    assert_eq!(checkpoint_pass_rows.len(), 2, "rows: {rows:?}");

    let checkpoint_1_idx = rows.iter().position(|row| row["kind"] == "checkpoint" && row["check"] == "pass").expect("first checkpoint pass row");
    let fired = rows.iter().find(|row| row["kind"] == "reset" && row["phase"] == "fired" && row["check"] == "harness").expect("reset fired harness row");
    let fired_idx = rows.iter().position(|row| row == fired).unwrap();
    assert!(checkpoint_1_idx < fired_idx, "rows: {rows:?}");

    assert!(rows.iter().any(|row| row["kind"] == "orientation"), "rows: {rows:?}");
    assert!(run_dir.join("threads/main.checkpoints/2.md").exists(), "rows: {rows:?}");
}

#[test]
fn the_body_compaction_window_sits_above_the_ceiling() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let _rows = run_session(&run_dir, "arx.toml", &["one".to_string()], &[]);

    let stub_init: serde_json::Value = serde_json::from_str(&fs::read_to_string(run_dir.join("stub-init.json")).unwrap()).unwrap();
    let compact_window: u64 = stub_init["compactWindow"].as_str().expect("compactWindow is a string").parse().unwrap();
    assert_eq!(compact_window, 1_000_000);
    assert!(compact_window > 800_000);

    let config_path = run_dir.join("arx.toml");
    let contents = fs::read_to_string(&config_path).unwrap();
    let needle = "body = \"claude-code\"\n";
    assert!(contents.contains(needle), "config: {contents}");
    let over_path = run_dir.join("arx-over.toml");
    let over_contents = contents.replacen(needle, &format!("{needle}window = 950000\n"), 1);
    fs::write(&over_path, over_contents).unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", over_path.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains(
            "main: window must stay below 900000; the body compacts near 967k of the 1M compaction window arx sets",
        ));
}

#[test]
fn reset_after_a_switch_respawns_with_the_switched_model() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let lines = vec!["/model stub-model-2".to_string(), "/reset".to_string(), "ok".to_string()];
    let rows = run_session(&run_dir, "arx.toml", &lines, &[]);

    let stub_init: serde_json::Value = serde_json::from_str(&fs::read_to_string(run_dir.join("stub-init.json")).unwrap()).unwrap();
    assert_eq!(stub_init["model"], "stub-model-2", "stub-init: {stub_init}");

    assert!(rows.iter().any(|row| row["kind"] == "orientation"), "rows: {rows:?}");
}

#[test]
fn a_fallback_immediately_followed_by_a_fatal_still_lands_before_the_floor_respawn() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let rows = run_session(&run_dir, "arx.toml", &["model-fallback-then-fatal".to_string()], &[]);

    let model_events: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "model").collect();
    assert_eq!(model_events.len(), 1, "rows: {rows:?}");
    assert_eq!(model_events[0]["from"], "stub-model");
    assert_eq!(model_events[0]["to"], "stub-model-2");

    let floor_row = rows.iter().find(|row| row["kind"] == "floor").expect("floor row");
    assert!(floor_row["overshoot"].as_u64().is_some(), "rows: {rows:?}");

    let stub_init: serde_json::Value = serde_json::from_str(&fs::read_to_string(run_dir.join("stub-init.json")).unwrap()).unwrap();
    assert_eq!(stub_init["model"], "stub-model-2", "stub-init: {stub_init}");
}

#[test]
fn a_fallback_that_lands_after_turn_end_is_still_picked_up_before_the_operator_reset() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_models(&config_path, &["stub-model", "stub-model-2"]);

    let lines = vec!["model-fallback-after-turn-end".to_string(), "/reset".to_string(), "ok".to_string()];
    let rows = run_session(&run_dir, "arx.toml", &lines, &[]);

    let model_events: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "model").collect();
    assert_eq!(model_events.len(), 1, "rows: {rows:?}");
    assert_eq!(model_events[0]["from"], "stub-model");
    assert_eq!(model_events[0]["to"], "stub-model-2");

    let stub_init: serde_json::Value = serde_json::from_str(&fs::read_to_string(run_dir.join("stub-init.json")).unwrap()).unwrap();
    assert_eq!(stub_init["model"], "stub-model-2", "stub-init: {stub_init}");
}

#[test]
fn a_hand_in_flight_survives_the_reset_and_lands_after_the_orientation() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    with_main_window_and_reserve(&config_path, 3000, 600);

    let author = "## Position\nnothing new.\n## Target\nland it.\n## Drift\nnone.\n## Hands in flight\n- h1 fake\n";
    plant_checkpoint_replies(&run_dir, author, None, Some("ok"));

    let lines = vec!["hand:dispatch:fake:[delay:1200] slow".to_string(), "/reset".to_string(), "ok".to_string()];
    let rows = run_session(&run_dir, "arx.toml", &lines, &[]);

    let written = fs::read_to_string(run_dir.join("threads/main.checkpoints/1.md")).unwrap();
    assert!(written.contains("h1"), "written: {written}");

    let spawn_idx = rows.iter().position(|row| row["kind"] == "spawn" && row["hand"] == "h1").expect("spawn row");
    let fired_idx = rows.iter().position(|row| row["kind"] == "reset" && row["phase"] == "fired").expect("reset fired row");
    assert!(spawn_idx < fired_idx, "rows: {rows:?}");

    let orientation_idx = rows.iter().position(|row| row["kind"] == "orientation").expect("orientation row");
    let delivered_idx = rows.iter().position(|row| row["kind"] == "delivered" && row["hand"] == "h1").expect("delivered row");
    assert!(delivered_idx > orientation_idx, "rows: {rows:?}");
}
