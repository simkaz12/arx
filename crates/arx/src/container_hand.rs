use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::claude_hand;
use crate::codex_hand;
use crate::container::{self, Container, ContainerSpec, HandRoot, Mount, TraceIndex, Worktree};
use crate::ledger::{BodyKind, Control, Delivery, HandId, HandRole, HandSpec};
use crate::native_hand::{self, ContainerHost, Outcome};

const FAKE_HAND_SH: &str = include_str!("fake_hand.sh");

pub struct Prepared {
    pub root: HandRoot,
    pub toplevel: PathBuf,
    pub base: Option<String>,
    pub nonce: String,
    pub session_id: String,
}

#[derive(Clone, Copy)]
enum Wire {
    Lines,
    Json,
    Codex,
}

impl Wire {
    fn from_kind(kind: BodyKind) -> Wire {
        match kind {
            BodyKind::ClaudeCode => Wire::Json,
            BodyKind::Codex => Wire::Codex,
            BodyKind::Fake | BodyKind::Native => Wire::Lines,
        }
    }
}

fn nonce() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}", std::process::id())
}

fn docker_available() -> bool {
    Command::new("docker").arg("--version").output().is_ok()
}

fn image_present(image: &str) -> bool {
    Command::new("docker")
        .arg("image")
        .arg("inspect")
        .arg(image)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn claude_credential_source() -> PathBuf {
    if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join(".credentials.json");
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    Path::new(&home).join(".claude").join(".credentials.json")
}

fn inject_claude_credential(home: &Path) -> Result<(), String> {
    let source = claude_credential_source();
    let refusal = || format!("dispatch: no Claude Code credential at {}; log in on the host first", source.display());

    let text = fs::read_to_string(&source).map_err(|_| refusal())?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| refusal())?;
    let oauth = value.get("claudeAiOauth").cloned().ok_or_else(refusal)?;

    let dest_dir = home.join(".claude");
    fs::create_dir_all(&dest_dir).map_err(|err| format!("dispatch: failed to prepare credential directory: {err}"))?;
    fs::set_permissions(&dest_dir, fs::Permissions::from_mode(0o700))
        .map_err(|err| format!("dispatch: failed to set credential directory mode: {err}"))?;

    let dest = dest_dir.join(".credentials.json");
    let payload = serde_json::json!({"claudeAiOauth": oauth});
    let bytes = serde_json::to_vec(&payload).map_err(|err| format!("dispatch: failed to encode credential: {err}"))?;
    fs::write(&dest, bytes).map_err(|err| format!("dispatch: failed to write credential: {err}"))?;
    fs::set_permissions(&dest, fs::Permissions::from_mode(0o600))
        .map_err(|err| format!("dispatch: failed to set credential mode: {err}"))?;
    Ok(())
}

pub(crate) use crate::codex_auth::{codex_credential_source, inject_codex_credential};

pub fn prepare(spec: &HandSpec, hand: &str) -> Result<Prepared, String> {
    if !docker_available() {
        return Err("dispatch: docker not found on PATH".to_string());
    }

    if !image_present(&spec.image) {
        return Err(format!("dispatch: image {} is not present on this host", spec.image));
    }

    let toplevel = container::toplevel(&spec.config_dir)
        .map_err(|_| format!("dispatch: {} is not inside a git repository", spec.config_dir.display()))?;

    let nonce = nonce();
    let root = HandRoot::create(hand, &nonce, spec.write).map_err(|err| format!("dispatch: failed to prepare hand root: {err}"))?;

    if spec.kind == BodyKind::Codex {
        if let Err(err) = inject_codex_credential(&root.home) {
            root.remove();
            return Err(err);
        }
    }

    if spec.role == HandRole::Reviewer && spec.kind == BodyKind::Codex {
        if let Err(err) = fs::write(root.out.join("verdict-schema.json"), crate::reviewer::REVIEWER_SCHEMA) {
            root.remove();
            return Err(format!("dispatch: failed to write reviewer schema: {err}"));
        }
    }

    if spec.role == HandRole::Drain {
        if let Err(err) = fs::write(root.out.join("drain-schema.json"), crate::drain::DRAIN_SCHEMA) {
            root.remove();
            return Err(format!("dispatch: failed to write drain schema: {err}"));
        }
        let drain_input = spec.drain_input.as_deref().unwrap_or("{}");
        if let Err(err) = fs::write(root.out.join("drain-input.json"), drain_input) {
            root.remove();
            return Err(format!("dispatch: failed to write drain input: {err}"));
        }
    }

    let base = if spec.write {
        let worktree_path = root.worktree.clone().expect("write-enabled hand root reserves a worktree path");
        match Worktree::add(&toplevel, &worktree_path) {
            Ok(worktree) => Some(worktree.base),
            Err(_) => {
                root.remove();
                return Err(format!("dispatch: {} has no commit to base a worktree on", toplevel.display()));
            }
        }
    } else {
        None
    };

    if spec.kind == BodyKind::ClaudeCode {
        if let Err(err) = inject_claude_credential(&root.home) {
            if spec.write {
                let worktree_path = root.worktree.clone().expect("write-enabled hand root reserves a worktree path");
                Worktree {
                    path: worktree_path,
                    base: base.clone().unwrap_or_default(),
                }
                .remove(&toplevel);
            }
            root.remove();
            return Err(err);
        }
    }

    let session_id = claude_hand::generate_uuid_v4();

    Ok(Prepared { root, toplevel, base, nonce, session_id })
}

pub(crate) fn resolve_absolute(base: &Path, raw: PathBuf) -> PathBuf {
    let absolute = if raw.is_absolute() { raw } else { base.join(&raw) };
    absolute.canonicalize().unwrap_or(absolute)
}

pub(crate) fn credential_source_dir(kind: BodyKind) -> Option<PathBuf> {
    let raw = match kind {
        BodyKind::ClaudeCode => claude_credential_source().parent().map(Path::to_path_buf),
        BodyKind::Codex => codex_credential_source().parent().map(Path::to_path_buf),
        BodyKind::Fake | BodyKind::Native => None,
    }?;
    let cwd = std::env::current_dir().unwrap_or_default();
    Some(resolve_absolute(&cwd, raw))
}

fn plan(spec: &HandSpec, hand: &str, nonce: &str, root: &HandRoot, toplevel: &Path, _base: Option<&str>, argv: Vec<String>) -> ContainerSpec {
    let repo_src = if spec.write {
        root.worktree.clone().expect("write-enabled hand root reserves a worktree path")
    } else {
        toplevel.to_path_buf()
    };

    let rel = spec.config_dir.strip_prefix(toplevel).unwrap_or(Path::new(""));
    let workdir = if rel.as_os_str().is_empty() {
        "/hand/repo".to_string()
    } else {
        format!("/hand/repo/{}", rel.display())
    };

    let mut masks = Vec::new();
    if let Ok(store_rel) = spec.store_root.strip_prefix(toplevel) {
        masks.push(format!("/hand/repo/{}", store_rel.display()));
    }
    if let Some(credential_dir) = credential_source_dir(spec.kind) {
        let canonical_toplevel = toplevel.canonicalize().unwrap_or_else(|_| toplevel.to_path_buf());
        if let Ok(credential_rel) = credential_dir.strip_prefix(&canonical_toplevel) {
            masks.push(format!("/hand/repo/{}", credential_rel.display()));
        }
    }

    let mut mounts = vec![
        Mount {
            src: repo_src,
            dst: "/hand/repo".to_string(),
            readonly: !spec.write,
        },
        Mount {
            src: root.home.clone(),
            dst: "/hand/home".to_string(),
            readonly: false,
        },
        Mount {
            src: root.out.clone(),
            dst: "/hand/out".to_string(),
            readonly: false,
        },
    ];
    if spec.role == HandRole::Drain {
        mounts.push(Mount {
            src: spec.threads_dir.clone(),
            dst: "/hand/threads".to_string(),
            readonly: true,
        });
    }

    let env = forwarded_env(&spec.env);

    ContainerSpec {
        name: format!("arx-{hand}-{nonce}"),
        hand: hand.to_string(),
        thread: spec.thread.clone(),
        image: spec.image.clone(),
        user: root.uid_gid(),
        workdir,
        mounts,
        masks,
        env,
        argv,
    }
}

pub(crate) fn forwarded_env(names: &[String]) -> Vec<(String, String)> {
    let mut env = Vec::new();
    for name in names {
        if name == "HOME" || name == "CODEX_HOME" {
            continue;
        }
        if let Ok(value) = std::env::var(name) {
            env.push((name.clone(), value));
        }
    }
    env
}

fn reviewer_brief(spec: &HandSpec, brief: &str) -> String {
    match spec.role {
        HandRole::Reviewer => format!(
            "Return your verdict as a single JSON object matching this schema, and nothing else:\n{}\n\n{brief}",
            crate::reviewer::REVIEWER_SCHEMA
        ),
        HandRole::Drain => format!(
            "Return your suggestions as a single JSON object matching this schema, and nothing else:\n{}\n\n{brief}",
            crate::drain::DRAIN_SCHEMA
        ),
        HandRole::Hand | HandRole::Runner => brief.to_string(),
    }
}

fn build_argv(spec: &HandSpec, brief: &str, session_id: &str) -> Vec<String> {
    match spec.kind {
        BodyKind::Fake => vec![
            "sh".to_string(),
            "-c".to_string(),
            FAKE_HAND_SH.to_string(),
            "fake-hand".to_string(),
            brief.to_string(),
        ],
        BodyKind::ClaudeCode => {
            let mut argv = vec!["claude".to_string()];
            argv.extend(claude_hand::build_argv(spec, session_id));
            argv
        }
        BodyKind::Codex => {
            let mut argv = vec!["codex".to_string()];
            argv.extend(codex_hand::build_argv(spec));
            if spec.role == HandRole::Reviewer {
                argv.push("--output-schema".to_string());
                argv.push("/hand/out/verdict-schema.json".to_string());
            }
            if spec.role == HandRole::Drain {
                argv.push("--output-schema".to_string());
                argv.push("/hand/out/drain-schema.json".to_string());
            }
            argv
        }
        BodyKind::Native => vec!["true".to_string()],
    }
}

fn read_stdout_lines(pipe: impl Read, hand_id: &HandId, delivery_tx: &Sender<Delivery>) -> (Option<String>, Vec<String>) {
    let reader = BufReader::new(pipe);
    let mut verdict = None;
    let mut other = Vec::new();
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if let Some(question) = line.strip_prefix("stop:") {
            let _ = delivery_tx.send(Delivery::Stop {
                hand: hand_id.clone(),
                question: question.to_string(),
                wall: None,
            });
        } else if let Some(text) = line.strip_prefix("verdict:") {
            verdict = Some(text.to_string());
        } else {
            other.push(line);
        }
    }
    (verdict, other)
}

fn read_capped(mut pipe: impl Read, cap: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if buf.len() < cap {
                    let take = (cap - buf.len()).min(n);
                    buf.extend_from_slice(&chunk[..take]);
                }
            }
            Err(_) => break,
        }
    }
    buf
}

fn read_all(mut pipe: impl Read) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = pipe.read_to_end(&mut buf);
    buf
}

fn write_out_logs(root: &HandRoot, stdout_bytes: &[u8], stderr_bytes: &[u8]) {
    let _ = fs::write(root.out.join("stdout.log"), stdout_bytes);
    let _ = fs::write(root.out.join("stderr.log"), stderr_bytes);
}

fn find_session_file(root: &HandRoot, session_id: &str) -> Option<PathBuf> {
    let projects_dir = root.home.join(".claude").join("projects");
    let entries = fs::read_dir(&projects_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let candidate = path.join(format!("{session_id}.jsonl"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

enum StdoutResult {
    Lines(Option<String>, Vec<String>),
    Json(Vec<u8>),
    Codex(codex_hand::StreamState, Vec<u8>),
}

fn run_container_hand(spec: HandSpec, hand_id: HandId, brief: String, delivery_tx: Sender<Delivery>, control_rx: Receiver<Control>, prepared: Prepared) {
    let Prepared { root, toplevel, base, nonce, session_id } = prepared;
    let wire = Wire::from_kind(spec.kind);
    let argv = build_argv(&spec, &brief, &session_id);
    let container_spec = plan(&spec, &hand_id, &nonce, &root, &toplevel, base.as_deref(), argv);

    let mut container = match Container::run(&container_spec) {
        Ok(container) => container,
        Err(err) => {
            if spec.write {
                let worktree_path = root.worktree.clone().expect("write-enabled hand root reserves a worktree path");
                Worktree {
                    path: worktree_path,
                    base: base.clone().unwrap_or_default(),
                }
                .remove(&toplevel);
            }
            root.remove();
            let _ = delivery_tx.send(Delivery::Verdict {
                hand: hand_id,
                text: format!("failed to start container: {err}"),
                trace: None,
                bytes: None,
                usage: None,
                walls: Vec::new(),
                miss: None,
                diff: None,
            });
            return;
        }
    };

    let mut stdin = container.take_stdin().expect("piped stdin");
    let stdout = container.take_stdout().expect("piped stdout");
    let stderr = container.take_stderr().expect("piped stderr");

    let mut stdin_for_steer = match wire {
        Wire::Json | Wire::Codex => {
            let text = reviewer_brief(&spec, &brief);
            let _ = stdin.write_all(text.as_bytes());
            drop(stdin);
            None
        }
        Wire::Lines => Some(stdin),
    };

    let stdout_delivery_tx = delivery_tx.clone();
    let stdout_hand = hand_id.clone();
    let stdout_handle = thread::spawn(move || match wire {
        Wire::Lines => {
            let (verdict, other) = read_stdout_lines(stdout, &stdout_hand, &stdout_delivery_tx);
            StdoutResult::Lines(verdict, other)
        }
        Wire::Json => StdoutResult::Json(read_capped(stdout, claude_hand::MAX_STDOUT_BYTES)),
        Wire::Codex => {
            let (state, tail) = codex_hand::parse_stream(stdout);
            StdoutResult::Codex(state, tail)
        }
    });
    let stderr_handle = thread::spawn(move || read_all(stderr));

    let mut killed = false;
    loop {
        match control_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Control::Steer(text)) => {
                if let Some(stdin) = stdin_for_steer.as_mut() {
                    let _ = writeln!(stdin, "{text}");
                }
            }
            Ok(Control::Kill) => {
                killed = true;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {}
        }
        if killed {
            container.kill();
        }
        if stdout_handle.is_finished() {
            break;
        }
    }
    drop(stdin_for_steer);

    let stdout_result = stdout_handle.join().unwrap_or(StdoutResult::Lines(None, Vec::new()));
    let stderr_bytes = stderr_handle.join().unwrap_or_default();

    let status = container.wait();
    container.remove();

    let exit_label = if killed {
        "killed".to_string()
    } else {
        match &status {
            Ok(status) => status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".to_string()),
            Err(_) => "unknown".to_string(),
        }
    };

    let diff = if spec.write {
        let worktree_path = root.worktree.clone().expect("write-enabled hand root reserves a worktree path");
        let worktree = Worktree {
            path: worktree_path,
            base: base.clone().unwrap_or_default(),
        };
        worktree.diff().ok()
    } else {
        None
    };

    let (verdict_text, other_lines_for_log, raw_stdout_for_log, walls, extra, usage): (
        Option<String>,
        Vec<String>,
        Option<Vec<u8>>,
        Vec<String>,
        Vec<(PathBuf, String)>,
        Option<serde_json::Value>,
    ) = match stdout_result {
        StdoutResult::Lines(text, other) => (text, other, None, Vec::new(), Vec::new(), None),
        StdoutResult::Json(raw) => {
            let stderr_tail = if stderr_bytes.len() > claude_hand::STDERR_TAIL_BYTES {
                &stderr_bytes[stderr_bytes.len() - claude_hand::STDERR_TAIL_BYTES..]
            } else {
                &stderr_bytes[..]
            };
            let text = claude_hand::build_verdict_text(&status, &raw, stderr_tail);
            let parsed: Option<serde_json::Value> =
                std::str::from_utf8(&raw).ok().and_then(|text| serde_json::from_str(text.trim()).ok());
            let walls = parsed.as_ref().map(claude_hand::permission_denials).unwrap_or_default();

            let (extra, text) = match find_session_file(&root, &session_id) {
                Some(path) => (vec![(path, "session.jsonl".to_string())], text),
                None => (Vec::new(), format!("{text} (capture failed: session file not found)")),
            };

            let text = if killed { None } else { Some(text) };
            (text, Vec::new(), Some(raw), walls, extra, None)
        }
        StdoutResult::Codex(state, stdout_tail) => {
            let stderr_tail = if stderr_bytes.len() > claude_hand::STDERR_TAIL_BYTES {
                &stderr_bytes[stderr_bytes.len() - claude_hand::STDERR_TAIL_BYTES..]
            } else {
                &stderr_bytes[..]
            };
            let text = codex_hand::build_verdict_text(&state, &status, stderr_tail, &stdout_tail);
            let usage = state.usage.clone();

            let (extra, text) = match codex_hand::find_rollout_file(&root.home.join(".codex")) {
                Some(path) => (vec![(path, "session.jsonl".to_string())], text),
                None => (Vec::new(), format!("{text} (capture failed: no rollout under sessions/)")),
            };

            let text = if killed { None } else { Some(text) };
            (text, Vec::new(), None, Vec::new(), extra, usage)
        }
    };

    let stdout_bytes: Vec<u8> = match raw_stdout_for_log {
        Some(raw) => raw,
        None => {
            let mut joined = other_lines_for_log.join("\n");
            if !other_lines_for_log.is_empty() {
                joined.push('\n');
            }
            joined.into_bytes()
        }
    };
    write_out_logs(&root, &stdout_bytes, &stderr_bytes);

    let trace_dir = spec.threads_dir.join(format!("{}.{}", spec.thread, hand_id));
    let index = TraceIndex {
        hand: hand_id.clone(),
        agent: spec.agent.clone(),
        thread: spec.thread.clone(),
        image: spec.image.clone(),
        container: container_spec.name.clone(),
        exit: exit_label,
        write: spec.write,
        base: base.clone(),
    };
    let sweep_result = container::sweep(&trace_dir, &index, &root.out, diff.as_deref(), spec.kind.label(), &extra);

    if spec.write {
        let worktree_path = root.worktree.clone().expect("write-enabled hand root reserves a worktree path");
        Worktree {
            path: worktree_path,
            base: base.unwrap_or_default(),
        }
        .remove(&toplevel);
    }
    root.remove();

    let trace_handle = format!("threads/{}.{}", spec.thread, hand_id);
    let bytes = sweep_result.as_ref().ok().map(|sweep| sweep.bytes);
    let diff_handle = sweep_result
        .as_ref()
        .ok()
        .filter(|sweep| sweep.diff)
        .map(|_| format!("{trace_handle}/worktree.diff"));
    match verdict_text {
        Some(text) => {
            let _ = delivery_tx.send(Delivery::Verdict {
                hand: hand_id,
                text,
                trace: Some(trace_handle),
                bytes,
                usage,
                walls,
                miss: None,
                diff: diff_handle,
            });
        }
        None => {
            let _ = delivery_tx.send(Delivery::Killed {
                hand: hand_id,
                trace: Some(trace_handle),
                bytes,
            });
        }
    }
}

pub fn spawn_container_hand(spec: HandSpec, hand_id: HandId, brief: String, delivery_tx: Sender<Delivery>, control_rx: Receiver<Control>, prepared: Prepared) {
    thread::spawn(move || run_container_hand(spec, hand_id, brief, delivery_tx, control_rx, prepared));
}

fn wait_until_container_running(name: &str, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let output = Command::new("docker").arg("inspect").arg("-f").arg("{{.State.Running}}").arg(name).output();
        if let Ok(output) = output {
            if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true" {
                return true;
            }
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn remove_worktree_if_write(spec: &HandSpec, root: &HandRoot, toplevel: &Path, base: &Option<String>) {
    if spec.write {
        let worktree_path = root.worktree.clone().expect("write-enabled hand root reserves a worktree path");
        Worktree {
            path: worktree_path,
            base: base.clone().unwrap_or_default(),
        }
        .remove(toplevel);
    }
}

fn refuse_native_in_container(hand_id: HandId, text: String, delivery_tx: &Sender<Delivery>, root: &HandRoot, toplevel: &Path, spec: &HandSpec, base: &Option<String>) {
    remove_worktree_if_write(spec, root, toplevel, base);
    root.remove();
    let _ = delivery_tx.send(Delivery::Verdict {
        hand: hand_id,
        text,
        trace: None,
        bytes: None,
        usage: None,
        walls: Vec::new(),
        miss: None,
        diff: None,
    });
}

fn run_native_in_container(spec: HandSpec, hand_id: HandId, brief: String, delivery_tx: Sender<Delivery>, control_rx: Receiver<Control>, prepared: Prepared) {
    let Prepared {
        root,
        toplevel,
        base,
        nonce,
        session_id: _,
    } = prepared;

    let Some(provider_spec) = spec.provider.clone() else {
        refuse_native_in_container(
            hand_id,
            "native hand failed — no provider configured for this dispatch".to_string(),
            &delivery_tx,
            &root,
            &toplevel,
            &spec,
            &base,
        );
        return;
    };
    let key = std::env::var(&provider_spec.api_key_env).unwrap_or_default();
    if key.is_empty() {
        refuse_native_in_container(
            hand_id,
            format!("native hand failed — env {} is empty", provider_spec.api_key_env),
            &delivery_tx,
            &root,
            &toplevel,
            &spec,
            &base,
        );
        return;
    }

    let argv = vec!["sh".to_string(), "-c".to_string(), "while :; do sleep 3600; done".to_string()];
    let container_spec = plan(&spec, &hand_id, &nonce, &root, &toplevel, base.as_deref(), argv);
    let container_name = container_spec.name.clone();

    let mut container = match Container::run(&container_spec) {
        Ok(c) => c,
        Err(err) => {
            refuse_native_in_container(hand_id, format!("failed to start container: {err}"), &delivery_tx, &root, &toplevel, &spec, &base);
            return;
        }
    };
    drop(container.take_stdin());
    drop(container.take_stdout());
    drop(container.take_stderr());
    wait_until_container_running(&container_name, Duration::from_secs(10));

    if let Err(err) = crate::gate::Table::new(&spec.gate, "repo", &spec.user_gate, "user") {
        container.kill();
        let _ = container.wait();
        container.remove();
        refuse_native_in_container(hand_id, format!("native hand failed — {err}"), &delivery_tx, &root, &toplevel, &spec, &base);
        return;
    }

    let mut container = Arc::new(container);
    let container_for_loop = Arc::clone(&container);
    let container_for_thread = Arc::clone(&container);

    let cancel: crate::provider::Cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_thread = cancel.clone();
    let (steer_tx, steer_rx) = mpsc::channel::<String>();

    let transcript_path = root.root.join("transcript.jsonl");
    let transcript_path_for_thread = transcript_path.clone();
    let provider = crate::provider::build_provider(provider_spec.kind, &provider_spec.base_url, key, &spec.model, cancel.clone());

    let spec_for_thread = spec.clone();
    let hand_id_for_thread = hand_id.clone();
    let delivery_tx_for_thread = delivery_tx.clone();

    let run_handle = thread::spawn(move || {
        let host = ContainerHost {
            container: container_for_thread,
        };
        let mut trace = match native_hand::Trace::open(&transcript_path_for_thread) {
            Ok(t) => t,
            Err(err) => return Outcome::TraceFailed(err.to_string()),
        };
        let reviewer_text = reviewer_brief(&spec_for_thread, &brief);
        let mut window = crate::provider::Window::new(&reviewer_text);
        let mut gate_table = crate::gate::Table::new(&spec_for_thread.gate, "repo", &spec_for_thread.user_gate, "user")
            .expect("validated before the container started");
        native_hand::run_brief(
            provider.as_ref(),
            &spec_for_thread,
            &hand_id_for_thread,
            &mut trace,
            &cancel_for_thread,
            &mut window,
            &host,
            &mut gate_table,
            &steer_rx,
            &delivery_tx_for_thread,
        )
    });

    loop {
        match control_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Control::Steer(text)) => {
                let _ = steer_tx.send(text);
            }
            Ok(Control::Kill) => {
                cancel.store(true, Ordering::SeqCst);
                container_for_loop.kill();
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {}
        }
        if run_handle.is_finished() {
            break;
        }
    }
    drop(container_for_loop);

    let outcome = run_handle.join().unwrap_or(Outcome::Killed);

    let status = {
        let container = Arc::get_mut(&mut container).expect("no other references to the container remain after the run thread joins");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            container.kill();
            match container.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        break container.wait();
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(err) => break Err(err),
            }
        }
    };
    container.remove();

    if let Outcome::TraceFailed(err) = outcome {
        remove_worktree_if_write(&spec, &root, &toplevel, &base);
        root.remove();
        let _ = delivery_tx.send(Delivery::Verdict {
            hand: hand_id,
            text: format!("native hand failed — trace write error: {err}"),
            trace: None,
            bytes: None,
            usage: None,
            walls: Vec::new(),
            miss: None,
            diff: None,
        });
        return;
    }

    let killed = matches!(outcome, Outcome::Killed);
    let exit_label = if killed {
        "killed".to_string()
    } else {
        match &status {
            Ok(status) => status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".to_string()),
            Err(_) => "unknown".to_string(),
        }
    };

    let diff = if spec.write {
        let worktree_path = root.worktree.clone().expect("write-enabled hand root reserves a worktree path");
        let worktree = Worktree {
            path: worktree_path,
            base: base.clone().unwrap_or_default(),
        };
        worktree.diff().ok()
    } else {
        None
    };

    let trace_dir = spec.threads_dir.join(format!("{}.{}", spec.thread, hand_id));
    let index = TraceIndex {
        hand: hand_id.clone(),
        agent: spec.agent.clone(),
        thread: spec.thread.clone(),
        image: spec.image.clone(),
        container: container_name,
        exit: exit_label,
        write: spec.write,
        base: base.clone(),
    };
    let mut extra = vec![(transcript_path, "transcript.jsonl".to_string())];
    if let Ok(entries) = fs::read_dir(&root.root) {
        let mut rewinds: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("rewind-") && name.ends_with(".jsonl"))
                    .unwrap_or(false)
            })
            .collect();
        rewinds.sort();
        for path in rewinds {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            extra.push((path, name));
        }
    }
    let sweep_result = container::sweep(&trace_dir, &index, &root.out, diff.as_deref(), spec.kind.label(), &extra);

    remove_worktree_if_write(&spec, &root, &toplevel, &base);
    root.remove();

    let trace_handle = format!("threads/{}.{}", spec.thread, hand_id);
    let bytes = sweep_result.as_ref().ok().map(|sweep| sweep.bytes);
    let diff_handle = sweep_result
        .as_ref()
        .ok()
        .filter(|sweep| sweep.diff)
        .map(|_| format!("{trace_handle}/worktree.diff"));
    match outcome {
        Outcome::Delivered { text, usage, miss } => {
            let _ = delivery_tx.send(Delivery::Verdict {
                hand: hand_id,
                text,
                trace: Some(trace_handle),
                bytes,
                usage,
                walls: Vec::new(),
                miss,
                diff: diff_handle,
            });
        }
        Outcome::Killed => {
            let _ = delivery_tx.send(Delivery::Killed {
                hand: hand_id,
                trace: Some(trace_handle),
                bytes,
            });
        }
        Outcome::TraceFailed(_) => unreachable!("handled above"),
    }
}

pub fn spawn_native_in_container(spec: HandSpec, hand_id: HandId, brief: String, delivery_tx: Sender<Delivery>, control_rx: Receiver<Control>, prepared: Prepared) {
    thread::spawn(move || run_native_in_container(spec, hand_id, brief, delivery_tx, control_rx, prepared));
}
