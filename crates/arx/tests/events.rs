use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::str::contains;

#[path = "../src/events.rs"]
mod events;

mod common;
use common::fresh_run_fixture;

use events::{every_kind, Event, KINDS};

fn threads_dir(workspace_run: &Path) -> std::path::PathBuf {
    let dir = workspace_run.join("threads");
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn every_kind_writes_and_arx_events_counts_it() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    let threads = threads_dir(&run_dir);
    let log_path = threads.join("main.jsonl");

    let mut body = String::new();
    body.push_str("{\"kind\":\"system\",\"text\":\"you are main\"}\n");
    body.push_str("{\"kind\":\"turn\",\"turn\":1,\"text\":\"hello\"}\n");
    body.push_str("{\"kind\":\"usage\",\"usage\":{\"input_tokens\":1}}\n");
    for event in every_kind() {
        body.push_str(&serde_json::to_string(&event).unwrap());
        body.push('\n');
    }
    fs::write(&log_path, body).unwrap();

    let assert = Command::cargo_bin("arx")
        .unwrap()
        .args(["events", "--path", config_path.to_str().unwrap(), "main"])
        .assert()
        .success();

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    for kind in KINDS {
        assert!(
            stdout.lines().any(|line| line.trim_start().starts_with(kind) && line.contains('1')),
            "no count line for {kind} in: {stdout}"
        );
    }
    for row_kind in ["system", "turn", "usage"] {
        assert!(
            stdout.lines().any(|line| line.trim_start().starts_with(row_kind) && line.contains('1')),
            "no count line for {row_kind} in: {stdout}"
        );
    }

    let mut last_index: isize = -1;
    for kind in KINDS {
        let heading = format!("## {kind} (1)");
        let index = stdout.find(&heading).unwrap_or_else(|| panic!("missing heading {heading} in: {stdout}"));
        assert!(index as isize > last_index, "heading {heading} out of KINDS order in: {stdout}");
        last_index = index as isize;
    }
}

#[test]
fn gate_row_bytes_are_unchanged() {
    let event = Event::Gate {
        tool: "save".to_string(),
        verdict: "allow".to_string(),
        by: None,
        reason: None,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert_eq!(json, "{\"kind\":\"gate\",\"tool\":\"save\",\"verdict\":\"allow\"}");
}

#[test]
fn events_reader_skips_turns_and_errors_on_a_bad_event_row() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    let threads = threads_dir(&run_dir);
    let log_path = threads.join("main.jsonl");

    let mut body = String::new();
    body.push_str("{\"kind\":\"turn\",\"turn\":1,\"text\":\"hello\"}\n");
    body.push_str(&serde_json::to_string(&Event::Gate {
        tool: "save".to_string(),
        verdict: "allow".to_string(),
        by: None,
        reason: None,
    })
    .unwrap());
    body.push('\n');
    body.push_str("{\"kind\":\"nudge\",\"seat\":\"main\"}\n");
    fs::write(&log_path, body).unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["events", "--path", config_path.to_str().unwrap(), "main"])
        .assert()
        .failure()
        .stderr(contains("malformed transcript line at line 3"));
}
