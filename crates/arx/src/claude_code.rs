//! The claude-code adapter in the main role, SDK sidecar route, `b8d59164`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;

use arx_engine::StoreCall;

pub use crate::main_body::{
    Attachment, Control, FileCall, HandCall, MainError as ClaudeCodeError, SpawnOpts, SystemPrompt, TurnOutcome, is_context_window_message,
};

const SIGKILL: i32 = 9;

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[derive(Debug, Deserialize)]
struct Event {
    ev: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    stopped: bool,
    #[serde(default)]
    usage: Option<serde_json::Value>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    input: Option<serde_json::Value>,
    #[serde(default)]
    verb: Option<String>,
    #[serde(default)]
    args: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    models: Option<Vec<String>>,
}

enum ReaderMsg {
    Line(String),
    Eof,
    Err(std::io::Error),
}

struct Inner {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    rx: Mutex<Receiver<ReaderMsg>>,
}

#[derive(Clone)]
pub struct ClaudeCodeMain(Arc<Inner>);

fn write_line(stdin: &mut ChildStdin, value: &serde_json::Value) -> std::io::Result<()> {
    writeln!(stdin, "{value}")?;
    stdin.flush()
}

fn parse_event(line: &str) -> Result<Event, ClaudeCodeError> {
    serde_json::from_str(line.trim()).map_err(|_| ClaudeCodeError::ProtocolMalformed {
        line: line.trim_end().to_string(),
    })
}

fn kill_and_reap(child: &mut Child) {
    if matches!(child.try_wait(), Ok(None)) {
        unsafe {
            kill(-(child.id() as i32), SIGKILL);
        }
    }
    let _ = child.wait();
}

fn start(sidecar_dir: &Path, opts: &SpawnOpts) -> Result<(Child, ChildStdin, Receiver<ReaderMsg>), ClaudeCodeError> {
    let entry = sidecar_dir.join("claude-main.mjs");
    if !entry.exists() {
        return Err(ClaudeCodeError::EntryMissing { path: entry });
    }

    let mut child = Command::new("node")
        .arg("--no-warnings")
        .arg(&entry)
        .env("CLAUDE_CODE_AUTO_COMPACT_WINDOW", "1000000")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .process_group(0)
        .spawn()
        .map_err(|source| ClaudeCodeError::SpawnIo { source })?;

    let stdout = child.stdout.take().expect("piped stdout");
    let mut stdin = child.stdin.take().expect("piped stdin");

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = tx.send(ReaderMsg::Eof);
                    break;
                }
                Ok(_) => {
                    if tx.send(ReaderMsg::Line(line)).is_err() {
                        break;
                    }
                }
                Err(source) => {
                    let _ = tx.send(ReaderMsg::Err(source));
                    break;
                }
            }
        }
    });

    let init = json!({
        "op": "init",
        "model": opts.model,
        "systemPrompt": {
            "static": opts.system_prompt.static_block,
            "dynamic": opts.system_prompt.dynamic,
        },
        "cwd": opts.cwd.display().to_string(),
    });
    if write_line(&mut stdin, &init).is_err() {
        kill_and_reap(&mut child);
        return Err(ClaudeCodeError::ReadyTimeout);
    }

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            kill_and_reap(&mut child);
            return Err(ClaudeCodeError::ReadyTimeout);
        }
        match rx.recv_timeout(remaining) {
            Ok(ReaderMsg::Line(line)) => {
                let event = match parse_event(&line) {
                    Ok(event) => event,
                    Err(err) => {
                        kill_and_reap(&mut child);
                        return Err(err);
                    }
                };
                match event.ev.as_str() {
                    "ready" => break,
                    "usage_limit" => {
                        kill_and_reap(&mut child);
                        return Err(ClaudeCodeError::UsageLimitReached {
                            message: event.message.unwrap_or_default(),
                        });
                    }
                    "fatal" => {
                        kill_and_reap(&mut child);
                        return Err(ClaudeCodeError::BodyFatal {
                            message: event.message.unwrap_or_default(),
                        });
                    }
                    _ => continue,
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                kill_and_reap(&mut child);
                return Err(ClaudeCodeError::ReadyTimeout);
            }
            _ => {
                kill_and_reap(&mut child);
                return Err(ClaudeCodeError::ReadyTimeout);
            }
        }
    }

    Ok((child, stdin, rx))
}

impl ClaudeCodeMain {
    pub fn spawn(sidecar_dir: &Path, opts: SpawnOpts) -> Result<Self, ClaudeCodeError> {
        let (child, stdin, rx) = start(sidecar_dir, &opts)?;
        Ok(ClaudeCodeMain(Arc::new(Inner {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            rx: Mutex::new(rx),
        })))
    }

    pub fn respawn(&self, sidecar: &Path, opts: &SpawnOpts) -> Result<(), ClaudeCodeError> {
        let mut child_guard = self.0.child.lock().expect("sidecar child mutex poisoned");
        kill_and_reap(&mut child_guard);
        let (new_child, new_stdin, new_rx) = start(sidecar, opts)?;
        *child_guard = new_child;
        drop(child_guard);
        *self.0.stdin.lock().expect("sidecar stdin mutex poisoned") = new_stdin;
        *self.0.rx.lock().expect("sidecar stdout mutex poisoned") = new_rx;
        Ok(())
    }

    pub fn take_pending_model_fallback(&self) -> Option<(String, String)> {
        let rx = self.0.rx.lock().expect("sidecar stdout mutex poisoned");
        let mut found = None;
        while let Ok(ReaderMsg::Line(line)) = rx.recv_timeout(Duration::from_millis(20)) {
            if let Ok(event) = parse_event(&line) {
                if event.ev == "model_fallback" {
                    if let (Some(from), Some(to)) = (event.from, event.to) {
                        found = Some((from, to));
                    }
                }
            }
        }
        found
    }

    pub fn set_model(&mut self, model: &str) -> Result<(), ClaudeCodeError> {
        {
            let mut stdin = self.0.stdin.lock().expect("sidecar stdin mutex poisoned");
            let op = json!({"op": "set_model", "model": model});
            write_line(&mut stdin, &op).map_err(|source| ClaudeCodeError::SpawnIo { source })?;
        }

        let rx = self.0.rx.lock().expect("sidecar stdout mutex poisoned");
        loop {
            match rx.recv() {
                Ok(ReaderMsg::Line(line)) => {
                    let event = parse_event(&line)?;
                    match event.ev.as_str() {
                        "model_set" => {
                            return match event.error {
                                Some(message) => Err(ClaudeCodeError::ModelSwitchFailed { message }),
                                None => Ok(()),
                            };
                        }
                        "fatal" => {
                            let message = event.message.unwrap_or_default();
                            return Err(if is_context_window_message(&message) {
                                ClaudeCodeError::ContextWindowExceeded { message }
                            } else {
                                ClaudeCodeError::BodyFatal { message }
                            });
                        }
                        "usage_limit" => {
                            return Err(ClaudeCodeError::UsageLimitReached {
                                message: event.message.unwrap_or_default(),
                            });
                        }
                        _ => continue,
                    }
                }
                Ok(ReaderMsg::Eof) | Ok(ReaderMsg::Err(_)) | Err(_) => {
                    return Err(ClaudeCodeError::ChildDiedMidTurn);
                }
            }
        }
    }

    /// Compat wrapper for a caller with no control lane (the stub-driven integration
    /// tests among them): runs the turn with a control poll that never signals a stop.
    pub fn run_turn(
        &mut self,
        prompt: &str,
        on_delta: impl FnMut(&str),
        on_model_fallback: impl FnMut(&str, &str),
        on_store: impl FnMut(&StoreCall) -> serde_json::Value,
        on_hand: impl FnMut(&HandCall) -> serde_json::Value,
        on_file: impl FnMut(&FileCall) -> serde_json::Value,
    ) -> Result<TurnOutcome, ClaudeCodeError> {
        self.run_turn_controlled(prompt, &[], on_delta, on_model_fallback, on_store, on_hand, on_file, &mut || None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_turn_controlled(
        &mut self,
        prompt: &str,
        images: &[Attachment],
        mut on_delta: impl FnMut(&str),
        mut on_model_fallback: impl FnMut(&str, &str),
        mut on_store: impl FnMut(&StoreCall) -> serde_json::Value,
        mut on_hand: impl FnMut(&HandCall) -> serde_json::Value,
        mut on_file: impl FnMut(&FileCall) -> serde_json::Value,
        on_control: &mut dyn FnMut() -> Option<Control>,
    ) -> Result<TurnOutcome, ClaudeCodeError> {
        // A stop pressed at the tail of the previous turn, or one left unconsumed by an
        // interrupt already sent that turn, sits in the same persistent control channel
        // across a call boundary (and across a respawn, which never touches it) — drained
        // here so it can never fire against a turn it was never meant for.
        while on_control().is_some() {}
        {
            let mut stdin = self.0.stdin.lock().expect("sidecar stdin mutex poisoned");
            let mut op = json!({"op": "prompt", "text": prompt});
            if !images.is_empty() {
                op["images"] = json!(images
                    .iter()
                    .map(|a| json!({"path": a.path.display().to_string(), "media_type": a.media_type}))
                    .collect::<Vec<_>>());
            }
            write_line(&mut stdin, &op).map_err(|source| ClaudeCodeError::SpawnIo { source })?;
        }

        let rx = self.0.rx.lock().expect("sidecar stdout mutex poisoned");
        let mut text = String::new();
        let mut usage = None;
        let mut models = None;
        let mut stopped = false;
        let mut interrupt_sent = false;
        loop {
            // Polled every iteration, not only on a timeout: a sidecar streaming faster
            // than the 50 ms wait would otherwise never hit the timeout arm, and a stop
            // pressed mid-burst would sit unseen until the stream happened to pause.
            if !interrupt_sent && on_control() == Some(Control::Stop) {
                interrupt_sent = true;
                let mut stdin = self.0.stdin.lock().expect("sidecar stdin mutex poisoned");
                let op = json!({"op": "interrupt"});
                write_line(&mut stdin, &op).map_err(|source| ClaudeCodeError::SpawnIo { source })?;
            }
            match rx.recv_timeout(Duration::from_millis(50)) {
                Err(RecvTimeoutError::Timeout) => continue,
                Ok(ReaderMsg::Line(line)) => {
                    let event = parse_event(&line)?;
                    match event.ev.as_str() {
                        "delta" => on_delta(&event.text.unwrap_or_default()),
                        "turn" => text = event.text.unwrap_or_default(),
                        "usage" => {
                            usage = event.usage;
                            models = event.models;
                        }
                        "model_fallback" => {
                            if let (Some(from), Some(to)) = (event.from, event.to) {
                                on_model_fallback(&from, &to);
                            }
                        }
                        "turn_end" => {
                            stopped = event.stopped;
                            break;
                        }
                        "store" => {
                            let id = event.id.ok_or_else(|| ClaudeCodeError::ProtocolMalformed {
                                line: line.trim_end().to_string(),
                            })?;
                            let call = StoreCall {
                                verb: event.verb.unwrap_or_default(),
                                args: event.args.unwrap_or(serde_json::Value::Null),
                            };
                            let result = on_store(&call);
                            let payload = json!({"op": "store_result", "id": id, "result": result});
                            let mut stdin = self.0.stdin.lock().expect("sidecar stdin mutex poisoned");
                            write_line(&mut stdin, &payload).map_err(|source| ClaudeCodeError::SpawnIo { source })?;
                        }
                        "hand" => {
                            let id = event.id.ok_or_else(|| ClaudeCodeError::ProtocolMalformed {
                                line: line.trim_end().to_string(),
                            })?;
                            let call = HandCall {
                                verb: event.verb.unwrap_or_default(),
                                args: event.args.unwrap_or(serde_json::Value::Null),
                            };
                            let result = on_hand(&call);
                            let payload = json!({"op": "hand_result", "id": id, "result": result});
                            let mut stdin = self.0.stdin.lock().expect("sidecar stdin mutex poisoned");
                            write_line(&mut stdin, &payload).map_err(|source| ClaudeCodeError::SpawnIo { source })?;
                        }
                        "file" => {
                            let id = event.id.ok_or_else(|| ClaudeCodeError::ProtocolMalformed {
                                line: line.trim_end().to_string(),
                            })?;
                            let call = FileCall {
                                tool: event.tool.unwrap_or_default(),
                                args: event.args.unwrap_or(serde_json::Value::Null),
                            };
                            let result = on_file(&call);
                            let payload = json!({"op": "file_result", "id": id, "result": result});
                            let mut stdin = self.0.stdin.lock().expect("sidecar stdin mutex poisoned");
                            write_line(&mut stdin, &payload).map_err(|source| ClaudeCodeError::SpawnIo { source })?;
                        }
                        "fatal" => {
                            let message = event.message.unwrap_or_default();
                            return Err(if is_context_window_message(&message) {
                                ClaudeCodeError::ContextWindowExceeded { message }
                            } else {
                                ClaudeCodeError::BodyFatal { message }
                            });
                        }
                        "usage_limit" => {
                            return Err(ClaudeCodeError::UsageLimitReached {
                                message: event.message.unwrap_or_default(),
                            });
                        }
                        _ => {}
                    }
                }
                Ok(ReaderMsg::Eof) | Ok(ReaderMsg::Err(_)) | Err(_) => {
                    return Err(ClaudeCodeError::ChildDiedMidTurn);
                }
            }
        }
        Ok(TurnOutcome { text, usage, models, stopped })
    }

    pub fn kill(&mut self) {
        let mut child = self.0.child.lock().expect("sidecar child mutex poisoned");
        kill_and_reap(&mut child);
    }

    pub fn shutdown(self) {
        {
            let mut stdin = self.0.stdin.lock().expect("sidecar stdin mutex poisoned");
            let op = json!({"op": "shutdown"});
            let _ = write_line(&mut stdin, &op);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let mut child = self.0.child.lock().expect("sidecar child mutex poisoned");
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    drop(child);
                    thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
        let mut child = self.0.child.lock().expect("sidecar child mutex poisoned");
        kill_and_reap(&mut child);
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            kill_and_reap(&mut child);
        }
    }
}
