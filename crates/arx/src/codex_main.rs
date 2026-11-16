use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::main_body::{
    Attachment, Control, FileCall, HandCall, MainError, SpawnOpts, TurnOutcome,
    is_context_window_message,
};
use arx_engine::StoreCall;
use serde_json::{Value, json};

unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

pub(crate) const STDERR_TAIL_BYTES: usize = 16 * 1024;

struct Inner {
    child: Mutex<Child>,
    input: Mutex<ChildStdin>,
    output: Mutex<Receiver<Result<Value, MainError>>>,
    pending: Mutex<VecDeque<Value>>,
    stderr_tail: Arc<Mutex<Vec<u8>>>,
    stderr_thread: Mutex<Option<JoinHandle<()>>>,
    thread: String,
    model: Mutex<String>,
    requested_model: Mutex<Option<String>>,
    model_selection_result: Mutex<Option<Result<String, String>>>,
    next_id: AtomicU64,
    home: PathBuf,
}

#[derive(Clone)]
pub struct CodexMain(Arc<Inner>);

fn reap(child: &mut Child) {
    if matches!(child.try_wait(), Ok(None)) {
        unsafe {
            kill(-(child.id() as i32), 9);
        }
    }
    let _ = child.wait();
}

fn non_temporary_base(value: std::ffi::OsString) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return None;
    }
    let resolved = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    let system_temp =
        fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
    (!resolved.starts_with(&system_temp)
        && !resolved.starts_with("/tmp")
        && !resolved.starts_with("/var/tmp"))
    .then_some(path)
}

fn session_root() -> Result<PathBuf, MainError> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|path| !path.is_empty())
        .and_then(non_temporary_base)
        .or_else(|| {
            std::env::var_os("XDG_CACHE_HOME")
                .filter(|path| !path.is_empty())
                .and_then(non_temporary_base)
        })
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|path| !path.is_empty())
                .map(|path| PathBuf::from(path).join(".cache"))
                .and_then(|path| non_temporary_base(path.into_os_string()))
        })
        .ok_or_else(|| MainError::BodyFatal {
            message: "cannot locate a private non-temporary directory for Codex main".into(),
        })?;
    let root = base.join("arx/codex-main");
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&root)
        .map_err(|source| MainError::SpawnIo { source })?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
        .map_err(|source| MainError::SpawnIo { source })?;
    Ok(root)
}

fn drain_stderr(mut stderr: ChildStderr, tail: Arc<Mutex<Vec<u8>>>) {
    let mut chunk = [0; 8192];
    loop {
        let read = match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
        let mut tail = tail.lock().expect("codex stderr mutex poisoned");
        tail.extend_from_slice(&chunk[..read]);
        let overflow = tail.len().saturating_sub(STDERR_TAIL_BYTES);
        if overflow > 0 {
            tail.drain(..overflow);
        }
    }
}

impl Inner {
    fn stop(&self) {
        reap(&mut self.child.lock().expect("codex child mutex poisoned"));
        if let Some(thread) = self
            .stderr_thread
            .lock()
            .expect("codex stderr thread mutex poisoned")
            .take()
        {
            let _ = thread.join();
        }
    }

    fn failure_with_stderr(&self, fallback: MainError) -> MainError {
        self.stop();
        let stderr = self
            .stderr_tail
            .lock()
            .expect("codex stderr mutex poisoned");
        let message = String::from_utf8_lossy(&stderr).trim().to_string();
        if message.is_empty() {
            fallback
        } else {
            MainError::BodyFatal {
                message: format!("codex app-server stopped: {message}"),
            }
        }
    }
}

#[cfg(test)]
fn request_timeout() -> Duration {
    std::env::var("ARX_CODEX_READY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(15))
}

#[cfg(not(test))]
fn request_timeout() -> Duration {
    Duration::from_secs(15)
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.stop();
        let _ = fs::remove_dir_all(&self.home);
    }
}

impl CodexMain {
    pub fn spawn(opts: &SpawnOpts) -> Result<Self, MainError> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let home = session_root()?.join(format!("arx-main-codex-{}-{nonce}", std::process::id()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&home)
            .map_err(|source| MainError::SpawnIo { source })?;
        if let Err(message) = crate::codex_auth::inject_codex_credential(&home) {
            let _ = fs::remove_dir_all(&home);
            return Err(MainError::BodyFatal { message });
        }
        let codex_home = home.join(".codex");
        let spawned = Command::new("codex")
            .args([
                "app-server",
                "--listen",
                "stdio://",
                "-c",
                "project_doc_max_bytes=0",
                "-c",
                "approval_policy=\"never\"",
                "-c",
                "shell_environment_policy.inherit=\"none\"",
                "-c",
                "features.shell_tool=false",
                "-c",
                "features.view_image=false",
                "-c",
                "web_search=\"disabled\"",
            ])
            .env("CODEX_HOME", &codex_home)
            .env_remove("CODEX_THREAD_ID")
            .current_dir(&opts.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(source) => {
                let _ = fs::remove_dir_all(&home);
                return Err(MainError::SpawnIo { source });
            }
        };
        let input = child.stdin.take().expect("piped codex stdin");
        let output = child.stdout.take().expect("piped codex stdout");
        let stderr = child.stderr.take().expect("piped codex stderr");
        let stderr_tail = Arc::new(Mutex::new(Vec::new()));
        let stderr_for_thread = Arc::clone(&stderr_tail);
        let stderr_thread = thread::spawn(move || drain_stderr(stderr, stderr_for_thread));
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let value = match line {
                    Ok(line) => serde_json::from_str(&line)
                        .map_err(|_| MainError::ProtocolMalformed { line }),
                    Err(source) => Err(MainError::SpawnIo { source }),
                };
                if tx.send(value).is_err() {
                    break;
                }
            }
        });
        let mut body = Self(Arc::new(Inner {
            child: Mutex::new(child),
            input: Mutex::new(input),
            output: Mutex::new(rx),
            pending: Mutex::new(VecDeque::new()),
            stderr_tail,
            stderr_thread: Mutex::new(Some(stderr_thread)),
            thread: String::new(),
            model: Mutex::new(opts.model.clone()),
            requested_model: Mutex::new(None),
            model_selection_result: Mutex::new(None),
            next_id: AtomicU64::new(1),
            home,
        }));
        body.request(
            "initialize",
            json!({"clientInfo":{"name":"arx","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}}),
        )?;
        body.send(json!({"method":"initialized"}))?;
        let result = body.request("thread/start", json!({
            "model":opts.model, "cwd":opts.cwd, "baseInstructions":format!("{}\n\n{}",opts.system_prompt.static_block,opts.system_prompt.dynamic),
            "allowProviderModelFallback":false, "approvalPolicy":"never", "sandbox":"danger-full-access", "ephemeral":true,
            "config":{"project_doc_max_bytes":0}, "dynamicTools":crate::main_tools::dynamic_tools(),
        }))?;
        let thread = result["thread"]["id"]
            .as_str()
            .ok_or_else(|| MainError::ProtocolMalformed {
                line: "thread/start response lacks thread.id".into(),
            })?
            .to_string();
        Arc::get_mut(&mut body.0)
            .expect("unshared new codex body")
            .thread = thread;
        Ok(body)
    }

    fn send(&self, message: Value) -> Result<(), MainError> {
        let mut input = self.0.input.lock().expect("codex input mutex poisoned");
        if let Err(source) = writeln!(input, "{message}").and_then(|_| input.flush()) {
            return Err(self.0.failure_with_stderr(MainError::SpawnIo { source }));
        }
        Ok(())
    }

    fn begin(&self, method: &str, params: Value) -> Result<u64, MainError> {
        let id = self.0.next_id.fetch_add(1, Ordering::Relaxed);
        self.send(json!({"id":id,"method":method,"params":params}))?;
        Ok(id)
    }

    fn request(&self, method: &str, params: Value) -> Result<Value, MainError> {
        let id = self.begin(method, params)?;
        let deadline = Instant::now() + request_timeout();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let value = match self
                .0
                .output
                .lock()
                .expect("codex output mutex poisoned")
                .recv_timeout(remaining)
            {
                Ok(value) => value?,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(self.0.failure_with_stderr(MainError::ReadyTimeout));
                }
                Err(_) => return Err(self.transport_failure()),
            };
            if value["id"] == id && value.get("method").is_none() {
                return response(value);
            }
            self.0
                .pending
                .lock()
                .expect("codex pending mutex poisoned")
                .push_back(value);
        }
    }

    pub fn set_model(&mut self, model: &str) -> Result<(), MainError> {
        if model
            == self
                .0
                .model
                .lock()
                .expect("codex model mutex poisoned")
                .as_str()
        {
            *self
                .0
                .requested_model
                .lock()
                .expect("codex model mutex poisoned") = None;
            return Ok(());
        }
        let mut cursor = Value::Null;
        loop {
            let available = self.request(
                "model/list",
                json!({"cursor":cursor, "limit":100, "includeHidden":true}),
            )?;
            if available["data"].as_array().is_some_and(|rows| {
                rows.iter()
                    .any(|row| row["model"] == model || row["id"] == model)
            }) {
                break;
            }
            cursor = available["nextCursor"].clone();
            if cursor.is_null() {
                return Err(MainError::ModelSwitchFailed {
                    message: format!(
                        "Codex did not list model '{model}'; selection was not changed"
                    ),
                });
            }
        }
        let active = self.0.model.lock().expect("codex model mutex poisoned");
        *self
            .0
            .requested_model
            .lock()
            .expect("codex model mutex poisoned") =
            (model != active.as_str()).then(|| model.to_string());
        Ok(())
    }

    pub fn take_model_selection_result(&self) -> Option<Result<String, String>> {
        self.0
            .model_selection_result
            .lock()
            .expect("codex model mutex poisoned")
            .take()
    }

    pub fn kill(&mut self) {
        self.0.stop();
        let _ = fs::remove_dir_all(&self.0.home);
    }

    fn transport_failure(&self) -> MainError {
        self.0.failure_with_stderr(MainError::ChildDiedMidTurn)
    }

    pub fn shutdown(mut self) {
        self.kill();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_turn_controlled(
        &mut self,
        prompt: &str,
        images: &[Attachment],
        mut on_delta: impl FnMut(&str),
        mut on_store: impl FnMut(&StoreCall) -> Value,
        mut on_hand: impl FnMut(&HandCall) -> Value,
        mut on_file: impl FnMut(&FileCall) -> Value,
        on_control: &mut dyn FnMut() -> Option<Control>,
    ) -> Result<TurnOutcome, MainError> {
        while on_control().is_some() {}
        let previous_model = self
            .0
            .model
            .lock()
            .expect("codex model mutex poisoned")
            .clone();
        let mut requested = self
            .0
            .requested_model
            .lock()
            .expect("codex model mutex poisoned")
            .clone();
        let mut model = requested.clone().unwrap_or_else(|| previous_model.clone());
        let mut input = vec![json!({"type":"text","text":prompt})];
        input.extend(
            images
                .iter()
                .map(|image| json!({"type":"localImage","path":image.path})),
        );
        let mut id = self.begin(
            "turn/start",
            json!({"threadId":self.0.thread,"model":model,"input":input}),
        )?;
        let result = (|| {
            let mut turn = None;
            let mut activity = false;
            let mut stop_requested = false;
            let mut interrupt_sent = false;
            let mut text = String::new();
            let mut streamed = String::new();
            let mut usage = None;
            let mut completed_items = HashSet::new();
            let mut calls = HashSet::new();
            loop {
                if on_control() == Some(Control::Stop) {
                    stop_requested = true;
                }
                if stop_requested && !interrupt_sent {
                    if let Some(turn) = &turn {
                        self.begin(
                            "turn/interrupt",
                            json!({"threadId":self.0.thread,"turnId":turn}),
                        )?;
                        interrupt_sent = true;
                    }
                }
                let pending = self
                    .0
                    .pending
                    .lock()
                    .expect("codex pending mutex poisoned")
                    .pop_front();
                let value = if let Some(value) = pending {
                    value
                } else {
                    match self
                        .0
                        .output
                        .lock()
                        .expect("codex output mutex poisoned")
                        .recv_timeout(Duration::from_millis(50))
                    {
                        Ok(value) => value?,
                        Err(RecvTimeoutError::Timeout) => continue,
                        Err(_) => return Err(self.transport_failure()),
                    }
                };
                if value["id"] == id && value.get("method").is_none() {
                    if turn.is_none()
                        && !activity
                        && requested.is_some()
                        && value.get("error").is_some()
                    {
                        let refused = requested.take().expect("requested model");
                        *self
                            .0
                            .requested_model
                            .lock()
                            .expect("codex model mutex poisoned") = None;
                        *self
                            .0
                            .model_selection_result
                            .lock()
                            .expect("codex model mutex poisoned") = Some(Err(format!(
                            "Codex refused '{refused}': {}; keeping {previous_model}",
                            value["error"]
                        )));
                        model = previous_model.clone();
                        id = self.begin(
                            "turn/start",
                            json!({"threadId":self.0.thread,"model":model,"input":input}),
                        )?;
                        continue;
                    }
                    let result = response(value)?;
                    turn = Some(required(&result["turn"], "id")?.to_string());
                    if let Some(selected) = requested.take() {
                        *self.0.model.lock().expect("codex model mutex poisoned") =
                            selected.clone();
                        *self
                            .0
                            .requested_model
                            .lock()
                            .expect("codex model mutex poisoned") = None;
                        *self
                            .0
                            .model_selection_result
                            .lock()
                            .expect("codex model mutex poisoned") = Some(Ok(selected));
                    }
                    continue;
                }
                let method = value["method"].as_str().unwrap_or("");
                let params = &value["params"];
                if params.get("threadId").is_some_and(|t| t != &self.0.thread) {
                    continue;
                }
                if let Some(active) = &turn {
                    if params.get("turnId").is_some_and(|t| t != active) {
                        continue;
                    }
                }
                if method.starts_with("item/") {
                    activity = true;
                }
                if matches!(method, "item/started" | "item/completed")
                    && params["item"]["type"] == "contextCompaction"
                {
                    return Err(MainError::VendorCompaction { text: streamed });
                }
                match method {
                    "turn/started" => turn = Some(required(&params["turn"], "id")?.to_string()),
                    "item/agentMessage/delta" => {
                        let delta = required(params, "delta")?;
                        streamed.push_str(delta);
                        on_delta(delta);
                    }
                    "item/completed" if params["item"]["type"] == "agentMessage" => {
                        let item = &params["item"];
                        if completed_items.insert(required(item, "id")?.to_string()) {
                            text.push_str(required(item, "text")?);
                        }
                    }
                    "thread/tokenUsage/updated" => {
                        let raw = &params["tokenUsage"];
                        let last = &raw["last"];
                        usage = Some(
                            json!({"input_tokens":last["inputTokens"],"output_tokens":last["outputTokens"],"codex":raw}),
                        );
                    }
                    "item/tool/call" => {
                        let request_id =
                            value
                                .get("id")
                                .ok_or_else(|| MainError::ProtocolMalformed {
                                    line: "tool request lacks id".into(),
                                })?;
                        let call_id = required(params, "callId")?;
                        required(params, "threadId")?;
                        required(params, "turnId")?;
                        if !calls.insert(call_id.to_string()) {
                            return Err(MainError::ProtocolMalformed {
                                line: "duplicate tool call id".into(),
                            });
                        }
                        let tool = required(params, "tool")?;
                        let args = params.get("arguments").cloned().ok_or_else(|| {
                            MainError::ProtocolMalformed {
                                line: "tool request lacks arguments".into(),
                            }
                        })?;
                        let result = match tool {
                            "arx_ping" => json!({"text":"pong"}),
                            "save" | "recall" | "decisions_query" | "decision" | "drain" => {
                                on_store(&StoreCall {
                                    verb: tool.into(),
                                    args,
                                })
                            }
                            "dispatch" | "tap" | "steer" | "kill" => on_hand(&HandCall {
                                verb: tool.into(),
                                args,
                            }),
                            "read_file" | "edit_file" | "list_files" => on_file(&FileCall {
                                tool: tool.into(),
                                args,
                            }),
                            _ => json!({"error":format!("unknown arx tool: {tool}")}),
                        };
                        let success = result.get("error").is_none() && result["isError"] != true;
                        let text = result["text"]
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| result.to_string());
                        self.send(json!({"id":request_id,"result":{"contentItems":[{"type":"inputText","text":text}],"success":success}}))?;
                    }
                    "turn/completed" => {
                        let ended = &params["turn"];
                        if turn.as_deref() != ended["id"].as_str() {
                            continue;
                        }
                        if ended["status"] == "failed" {
                            return Err(body_error(ended["error"].to_string()));
                        }
                        if text.is_empty() {
                            text = streamed;
                        }
                        return Ok(TurnOutcome {
                            text,
                            usage,
                            models: Some(vec![model]),
                            stopped: ended["status"] == "interrupted",
                        });
                    }
                    "error" if params["willRetry"] != true => {
                        return Err(body_error(params["error"].to_string()));
                    }
                    _ if value.get("id").is_some() && value.get("method").is_some() => {
                        self.send(json!({"id":value["id"],"error":{"code":-32601,"message":"arx main does not support this request"}}))?;
                    }
                    _ => {}
                }
            }
        })();
        if result.is_err() {
            self.kill();
        }
        result
    }
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str, MainError> {
    value[key]
        .as_str()
        .ok_or_else(|| MainError::ProtocolMalformed {
            line: format!("missing string {key}"),
        })
}

fn response(value: Value) -> Result<Value, MainError> {
    if let Some(error) = value.get("error") {
        return Err(body_error(error.to_string()));
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| MainError::ProtocolMalformed {
            line: "response lacks result".into(),
        })
}

fn body_error(message: String) -> MainError {
    if is_context_window_message(&message) {
        MainError::ContextWindowExceeded { message }
    } else {
        MainError::BodyFatal { message }
    }
}
