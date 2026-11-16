#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use arx_engine::{ClaimHandle, ClaimKind, Engine, SaveIntent, Store};
use assert_cmd::cargo::cargo_bin;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

fn hash_fixture_dir(dir: &Path) -> String {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    entries.sort();
    let mut hasher = Sha256::new();
    for path in entries {
        hasher.update(path.file_name().unwrap().to_string_lossy().as_bytes());
        hasher.update(fs::read(&path).unwrap());
    }
    hasher.finalize().iter().take(6).map(|byte| format!("{byte:02x}")).collect()
}

pub fn claude_stub_tag() -> String {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-stub");
    format!("arx-claude-stub:{}", hash_fixture_dir(&fixture_dir))
}

pub fn codex_stub_tag() -> String {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex-stub");
    format!("arx-codex-stub:{}", hash_fixture_dir(&fixture_dir))
}

pub fn stub_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stub-sidecar")
}

pub fn stub_claude_bin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bin")
}

pub fn claude_hand_path() -> String {
    format!("{}:{}", stub_claude_bin_dir().display(), std::env::var("PATH").unwrap_or_default())
}

static CLAUDE_STUB_IMAGE_ONCE: std::sync::Once = std::sync::Once::new();

pub fn ensure_claude_stub_image() {
    CLAUDE_STUB_IMAGE_ONCE.call_once(|| {
        let tag = claude_stub_tag();
        let inspect = StdCommand::new("docker")
            .args(["image", "inspect", &tag])
            .output()
            .expect("docker image inspect");
        if inspect.status.success() {
            return;
        }
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-stub");
        let build = StdCommand::new("docker")
            .args(["build", "-q", "-t", &tag])
            .arg(&fixture_dir)
            .output()
            .expect("docker build arx-claude-stub");
        assert!(
            build.status.success(),
            "docker build of {tag} failed: {}",
            String::from_utf8_lossy(&build.stderr)
        );
    });
}

pub fn plant_stub_credential(workspace: &Path) -> PathBuf {
    let dir = workspace.join("claude-config");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(".credentials.json"),
        r#"{"claudeAiOauth":{"accessToken":"stub-token","refreshToken":"stub-refresh"},"mcpOAuth":{"other":{}}}"#,
    )
    .unwrap();
    dir
}

static CODEX_STUB_IMAGE_ONCE: std::sync::Once = std::sync::Once::new();

pub fn ensure_codex_stub_image() {
    CODEX_STUB_IMAGE_ONCE.call_once(|| {
        let tag = codex_stub_tag();
        let inspect = StdCommand::new("docker")
            .args(["image", "inspect", &tag])
            .output()
            .expect("docker image inspect");
        if inspect.status.success() {
            return;
        }
        let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex-stub");
        let build = StdCommand::new("docker")
            .args(["build", "-q", "-t", &tag])
            .arg(&fixture_dir)
            .output()
            .expect("docker build arx-codex-stub");
        assert!(
            build.status.success(),
            "docker build of {tag} failed: {}",
            String::from_utf8_lossy(&build.stderr)
        );
    });
}

pub fn plant_stub_codex_credential(workspace: &Path) -> PathBuf {
    let dir = workspace.join("codex-home");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("auth.json"),
        r#"{"auth_mode":"chatgpt","tokens":{"access_token":"stub","refresh_token":"stub","id_token":"stub","account_id":"stub"},"last_refresh":"2026-09-07T00:00:00Z"}"#,
    )
    .unwrap();
    dir
}

/// Repoints `[roles].reviewer` at `model`, adding a `[[models]]` row for it (or
/// patching an existing one's `image` in) so a dispatch of the reviewer actually
/// runs on `body`/`image` rather than the run fixture's own `gpt-5.6-sol`.
pub fn rewrite_reviewer_definition(workspace: &Path, body: &str, model: &str, image: &str) {
    let config_path = workspace.join("run/arx.toml");
    let contents = fs::read_to_string(&config_path).unwrap();

    let entry_marker = format!("id = \"{model}\"\nbody = \"{body}\"\n");
    let updated = if contents.contains(&entry_marker) {
        let patched_entry = format!("{entry_marker}image = \"{image}\"\n");
        contents.replacen(&entry_marker, &patched_entry, 1)
    } else {
        format!("{contents}\n[[models]]\nid = \"{model}\"\nbody = \"{body}\"\nimage = \"{image}\"\n")
    };

    let role_needle = "reviewer = \"gpt-5.6-sol\"\n";
    let updated = if updated.contains(role_needle) {
        updated.replacen(role_needle, &format!("reviewer = \"{model}\"\n"), 1)
    } else {
        updated
    };
    fs::write(&config_path, updated).unwrap();
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn copy_run_fixture(fixtures: &Path, workspace: &Path) {
    copy_dir_recursive(&fixtures.join("run"), &workspace.join("run"));
    copy_dir_recursive(&fixtures.join("stub-sidecar"), &workspace.join("stub-sidecar"));
    let config_path = workspace.join("run/arx.toml");
    let contents = fs::read_to_string(&config_path).unwrap();
    let contents = contents
        .replace("arx-claude-stub:test", &claude_stub_tag())
        .replace("arx-codex-stub:test", &codex_stub_tag());
    fs::write(&config_path, contents).unwrap();
}

pub fn fresh_run_fixture() -> tempfile::TempDir {
    fresh_run_fixture_with_gate(None)
}

pub fn fresh_run_fixture_with_gate(gate_toml: Option<&str>) -> tempfile::TempDir {
    let workspace = tempdir().unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    copy_run_fixture(&fixtures, workspace.path());
    if let Some(gate_toml) = gate_toml {
        let config_path = workspace.path().join("run/arx.toml");
        let mut contents = fs::read_to_string(&config_path).unwrap();
        contents.push_str(gate_toml);
        fs::write(&config_path, contents).unwrap();
    }
    let run_dir = workspace.path().join("run");
    let git = |args: &[&str]| {
        let status = StdCommand::new("git").arg("-C").arg(&run_dir).args(args).status().unwrap();
        assert!(status.success(), "git {args:?} failed in {}", run_dir.display());
    };
    git(&["init", "-q"]);
    git(&["-c", "user.name=arx", "-c", "user.email=arx@localhost", "commit", "-q", "--allow-empty", "-m", "init"]);
    workspace
}

pub fn fresh_run_fixture_with_store_scope(scope: &str) -> tempfile::TempDir {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let mut contents = fs::read_to_string(&config_path).unwrap();
    contents.push_str(&format!("\n[store]\nscope = \"{scope}\"\n"));
    fs::write(&config_path, contents).unwrap();
    fs::create_dir_all(workspace.path().join("run/store").join(scope)).unwrap();
    workspace
}

pub fn fresh_run_fixture_without_git() -> tempfile::TempDir {
    let workspace = tempdir().unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    copy_run_fixture(&fixtures, workspace.path());
    workspace
}

/// Mirrors `tests/fixtures/run/arx.toml`'s registry and `[roles]` for a
/// from-scratch workspace that copies only `run/agents/` and writes its own
/// `[main]`/`[store]`/`sidecar`.
pub fn run_fixture_toml(sidecar: &str, store_scope: &str, store_root: Option<&Path>) -> String {
    let store_root_line = store_root.map(|root| format!("root = \"{}\"\n", root.display())).unwrap_or_default();
    let claude_tag = claude_stub_tag();
    let codex_tag = codex_stub_tag();
    format!(
        "sidecar = \"{sidecar}\"\n\n[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"stub-model\"\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\nimage = \"{claude_tag}\"\n\n[[models]]\nid = \"gpt-5.6-sol\"\nbody = \"codex\"\nimage = \"{codex_tag}\"\n\n[[models]]\nid = \"fake-model\"\nbody = \"native\"\nprovider = \"fake\"\n\n[[models]]\nid = \"fake-model-anthropic\"\nbody = \"native\"\nprovider = \"stubbed\"\n\n[roles]\nmain = \"stub-model\"\nrunner = \"gpt-5.6-sol\"\nreviewer = \"gpt-5.6-sol\"\ndrain = \"gpt-5.6-sol\"\nhand-an = \"fake-model-anthropic\"\nhand-cc = \"claude-sonnet-5\"\nhand-cw = \"claude-sonnet-5\"\nhand-cx = \"gpt-5.6-sol\"\nhand-cxw = \"gpt-5.6-sol\"\nhand-ds = \"fake-model\"\nhand-ex = \"claude-sonnet-5\"\n\n[store]\nscope = \"{store_scope}\"\n{store_root_line}\n[providers.fake]\nkind = \"openai-compatible\"\nbase_url = \"http://127.0.0.1:0\"\napi_key_env = \"ARX_TEST_FAKE_KEY\"\n\n[providers.stubbed]\nkind = \"anthropic\"\nbase_url = \"http://127.0.0.1:0\"\napi_key_env = \"ARX_TEST_FAKE_KEY\"\n"
    )
}

pub fn native_run_fixture(base_url: &str) -> tempfile::TempDir {
    let workspace = fresh_run_fixture();
    let config_path = workspace.path().join("run/arx.toml");
    let contents = fs::read_to_string(&config_path).unwrap();
    let updated = contents.replace("http://127.0.0.1:0", base_url);
    fs::write(&config_path, updated).unwrap();
    workspace
}

pub enum FakeScript {
    Sse(Vec<String>),
    Status(u16, String),
    StatusWithRetryAfter(u16, String, u64),
    Stall { first: String },
    Sequence(Vec<FakeScript>),
}

#[derive(Default)]
pub struct FakeState {
    pub requests: Mutex<Vec<(String, String, String)>>,
    pub headers: Mutex<Vec<Vec<(String, String)>>>,
    pub disconnected: AtomicBool,
    pub connections: AtomicUsize,
}

fn read_fake_request(stream: &TcpStream) -> Option<(String, Option<String>, String, Vec<(String, String)>)> {
    let mut reader = std::io::BufReader::new(stream.try_clone().ok()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;
    let path = request_line.split_whitespace().nth(1)?.to_string();

    let mut content_length: usize = 0;
    let mut authorization: Option<String> = None;
    let mut captured_headers: Vec<(String, String)> = Vec::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            let name_lower = name.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if name_lower == "content-length" {
                content_length = value.parse().unwrap_or(0);
            } else if name_lower == "authorization" {
                authorization = Some(value.clone());
            }
            if name_lower == "x-api-key" || name_lower == "anthropic-version" {
                captured_headers.push((name_lower, value));
            }
        }
    }

    let mut body_buf = vec![0u8; content_length];
    reader.read_exact(&mut body_buf).ok()?;
    Some((path, authorization, String::from_utf8_lossy(&body_buf).into_owned(), captured_headers))
}

fn status_reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        401 => "Unauthorized",
        _ => "Error",
    }
}

pub fn handle_fake_connection(stream: TcpStream, state: &FakeState, script: &FakeScript) {
    handle_fake_connection_at(stream, state, script, 0);
}

fn handle_fake_connection_at(mut stream: TcpStream, state: &FakeState, script: &FakeScript, connection_index: usize) {
    if let FakeScript::Sequence(scripts) = script {
        match scripts.get(connection_index) {
            Some(step) => handle_fake_connection_at(stream, state, step, connection_index),
            None => {
                let _ = read_fake_request(&stream);
                let response =
                    "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(response.as_bytes());
            }
        }
        return;
    }

    let Some((path, authorization, body, extra_headers)) = read_fake_request(&stream) else {
        return;
    };
    state
        .requests
        .lock()
        .unwrap()
        .push((path, authorization.unwrap_or_default(), body));
    state.headers.lock().unwrap().push(extra_headers);

    match script {
        FakeScript::Sequence(_) => unreachable!("handled above"),
        FakeScript::Sse(events) => {
            if stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n")
                .is_err()
            {
                state.disconnected.store(true, Ordering::SeqCst);
                return;
            }
            for event in events {
                if stream.write_all(format!("data: {event}\n\n").as_bytes()).is_err() {
                    state.disconnected.store(true, Ordering::SeqCst);
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            let _ = stream.write_all(b"data: [DONE]\n\n");
        }
        FakeScript::Status(code, body) => {
            let response = format!(
                "HTTP/1.1 {code} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                status_reason(*code),
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
        FakeScript::StatusWithRetryAfter(code, body, retry_after) => {
            let response = format!(
                "HTTP/1.1 {code} {}\r\nContent-Type: application/json\r\nRetry-After: {retry_after}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                status_reason(*code),
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
        FakeScript::Stall { first } => {
            if stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n")
                .is_err()
            {
                state.disconnected.store(true, Ordering::SeqCst);
                return;
            }
            if stream.write_all(format!("data: {first}\n\n").as_bytes()).is_err() {
                state.disconnected.store(true, Ordering::SeqCst);
                return;
            }
            let deadline = Instant::now() + Duration::from_secs(30);
            while Instant::now() < deadline {
                thread::sleep(Duration::from_millis(100));
                if stream.write_all(b": keep-alive\n\n").is_err() {
                    state.disconnected.store(true, Ordering::SeqCst);
                    return;
                }
            }
        }
    }
}

fn spawn_fake_server(script: FakeScript) -> (String, Arc<FakeState>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let state = Arc::new(FakeState::default());
    let state_for_thread = Arc::clone(&state);
    let script = Arc::new(script);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let state = Arc::clone(&state_for_thread);
            let script = Arc::clone(&script);
            let connection_index = state.connections.fetch_add(1, Ordering::SeqCst);
            thread::spawn(move || {
                handle_fake_connection_at(stream, &state, &script, connection_index);
            });
        }
    });
    (format!("http://{addr}"), state)
}

pub fn fake_openai_server(script: FakeScript) -> (String, Arc<FakeState>) {
    spawn_fake_server(script)
}

pub fn fake_anthropic_server(script: FakeScript) -> (String, Arc<FakeState>) {
    spawn_fake_server(script)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub card: String,
    pub operator: Vec<String>,
    pub expect: Vec<Expect>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub verdict: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub contains: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub hand: Option<String>,
    #[serde(default)]
    pub by: Option<String>,
}

pub fn load_script(path: &Path) -> Script {
    let contents = fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read script {}: {err}", path.display()));
    serde_json::from_str(&contents).unwrap_or_else(|err| panic!("failed to parse script {}: {err}", path.display()))
}

pub fn migrate_retired_judge_dispatch(script: &mut Script) {
    for line in &mut script.operator {
        *line = line
            .replace("hand:dispatch:judge:", "hand:dispatch:reviewer:")
            .replace("Judge seeded.diff", "Reviewer seeded.diff");
    }
    for expect in &mut script.expect {
        expect.name = expect.name.replace("judge", "reviewer");
        expect.after = expect.after.take().map(|value| value.replace("judge", "reviewer"));
        expect.contains = expect
            .contains
            .take()
            .map(|value| value.replace("hand:dispatch:judge:", "hand:dispatch:reviewer:"));
        if expect.role.as_deref() == Some("judge") {
            expect.role = Some("reviewer".to_string());
        }
    }
}

pub fn check_script(rows: &[serde_json::Value], script: &Script) -> Result<(), Vec<String>> {
    let mut name_position: HashMap<&str, usize> = HashMap::new();
    let mut validation_failures = Vec::new();
    for (position, expect) in script.expect.iter().enumerate() {
        if name_position.insert(&expect.name, position).is_some() {
            validation_failures.push(format!("{}: duplicate predicate name", expect.name));
        }
    }
    for (position, expect) in script.expect.iter().enumerate() {
        if let Some(after_name) = &expect.after {
            match name_position.get(after_name.as_str()) {
                None => {
                    validation_failures.push(format!("{}: after names unknown predicate {after_name}", expect.name));
                }
                Some(&after_position) if after_position >= position => {
                    validation_failures
                        .push(format!("{}: after names {after_name}, which is not earlier in the script", expect.name));
                }
                _ => {}
            }
        }
    }
    if !validation_failures.is_empty() {
        return Err(validation_failures);
    }

    let mut bound: HashMap<&str, usize> = HashMap::new();
    let mut used = vec![false; rows.len()];
    let mut failures = Vec::new();

    for expect in &script.expect {
        let candidate = rows.iter().enumerate().find(|(i, row)| {
            if used[*i] {
                return false;
            }
            if row["kind"].as_str() != Some(expect.kind.as_str()) {
                return false;
            }
            if let Some(tool) = &expect.tool {
                let row_tool = row["tool"].as_str().unwrap_or("");
                if row_tool != tool.as_str() && !row_tool.ends_with(&format!("__{tool}")) {
                    return false;
                }
            }
            if let Some(verdict) = &expect.verdict {
                if row["verdict"].as_str() != Some(verdict.as_str()) {
                    return false;
                }
            }
            if let Some(role) = &expect.role {
                if row["role"].as_str() != Some(role.as_str()) {
                    return false;
                }
            }
            if let Some(contains) = &expect.contains {
                let text = row["text"].as_str().unwrap_or("");
                if !text.contains(contains.as_str()) {
                    return false;
                }
            }
            if let Some(hand) = &expect.hand {
                if row["hand"].as_str() != Some(hand.as_str()) {
                    return false;
                }
            }
            if let Some(by) = &expect.by {
                if row["by"].as_str() != Some(by.as_str()) {
                    return false;
                }
            }
            true
        });

        match candidate {
            None => {
                failures.push(format!("{}: no unbound row matches", expect.name));
            }
            Some((i, _)) => {
                used[i] = true;
                bound.insert(&expect.name, i);
                if let Some(after_name) = &expect.after {
                    if let Some(&j) = bound.get(after_name.as_str()) {
                        if i <= j {
                            failures.push(format!("{}: bound at {i}, not after {after_name} at {j}", expect.name));
                        }
                    }
                }
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

const SESSION_DEADLINE: Duration = Duration::from_secs(5);

fn descendant_pids(root: u32) -> Vec<u32> {
    let output = match StdCommand::new("ps").args(["-eo", "pid=,ppid="]).output() {
        Ok(output) => output,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let pid = parts.next().and_then(|p| p.parse::<u32>().ok());
        let ppid = parts.next().and_then(|p| p.parse::<u32>().ok());
        if let (Some(pid), Some(ppid)) = (pid, ppid) {
            children.entry(ppid).or_default().push(pid);
        }
    }
    let mut descendants = Vec::new();
    let mut frontier = vec![root];
    while let Some(pid) = frontier.pop() {
        if let Some(kids) = children.get(&pid) {
            for &kid in kids {
                descendants.push(kid);
                frontier.push(kid);
            }
        }
    }
    descendants
}

fn kill_descendants(pids: &[u32]) {
    for &pid in pids {
        let _ = StdCommand::new("kill").args(["-KILL", &pid.to_string()]).status();
    }
}

fn run_session_inner(
    workspace: &Path,
    config: &str,
    lines: &[String],
    env: &[(&str, String)],
) -> (Vec<serde_json::Value>, String, String) {
    run_session_inner_with_deadline(workspace, config, lines, env, SESSION_DEADLINE)
}

fn run_session_inner_with_deadline(
    workspace: &Path,
    config: &str,
    lines: &[String],
    env: &[(&str, String)],
    session_deadline: Duration,
) -> (Vec<serde_json::Value>, String, String) {
    let mut cmd = StdCommand::new(cargo_bin("arx"));
    cmd.args(["run", config])
        .current_dir(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut path_overridden = false;
    let mut user_gate_overridden = false;
    let mut roster_dir_overridden = false;
    for (key, value) in env {
        if *key == "PATH" {
            path_overridden = true;
        }
        if *key == "ARX_USER_GATE" {
            user_gate_overridden = true;
        }
        if *key == "ARX_ROSTER_DIR" {
            roster_dir_overridden = true;
        }
        cmd.env(key, value);
    }
    if !path_overridden {
        cmd.env("PATH", claude_hand_path());
    }
    if !user_gate_overridden {
        cmd.env("ARX_USER_GATE", workspace.join("user-gate.toml"));
    }
    if !roster_dir_overridden {
        cmd.env("ARX_ROSTER_DIR", workspace.join("roster"));
    }

    let mut child = cmd.spawn().unwrap();
    let root_pid = child.id();
    let deadline = Instant::now() + session_deadline;

    let mut stdout = child.stdout.take().unwrap();
    let stdout_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let mut stderr = child.stderr.take().unwrap();
    let stderr_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });

    let mut stdin = child.stdin.take().unwrap();
    let mut input = lines.join("\n");
    input.push('\n');
    thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });

    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let descendants = descendant_pids(root_pid);
            let _ = child.kill();
            let _ = child.wait();
            kill_descendants(&descendants);
            panic!("arx run over {config} in {} did not exit within {session_deadline:?}", workspace.display());
        }
        thread::sleep(Duration::from_millis(20));
    };

    let stdout_bytes = stdout_handle.join().unwrap();
    let stderr_bytes = stderr_handle.join().unwrap();

    assert!(
        status.success(),
        "arx run failed: stdout={} stderr={}",
        String::from_utf8_lossy(&stdout_bytes),
        String::from_utf8_lossy(&stderr_bytes)
    );

    let log_path = workspace.join("threads/main.jsonl");
    let contents = fs::read_to_string(&log_path).unwrap();
    let rows = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    (
        rows,
        String::from_utf8_lossy(&stdout_bytes).into_owned(),
        String::from_utf8_lossy(&stderr_bytes).into_owned(),
    )
}

pub fn run_session(workspace: &Path, config: &str, lines: &[String], env: &[(&str, String)]) -> Vec<serde_json::Value> {
    run_session_inner(workspace, config, lines, env).0
}

pub fn run_session_with_stdout(workspace: &Path, config: &str, lines: &[String], env: &[(&str, String)]) -> (Vec<serde_json::Value>, String) {
    let (rows, stdout, _) = run_session_inner(workspace, config, lines, env);
    (rows, stdout)
}

pub fn run_session_with_output(
    workspace: &Path,
    config: &str,
    lines: &[String],
    env: &[(&str, String)],
) -> (Vec<serde_json::Value>, String, String) {
    run_session_inner(workspace, config, lines, env)
}

pub fn run_session_expect_failure_with_stdout(
    workspace: &Path,
    config: &str,
    lines: &[String],
    env: &[(&str, String)],
) -> (i32, String, String) {
    let mut cmd = StdCommand::new(cargo_bin("arx"));
    cmd.args(["run", config])
        .current_dir(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut path_overridden = false;
    let mut user_gate_overridden = false;
    let mut roster_dir_overridden = false;
    for (key, value) in env {
        if *key == "PATH" {
            path_overridden = true;
        }
        if *key == "ARX_USER_GATE" {
            user_gate_overridden = true;
        }
        if *key == "ARX_ROSTER_DIR" {
            roster_dir_overridden = true;
        }
        cmd.env(key, value);
    }
    if !path_overridden {
        cmd.env("PATH", claude_hand_path());
    }
    if !user_gate_overridden {
        cmd.env("ARX_USER_GATE", workspace.join("user-gate.toml"));
    }
    if !roster_dir_overridden {
        cmd.env("ARX_ROSTER_DIR", workspace.join("roster"));
    }

    let mut child = cmd.spawn().unwrap();
    let root_pid = child.id();
    let deadline = Instant::now() + SESSION_DEADLINE;

    let mut stdout = child.stdout.take().unwrap();
    let stdout_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let mut stderr = child.stderr.take().unwrap();
    let stderr_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });

    let mut stdin = child.stdin.take().unwrap();
    let mut input = lines.join("\n");
    input.push('\n');
    thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });

    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let descendants = descendant_pids(root_pid);
            let _ = child.kill();
            let _ = child.wait();
            kill_descendants(&descendants);
            panic!("arx run over {config} in {} did not exit within {SESSION_DEADLINE:?}", workspace.display());
        }
        thread::sleep(Duration::from_millis(20));
    };

    let stdout_bytes = stdout_handle.join().unwrap();
    let stderr_bytes = stderr_handle.join().unwrap();

    (
        status.code().unwrap_or(-1),
        String::from_utf8_lossy(&stdout_bytes).into_owned(),
        String::from_utf8_lossy(&stderr_bytes).into_owned(),
    )
}

pub fn run_session_expect_failure(workspace: &Path, config: &str, lines: &[String], env: &[(&str, String)]) -> (i32, String) {
    let (code, _, stderr) = run_session_expect_failure_with_stdout(workspace, config, lines, env);
    (code, stderr)
}

pub fn run_session_with_deadline(
    workspace: &Path,
    config: &str,
    lines: &[String],
    env: &[(&str, String)],
    deadline: Duration,
) -> Vec<serde_json::Value> {
    run_session_inner_with_deadline(workspace, config, lines, env, deadline).0
}

pub fn head_count(root: &Path) -> u64 {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-list")
        .arg("--count")
        .arg("HEAD")
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().parse().unwrap()
}

pub fn scope_md_files(scope_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(scope_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

pub fn with_main_checkpoint(config_path: &Path, delta: u64) {
    let contents = fs::read_to_string(config_path).unwrap();
    let needle = "body = \"claude-code\"\n";
    assert!(contents.contains(needle), "config: {contents}");
    let updated = contents.replacen(needle, &format!("{needle}checkpoint = {delta}\n"), 1);
    fs::write(config_path, updated).unwrap();
}

/// `main.models` is retired; every registry id is available to every
/// position's `/model`. This just makes sure each named id has a `[[models]]`
/// row (skipping ones already registered), so tests can still switch main
/// onto it by name.
pub fn with_main_models(config_path: &Path, models: &[&str]) {
    let contents = fs::read_to_string(config_path).unwrap();
    let mut addition = String::new();
    for model in models {
        let marker = format!("id = \"{model}\"");
        if !contents.contains(&marker) {
            addition.push_str(&format!("\n[[models]]\nid = \"{model}\"\nbody = \"claude-code\"\n"));
        }
    }
    if addition.is_empty() {
        return;
    }
    fs::write(config_path, format!("{contents}{addition}")).unwrap();
}

pub fn with_main_window_and_reserve(config_path: &Path, window: u64, reserve: u64) {
    let contents = fs::read_to_string(config_path).unwrap();
    if contents.contains("window =") || contents.contains("reserve =") {
        return;
    }
    let needle = "body = \"claude-code\"\n";
    assert!(contents.contains(needle), "config: {contents}");
    let updated = contents.replacen(needle, &format!("{needle}window = {window}\nreserve = {reserve}\n"), 1);
    fs::write(config_path, updated).unwrap();
}

pub fn plant_checkpoint_replies(workspace: &Path, author: &str, author2: Option<&str>, truth: Option<&str>) {
    fs::write(workspace.join("checkpoint-author.md"), author).unwrap();
    if let Some(author2) = author2 {
        fs::write(workspace.join("checkpoint-author-2.md"), author2).unwrap();
    }
    if let Some(truth) = truth {
        fs::write(workspace.join("checkpoint-truth.md"), truth).unwrap();
    }
}

pub fn hash_of(path: &Path) -> String {
    let output = StdCommand::new("git").arg("hash-object").arg(path).output().unwrap();
    assert!(
        output.status.success(),
        "git hash-object {} failed: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub struct PlantedScope {
    pub hot_id: String,
    pub plain_id: String,
    pub ember_id: String,
}

pub fn plant_engine_scope(root: &Path, scope: &str) -> PlantedScope {
    fs::create_dir_all(root.join(scope)).unwrap();
    let mut engine = Engine::open(root, "seed").unwrap();
    let handle = ClaimHandle {
        thread: "seed".to_string(),
        turn: 1,
    };

    engine
        .save(
            &SaveIntent {
                scope: scope.to_string(),
                kind: ClaimKind::Fact,
                question: "what is the launch codename".to_string(),
                answer: "skylark".to_string(),
                why: "ops handoff".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();
    engine
        .save(
            &SaveIntent {
                scope: scope.to_string(),
                kind: ClaimKind::Fact,
                question: "what is the fallback contact".to_string(),
                answer: "orion desk".to_string(),
                why: "ops handoff".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();
    engine.drain().unwrap();

    let hot_id = engine.recall(scope, "launch codename", 1).unwrap()[0].id.clone();
    let plain_id = engine.recall(scope, "fallback contact", 1).unwrap()[0].id.clone();

    let claim_path = root.join(scope).join(format!("{hot_id}.md"));
    let text = fs::read_to_string(&claim_path).unwrap();
    let edited = text.replacen("kind: fact\n", "kind: fact\nlabel: hot\n", 1);
    assert_ne!(text, edited, "expected to find 'kind: fact' to rewrite");
    fs::write(&claim_path, edited).unwrap();

    let mut engine = Engine::open(root, "seed").unwrap();
    let ember = engine.ember(scope, "keep replies short", &handle).unwrap();

    PlantedScope {
        hot_id,
        plain_id,
        ember_id: ember.id,
    }
}

pub fn transcript_rows(run_dir: &Path) -> Vec<serde_json::Value> {
    let contents = fs::read_to_string(run_dir.join("threads/main.jsonl")).unwrap_or_default();
    contents.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

pub fn poll_deadline<F: FnMut() -> bool>(deadline: Instant, mut check: F) -> bool {
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

pub struct LiveSession {
    child: std::process::Child,
}

impl LiveSession {
    pub fn spawn(run_dir: &Path, config: &str, env: &[(&str, String)]) -> LiveSession {
        let mut cmd = StdCommand::new(cargo_bin("arx"));
        cmd.args(["run", config])
            .current_dir(run_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut path_overridden = false;
        let mut user_gate_overridden = false;
        let mut roster_dir_overridden = false;
        for (key, value) in env {
            if *key == "PATH" {
                path_overridden = true;
            }
            if *key == "ARX_USER_GATE" {
                user_gate_overridden = true;
            }
            if *key == "ARX_ROSTER_DIR" {
                roster_dir_overridden = true;
            }
            cmd.env(key, value);
        }
        if !path_overridden {
            cmd.env("PATH", claude_hand_path());
        }
        if !user_gate_overridden {
            cmd.env("ARX_USER_GATE", run_dir.join("user-gate.toml"));
        }
        if !roster_dir_overridden {
            cmd.env("ARX_ROSTER_DIR", run_dir.join("roster"));
        }
        let child = cmd.spawn().unwrap();
        LiveSession { child }
    }

    pub fn write_line(&mut self, line: &str) {
        let stdin = self.child.stdin.as_mut().expect("live session stdin");
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    pub fn close_stdin(&mut self) {
        self.child.stdin.take();
    }

    pub fn drain_output(&mut self) {
        let mut stdout = self.child.stdout.take().expect("live session stdout");
        let mut stderr = self.child.stderr.take().expect("live session stderr");
        thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = stdout.read_to_end(&mut sink);
        });
        thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = stderr.read_to_end(&mut sink);
        });
    }

    pub fn wait_success(&mut self, deadline: Instant) -> bool {
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.success();
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                return false;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

pub fn wait_for_row(
    session: &mut LiveSession,
    run_dir: &Path,
    deadline: Instant,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> Option<serde_json::Value> {
    loop {
        if let Some(row) = transcript_rows(run_dir).into_iter().find(&predicate) {
            return Some(row);
        }
        if Instant::now() >= deadline {
            return None;
        }
        session.write_line("waiting");
        thread::sleep(Duration::from_millis(200));
    }
}
