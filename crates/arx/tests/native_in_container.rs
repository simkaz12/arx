use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command as StdCommand, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::cargo::cargo_bin;
use sha2::{Digest, Sha256};

mod common;
use common::*;

static DOCKER_STATE_SERIAL: Mutex<()> = Mutex::new(());

fn serialize_on_docker_state() -> std::sync::MutexGuard<'static, ()> {
    DOCKER_STATE_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn ledger_rows(run_dir: &Path) -> Vec<serde_json::Value> {
    let contents = fs::read_to_string(run_dir.join("threads/main.ledger.jsonl")).unwrap_or_default();
    contents.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

fn log_rows(run_dir: &Path) -> Vec<serde_json::Value> {
    let contents = fs::read_to_string(run_dir.join("threads/main.jsonl")).unwrap_or_default();
    contents.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

fn transcript_rows(run_dir: &Path, hand: &str) -> Vec<serde_json::Value> {
    let contents = fs::read_to_string(run_dir.join(format!("threads/main.{hand}/transcript.jsonl"))).unwrap();
    contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

fn poll<F: FnMut() -> bool>(deadline: Instant, mut check: F) -> bool {
    loop {
        if check() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_exit(child: &mut std::process::Child) {
    wait_for_exit_ex(child, None)
}

fn wait_for_exit_ex(child: &mut std::process::Child, debug_run_dir: Option<&Path>) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "arx run exited with {status:?}");
            return;
        }
        if Instant::now() >= deadline {
            if let Some(run_dir) = debug_run_dir {
                eprintln!("DEBUG ledger at timeout: {:?}", ledger_rows(run_dir));
                eprintln!("DEBUG log at timeout: {:?}", log_rows(run_dir));
            }
            let _ = child.kill();
            panic!("arx run did not exit within 15 seconds");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

struct Session {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
}

impl Session {
    fn spawn(run_dir: &Path) -> Session {
        let mut child = StdCommand::new(cargo_bin("arx"))
            .args(["run", "arx.toml"])
            .current_dir(run_dir)
            .env("ARX_TEST_FAKE_KEY", "test-key")
            .env("PATH", claude_hand_path())
            .env("ARX_USER_GATE", run_dir.join("user-gate.toml"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = stdout.read_to_end(&mut sink);
        });
        thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = stderr.read_to_end(&mut sink);
        });
        Session {
            child,
            stdin: Some(stdin),
        }
    }

    fn write_line(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin already closed");
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    fn close_stdin(&mut self) {
        self.stdin.take();
    }
}

fn tool_call_step(call_id: &str, name: &str, arguments: serde_json::Value) -> FakeScript {
    let arguments_str = arguments.to_string();
    let delta = serde_json::json!({
        "choices": [{
            "delta": {
                "tool_calls": [{
                    "index": 0,
                    "id": call_id,
                    "type": "function",
                    "function": { "name": name, "arguments": arguments_str }
                }]
            }
        }]
    });
    let finish = serde_json::json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]});
    FakeScript::Sse(vec![delta.to_string(), finish.to_string()])
}

fn end_turn_step(text: &str) -> FakeScript {
    let delta = serde_json::json!({"choices":[{"delta":{"content": text}}]});
    let finish = serde_json::json!({"choices":[{"delta":{},"finish_reason":"stop"}]});
    FakeScript::Sse(vec![delta.to_string(), finish.to_string()])
}

fn set_base_url(config_path: &Path, base_url: &str) {
    let contents = fs::read_to_string(config_path).unwrap();
    let updated = contents.replace("http://127.0.0.1:0", base_url);
    fs::write(config_path, updated).unwrap();
}

fn append_gate(config_path: &Path, toml_fragment: &str) {
    let mut contents = fs::read_to_string(config_path).unwrap();
    contents.push_str(toml_fragment);
    fs::write(config_path, contents).unwrap();
}

fn jsonl_files_recursive(dir: &Path) -> Vec<std::path::PathBuf> {
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

#[test]
fn read_file_executes_inside_the_container_and_the_store_is_masked() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let planted = plant_engine_scope(&run_dir.join("store"), "testscope");
    let store_rel = format!("store/testscope/{}.md", planted.hot_id);

    let (base_url, _state) = fake_openai_server(FakeScript::Sequence(vec![
        tool_call_step("call_1", "read_file", serde_json::json!({"path": "arx.toml"})),
        tool_call_step("call_2", "read_file", serde_json::json!({"path": store_rel})),
        end_turn_step("done"),
    ]));

    let config_path = run_dir.join("arx.toml");
    set_base_url(&config_path, &base_url);

    run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-ds:read arx.toml then the claim file".to_string()],
        &[("ARX_TEST_FAKE_KEY", "test-key".to_string())],
    );

    assert!(!run_dir.join("threads/main.h1.jsonl").exists(), "flat trace path must not exist");
    assert!(run_dir.join("threads/main.h1/transcript.jsonl").exists(), "transcript.jsonl missing");
    assert!(run_dir.join("threads/main.h1/trace.md").exists(), "trace.md missing");

    let trace_rows = transcript_rows(&run_dir, "h1");
    let tool_results: Vec<&serde_json::Value> = trace_rows.iter().filter(|row| row["kind"] == "tool_result").collect();
    assert_eq!(tool_results.len(), 2, "tool_result rows: {tool_results:?}");
    assert!(
        tool_results[0]["content"].as_str().unwrap().contains("sidecar"),
        "first tool_result: {:?}",
        tool_results[0]
    );
    let masked_content = tool_results[1]["content"].as_str().unwrap();
    assert!(!masked_content.contains("skylark"), "masked tool_result leaked the claim: {masked_content}");
    assert!(masked_content.contains("No such file") || masked_content.starts_with("read_file error"), "masked tool_result: {masked_content}");
}

#[test]
fn a_deny_rule_reaches_the_model_as_the_tool_result() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    fs::write(run_dir.join("secret.txt"), "top secret").unwrap();

    let (base_url, _state) = fake_openai_server(FakeScript::Sequence(vec![
        tool_call_step("call_1", "read_file", serde_json::json!({"path": "secret.txt"})),
        end_turn_step("done"),
    ]));

    let config_path = run_dir.join("arx.toml");
    set_base_url(&config_path, &base_url);
    append_gate(&config_path, "\n[gate]\ndeny = [\"read_file(secret*)\"]\n");

    run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-ds:read secret.txt".to_string()],
        &[("ARX_TEST_FAKE_KEY", "test-key".to_string())],
    );

    let trace_rows = transcript_rows(&run_dir, "h1");
    let gate_row = trace_rows.iter().find(|row| row["kind"] == "gate").expect("gate row");
    assert_eq!(gate_row["verdict"], "deny");
    let tool_result = trace_rows.iter().find(|row| row["kind"] == "tool_result").expect("tool_result row");
    let content = tool_result["content"].as_str().unwrap();
    assert!(content.starts_with("denied:"), "content: {content}");
    assert!(!content.contains("top secret"), "content: {content}");
}

#[test]
fn recall_is_served_on_the_host_when_listed() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    plant_engine_scope(&run_dir.join("store"), "testscope");

    fs::write(
        run_dir.join("agents/hand-dr.md"),
        "---\ntools: [read_file, recall]\n---\nDeepSeek-shaped native hand with recall on its surface.\n",
    )
    .unwrap();
    let config_path = run_dir.join("arx.toml");
    let contents = fs::read_to_string(&config_path).unwrap();
    fs::write(&config_path, contents.replacen("hand-ds = \"fake-model\"\n", "hand-ds = \"fake-model\"\nhand-dr = \"fake-model\"\n", 1)).unwrap();

    let (base_url, _state) = fake_openai_server(FakeScript::Sequence(vec![
        tool_call_step("call_1", "recall", serde_json::json!({"query": "launch codename"})),
        tool_call_step("call_2", "read_file", serde_json::json!({"path": "store/testscope/nope.md"})),
        end_turn_step("done"),
    ]));

    let config_path = run_dir.join("arx.toml");
    set_base_url(&config_path, &base_url);
    append_gate(&config_path, "\n[store]\nscope = \"testscope\"\n");

    run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-dr:recall the codename".to_string()],
        &[("ARX_TEST_FAKE_KEY", "test-key".to_string())],
    );

    let trace_rows = transcript_rows(&run_dir, "h1");
    let tool_results: Vec<&serde_json::Value> = trace_rows.iter().filter(|row| row["kind"] == "tool_result").collect();
    assert_eq!(tool_results.len(), 2, "tool_result rows: {tool_results:?}");
    assert!(
        tool_results[0]["content"].as_str().unwrap().contains("skylark"),
        "recall tool_result: {:?}",
        tool_results[0]
    );
    let masked_content = tool_results[1]["content"].as_str().unwrap();
    assert!(masked_content.contains("No such file") || masked_content.starts_with("read_file error"), "read_file tool_result: {masked_content}");
}

#[test]
fn the_key_is_nowhere_under_threads_or_in_docker_inspect() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let (base_url, _state) = fake_openai_server(FakeScript::Stall {
        first: r#"{"choices":[{"delta":{"content":"partial"}}]}"#.to_string(),
    });

    let config_path = run_dir.join("arx.toml");
    set_base_url(&config_path, &base_url);

    let mut session = Session::spawn(&run_dir);
    session.write_line("hand:dispatch:hand-ds:say the word");

    let deadline = Instant::now() + Duration::from_secs(10);
    assert!(
        poll(deadline, || ledger_rows(&run_dir).iter().any(|row| row["kind"] == "spawn" && row["hand"] == "h1")),
        "hand h1 never spawned"
    );

    let root = ledger_rows(&run_dir)
        .into_iter()
        .find(|row| row["kind"] == "spawn" && row["hand"] == "h1")
        .and_then(|row| row["root"].as_str().map(|s| s.to_string()))
        .expect("spawn row carries root");
    let root_name = Path::new(&root).file_name().unwrap().to_str().unwrap().to_string();
    let container_name = format!("arx-{root_name}");

    let inspect_deadline = Instant::now() + Duration::from_secs(10);
    let mut inspected = String::new();
    let found = poll(inspect_deadline, || {
        let output = StdCommand::new("docker").args(["inspect", &container_name]).output().unwrap();
        if output.status.success() {
            inspected = String::from_utf8_lossy(&output.stdout).into_owned();
            true
        } else {
            false
        }
    });
    assert!(found, "container {container_name} never became inspectable");
    assert!(!inspected.contains("test-key"), "docker inspect leaked the api key: {inspected}");

    session.write_line("hand:kill:h1");
    session.close_stdin();
    wait_for_exit(&mut session.child);

    for path in jsonl_files_recursive(&run_dir.join("threads")) {
        let contents = fs::read_to_string(&path).unwrap();
        assert!(!contents.contains("test-key"), "{}: leaked the api key verbatim: {contents}", path.display());
    }
}

#[test]
fn a_native_hand_s_over_threshold_read_is_externalized_inside_the_loop() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let line = format!("{}\n", "a".repeat(39));
    let content = line.repeat(1_200);
    fs::write(run_dir.join("big.txt"), &content).unwrap();

    let (base_url, _state) = fake_openai_server(FakeScript::Sequence(vec![
        tool_call_step("call_1", "read_file", serde_json::json!({"path": "big.txt"})),
        tool_call_step(
            "call_2",
            "read_file",
            serde_json::json!({"path": "/hand/traces/1-1.md", "offset": 1, "limit": 5}),
        ),
        end_turn_step("done"),
    ]));

    let config_path = run_dir.join("arx.toml");
    set_base_url(&config_path, &base_url);

    run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-ds:read big.txt then its handle".to_string()],
        &[("ARX_TEST_FAKE_KEY", "test-key".to_string())],
    );

    let trace_rows = transcript_rows(&run_dir, "h1");
    let tool_results: Vec<&serde_json::Value> = trace_rows.iter().filter(|row| row["kind"] == "tool_result").collect();
    assert_eq!(tool_results.len(), 2, "tool_result rows: {tool_results:?}");
    let first_content = tool_results[0]["content"].as_str().unwrap();
    assert!(
        first_content.starts_with("[arx] externalized read_file result: /hand/traces/1-1.md ("),
        "first tool_result: {first_content}"
    );
    let second_content = tool_results[1]["content"].as_str().unwrap();
    assert!(
        second_content.contains("producer: externalized tool result") && second_content.contains("tool: read_file"),
        "second tool_result: {second_content}"
    );

    let admission_and_pull_kinds: Vec<&str> = trace_rows
        .iter()
        .filter_map(|row| row["kind"].as_str())
        .filter(|kind| *kind == "admission" || *kind == "pull")
        .collect();
    assert_eq!(admission_and_pull_kinds, vec!["admission", "pull", "admission"], "trace rows: {trace_rows:?}");

    let admission_rows: Vec<&serde_json::Value> = trace_rows.iter().filter(|row| row["kind"] == "admission").collect();
    assert_eq!(admission_rows[0]["outcome"], "externalized");
    assert_eq!(admission_rows[1]["outcome"], "inline");

    let trace_path = run_dir.join("threads/main.h1.traces/1-1.md");
    assert!(trace_path.exists(), "threads/main.h1.traces/1-1.md missing on the host");
    let trace_contents = fs::read_to_string(&trace_path).unwrap();
    let sha_line = trace_contents.lines().find(|l| l.starts_with("sha256: ")).expect("sha256 frontmatter line");
    let sha = sha_line.strip_prefix("sha256: ").unwrap();
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let recomputed: String = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(sha, recomputed, "sha256 mismatch for big.txt");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut listed = String::new();
    poll(deadline, || {
        let output = StdCommand::new("docker")
            .args(["ps", "-a", "--filter", "label=arx.hand=h1", "--format", "{{.Names}}"])
            .output()
            .unwrap();
        listed = String::from_utf8_lossy(&output.stdout).trim().to_string();
        listed.is_empty()
    });
    assert!(listed.is_empty(), "still listed: {listed}");
}

#[test]
fn a_native_hand_survives_a_context_window_miss_and_finishes_the_brief() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let miss_body =
        r#"{"error":{"message":"This model's maximum context length is 8192 tokens. However, you requested 9000 tokens","type":"invalid_request_error"}}"#;
    let (base_url, state) = fake_openai_server(FakeScript::Sequence(vec![
        tool_call_step("call_1", "read_file", serde_json::json!({"path": "arx.toml"})),
        FakeScript::Status(400, miss_body.to_string()),
        end_turn_step("state object text"),
        end_turn_step("ARX-L2-13-OK"),
    ]));

    let config_path = run_dir.join("arx.toml");
    set_base_url(&config_path, &base_url);

    run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:hand-ds:say the word".to_string()],
        &[("ARX_TEST_FAKE_KEY", "test-key".to_string())],
    );

    let rows = log_rows(&run_dir);
    let verdict_row = rows
        .iter()
        .find(|row| row["kind"] == "turn" && row["role"] == "arx" && row["text"].as_str().map(|t| t.contains("ARX-L2-13-OK")).unwrap_or(false))
        .expect("verdict injection carrying ARX-L2-13-OK");
    assert!(verdict_row["text"].as_str().unwrap().contains("ARX-L2-13-OK"), "verdict row: {verdict_row:?}");
    assert!(!rows.iter().any(|row| row["kind"] == "miss"), "main's log rows: {rows:?}");

    let trace_rows = transcript_rows(&run_dir, "h1");
    let kinds: Vec<&str> = trace_rows
        .iter()
        .filter_map(|row| row["kind"].as_str())
        .filter(|kind| ["miss", "rewind", "state_object", "respawn"].contains(kind))
        .collect();
    assert_eq!(kinds, vec!["miss", "rewind", "state_object", "respawn"], "trace rows: {trace_rows:?}");

    let rewind_archive = run_dir.join("threads/main.h1/rewind-1.jsonl");
    assert!(rewind_archive.exists(), "rewind-1.jsonl missing from the trace directory");

    let trace_md = fs::read_to_string(run_dir.join("threads/main.h1/trace.md")).unwrap();
    assert!(trace_md.contains("rewind-1.jsonl"), "trace.md: {trace_md}");

    let requests = state.requests.lock().unwrap();
    assert_eq!(requests.len(), 4, "requests: {requests:?}");
    let fourth_body: serde_json::Value = serde_json::from_str(&requests[3].2).unwrap();
    let messages = fourth_body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3, "messages: {messages:?}");
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"], "say the word");
    assert_eq!(messages[2]["role"], "user");
    assert!(
        messages[2]["content"].as_str().unwrap().contains("state object text"),
        "object message: {:?}",
        messages[2]
    );
}
