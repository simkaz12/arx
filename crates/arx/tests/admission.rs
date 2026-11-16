use std::fs;

mod common;
use common::*;

fn plant_big_txt(run_dir: &std::path::Path) -> String {
    let line = format!("{}\n", "a".repeat(39));
    let content = line.repeat(1_200);
    fs::write(run_dir.join("big.txt"), &content).unwrap();
    content
}

fn stub_file_text(rows: &[serde_json::Value], nth: usize) -> String {
    let main_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "turn" && row["role"] == "main").collect();
    let text = main_rows[nth]["text"].as_str().unwrap();
    let payload: serde_json::Value = serde_json::from_str(text.strip_prefix("stub-file:").unwrap()).unwrap();
    payload["text"].as_str().unwrap().to_string()
}

#[test]
fn an_over_threshold_read_externalizes_and_the_handle_reads_back() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let content = plant_big_txt(&run_dir);
    let env = [("ARX_TEST_FAKE_KEY", "test-key".to_string())];

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &[
            "file:read_file:{\"path\":\"big.txt\"}".to_string(),
            "file:read_file:{\"path\":\"threads/main.traces/1-1.md\",\"offset\":1,\"limit\":20}".to_string(),
        ],
        &env,
    );

    let first_text = stub_file_text(&rows, 0);
    let handle = "threads/main.traces/1-1.md";
    assert!(
        first_text.starts_with(&format!("[arx] externalized read_file result: {handle} (")),
        "first text: {first_text}"
    );

    let trace_path = run_dir.join(handle);
    let trace_contents = fs::read_to_string(&trace_path).unwrap();
    for key in ["producer", "tool", "source", "sha256", "thread", "turn", "captured_at", "bytes", "lines"] {
        assert!(trace_contents.contains(&format!("{key}: ")), "missing {key} in {trace_contents}");
    }
    let sha_line = trace_contents.lines().find(|l| l.starts_with("sha256: ")).unwrap();
    let sha = sha_line.strip_prefix("sha256: ").unwrap();
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let recomputed: String = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(sha, recomputed, "sha256 mismatch for big.txt");

    let second_text = stub_file_text(&rows, 1);
    let line = format!("{}\n", "a".repeat(39));
    assert!(second_text.contains(&line), "second text: {second_text}");

    let admission_rows: Vec<&serde_json::Value> = rows.iter().filter(|r| r["kind"] == "admission").collect();
    let pull_rows: Vec<&serde_json::Value> = rows.iter().filter(|r| r["kind"] == "pull").collect();
    assert_eq!(admission_rows.len(), 2, "rows: {rows:?}");
    assert_eq!(pull_rows.len(), 1, "rows: {rows:?}");
    assert_eq!(admission_rows[0]["outcome"], "externalized");
    assert_eq!(admission_rows[0]["handle"], handle);
    assert_eq!(pull_rows[0]["handle"], handle);
    assert_eq!(pull_rows[0]["path"], format!("{handle}#L1-L20"));
    assert_eq!(admission_rows[1]["outcome"], "inline");

    let kinds: Vec<&str> = rows.iter().filter_map(|r| r["kind"].as_str()).filter(|k| *k == "admission" || *k == "pull").collect();
    assert_eq!(kinds, vec!["admission", "pull", "admission"], "rows: {rows:?}");
}

#[test]
fn a_whole_read_of_the_handle_is_an_echo_and_mints_nothing() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    plant_big_txt(&run_dir);
    let env = [("ARX_TEST_FAKE_KEY", "test-key".to_string())];

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &[
            "file:read_file:{\"path\":\"big.txt\"}".to_string(),
            "file:read_file:{\"path\":\"threads/main.traces/1-1.md\"}".to_string(),
        ],
        &env,
    );

    let second_text = stub_file_text(&rows, 1);
    assert!(
        second_text.starts_with("[arx] this result is the whole of threads/main.traces/1-1.md; the handle is unchanged ("),
        "second text: {second_text}"
    );

    let echo_rows: Vec<&serde_json::Value> = rows.iter().filter(|r| r["kind"] == "echo").collect();
    assert_eq!(echo_rows.len(), 1, "rows: {rows:?}");
    let admission_rows: Vec<&serde_json::Value> = rows.iter().filter(|r| r["kind"] == "admission").collect();
    assert_eq!(admission_rows.len(), 1, "rows: {rows:?}");

    let entries: Vec<_> = fs::read_dir(run_dir.join("threads/main.traces")).unwrap().collect();
    assert_eq!(entries.len(), 1, "entries: {entries:?}");
}

#[test]
fn a_repeat_read_hits_the_cache_until_the_source_changes() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    plant_big_txt(&run_dir);
    let env = [("ARX_TEST_FAKE_KEY", "test-key".to_string())];

    let rows1 = run_session(
        &run_dir,
        "arx.toml",
        &[
            "file:read_file:{\"path\":\"big.txt\"}".to_string(),
            "file:read_file:{\"path\":\"big.txt\"}".to_string(),
        ],
        &env,
    );
    let admission_rows1: Vec<&serde_json::Value> = rows1.iter().filter(|r| r["kind"] == "admission").collect();
    assert_eq!(admission_rows1.len(), 2, "rows1: {rows1:?}");
    assert_eq!(admission_rows1[0]["outcome"], "externalized");
    assert_eq!(admission_rows1[1]["outcome"], "cached");
    assert_eq!(admission_rows1[0]["handle"], admission_rows1[1]["handle"]);
    let first_handle = admission_rows1[0]["handle"].as_str().unwrap().to_string();

    let other_line = format!("{}\n", "b".repeat(39));
    fs::write(run_dir.join("big.txt"), other_line.repeat(1_200)).unwrap();

    let rows2 = run_session(&run_dir, "arx.toml", &["file:read_file:{\"path\":\"big.txt\"}".to_string()], &env);
    let admission_rows2: Vec<&serde_json::Value> = rows2.iter().filter(|r| r["kind"] == "admission").collect();
    assert_eq!(admission_rows2.len(), 3, "rows2: {rows2:?}");
    assert_eq!(admission_rows2[2]["outcome"], "externalized");
    let second_handle = admission_rows2[2]["handle"].as_str().unwrap().to_string();
    assert_ne!(first_handle, second_handle, "a changed source must mint a new handle");

    let sha_of = |path: &str| -> String {
        let contents = fs::read_to_string(run_dir.join(path)).unwrap();
        contents.lines().find(|l| l.starts_with("sha256: ")).unwrap().to_string()
    };
    assert_ne!(sha_of(&first_handle), sha_of(&second_handle), "the sha must change with the source");

    let entries: Vec<_> = fs::read_dir(run_dir.join("threads/main.traces")).unwrap().collect();
    assert_eq!(entries.len(), 2, "entries: {entries:?}");
}

#[test]
fn an_under_threshold_read_is_inline_and_uncached() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let env = [("ARX_TEST_FAKE_KEY", "test-key".to_string())];

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &[
            "file:read_file:{\"path\":\"prompts/main.md\"}".to_string(),
            "file:read_file:{\"path\":\"prompts/main.md\"}".to_string(),
        ],
        &env,
    );

    let admission_rows: Vec<&serde_json::Value> = rows.iter().filter(|r| r["kind"] == "admission").collect();
    assert_eq!(admission_rows.len(), 2, "rows: {rows:?}");
    assert!(admission_rows.iter().all(|r| r["outcome"] == "inline"), "rows: {rows:?}");
    assert!(!run_dir.join("threads/main.traces").exists(), "no traces dir should exist for inline-only reads");
}

#[test]
fn a_store_result_writes_no_admission_row() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let env = [("ARX_TEST_FAKE_KEY", "test-key".to_string())];

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["store:recall:{\"query\":\"anything\",\"scope\":\"arx\"}".to_string()],
        &env,
    );

    let gate_row = rows
        .iter()
        .find(|r| r["kind"] == "gate" && r["tool"] == "mcp__arx__recall")
        .expect("a gate row for recall");
    assert_eq!(gate_row["verdict"], "allow");
    assert!(!rows.iter().any(|r| r["kind"] == "admission"), "rows: {rows:?}");
}

#[test]
fn an_override_raises_one_tools_threshold() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");
    let mut contents = fs::read_to_string(&config_path).unwrap();
    contents.push_str("\n[admission.overrides]\nread_file = 20000\n");
    fs::write(&config_path, contents).unwrap();

    let content = plant_big_txt(&run_dir);
    let env = [("ARX_TEST_FAKE_KEY", "test-key".to_string())];

    let rows = run_session(&run_dir, "arx.toml", &["file:read_file:{\"path\":\"big.txt\"}".to_string()], &env);

    let admission_row = rows.iter().find(|r| r["kind"] == "admission").expect("an admission row");
    assert_eq!(admission_row["outcome"], "inline");

    let text = stub_file_text(&rows, 0);
    assert_eq!(text, content);
}
