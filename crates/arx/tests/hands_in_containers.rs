use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command as StdCommand, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use arx_engine::{ClaimHandle, ClaimKind, Engine, SaveIntent, Store};
use assert_cmd::cargo::cargo_bin;
use sha2::{Digest, Sha256};

mod common;
use common::*;

static DOCKER_STATE_SERIAL: Mutex<()> = Mutex::new(());

fn serialize_on_docker_state() -> std::sync::MutexGuard<'static, ()> {
    DOCKER_STATE_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn ledger_rows(workspace_run: &Path) -> Vec<serde_json::Value> {
    let contents = fs::read_to_string(workspace_run.join("threads/main.ledger.jsonl")).unwrap_or_default();
    contents.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

fn transcript_rows(workspace_run: &Path) -> Vec<serde_json::Value> {
    let contents = fs::read_to_string(workspace_run.join("threads/main.jsonl")).unwrap_or_default();
    contents.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

fn arx_verdict_text(rows: &[serde_json::Value], hand: &str) -> Option<String> {
    rows.iter()
        .find(|row| {
            row["kind"] == "turn"
                && row["role"] == "arx"
                && row["text"].as_str().map(|t| t.starts_with(&format!("[hand {hand} verdict]"))).unwrap_or(false)
        })
        .and_then(|row| row["text"].as_str())
        .map(|s| s.to_string())
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

fn spawn_row_root(workspace_run: &Path, hand: &str) -> Option<String> {
    ledger_rows(workspace_run)
        .into_iter()
        .find(|row| row["kind"] == "spawn" && row["hand"] == hand)
        .and_then(|row| row["root"].as_str().map(|s| s.to_string()))
}

struct RunningSession {
    child: std::process::Child,
}

impl RunningSession {
    fn spawn(run_dir: &Path) -> RunningSession {
        let child = StdCommand::new(cargo_bin("arx"))
            .args(["run", "arx.toml"])
            .current_dir(run_dir)
            .env("PATH", claude_hand_path())
            .env("ARX_USER_GATE", run_dir.join("user-gate.toml"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        RunningSession { child }
    }

    fn write_line_and_close_stdin(&mut self, line: &str) {
        let mut stdin = self.child.stdin.take().unwrap();
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    fn drain_output(&mut self) {
        let mut stdout = self.child.stdout.take().unwrap();
        let mut stderr = self.child.stderr.take().unwrap();
        thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = stdout.read_to_end(&mut sink);
        });
        thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = stderr.read_to_end(&mut sink);
        });
    }

    fn wait_success(&mut self, deadline: Instant) -> bool {
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

#[test]
fn a_container_hand_sees_no_host_env_and_an_empty_home() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let rows = run_session(
        &run_dir,
        "arx.toml",
        &["hand:dispatch:fake:[probe]".to_string()],
        &[("ARX_CANARY", "1".to_string())],
    );

    let verdict = arx_verdict_text(&rows, "h1").expect("verdict injection for h1");
    assert!(verdict.contains("home=/hand/home"), "verdict: {verdict}");
    assert!(verdict.contains("home_entries=0"), "verdict: {verdict}");
    assert!(verdict.contains("cwd=/hand/repo"), "verdict: {verdict}");
    assert!(verdict.contains("env="), "verdict: {verdict}");
    let env_part = verdict.split("env=").nth(1).unwrap().split(' ').next().unwrap();
    let names: Vec<&str> = env_part.split(',').collect();
    assert!(names.contains(&"HOME"), "names: {names:?}");
    assert!(names.contains(&"HOSTNAME"), "names: {names:?}");
    assert!(names.contains(&"PATH"), "names: {names:?}");
    assert!(!names.contains(&"ARX_CANARY"), "names: {names:?}");
    assert!(!verdict.contains("ARX_CANARY"), "verdict: {verdict}");
}

#[test]
fn a_write_enabled_hand_writes_in_its_own_worktree_and_nowhere_else() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let head = StdCommand::new("git").arg("-C").arg(&run_dir).arg("rev-parse").arg("HEAD").output().unwrap();
    let head_sha = String::from_utf8_lossy(&head.stdout).trim().to_string();

    let mut session = RunningSession::spawn(&run_dir);
    session.write_line_and_close_stdin("hand:dispatch:fake-write:[write:hello.txt=from-the-hand][delay:4000]");
    session.drain_output();

    let spawn_deadline = Instant::now() + Duration::from_secs(3);
    let mut root: Option<String> = None;
    poll(spawn_deadline, || {
        root = spawn_row_root(&run_dir, "h1");
        root.is_some()
    });
    let root = root.expect("spawn row with root within 3s");
    let root_path = Path::new(&root);

    let write_deadline = Instant::now() + Duration::from_secs(3);
    let wrote = poll(write_deadline, || root_path.join("worktree/hello.txt").exists());
    assert!(wrote, "hello.txt did not appear under the worktree within 3s");
    assert!(!run_dir.join("hello.txt").exists(), "the write leaked into the live tree");

    let exit_deadline = Instant::now() + Duration::from_secs(15);
    assert!(session.wait_success(exit_deadline), "arx run did not exit successfully");

    assert!(!root_path.exists(), "hand root still present after teardown: {}", root_path.display());

    let worktree_list = StdCommand::new("git").arg("-C").arg(&run_dir).arg("worktree").arg("list").output().unwrap();
    let list_text = String::from_utf8_lossy(&worktree_list.stdout);
    assert_eq!(list_text.lines().count(), 1, "worktree list: {list_text}");

    let containers = StdCommand::new("docker")
        .args(["ps", "-a", "--filter", "label=arx.hand", "--filter", "name=arx-h1-", "--format", "{{.Names}}"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&containers.stdout).trim().is_empty(), "container still listed");

    let trace_dir = run_dir.join("threads/main.h1");
    let trace_md = fs::read_to_string(trace_dir.join("trace.md")).unwrap();
    assert!(trace_md.contains("exit: 0"), "trace.md: {trace_md}");
    assert!(trace_md.contains("write: true"), "trace.md: {trace_md}");
    assert!(trace_md.contains(&format!("base: {head_sha}")), "trace.md: {trace_md}");
    assert!(trace_md.contains("path: worktree.diff"), "trace.md: {trace_md}");

    let diff_path = trace_dir.join("worktree.diff");
    let diff_bytes = fs::metadata(&diff_path).unwrap().len();
    assert!(diff_bytes > 0, "worktree.diff is empty");

    let apply = StdCommand::new("git").arg("-C").arg(&run_dir).arg("apply").arg(&diff_path).output().unwrap();
    assert!(apply.status.success(), "git apply failed: {}", String::from_utf8_lossy(&apply.stderr));
    let applied = fs::read_to_string(run_dir.join("hello.txt")).unwrap();
    assert_eq!(applied, "from-the-hand\n");
}

#[test]
fn a_killed_write_hand_still_leaves_an_applicable_diff() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let mut session = RunningSession::spawn(&run_dir);
    {
        let mut stdin = session.child.stdin.take().unwrap();
        stdin.write_all(b"hand:dispatch:fake-write:[write:late.txt=still-here][delay:60000]\n").unwrap();
        stdin.write_all(b"hand:kill:h1\n").unwrap();
    }
    session.drain_output();

    let spawn_deadline = Instant::now() + Duration::from_secs(3);
    let mut root: Option<String> = None;
    poll(spawn_deadline, || {
        root = spawn_row_root(&run_dir, "h1");
        root.is_some()
    });
    let root = root.expect("spawn row with root within 3s");
    let root_path = Path::new(&root);

    let write_deadline = Instant::now() + Duration::from_secs(3);
    let wrote = poll(write_deadline, || root_path.join("worktree/late.txt").exists());
    assert!(wrote, "late.txt did not appear under the worktree within 3s");

    let exit_deadline = Instant::now() + Duration::from_secs(15);
    assert!(session.wait_success(exit_deadline), "arx run did not exit successfully");

    let rows = transcript_rows(&run_dir);
    let killed_row = rows.iter().find(|row| {
        row["kind"] == "turn"
            && row["role"] == "arx"
            && row["text"].as_str() == Some("[hand h1 killed] exit: killed (trace: threads/main.h1)")
    });
    assert!(killed_row.is_some(), "rows: {rows:?}");

    assert!(!root_path.exists(), "hand root still present after teardown");

    let containers = StdCommand::new("docker")
        .args(["ps", "-a", "--filter", "label=arx.hand", "--filter", "name=arx-h1-", "--format", "{{.Names}}"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&containers.stdout).trim().is_empty(), "container still listed");

    let trace_dir = run_dir.join("threads/main.h1");
    let trace_md = fs::read_to_string(trace_dir.join("trace.md")).unwrap();
    assert!(trace_md.contains("exit: killed"), "trace.md: {trace_md}");

    let diff_path = trace_dir.join("worktree.diff");
    let apply = StdCommand::new("git").arg("-C").arg(&run_dir).arg("apply").arg(&diff_path).output().unwrap();
    assert!(apply.status.success(), "git apply failed: {}", String::from_utf8_lossy(&apply.stderr));
    let applied = fs::read_to_string(run_dir.join("late.txt")).unwrap();
    assert_eq!(applied, "still-here\n");

    let ledger = ledger_rows(&run_dir);
    let killed_ledger_row = ledger.iter().find(|row| row["kind"] == "killed" && row["hand"] == "h1");
    assert!(killed_ledger_row.is_some(), "ledger rows: {ledger:?}");
    assert_eq!(killed_ledger_row.unwrap()["trace"], "threads/main.h1");
}

#[test]
fn the_store_path_is_not_readable_from_a_container() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");

    let scope_dir = run_dir.join("store/testscope");
    fs::create_dir_all(&scope_dir).unwrap();
    fs::write(scope_dir.join("cnd0000001.md"), "---\nkind: fact\n---\nplaceholder\n").unwrap();
    let claim_name = fs::read_dir(&scope_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name()
        .to_string_lossy()
        .into_owned();
    let claim_path = format!("store/testscope/{claim_name}");

    let lines = vec![
        format!("hand:dispatch:fake:[read:{claim_path}]"),
        format!("hand:dispatch:fake-write:[read:{claim_path}]"),
        "hand:dispatch:fake:[read:store]".to_string(),
        "hand:dispatch:fake-write:[read:store]".to_string(),
        "hand:dispatch:fake:[read:arx.toml]".to_string(),
    ];

    let rows = run_session(&run_dir, "arx.toml", &lines, &[]);

    let v1 = arx_verdict_text(&rows, "h1").expect("h1 verdict");
    assert!(v1.contains(&format!("read:{claim_path}=fail")), "v1: {v1}");
    let v2 = arx_verdict_text(&rows, "h2").expect("h2 verdict");
    assert!(v2.contains(&format!("read:{claim_path}=fail")), "v2: {v2}");
    let v3 = arx_verdict_text(&rows, "h3").expect("h3 verdict");
    assert!(v3.contains("read:store=fail"), "v3: {v3}");
    let v4 = arx_verdict_text(&rows, "h4").expect("h4 verdict");
    assert!(v4.contains("read:store=fail"), "v4: {v4}");
    let v5 = arx_verdict_text(&rows, "h5").expect("h5 verdict");
    assert!(v5.contains("read:arx.toml=ok"), "v5: {v5}");
}

#[test]
fn dispatch_is_refused_outside_a_git_repository() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture_without_git();
    let run_dir = workspace.path().join("run");

    let rows = run_session(&run_dir, "arx.toml", &["hand:dispatch:fake:x".to_string()], &[]);

    let echo = rows
        .iter()
        .find(|row| {
            row["kind"] == "turn"
                && row["role"] == "main"
                && row["text"].as_str().map(|t| t.starts_with("stub-hand:")).unwrap_or(false)
        })
        .and_then(|row| row["text"].as_str())
        .expect("stub-hand echo row");
    assert!(echo.contains("is not inside a git repository"), "echo: {echo}");

    let ledger = ledger_rows(&run_dir);
    assert!(
        ledger.iter().any(|row| row["kind"] == "dispatch" && row["refused"] == true),
        "ledger: {ledger:?}"
    );
    assert!(!ledger.iter().any(|row| row["kind"] == "spawn"), "ledger: {ledger:?}");

    let containers = StdCommand::new("docker")
        .args(["ps", "-a", "--filter", "label=arx.hand", "--format", "{{.Names}}"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&containers.stdout).trim().is_empty(), "a container was started");
}

#[test]
fn sweep_carries_sha_and_timestamp_and_leaves_the_store_alone() {
    let _guard = serialize_on_docker_state();
    let workspace = fresh_run_fixture_with_store_scope("testscope");
    let run_dir = workspace.path().join("run");
    let store_root = run_dir.join("store");

    let mut engine = Engine::open(&store_root, "seed").unwrap();
    engine
        .save(
            &SaveIntent {
                scope: "testscope".to_string(),
                kind: ClaimKind::Fact,
                question: "what is the sweep sha for".to_string(),
                answer: "the swept file bytes".to_string(),
                why: "quarantine seed for the sweep test".to_string(),
                supersedes: None,
            },
            &ClaimHandle {
                thread: "seed".to_string(),
                turn: 1,
            },
        )
        .unwrap();
    drop(engine);

    let quarantine_dir = store_root.join("quarantine/seed");
    let before_head = head_count(&store_root);
    let mut before_quarantine: Vec<String> = fs::read_dir(&quarantine_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    before_quarantine.sort();
    assert_eq!(before_quarantine.len(), 1, "expected exactly one quarantined candidate");

    let rows = run_session(&run_dir, "arx.toml", &["hand:dispatch:fake:[probe]".to_string()], &[]);
    assert!(arx_verdict_text(&rows, "h1").is_some(), "expected h1 verdict");

    assert_eq!(head_count(&store_root), before_head, "sweep must not touch the store");
    let mut after_quarantine: Vec<String> = fs::read_dir(&quarantine_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    after_quarantine.sort();
    assert_eq!(after_quarantine, before_quarantine, "sweep must not touch the quarantine");

    let trace_dir = run_dir.join("threads/main.h1");
    let trace_md = fs::read_to_string(trace_dir.join("trace.md")).unwrap();
    assert!(trace_md.contains("captured_at: "), "trace.md: {trace_md}");

    let frontmatter = trace_md.split("---\n").nth(1).unwrap();
    let captured_line = frontmatter
        .lines()
        .find(|line| line.starts_with("captured_at: "))
        .expect("captured_at line");
    let captured_value = captured_line.trim_start_matches("captured_at: ");
    assert!(
        !captured_value.is_empty() && captured_value.chars().all(|c| c.is_ascii_digit()),
        "captured_at value: {captured_value}"
    );

    let lines: Vec<&str> = frontmatter.lines().collect();
    let mut checked = 0;
    let mut i = 0;
    while i < lines.len() {
        if let Some(path) = lines[i].trim_start().strip_prefix("- path: ") {
            let sha_line = lines[i + 2].trim_start();
            let sha = sha_line.strip_prefix("sha256: ").expect("sha256 line after bytes");
            assert_eq!(sha.len(), 64, "sha256 {sha} for {path} is not 64 hex chars");
            assert!(
                sha.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "sha256 {sha} for {path} is not lowercase hex"
            );
            let contents = fs::read(trace_dir.join(path)).unwrap();
            let mut hasher = Sha256::new();
            hasher.update(&contents);
            let recomputed: String = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
            assert_eq!(sha, recomputed, "sha256 mismatch for {path}");
            checked += 1;
            i += 3;
        } else {
            i += 1;
        }
    }
    assert!(checked > 0, "expected at least one paths: entry with sha256 in {trace_md}");
}
