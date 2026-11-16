use std::fs;

use assert_cmd::Command;

mod common;
use common::*;

#[test]
fn read_file_and_recall_rules_match_their_subjects() {
    let (base_url, _state) = fake_openai_server(FakeScript::Sequence(vec![
        FakeScript::Sse(vec![
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read_file","arguments":"{\"path\": \"probe.txt\"}"}}]}}]}"#.to_string(),
            r#"{"choices":[{"delta":{"tool_calls":[{"index":1,"id":"call_2","type":"function","function":{"name":"recall","arguments":"{\"query\": \"codename\"}"}}]}}]}"#.to_string(),
            r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#.to_string(),
        ]),
        FakeScript::Sse(vec![
            r#"{"choices":[{"delta":{"content":"done"}}]}"#.to_string(),
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string(),
        ]),
    ]));
    let workspace = native_run_fixture(&base_url);
    let config_path = workspace.path().join("run/arx.toml");
    let mut contents = fs::read_to_string(&config_path).unwrap();
    contents.push_str("\n[gate]\ndeny = [\"read_file(probe.txt)\", \"recall(codename)\"]\n");
    fs::write(&config_path, contents).unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["run", config_path.to_str().unwrap()])
        .env("ARX_TEST_FAKE_KEY", "test-key")
        .env("PATH", &claude_hand_path())
        .write_stdin("hand:dispatch:hand-ds:read the probe and recall the codename\n")
        .assert()
        .success();

    let trace_contents = fs::read_to_string(workspace.path().join("run/threads/main.h1/transcript.jsonl")).unwrap();
    let trace_rows: Vec<serde_json::Value> = trace_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let gate_rows: Vec<&serde_json::Value> = trace_rows.iter().filter(|row| row["kind"] == "gate").collect();
    assert_eq!(gate_rows.len(), 2, "gate rows: {gate_rows:?}");
    assert_eq!(gate_rows[0]["tool"], "read_file");
    assert_eq!(gate_rows[0]["verdict"], "deny");
    assert_eq!(gate_rows[1]["tool"], "recall");
    assert_eq!(gate_rows[1]["verdict"], "deny");

    let tool_result_rows: Vec<&serde_json::Value> = trace_rows.iter().filter(|row| row["kind"] == "tool_result").collect();
    assert_eq!(tool_result_rows.len(), 2, "tool_result rows: {tool_result_rows:?}");
    let first = tool_result_rows[0]["content"].as_str().unwrap();
    assert!(first.starts_with("denied:") && first.contains("probe.txt"), "content: {first}");
    let second = tool_result_rows[1]["content"].as_str().unwrap();
    assert!(second.starts_with("denied:") && second.contains("codename"), "content: {second}");
}
