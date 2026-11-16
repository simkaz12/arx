//! The native adapter in the hand role: a provider loop in arx's process, tool calls gated
//! and executed against a `ToolHost` seam, no subprocess for the loop itself.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use serde_json::json;

use arx_engine::{Engine, Store};

use crate::codex_hand::composed_instructions;
use crate::gate::{Gate, GateCall, GateVerdict};
use crate::ledger::{Control, Delivery, HandId, HandSpec, ProviderSpec};
use crate::provider::{Cancel, Chunk, ContentBlock, Message, Provider, Role, Termination, ToolDef, Window};

pub(crate) struct Trace {
    file: File,
    path: PathBuf,
}

impl Trace {
    pub(crate) fn open(path: &Path) -> std::io::Result<Trace> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Trace { file, path: path.to_path_buf() })
    }

    fn write(&mut self, value: &serde_json::Value) -> std::io::Result<()> {
        let mut buf = serde_json::to_string(value).expect("trace row always serializes");
        buf.push('\n');
        self.file.write_all(buf.as_bytes())
    }

    fn archive_dir(&self) -> PathBuf {
        self.path.parent().map(|parent| parent.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
    }
}

pub(crate) enum Outcome {
    Delivered {
        text: String,
        usage: Option<serde_json::Value>,
        miss: Option<u64>,
    },
    Killed,
    TraceFailed(String),
}

pub(crate) trait ToolHost {
    fn read_file(&self, path: &str) -> Result<String, String>;
    fn handle_prefix(&self, host_dir: &Path) -> String;
}

pub(crate) struct LocalHost {
    pub(crate) cwd: PathBuf,
}

impl ToolHost for LocalHost {
    fn read_file(&self, path: &str) -> Result<String, String> {
        let canonical_cwd = self.cwd.canonicalize().map_err(|err| format!("read_file error: {err}"))?;
        let candidate = self.cwd.join(path);
        let canonical_candidate = candidate.canonicalize().map_err(|err| format!("read_file error: {err}"))?;
        if !canonical_candidate.starts_with(&canonical_cwd) {
            return Err("read_file error: path escapes the working directory".to_string());
        }
        match fs::read(&canonical_candidate) {
            Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Err(err) => Err(format!("read_file error: {err}")),
        }
    }

    fn handle_prefix(&self, host_dir: &Path) -> String {
        host_dir.display().to_string()
    }
}

pub(crate) struct ContainerHost {
    pub(crate) container: Arc<crate::container::Container>,
}

const READ_FILE_EXEC_SCRIPT: &str = "p=$(realpath \"$1\") || exit 3; case \"$p\" in /hand/repo/*) cat -- \"$p\";; *) echo \"read_file error: path escapes the working directory\" >&2; exit 3;; esac";

impl ToolHost for ContainerHost {
    fn read_file(&self, path: &str) -> Result<String, String> {
        let argv = vec![
            "sh".to_string(),
            "-c".to_string(),
            READ_FILE_EXEC_SCRIPT.to_string(),
            "_".to_string(),
            path.to_string(),
        ];
        match self.container.exec(&argv) {
            Ok(output) => {
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
                } else if output.status.code() == Some(3) {
                    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
                } else {
                    Err(format!("read_file error: {}", String::from_utf8_lossy(&output.stderr).trim()))
                }
            }
            Err(err) => Err(format!("read_file error: {err}")),
        }
    }

    fn handle_prefix(&self, _host_dir: &Path) -> String {
        "/hand/traces".to_string()
    }
}

fn read_file_def() -> ToolDef {
    ToolDef {
        name: "read_file".to_string(),
        description: "Read a UTF-8 text file relative to the working directory. offset is a 1-based line, limit a line count. A result over the admission threshold arrives as its head plus a handle; read the handle with offset and limit.".to_string(),
        parameters: serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}),
    }
}

fn recall_def() -> ToolDef {
    ToolDef {
        name: "recall".to_string(),
        description: "Recall claims from the store by a lexical query.".to_string(),
        parameters: serde_json::json!({"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}),
    }
}

fn push_block(blocks: &mut Vec<ContentBlock>, block: ContentBlock) {
    if let ContentBlock::Text(text) = &block {
        if let Some(ContentBlock::Text(existing)) = blocks.last_mut() {
            existing.push_str(text);
            return;
        }
    }
    blocks.push(block);
}

fn joined_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn write_incomplete(trace: &mut Trace, blocks: &[ContentBlock]) -> Result<(), Outcome> {
    if blocks.is_empty() {
        return Ok(());
    }
    trace
        .write(&json!({"kind": "incomplete", "content": blocks}))
        .map_err(|err| Outcome::TraceFailed(err.to_string()))
}

fn finish(text: String, usage: Option<serde_json::Value>, miss: Option<u64>, cancel: &Cancel) -> Outcome {
    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
        Outcome::Killed
    } else {
        Outcome::Delivered { text, usage, miss }
    }
}

fn miss_for(err: &crate::provider::ProviderError) -> Option<u64> {
    match err {
        crate::provider::ProviderError::ContextWindowExceeded { overshoot: Some(overshoot) } => {
            Some(overshoot.requested.saturating_sub(overshoot.maximum))
        }
        crate::provider::ProviderError::ContextWindowExceeded { overshoot: None } => Some(0),
        _ => None,
    }
}

const MAX_RESPAWNS: u32 = 2;

const RESPAWN_PROMPT: &str = "[arx] your window overflowed and was rewound. Write a compact state object for yourself: what you have done, what you learned, what is decided, what is open, and your next concrete move under the brief. Prose, no tool calls.";

enum MissOutcome {
    Retry,
    End(Outcome),
}

fn overshoot_of(err: &crate::provider::ProviderError) -> Option<crate::provider::Overshoot> {
    match err {
        crate::provider::ProviderError::ContextWindowExceeded { overshoot } => *overshoot,
        _ => None,
    }
}

fn rewind_target(overshoot: Option<crate::provider::Overshoot>, window: &Window) -> u64 {
    match overshoot {
        Some(overshoot) => overshoot.requested.saturating_sub(overshoot.maximum) + crate::provider::RESERVE_TOKENS,
        None => {
            let estimate = crate::provider::estimate_tokens(window.as_slice());
            (estimate / 4).max(crate::provider::RESERVE_TOKENS)
        }
    }
}

fn append_rewind_archive(dir: &Path, ordinal: u32, cut: &[Message]) -> std::io::Result<()> {
    let path = dir.join(format!("rewind-{ordinal}.jsonl"));
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    for message in cut {
        let mut buf = serde_json::to_string(message).expect("message always serializes");
        buf.push('\n');
        file.write_all(buf.as_bytes())?;
    }
    Ok(())
}

fn write_rewind(trace: &mut Trace, window: &mut Window, overshoot: Option<crate::provider::Overshoot>, ordinal: u32) -> Result<(), Outcome> {
    let target = rewind_target(overshoot, window);
    let cut = crate::provider::rewind_to_fit(window, target);
    let tokens_cut = crate::provider::estimate_tokens(&cut);
    if let Err(err) = append_rewind_archive(&trace.archive_dir(), ordinal, &cut) {
        return Err(Outcome::TraceFailed(err.to_string()));
    }
    if let Err(err) = trace.write(&json!({
        "kind": "rewind",
        "tokens_cut": tokens_cut,
        "messages_cut": cut.len(),
        "archive": format!("rewind-{ordinal}.jsonl"),
    })) {
        return Err(Outcome::TraceFailed(err.to_string()));
    }
    Ok(())
}

fn ladder_step(
    err: crate::provider::ProviderError,
    blocks: &[ContentBlock],
    trace: &mut Trace,
    cancel: &Cancel,
    window: &mut Window,
    ordinal: u32,
) -> Result<(), Outcome> {
    write_incomplete(trace, blocks)?;
    let miss = miss_for(&err);
    let is_miss = matches!(err, crate::provider::ProviderError::ContextWindowExceeded { .. });
    let kind = if is_miss { "miss" } else { "error" };
    if let Err(write_err) = trace.write(&json!({"kind": kind, "text": err.to_string()})) {
        return Err(Outcome::TraceFailed(write_err.to_string()));
    }
    if !is_miss || window.len() <= 1 {
        return Err(finish(format!("native hand failed — {err}"), None, miss, cancel));
    }

    write_rewind(trace, window, overshoot_of(&err), ordinal)?;
    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(Outcome::Killed);
    }
    Ok(())
}

fn author_state_object(
    spec: &HandSpec,
    provider: &dyn Provider,
    tools: &[ToolDef],
    trace: &mut Trace,
    cancel: &Cancel,
    window: &mut Window,
    ordinal: u32,
) -> Result<String, Outcome> {
    let instructions = composed_instructions(spec);
    loop {
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(Outcome::Killed);
        }

        let mut messages = window.as_slice().to_vec();
        messages.push(Message::user(RESPAWN_PROMPT));

        let stream = match provider.complete(&instructions, &messages, tools) {
            Ok(stream) => stream,
            Err(err) => {
                ladder_step(err, &[], trace, cancel, window, ordinal)?;
                continue;
            }
        };

        let mut content_blocks: Vec<ContentBlock> = Vec::new();
        let mut ladder_err = None;
        let mut saw_terminal = false;
        for item in stream {
            match item {
                Ok(Chunk::Content(block)) => push_block(&mut content_blocks, block),
                Ok(Chunk::Terminal { .. }) => {
                    saw_terminal = true;
                    break;
                }
                Err(err) => {
                    ladder_err = Some(err);
                    break;
                }
            }
            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(Outcome::Killed);
            }
        }

        if let Some(err) = ladder_err {
            ladder_step(err, &content_blocks, trace, cancel, window, ordinal)?;
            continue;
        }

        if !saw_terminal {
            write_incomplete(trace, &content_blocks)?;
            let text = "stream ended without a terminal chunk".to_string();
            if let Err(write_err) = trace.write(&json!({"kind": "error", "text": text})) {
                return Err(Outcome::TraceFailed(write_err.to_string()));
            }
            return Err(finish(format!("native hand failed — {text}"), None, None, cancel));
        }

        return Ok(joined_text(&content_blocks));
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_step_miss(
    spec: &HandSpec,
    err: crate::provider::ProviderError,
    blocks: &[ContentBlock],
    provider: &dyn Provider,
    tools: &[ToolDef],
    trace: &mut Trace,
    cancel: &Cancel,
    window: &mut Window,
    respawns: &mut u32,
) -> MissOutcome {
    if let Err(outcome) = write_incomplete(trace, blocks) {
        return MissOutcome::End(outcome);
    }
    let miss = miss_for(&err);
    let is_miss = matches!(err, crate::provider::ProviderError::ContextWindowExceeded { .. });
    let kind = if is_miss { "miss" } else { "error" };
    if let Err(write_err) = trace.write(&json!({"kind": kind, "text": err.to_string()})) {
        return MissOutcome::End(Outcome::TraceFailed(write_err.to_string()));
    }
    if !is_miss || window.len() <= 1 {
        return MissOutcome::End(finish(format!("native hand failed — {err}"), None, miss, cancel));
    }
    if *respawns >= MAX_RESPAWNS {
        return MissOutcome::End(finish(
            "native hand failed — context window exceeded after 2 respawns".to_string(),
            None,
            miss,
            cancel,
        ));
    }

    let ordinal = *respawns + 1;
    if let Err(outcome) = write_rewind(trace, window, overshoot_of(&err), ordinal) {
        return MissOutcome::End(outcome);
    }
    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
        return MissOutcome::End(Outcome::Killed);
    }

    let text = match author_state_object(spec, provider, tools, trace, cancel, window, ordinal) {
        Ok(text) => text,
        Err(outcome) => return MissOutcome::End(outcome),
    };
    if let Err(write_err) = trace.write(&json!({"kind": "state_object", "text": text})) {
        return MissOutcome::End(Outcome::TraceFailed(write_err.to_string()));
    }
    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
        return MissOutcome::End(Outcome::Killed);
    }

    let brief = joined_text(&window.as_slice()[0].content);
    *window = Window::new(&brief);
    window.append(Message::user(format!("[arx] state object from before the reset:\n{text}")));
    *respawns = ordinal;
    if let Err(write_err) = trace.write(&json!({"kind": "respawn", "ordinal": ordinal})) {
        return MissOutcome::End(Outcome::TraceFailed(write_err.to_string()));
    }
    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
        return MissOutcome::End(Outcome::Killed);
    }

    MissOutcome::Retry
}

fn execute_tool(
    name: &str,
    arguments: &serde_json::Value,
    spec: &HandSpec,
    hand_id: &str,
    host: &dyn ToolHost,
    recall_engine: &mut Option<Engine>,
    root: &crate::admission::TraceRoot,
) -> (String, crate::admission::Provenance, Option<String>, bool) {
    match name {
        "read_file" => {
            let Some(path) = arguments.get("path").and_then(|v| v.as_str()) else {
                return (
                    "read_file error: 'path' must be a string".to_string(),
                    crate::admission::Provenance {
                        source: "read_file".to_string(),
                        sha256: None,
                    },
                    None,
                    false,
                );
            };
            let offset = arguments.get("offset").and_then(|v| v.as_u64());
            let limit = arguments.get("limit").and_then(|v| v.as_u64());
            let ranged = offset.is_some() || limit.is_some();
            let primary_prefix = format!("{}/", root.handle_prefix);
            let fallback_prefix = format!("{}/", root.fallback_dir.display());

            let handle_dir = if path.starts_with(&primary_prefix) {
                Some((&primary_prefix, &root.host_dir))
            } else if path.starts_with(&fallback_prefix) {
                Some((&fallback_prefix, &root.fallback_dir))
            } else {
                None
            };

            let (read_result, trace_rooted): (Result<String, String>, Option<String>) = if let Some((prefix, dir)) = handle_dir {
                let rest = path.strip_prefix(prefix.as_str()).expect("prefix just matched");
                if rest.is_empty() || rest.contains('/') || rest == ".." {
                    return (
                        "read_file error: not a handle".to_string(),
                        crate::admission::Provenance {
                            source: path.to_string(),
                            sha256: None,
                        },
                        None,
                        false,
                    );
                }
                let file_path = dir.join(rest);
                let result = match fs::read(&file_path) {
                    Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
                    Err(err) => Err(format!("read_file error: {err}")),
                };
                (result, Some(path.to_string()))
            } else {
                (host.read_file(path), None)
            };

            let whole = match read_result {
                Ok(content) => content,
                Err(err) => {
                    return (
                        err,
                        crate::admission::Provenance {
                            source: path.to_string(),
                            sha256: None,
                        },
                        None,
                        false,
                    );
                }
            };

            let sha = crate::admission::sha256_hex(whole.as_bytes());
            let sliced = match crate::admission::slice_lines(&whole, offset, limit) {
                Ok(s) => s,
                Err(err) => {
                    return (
                        err,
                        crate::admission::Provenance {
                            source: path.to_string(),
                            sha256: Some(sha),
                        },
                        None,
                        false,
                    );
                }
            };
            let source = if ranged {
                let total_lines = whole.lines().count() as u64;
                let start_line = offset.unwrap_or(1);
                let end_line = match limit {
                    Some(l) => std::cmp::min(total_lines, start_line.saturating_sub(1).saturating_add(l)),
                    None => total_lines,
                };
                format!("{path}#L{start_line}-L{end_line}")
            } else {
                path.to_string()
            };

            (
                sliced,
                crate::admission::Provenance { source, sha256: Some(sha) },
                trace_rooted,
                ranged,
            )
        }
        "recall" => {
            let recall_provenance = crate::admission::Provenance {
                source: "recall".to_string(),
                sha256: None,
            };
            let Some(query) = arguments.get("query").and_then(|v| v.as_str()) else {
                return ("recall error: 'query' must be a string".to_string(), recall_provenance, None, false);
            };
            let Some(scope) = spec.store_scope.as_deref() else {
                return ("recall error: no store scope configured".to_string(), recall_provenance, None, false);
            };
            if recall_engine.is_none() {
                match Engine::open(&spec.store_root, &format!("hand-{hand_id}")) {
                    Ok(engine) => *recall_engine = Some(engine),
                    Err(err) => return (format!("recall error: {err}"), recall_provenance, None, false),
                }
            }
            let engine = recall_engine.as_mut().expect("just initialised above");
            let content = match engine.recall(scope, query, 10) {
                Ok(claims) => serde_json::to_string(&claims).unwrap_or_else(|_| "recall error: could not encode claims".to_string()),
                Err(err) => format!("recall error: {err}"),
            };
            (content, recall_provenance, None, false)
        }
        other => (
            format!("unknown tool '{other}'"),
            crate::admission::Provenance {
                source: other.to_string(),
                sha256: None,
            },
            None,
            false,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_brief_inner(
    provider: &dyn Provider,
    spec: &HandSpec,
    hand_id: &HandId,
    trace: &mut Trace,
    cancel: &Cancel,
    window: &mut Window,
    host: &dyn ToolHost,
    gate: &mut dyn Gate,
    steer_rx: &Receiver<String>,
    delivery_tx: &Sender<Delivery>,
    recall_engine: &mut Option<Engine>,
    admission_root: &crate::admission::TraceRoot,
    admission_cache: &mut crate::admission::Cache,
    admission_seq: &mut u64,
) -> Outcome {
    macro_rules! trace_write {
        ($value:expr) => {
            if let Err(err) = trace.write(&$value) {
                return Outcome::TraceFailed(err.to_string());
            }
        };
    }

    let mut tools = vec![read_file_def()];
    if spec.tools.iter().any(|t| t == "recall") {
        tools.push(recall_def());
    }
    let mut last_text = String::new();
    let mut last_tool_count = 0usize;
    let mut last_termination: Option<Termination> = None;
    let mut last_usage: Option<serde_json::Value> = None;
    let mut respawns: u32 = 0;
    let mut step: u32 = 1;
    let instructions = composed_instructions(spec);

    while step <= spec.steps {
        let termination: Termination;
        let content_blocks: Vec<ContentBlock>;
        let usage: Option<serde_json::Value>;

        'attempt: loop {
            trace_write!(json!({
                "kind": "request",
                "model": spec.model,
                "system": instructions,
                "messages": window.len(),
                "tools": tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
            }));

            let stream = match provider.complete(&instructions, window.as_slice(), &tools) {
                Ok(stream) => stream,
                Err(err) => match handle_step_miss(spec, err, &[], provider, &tools, trace, cancel, window, &mut respawns) {
                    MissOutcome::Retry => continue 'attempt,
                    MissOutcome::End(outcome) => return outcome,
                },
            };

            let mut blocks: Vec<ContentBlock> = Vec::new();
            let mut term: Option<Termination> = None;
            let mut used: Option<serde_json::Value> = None;

            for item in stream {
                match item {
                    Ok(Chunk::Content(block)) => push_block(&mut blocks, block),
                    Ok(Chunk::Terminal { termination: t, usage: u }) => {
                        term = Some(t);
                        used = u;
                    }
                    Err(err) => match handle_step_miss(spec, err, &blocks, provider, &tools, trace, cancel, window, &mut respawns) {
                        MissOutcome::Retry => continue 'attempt,
                        MissOutcome::End(outcome) => return outcome,
                    },
                }

                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    return Outcome::Killed;
                }

                if term.is_some() {
                    break;
                }
            }

            let Some(t) = term else {
                if let Err(outcome) = write_incomplete(trace, &blocks) {
                    return outcome;
                }
                let text = "stream ended without a terminal chunk".to_string();
                trace_write!(json!({"kind": "error", "text": text}));
                return finish(format!("native hand failed — {text}"), None, None, cancel);
            };

            termination = t;
            content_blocks = blocks;
            usage = used;
            break 'attempt;
        }

        trace_write!(json!({"kind": "assistant", "content": content_blocks}));
        trace_write!(json!({"kind": "termination", "termination": termination, "usage": usage}));

        window.append(Message {
            role: Role::Assistant,
            content: content_blocks.clone(),
        });

        last_text = joined_text(&content_blocks);
        last_usage = usage.clone();
        last_termination = Some(termination.clone());

        if termination != Termination::ToolUse {
            last_tool_count = 0;
            break;
        }

        if step == spec.steps {
            return finish(format!("native hand stopped at the step cap ({} steps)", spec.steps), usage, None, cancel);
        }

        let tool_calls: Vec<(String, String, serde_json::Value)> = content_blocks
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolCall { id, name, arguments } => Some((id.clone(), name.clone(), arguments.clone())),
                _ => None,
            })
            .collect();

        for (id, name, arguments) in &tool_calls {
            let call = GateCall {
                tool: name.clone(),
                input: arguments.clone(),
            };
            let verdict = gate.decide(&call);
            let verdict_label = match &verdict {
                GateVerdict::Allow => "allow",
                GateVerdict::Deny { .. } => "deny",
            };
            let mut gate_row = json!({"kind": "gate", "tool": name, "verdict": verdict_label});
            if let GateVerdict::Deny { reason } = &verdict {
                gate_row["reason"] = json!(reason);
            }
            trace_write!(gate_row);

            let (body, provenance, trace_rooted, ranged) = match verdict {
                GateVerdict::Allow => execute_tool(name, arguments, spec, hand_id, host, recall_engine, admission_root),
                GateVerdict::Deny { reason } => (
                    format!("denied: {reason}"),
                    crate::admission::Provenance {
                        source: name.clone(),
                        sha256: None,
                    },
                    None,
                    false,
                ),
            };

            let content = if name == "recall" {
                body
            } else {
                let req = crate::admission::Request {
                    tool: name,
                    args: arguments,
                    body: &body,
                    provenance,
                    trace_rooted,
                    ranged,
                    ordinal: step as u64,
                };
                match crate::admission::admit(&spec.admission, admission_cache, admission_root, admission_seq, req) {
                    Ok(admitted) => {
                        for event in &admitted.events {
                            trace_write!(serde_json::to_value(event).expect("event serializes"));
                        }
                        admitted.text
                    }
                    Err(err) => err,
                }
            };

            window.append(Message {
                role: Role::Tool,
                content: vec![ContentBlock::ToolResult {
                    call_id: id.clone(),
                    content: content.clone(),
                }],
            });
            trace_write!(json!({"kind": "tool_result", "call_id": id, "content": content}));

            if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                return Outcome::Killed;
            }
        }

        last_tool_count = tool_calls.len();
        step += 1;
    }

    let verdict_text = if !last_text.is_empty() {
        last_text
    } else if last_tool_count > 0 {
        format!("native hand completed one turn: {last_tool_count} tool call(s) executed, no text; multi-turn lands at L2-10")
    } else if let Some(Termination::Refusal { category }) = &last_termination {
        format!("native hand refused: {category}")
    } else {
        format!(
            "native hand completed one turn with empty content ({:?})",
            last_termination.expect("the loop always sets a termination before breaking")
        )
    };

    finish(verdict_text, last_usage, None, cancel)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_brief(
    provider: &dyn Provider,
    spec: &HandSpec,
    hand_id: &HandId,
    trace: &mut Trace,
    cancel: &Cancel,
    window: &mut Window,
    host: &dyn ToolHost,
    gate: &mut dyn Gate,
    steer_rx: &Receiver<String>,
    delivery_tx: &Sender<Delivery>,
) -> Outcome {
    let mut recall_engine: Option<Engine> = None;
    let host_dir = spec.threads_dir.join(format!("{}.{}.traces", spec.thread, hand_id));
    let admission_root = crate::admission::TraceRoot {
        host_dir: host_dir.clone(),
        handle_prefix: host.handle_prefix(&host_dir),
        fallback_dir: std::env::temp_dir().join("arx-traces").join(format!("{}.{}", spec.thread, hand_id)),
    };
    let mut admission_cache = crate::admission::Cache::new();
    let mut admission_seq: u64 = 0;
    let outcome = run_brief_inner(
        provider,
        spec,
        hand_id,
        trace,
        cancel,
        window,
        host,
        gate,
        steer_rx,
        delivery_tx,
        &mut recall_engine,
        &admission_root,
        &mut admission_cache,
        &mut admission_seq,
    );
    if recall_engine.is_some() {
        let quarantine_dir = spec.store_root.join("quarantine").join(format!("hand-{hand_id}"));
        let _ = fs::remove_dir_all(&quarantine_dir);
    }
    outcome
}

fn execute_turn(
    provider: &dyn Provider,
    spec: &HandSpec,
    hand_id: &HandId,
    brief: &str,
    delivery_tx: &Sender<Delivery>,
    cancel: &Cancel,
    steer_rx: &Receiver<String>,
) {
    let trace_path = spec.threads_dir.join(format!("{}.{}.jsonl", spec.thread, hand_id));
    let mut trace = match Trace::open(&trace_path) {
        Ok(trace) => trace,
        Err(err) => {
            let _ = delivery_tx.send(Delivery::Verdict {
                hand: hand_id.clone(),
                text: format!("native hand failed — trace open error: {err}"),
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

    let mut window = Window::new(brief);
    let host = LocalHost { cwd: spec.cwd.clone() };
    let mut gate = match crate::gate::Table::new(&spec.gate, "repo", &spec.user_gate, "user") {
        Ok(table) => table,
        Err(err) => {
            let _ = delivery_tx.send(Delivery::Verdict {
                hand: hand_id.clone(),
                text: format!("native hand failed — {err}"),
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

    let outcome = run_brief(provider, spec, hand_id, &mut trace, cancel, &mut window, &host, &mut gate, steer_rx, delivery_tx);

    match outcome {
        Outcome::Delivered { text, usage, miss } => {
            let _ = delivery_tx.send(Delivery::Verdict {
                hand: hand_id.clone(),
                text,
                trace: Some(format!("threads/{}.{}.jsonl", spec.thread, hand_id)),
                bytes: None,
                usage,
                walls: Vec::new(),
                miss,
                diff: None,
            });
        }
        Outcome::TraceFailed(err) => {
            let _ = delivery_tx.send(Delivery::Verdict {
                hand: hand_id.clone(),
                text: format!("native hand failed — trace write error: {err}"),
                trace: None,
                bytes: None,
                usage: None,
                walls: Vec::new(),
                miss: None,
                diff: None,
            });
        }
        Outcome::Killed => {}
    }
}

fn run_native_hand(
    spec: HandSpec,
    hand_id: HandId,
    brief: String,
    provider_spec: ProviderSpec,
    key: String,
    delivery_tx: Sender<Delivery>,
    cancel: Cancel,
    steer_rx: Receiver<String>,
) {
    let provider: Box<dyn Provider> =
        crate::provider::build_provider(provider_spec.kind, &provider_spec.base_url, key, &spec.model, cancel.clone());
    execute_turn(provider.as_ref(), &spec, &hand_id, &brief, &delivery_tx, &cancel, &steer_rx);
}

fn spawn_native_hand_threads(
    spec: HandSpec,
    hand_id: HandId,
    brief: String,
    delivery_tx: Sender<Delivery>,
    control_rx: Receiver<Control>,
) -> Option<thread::JoinHandle<()>> {
    let Some(provider_spec) = spec.provider.clone() else {
        let _ = delivery_tx.send(Delivery::Verdict {
            hand: hand_id,
            text: "native hand failed — no provider configured for this dispatch".to_string(),
            trace: None,
            bytes: None,
            usage: None,
            walls: Vec::new(),
            miss: None,
            diff: None,
        });
        return None;
    };

    let key = std::env::var(&provider_spec.api_key_env).unwrap_or_default();
    if key.is_empty() {
        let _ = delivery_tx.send(Delivery::Verdict {
            hand: hand_id,
            text: format!("native hand failed — env {} is empty", provider_spec.api_key_env),
            trace: None,
            bytes: None,
            usage: None,
            walls: Vec::new(),
            miss: None,
            diff: None,
        });
        return None;
    }

    let cancel: Cancel = Arc::new(AtomicBool::new(false));
    let watcher_cancel = Arc::clone(&cancel);
    let (steer_tx, steer_rx) = mpsc::channel::<String>();
    thread::spawn(move || loop {
        match control_rx.recv() {
            Ok(Control::Kill) => {
                watcher_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
                return;
            }
            Ok(Control::Steer(text)) => {
                let _ = steer_tx.send(text);
            }
            Err(_) => return,
        }
    });

    Some(thread::spawn(move || {
        run_native_hand(spec, hand_id, brief, provider_spec, key, delivery_tx, cancel, steer_rx);
    }))
}

pub fn spawn_native_hand(spec: HandSpec, hand_id: HandId, brief: String, delivery_tx: Sender<Delivery>, control_rx: Receiver<Control>) {
    spawn_native_hand_threads(spec, hand_id, brief, delivery_tx, control_rx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io::Read;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use crate::gate::{GateConfig, Table};
    use crate::ledger::{BodyKind, HandRole};
    use crate::provider::{ChunkStream, Overshoot, ProviderError, ProviderKind};

    struct ScriptedProvider {
        scripts: Mutex<VecDeque<Vec<Result<Chunk, ProviderError>>>>,
        calls: AtomicUsize,
        calls_seen: Mutex<Vec<Vec<Message>>>,
        tools_seen: Mutex<Vec<Vec<ToolDef>>>,
    }

    impl ScriptedProvider {
        fn new(script: Vec<Result<Chunk, ProviderError>>) -> ScriptedProvider {
            ScriptedProvider::sequence(VecDeque::from(vec![script]))
        }

        fn sequence(scripts: VecDeque<Vec<Result<Chunk, ProviderError>>>) -> ScriptedProvider {
            ScriptedProvider {
                scripts: Mutex::new(scripts),
                calls: AtomicUsize::new(0),
                calls_seen: Mutex::new(Vec::new()),
                tools_seen: Mutex::new(Vec::new()),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn messages_at(&self, call_index: usize) -> Vec<Message> {
            self.calls_seen.lock().unwrap()[call_index].clone()
        }

        fn tools_at(&self, call_index: usize) -> Vec<ToolDef> {
            self.tools_seen.lock().unwrap()[call_index].clone()
        }
    }

    impl Provider for ScriptedProvider {
        fn complete(&self, _system: &str, messages: &[Message], tools: &[ToolDef]) -> Result<ChunkStream, ProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.calls_seen.lock().unwrap().push(messages.to_vec());
            self.tools_seen.lock().unwrap().push(tools.to_vec());
            let items = self.scripts.lock().unwrap().pop_front().unwrap_or_default();
            Ok(Box::new(items.into_iter()))
        }
    }

    fn sample_spec(dir: &Path) -> HandSpec {
        HandSpec {
            agent: "hand-ds".to_string(),
            kind: BodyKind::Native,
            role: HandRole::Hand,
            model: "fake-model".to_string(),
            cwd: dir.to_path_buf(),
            threads_dir: dir.join("threads"),
            thread: "main".to_string(),
            provider: Some(ProviderSpec {
                kind: ProviderKind::Anthropic,
                base_url: "http://127.0.0.1:0".to_string(),
                api_key_env: "ARX_TEST_FAKE_KEY".to_string(),
            }),
            write: false,
            image: crate::ledger::DEFAULT_HAND_IMAGE.to_string(),
            env: Vec::new(),
            store_root: dir.join("store"),
            config_dir: dir.to_path_buf(),
            tools: Vec::new(),
            steps: crate::config::DEFAULT_STEPS,
            window: None,
            gate: GateConfig::default(),
            user_gate: GateConfig::default(),
            store_scope: None,
            drain_input: None,
            admission: crate::admission::AdmissionPolicy::default(),
            role_line: None,
        }
    }

    fn unset_cancel() -> Cancel {
        Arc::new(AtomicBool::new(false))
    }

    fn unset_steer() -> Receiver<String> {
        mpsc::channel().1
    }

    fn allow_all_gate() -> Table {
        Table::new(&GateConfig::default(), "repo", &GateConfig::default(), "user").unwrap()
    }

    fn open_trace(spec: &HandSpec, hand_id: &str) -> Trace {
        Trace::open(&spec.threads_dir.join(format!("{}.{hand_id}.jsonl", spec.thread))).unwrap()
    }

    fn trace_rows(spec: &HandSpec, hand_id: &str) -> Vec<serde_json::Value> {
        let path = spec.threads_dir.join(format!("{}.{}.jsonl", spec.thread, hand_id));
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn text_turn_delivers_joined_text_with_one_complete_call() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![
            Ok(Chunk::Content(ContentBlock::Text("ARX-".to_string()))),
            Ok(Chunk::Content(ContentBlock::Text("L1-10 OK".to_string()))),
            Ok(Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: None,
            }),
        ]);
        let (delivery_tx, delivery_rx) = mpsc::channel();
        let cancel = unset_cancel();
        let steer_rx = unset_steer();

        execute_turn(&provider, &spec, &"h1".to_string(), "brief", &delivery_tx, &cancel, &steer_rx);

        match delivery_rx.try_recv().unwrap() {
            Delivery::Verdict { text, trace, .. } => {
                assert_eq!(text, "ARX-L1-10 OK");
                assert_eq!(trace, Some("threads/main.h1.jsonl".to_string()));
            }
            other => panic!("expected verdict, got {other:?}"),
        }
        assert_eq!(provider.call_count(), 1);
    }

    #[test]
    fn tool_turn_executes_gate_and_appends_tool_result_to_window() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-L1-10-PROBE").unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "probe.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("done".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read the probe");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => assert_eq!(text, "done"),
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }
        assert_eq!(provider.call_count(), 2);

        let roles: Vec<Role> = window.as_slice().iter().map(|m| m.role).collect();
        assert_eq!(roles, vec![Role::User, Role::Assistant, Role::Tool, Role::Assistant]);

        let rows = trace_rows(&spec, "h1");
        let gate_row = rows.iter().find(|r| r["kind"] == "gate").expect("gate row");
        assert_eq!(gate_row["tool"], "read_file");
        assert_eq!(gate_row["verdict"], "allow");
        let tool_result_row = rows.iter().find(|r| r["kind"] == "tool_result").expect("tool_result row");
        assert!(
            tool_result_row["content"].as_str().unwrap().contains("ARX-L1-10-PROBE"),
            "tool_result row: {tool_result_row:?}"
        );
    }

    #[test]
    fn run_brief_writes_the_role_line_into_the_request_trace_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = sample_spec(dir.path());
        spec.role_line = Some("Native body driven straight off the provider loop.".to_string());
        let provider = ScriptedProvider::new(vec![
            Ok(Chunk::Content(ContentBlock::Text("done".to_string()))),
            Ok(Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: None,
            }),
        ]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("do the thing");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        let rows = trace_rows(&spec, "h1");
        let request_row = rows.iter().find(|r| r["kind"] == "request").expect("request row");
        assert_eq!(
            request_row["system"].as_str().unwrap(),
            format!("{}\nNative body driven straight off the provider loop.", crate::codex_hand::ARX_HAND_INSTRUCTIONS)
        );
    }

    #[test]
    fn run_brief_leaves_the_request_trace_system_unchanged_when_the_role_line_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        assert!(spec.role_line.is_none());
        let provider = ScriptedProvider::new(vec![
            Ok(Chunk::Content(ContentBlock::Text("done".to_string()))),
            Ok(Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: None,
            }),
        ]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("do the thing");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        let rows = trace_rows(&spec, "h1");
        let request_row = rows.iter().find(|r| r["kind"] == "request").expect("request row");
        assert_eq!(request_row["system"].as_str().unwrap(), crate::codex_hand::ARX_HAND_INSTRUCTIONS);
    }

    #[test]
    fn an_over_threshold_tool_result_is_externalized_before_it_enters_the_window() {
        let dir = tempfile::tempdir().unwrap();
        let content = "a".repeat(48 * 1024);
        fs::write(dir.path().join("big.txt"), &content).unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "big.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("done".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read the big file");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => assert_eq!(text, "done"),
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }

        let host_dir = spec.threads_dir.join("main.h1.traces");
        let handle = format!("{}/1-1.md", host_dir.display());

        let tool_message = window.as_slice().iter().find(|m| m.role == Role::Tool).expect("a tool result message");
        let ContentBlock::ToolResult { content, .. } = &tool_message.content[0] else {
            panic!("expected a tool result block");
        };
        assert!(
            content.starts_with(&format!("[arx] externalized read_file result: {handle}")),
            "content: {content}"
        );

        assert!(host_dir.join("1-1.md").exists());
        let trace_contents = fs::read_to_string(host_dir.join("1-1.md")).unwrap();
        assert!(trace_contents.starts_with("---\nproducer: externalized tool result\n"), "trace: {trace_contents}");

        let rows = trace_rows(&spec, "h1");
        let admission_rows: Vec<&serde_json::Value> = rows.iter().filter(|r| r["kind"] == "admission").collect();
        assert_eq!(admission_rows.len(), 1, "rows: {rows:?}");
        assert_eq!(admission_rows[0]["outcome"], "externalized");
        assert!(rows.iter().all(|r| r["kind"] != "pull"), "rows: {rows:?}");
    }

    #[test]
    fn a_pull_of_a_native_handle_is_served_from_the_host_and_a_whole_read_echoes() {
        let dir = tempfile::tempdir().unwrap();
        let line = format!("{}\n", "a".repeat(39));
        let content = line.repeat(1_200);
        fs::write(dir.path().join("big.txt"), &content).unwrap();
        let spec = sample_spec(dir.path());
        let host_dir = spec.threads_dir.join("main.h1.traces");
        let handle_path = format!("{}/1-1.md", host_dir.display());

        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "big.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_2".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": handle_path, "offset": 1, "limit": 5}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_3".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": handle_path}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("done".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read the big file then its handle");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => assert_eq!(text, "done"),
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }

        let rows = trace_rows(&spec, "h1");
        let pull_rows: Vec<&serde_json::Value> = rows.iter().filter(|r| r["kind"] == "pull").collect();
        assert_eq!(pull_rows.len(), 1, "rows: {rows:?}");
        let echo_rows: Vec<&serde_json::Value> = rows.iter().filter(|r| r["kind"] == "echo").collect();
        assert_eq!(echo_rows.len(), 1, "rows: {rows:?}");
        let admission_rows: Vec<&serde_json::Value> = rows.iter().filter(|r| r["kind"] == "admission").collect();
        assert_eq!(admission_rows.len(), 2, "rows: {rows:?}");

        let entries: Vec<_> = fs::read_dir(&host_dir).unwrap().collect();
        assert_eq!(entries.len(), 1, "entries: {entries:?}");

        let fallback_dir = std::env::temp_dir().join("arx-traces").join(format!("native-fallback-pull-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&fallback_dir);
        fs::create_dir_all(&fallback_dir).unwrap();
        fs::write(fallback_dir.join("2-2.md"), "---\nfallback frontmatter\n---\nfallback content\n").unwrap();
        let fallback_root = crate::admission::TraceRoot {
            host_dir: host_dir.clone(),
            handle_prefix: host.handle_prefix(&host_dir),
            fallback_dir: fallback_dir.clone(),
        };
        let mut recall_engine: Option<Engine> = None;
        let fallback_handle = format!("{}/2-2.md", fallback_dir.display());
        let (body, _provenance, trace_rooted, _ranged) = execute_tool(
            "read_file",
            &serde_json::json!({"path": fallback_handle}),
            &spec,
            "h1",
            &host,
            &mut recall_engine,
            &fallback_root,
        );
        assert!(body.contains("fallback content"), "body: {body}");
        assert_eq!(trace_rooted, Some(fallback_handle));
        let _ = fs::remove_dir_all(&fallback_dir);
    }

    #[test]
    fn tool_call_path_escaping_cwd_returns_a_read_file_error() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "../etc/passwd"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("done".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("escape");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );
        assert_eq!(provider.call_count(), 2);

        let rows = trace_rows(&spec, "h1");
        let tool_result_row = rows.iter().find(|r| r["kind"] == "tool_result").expect("tool_result row");
        assert!(
            tool_result_row["content"].as_str().unwrap().starts_with("read_file error"),
            "tool_result row: {tool_result_row:?}"
        );
    }

    #[test]
    fn scripted_fatal_error_becomes_the_failure_verdict() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![Err(ProviderError::Fatal { kind: "boom".to_string() })]);
        let (delivery_tx, delivery_rx) = mpsc::channel();
        let cancel = unset_cancel();
        let steer_rx = unset_steer();

        execute_turn(&provider, &spec, &"h1".to_string(), "brief", &delivery_tx, &cancel, &steer_rx);

        match delivery_rx.try_recv().unwrap() {
            Delivery::Verdict { text, .. } => assert_eq!(text, "native hand failed — fatal: boom"),
            other => panic!("expected verdict, got {other:?}"),
        }
    }

    #[test]
    fn content_then_timeout_is_preserved_as_incomplete_evidence_and_the_hand_still_fails() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![
            Ok(Chunk::Content(ContentBlock::Text("partial before timeout".to_string()))),
            Err(ProviderError::Fatal {
                kind: "stream read error: timed out waiting for bytes".to_string(),
            }),
        ]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("say the word");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => {
                assert_eq!(text, "native hand failed — fatal: stream read error: timed out waiting for bytes");
            }
            Outcome::Killed => panic!("expected a delivered failure verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered failure verdict, got TraceFailed({err})"),
        }
        assert_eq!(window.len(), 1, "partial content must not be promoted into the window");

        let rows = trace_rows(&spec, "h1");
        let incomplete_index = rows
            .iter()
            .position(|row| row["kind"] == "incomplete")
            .unwrap_or_else(|| panic!("incomplete row missing: {rows:?}"));
        let expected_content = serde_json::to_value(vec![ContentBlock::Text("partial before timeout".to_string())]).unwrap();
        assert_eq!(rows[incomplete_index]["content"], expected_content);

        let error_index = rows
            .iter()
            .position(|row| row["kind"] == "error")
            .unwrap_or_else(|| panic!("error row missing: {rows:?}"));
        assert!(incomplete_index < error_index, "rows: {rows:?}");
        assert!(!rows.iter().any(|row| row["kind"] == "assistant"), "rows: {rows:?}");
    }

    #[test]
    fn content_then_disconnect_is_preserved_as_incomplete_evidence_and_the_hand_still_fails() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![Ok(Chunk::Content(ContentBlock::Text(
            "partial before disconnect".to_string(),
        )))]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("say the word");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => {
                assert_eq!(text, "native hand failed — stream ended without a terminal chunk");
            }
            Outcome::Killed => panic!("expected a delivered failure verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered failure verdict, got TraceFailed({err})"),
        }
        assert_eq!(window.len(), 1, "partial content must not be promoted into the window");

        let rows = trace_rows(&spec, "h1");
        let incomplete_index = rows
            .iter()
            .position(|row| row["kind"] == "incomplete")
            .unwrap_or_else(|| panic!("incomplete row missing: {rows:?}"));
        let expected_content = serde_json::to_value(vec![ContentBlock::Text("partial before disconnect".to_string())]).unwrap();
        assert_eq!(rows[incomplete_index]["content"], expected_content);

        let error_index = rows
            .iter()
            .position(|row| row["kind"] == "error")
            .unwrap_or_else(|| panic!("error row missing: {rows:?}"));
        assert!(incomplete_index < error_index, "rows: {rows:?}");
    }

    #[test]
    fn content_then_malformed_stream_is_preserved_as_incomplete_evidence_and_the_hand_still_fails() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![
            Ok(Chunk::Content(ContentBlock::Text("partial before malformed".to_string()))),
            Err(ProviderError::Fatal {
                kind: "malformed SSE data: not json".to_string(),
            }),
        ]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("say the word");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => {
                assert_eq!(text, "native hand failed — fatal: malformed SSE data: not json");
            }
            Outcome::Killed => panic!("expected a delivered failure verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered failure verdict, got TraceFailed({err})"),
        }
        assert_eq!(window.len(), 1, "partial content must not be promoted into the window");

        let rows = trace_rows(&spec, "h1");
        let incomplete_index = rows
            .iter()
            .position(|row| row["kind"] == "incomplete")
            .unwrap_or_else(|| panic!("incomplete row missing: {rows:?}"));
        let expected_content = serde_json::to_value(vec![ContentBlock::Text("partial before malformed".to_string())]).unwrap();
        assert_eq!(rows[incomplete_index]["content"], expected_content);

        let error_index = rows
            .iter()
            .position(|row| row["kind"] == "error")
            .unwrap_or_else(|| panic!("error row missing: {rows:?}"));
        assert!(incomplete_index < error_index, "rows: {rows:?}");
    }

    #[test]
    fn a_successful_terminal_stream_writes_no_incomplete_row() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![
            Ok(Chunk::Content(ContentBlock::Text("all good".to_string()))),
            Ok(Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: None,
            }),
        ]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("say the word");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => assert_eq!(text, "all good"),
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }

        let rows = trace_rows(&spec, "h1");
        assert!(!rows.iter().any(|row| row["kind"] == "incomplete"), "rows: {rows:?}");
    }

    #[test]
    fn an_error_before_any_content_writes_no_incomplete_row() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![Err(ProviderError::Fatal { kind: "boom".to_string() })]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("say the word");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => assert_eq!(text, "native hand failed — fatal: boom"),
            Outcome::Killed => panic!("expected a delivered failure verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered failure verdict, got TraceFailed({err})"),
        }

        let rows = trace_rows(&spec, "h1");
        assert!(!rows.iter().any(|row| row["kind"] == "incomplete"), "rows: {rows:?}");
    }

    #[test]
    fn content_then_error_during_state_object_authoring_is_preserved_as_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-R14-PROBE").unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "probe.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![Err(ProviderError::ContextWindowExceeded {
                overshoot: Some(Overshoot {
                    requested: 1_000_000,
                    maximum: 100,
                }),
            })],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("half-authored state".to_string()))),
                Err(ProviderError::Fatal { kind: "boom".to_string() }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read probe then continue");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => assert_eq!(text, "native hand failed — fatal: boom"),
            Outcome::Killed => panic!("expected a delivered failure verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered failure verdict, got TraceFailed({err})"),
        }
        assert_eq!(provider.call_count(), 3);

        let rows = trace_rows(&spec, "h1");
        let incomplete_index = rows
            .iter()
            .position(|row| row["kind"] == "incomplete")
            .unwrap_or_else(|| panic!("incomplete row missing: {rows:?}"));
        let expected_content = serde_json::to_value(vec![ContentBlock::Text("half-authored state".to_string())]).unwrap();
        assert_eq!(rows[incomplete_index]["content"], expected_content);

        let error_index = rows
            .iter()
            .position(|row| row["kind"] == "error")
            .unwrap_or_else(|| panic!("error row missing: {rows:?}"));
        assert!(incomplete_index < error_index, "rows: {rows:?}");
        assert!(!rows.iter().any(|row| row["kind"] == "state_object"), "rows: {rows:?}");
        assert!(!rows.iter().any(|row| row["kind"] == "respawn"), "rows: {rows:?}");
    }

    #[test]
    fn content_then_disconnect_during_state_object_authoring_is_preserved_as_incomplete_and_the_hand_still_fails() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-R14-PROBE").unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "probe.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![Err(ProviderError::ContextWindowExceeded {
                overshoot: Some(Overshoot {
                    requested: 1_000_000,
                    maximum: 100,
                }),
            })],
            vec![Ok(Chunk::Content(ContentBlock::Text("half-authored before disconnect".to_string())))],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read probe then continue");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => {
                assert_eq!(text, "native hand failed — stream ended without a terminal chunk");
            }
            Outcome::Killed => panic!("expected a delivered failure verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered failure verdict, got TraceFailed({err})"),
        }
        assert_eq!(provider.call_count(), 3);

        let rows = trace_rows(&spec, "h1");
        let incomplete_index = rows
            .iter()
            .position(|row| row["kind"] == "incomplete")
            .unwrap_or_else(|| panic!("incomplete row missing: {rows:?}"));
        let expected_content = serde_json::to_value(vec![ContentBlock::Text("half-authored before disconnect".to_string())]).unwrap();
        assert_eq!(rows[incomplete_index]["content"], expected_content);

        let error_index = rows
            .iter()
            .position(|row| row["kind"] == "error")
            .unwrap_or_else(|| panic!("error row missing: {rows:?}"));
        assert!(incomplete_index < error_index, "rows: {rows:?}");
        assert!(!rows.iter().any(|row| row["kind"] == "state_object"), "rows: {rows:?}");
        assert!(!rows.iter().any(|row| row["kind"] == "respawn"), "rows: {rows:?}");
    }

    #[test]
    fn scripted_refusal_with_no_text_becomes_the_refusal_line() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![Ok(Chunk::Terminal {
            termination: Termination::Refusal {
                category: "content_filter".to_string(),
            },
            usage: None,
        })]);
        let (delivery_tx, delivery_rx) = mpsc::channel();
        let cancel = unset_cancel();
        let steer_rx = unset_steer();

        execute_turn(&provider, &spec, &"h1".to_string(), "brief", &delivery_tx, &cancel, &steer_rx);

        match delivery_rx.try_recv().unwrap() {
            Delivery::Verdict { text, .. } => assert_eq!(text, "native hand refused: content_filter"),
            other => panic!("expected verdict, got {other:?}"),
        }
    }

    #[test]
    fn kill_sent_before_stream_consumed_yields_no_delivery() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![
            Ok(Chunk::Content(ContentBlock::Text("should never surface".to_string()))),
            Ok(Chunk::Terminal {
                termination: Termination::EndTurn,
                usage: None,
            }),
        ]);
        let (delivery_tx, delivery_rx) = mpsc::channel();
        let cancel: Cancel = Arc::new(AtomicBool::new(true));
        let steer_rx = unset_steer();

        execute_turn(&provider, &spec, &"h1".to_string(), "brief", &delivery_tx, &cancel, &steer_rx);

        assert!(delivery_rx.try_recv().is_err());
    }

    #[test]
    fn tool_call_under_max_tokens_termination_is_not_executed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-L1-10-PROBE").unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![
            Ok(Chunk::Content(ContentBlock::ToolCall {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "probe.txt"}),
            })),
            Ok(Chunk::Terminal {
                termination: Termination::MaxTokens,
                usage: None,
            }),
        ]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read the probe");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );
        match outcome {
            Outcome::Delivered { text, .. } => {
                assert!(text.contains("MaxTokens"), "text: {text}");
                assert!(!text.contains("tool call(s) executed"), "text: {text}");
            }
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }

        assert_eq!(window.len(), 2, "no tool message should be appended");
        let rows = trace_rows(&spec, "h1");
        assert!(!rows.iter().any(|r| r["kind"] == "gate"), "rows: {rows:?}");
        assert!(!rows.iter().any(|r| r["kind"] == "tool_result"), "rows: {rows:?}");
        assert_eq!(provider.call_count(), 1);
    }

    #[test]
    fn tool_call_under_refusal_termination_is_not_executed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-L1-10-PROBE").unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![
            Ok(Chunk::Content(ContentBlock::ToolCall {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "probe.txt"}),
            })),
            Ok(Chunk::Terminal {
                termination: Termination::Refusal {
                    category: "content_filter".to_string(),
                },
                usage: None,
            }),
        ]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read the probe");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );
        match outcome {
            Outcome::Delivered { text, .. } => assert_eq!(text, "native hand refused: content_filter"),
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }

        let rows = trace_rows(&spec, "h1");
        assert!(!rows.iter().any(|r| r["kind"] == "gate"), "rows: {rows:?}");
        assert!(!rows.iter().any(|r| r["kind"] == "tool_result"), "rows: {rows:?}");
    }

    #[test]
    fn an_unknown_tool_name_does_not_fall_through_to_read_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-L1-10-PROBE").unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "shell_exec".to_string(),
                    arguments: serde_json::json!({"path": "probe.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("done".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("brief");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        let rows = trace_rows(&spec, "h1");
        let tool_result_row = rows.iter().find(|r| r["kind"] == "tool_result").expect("tool_result row");
        let content = tool_result_row["content"].as_str().unwrap();
        assert_eq!(content, "unknown tool 'shell_exec'");
        assert!(!content.contains("ARX-L1-10-PROBE"), "content: {content}");
    }

    #[test]
    fn a_trace_write_failure_stops_the_turn_instead_of_a_silent_success() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![Ok(Chunk::Terminal {
            termination: Termination::EndTurn,
            usage: None,
        })]);
        let cancel = unset_cancel();
        let full_file = File::create("/dev/full").expect("/dev/full is a standard Linux device");
        let mut trace = Trace {
            file: full_file,
            path: PathBuf::from("/dev/full"),
        };
        let mut window = Window::new("brief");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );
        match outcome {
            Outcome::TraceFailed(err) => assert!(!err.is_empty()),
            Outcome::Delivered { text, .. } => panic!("expected TraceFailed, got Delivered({text})"),
            Outcome::Killed => panic!("expected TraceFailed, got Killed"),
        }
    }

    #[test]
    fn execute_turn_reports_a_trace_write_failure_with_no_trace_path() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        fs::create_dir_all(&spec.threads_dir).unwrap();
        let trace_path = spec.threads_dir.join(format!("{}.h1.jsonl", spec.thread));
        std::os::unix::fs::symlink("/dev/full", &trace_path).unwrap();
        let provider = ScriptedProvider::new(vec![Ok(Chunk::Terminal {
            termination: Termination::EndTurn,
            usage: None,
        })]);
        let (delivery_tx, delivery_rx) = mpsc::channel();
        let cancel = unset_cancel();
        let steer_rx = unset_steer();

        execute_turn(&provider, &spec, &"h1".to_string(), "brief", &delivery_tx, &cancel, &steer_rx);

        match delivery_rx.try_recv().unwrap() {
            Delivery::Verdict { text, trace, .. } => {
                assert!(text.contains("native hand failed — trace write error"), "text: {text}");
                assert_eq!(trace, None);
            }
            other => panic!("expected verdict, got {other:?}"),
        }
    }

    #[test]
    fn tool_use_then_end_turn_makes_two_calls_with_the_tool_result_in_the_second() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-L1-10-PROBE").unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "probe.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("done".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read the probe");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => assert_eq!(text, "done"),
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }
        assert_eq!(provider.call_count(), 2);

        let second_call_messages = provider.messages_at(1);
        let roles: Vec<Role> = second_call_messages.iter().map(|m| m.role).collect();
        assert_eq!(roles, vec![Role::User, Role::Assistant, Role::Tool]);
        match &second_call_messages[2].content[0] {
            ContentBlock::ToolResult { call_id, content } => {
                assert_eq!(call_id, "call_1");
                assert!(content.contains("ARX-L1-10-PROBE"), "content: {content}");
            }
            other => panic!("expected a ToolResult block, got {other:?}"),
        }
    }

    #[test]
    fn the_step_cap_ends_a_spinning_tool_loop_with_a_named_verdict() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-L1-10-PROBE").unwrap();
        let mut spec = sample_spec(dir.path());
        spec.steps = 3;
        let spinning_step = || {
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "probe.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ]
        };
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![spinning_step(), spinning_step(), spinning_step()]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("spin forever");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => assert_eq!(text, "native hand stopped at the step cap (3 steps)"),
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }
        assert_eq!(provider.call_count(), 3);

        let rows = trace_rows(&spec, "h1");
        let gate_rows = rows.iter().filter(|r| r["kind"] == "gate").count();
        assert_eq!(gate_rows, 2, "only the first two steps should execute a tool call: {rows:?}");
    }

    #[test]
    fn max_tokens_after_a_tool_step_ends_the_brief() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-L1-10-PROBE").unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "probe.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("truncated".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::MaxTokens,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read then stop");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, .. } => assert_eq!(text, "truncated"),
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }
        assert_eq!(provider.call_count(), 2);
    }

    #[test]
    fn a_deny_rule_returns_the_reason_as_the_tool_result_and_executes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("secret.txt"), "top secret").unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "secret.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("done".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read the secret");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let repo = GateConfig {
            deny: vec!["read_file(secret*)".to_string()],
            ..GateConfig::default()
        };
        let mut gate = Table::new(&repo, "repo", &GateConfig::default(), "user").unwrap();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        let rows = trace_rows(&spec, "h1");
        let gate_row = rows.iter().find(|r| r["kind"] == "gate").expect("gate row");
        assert_eq!(gate_row["verdict"], "deny");
        let tool_result_row = rows.iter().find(|r| r["kind"] == "tool_result").expect("tool_result row");
        let content = tool_result_row["content"].as_str().unwrap();
        assert!(content.starts_with("denied:"), "content: {content}");
        assert!(!content.contains("top secret"), "content: {content}");
    }

    #[test]
    fn recall_is_only_on_the_surface_when_listed() {
        let dir = tempfile::tempdir().unwrap();
        let mut spec = sample_spec(dir.path());
        spec.tools = vec!["read_file".to_string()];
        let provider = ScriptedProvider::new(vec![Ok(Chunk::Terminal {
            termination: Termination::EndTurn,
            usage: None,
        })]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("brief");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        let names: Vec<String> = provider.tools_at(0).iter().map(|t| t.name.clone()).collect();
        assert_eq!(names, vec!["read_file".to_string()], "tools offered: {names:?}");
    }

    #[test]
    fn recall_returns_the_engine_claims_as_json() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path().join("store");
        fs::create_dir_all(store_root.join("testscope")).unwrap();
        let hot_id = {
            let mut seed = Engine::open(&store_root, "seed").unwrap();
            let handle = arx_engine::ClaimHandle {
                thread: "seed".to_string(),
                turn: 1,
            };
            seed.save(
                &arx_engine::SaveIntent {
                    scope: "testscope".to_string(),
                    kind: arx_engine::ClaimKind::Fact,
                    question: "what is the launch codename".to_string(),
                    answer: "skylark".to_string(),
                    why: "ops handoff".to_string(),
                    supersedes: None,
                },
                &handle,
            )
            .unwrap();
            seed.drain().unwrap();
            seed.recall("testscope", "launch codename", 1).unwrap()[0].id.clone()
        };
        let mut spec = sample_spec(dir.path());
        spec.store_root = store_root;
        spec.store_scope = Some("testscope".to_string());
        spec.tools = vec!["read_file".to_string(), "recall".to_string()];

        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "recall".to_string(),
                    arguments: serde_json::json!({"query": "launch codename"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("done".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("recall the codename");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        let rows = trace_rows(&spec, "h1");
        let tool_result_row = rows.iter().find(|r| r["kind"] == "tool_result").expect("tool_result row");
        let content = tool_result_row["content"].as_str().unwrap();
        assert!(content.contains("skylark"), "content: {content}");
        assert!(content.contains(&hot_id), "content: {content}");

        let quarantine_dir = spec.store_root.join("quarantine").join("hand-h1");
        assert!(!quarantine_dir.exists(), "quarantine dir left behind: {}", quarantine_dir.display());
    }

    fn spawn_stalling_openai_server() -> (String, Arc<AtomicBool>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let disconnected = Arc::new(AtomicBool::new(false));
        let disconnected_for_thread = Arc::clone(&disconnected);
        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let mut buf = [0u8; 4096];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => {
                        if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                }
            }
            if stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n: keep-alive\n\n")
                .is_err()
            {
                disconnected_for_thread.store(true, Ordering::SeqCst);
                return;
            }
            let deadline = Instant::now() + Duration::from_secs(30);
            while Instant::now() < deadline {
                thread::sleep(Duration::from_millis(100));
                if stream.write_all(b": keep-alive\n\n").is_err() {
                    disconnected_for_thread.store(true, Ordering::SeqCst);
                    return;
                }
            }
        });
        (format!("http://{addr}"), disconnected)
    }

    #[test]
    fn a_kill_during_a_keep_alive_stall_ends_the_thread_within_five_seconds() {
        let (base_url, disconnected) = spawn_stalling_openai_server();
        let dir = tempfile::tempdir().unwrap();
        let mut spec = sample_spec(dir.path());
        let key_env = "ARX_L2_10A_KILL_TEST_KEY";
        spec.provider = Some(ProviderSpec {
            kind: ProviderKind::OpenAiCompatible,
            base_url,
            api_key_env: key_env.to_string(),
        });
        unsafe {
            std::env::set_var(key_env, "test-key");
        }

        let (delivery_tx, delivery_rx) = mpsc::channel();
        let (control_tx, control_rx) = mpsc::channel();

        let start = Instant::now();
        let handle = spawn_native_hand_threads(spec, "h1".to_string(), "say the word".to_string(), delivery_tx, control_rx)
            .expect("a configured provider and a present key spawn a hand thread");

        thread::sleep(Duration::from_millis(300));
        control_tx.send(Control::Kill).unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        while !handle.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(handle.is_finished(), "native hand thread did not end within 5 seconds of the kill");
        handle.join().unwrap();
        assert!(start.elapsed() < Duration::from_secs(5), "elapsed: {:?}", start.elapsed());

        assert!(delivery_rx.try_recv().is_err());

        let disconnect_deadline = Instant::now() + Duration::from_secs(5);
        while !disconnected.load(Ordering::SeqCst) && Instant::now() < disconnect_deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(disconnected.load(Ordering::SeqCst), "fake server never observed a disconnect");

        unsafe {
            std::env::remove_var(key_env);
        }
    }

    #[test]
    fn a_miss_mid_brief_rewinds_authors_and_respawns_then_finishes() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("big.txt"), "y".repeat(20000)).unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "big.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![Err(ProviderError::ContextWindowExceeded {
                overshoot: Some(Overshoot {
                    requested: 1_000_000,
                    maximum: 100,
                }),
            })],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("object text".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("ARX-L2-13-DONE".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read big then continue");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, miss, .. } => {
                assert_eq!(text, "ARX-L2-13-DONE");
                assert_eq!(miss, None);
            }
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }
        assert_eq!(provider.call_count(), 4);

        let third_call_messages = provider.messages_at(2);
        assert_eq!(third_call_messages, vec![Message::user("read big then continue"), Message::user(RESPAWN_PROMPT)]);

        let fourth_call_messages = provider.messages_at(3);
        assert_eq!(
            fourth_call_messages,
            vec![
                Message::user("read big then continue"),
                Message::user("[arx] state object from before the reset:\nobject text"),
            ]
        );

        let rows = trace_rows(&spec, "h1");
        let kinds: Vec<&str> = rows
            .iter()
            .filter_map(|row| row["kind"].as_str())
            .filter(|kind| ["miss", "rewind", "state_object", "respawn"].contains(kind))
            .collect();
        assert_eq!(kinds, vec!["miss", "rewind", "state_object", "respawn"], "rows: {rows:?}");

        let archive = fs::read_to_string(spec.threads_dir.join("rewind-1.jsonl")).unwrap();
        let archived: Vec<serde_json::Value> = archive.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(archived.len(), 2, "archive: {archive}");
    }

    #[test]
    fn a_miss_inside_authoring_rewinds_again_down_to_the_floor() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("big1.txt"), "y".repeat(20000)).unwrap();
        fs::write(dir.path().join("big2.txt"), "z".repeat(20000)).unwrap();
        let mut spec = sample_spec(dir.path());
        spec.admission.threshold_tokens = 1_000_000;
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "big1.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_2".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "big2.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![Err(ProviderError::ContextWindowExceeded {
                overshoot: Some(Overshoot { requested: 100, maximum: 1 }),
            })],
            vec![Err(ProviderError::ContextWindowExceeded {
                overshoot: Some(Overshoot { requested: 90, maximum: 1 }),
            })],
            vec![Err(ProviderError::ContextWindowExceeded {
                overshoot: Some(Overshoot { requested: 80, maximum: 1 }),
            })],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("floor object".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("ARX-L2-13-FLOOR".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read both then continue");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, miss, .. } => {
                assert_eq!(text, "ARX-L2-13-FLOOR");
                assert_eq!(miss, None);
            }
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }
        assert_eq!(provider.call_count(), 7);

        let rows = trace_rows(&spec, "h1");
        let kinds: Vec<&str> = rows
            .iter()
            .filter_map(|row| row["kind"].as_str())
            .filter(|kind| ["miss", "rewind", "state_object", "respawn"].contains(kind))
            .collect();
        assert_eq!(
            kinds,
            vec!["miss", "rewind", "miss", "rewind", "miss", "rewind", "state_object", "respawn"],
            "rows: {rows:?}"
        );

        let archive = fs::read_to_string(spec.threads_dir.join("rewind-1.jsonl")).unwrap();
        let archived: Vec<serde_json::Value> = archive.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(archived.len(), 4, "archive: {archive}");
    }

    #[test]
    fn a_third_miss_ends_the_brief_loud() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-L2-13-PROBE").unwrap();
        let spec = sample_spec(dir.path());
        let huge_overshoot = || {
            Some(Overshoot {
                requested: 1_000_000,
                maximum: 1,
            })
        };
        let provider = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "probe.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![Err(ProviderError::ContextWindowExceeded { overshoot: huge_overshoot() })],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("state one".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
            vec![Err(ProviderError::ContextWindowExceeded { overshoot: huge_overshoot() })],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("state two".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
            vec![Err(ProviderError::ContextWindowExceeded {
                overshoot: Some(Overshoot { requested: 5000, maximum: 4000 }),
            })],
        ]));
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("keep going past two respawns");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, miss, .. } => {
                assert_eq!(text, "native hand failed — context window exceeded after 2 respawns");
                assert_eq!(miss, Some(1000));
            }
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }
        assert_eq!(provider.call_count(), 6);

        let rows = trace_rows(&spec, "h1");
        let respawn_rows: Vec<&serde_json::Value> = rows.iter().filter(|row| row["kind"] == "respawn").collect();
        assert_eq!(respawn_rows.len(), 2, "rows: {rows:?}");
        assert_eq!(respawn_rows[0]["ordinal"], 1);
        assert_eq!(respawn_rows[1]["ordinal"], 2);
    }

    #[test]
    fn a_miss_on_the_first_step_ends_the_brief_as_before() {
        let dir = tempfile::tempdir().unwrap();
        let spec = sample_spec(dir.path());
        let provider = ScriptedProvider::new(vec![Err(ProviderError::ContextWindowExceeded {
            overshoot: Some(Overshoot {
                requested: 10000,
                maximum: 8192,
            }),
        })]);
        let cancel = unset_cancel();
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("say the word");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, _delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        match outcome {
            Outcome::Delivered { text, miss, .. } => {
                assert_eq!(text, "native hand failed — context window exceeded (requested 10000, maximum 8192)");
                assert_eq!(miss, Some(1808));
            }
            Outcome::Killed => panic!("expected a delivered verdict, got Killed"),
            Outcome::TraceFailed(err) => panic!("expected a delivered verdict, got TraceFailed({err})"),
        }
        assert_eq!(provider.call_count(), 1);
        assert_eq!(window.len(), 1, "nothing should be appended to the window");

        let rows = trace_rows(&spec, "h1");
        let miss_row = rows.iter().find(|row| row["kind"] == "miss").expect("miss row");
        assert!(miss_row["text"].as_str().unwrap().contains("context window exceeded"), "miss row: {miss_row:?}");
        assert!(!rows.iter().any(|row| row["kind"] == "rewind"), "rows: {rows:?}");
    }

    #[test]
    fn a_kill_between_stages_ends_the_brief_with_no_delivery() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("probe.txt"), "ARX-L2-13-PROBE").unwrap();
        let spec = sample_spec(dir.path());

        struct KillAfter {
            inner: ScriptedProvider,
            cancel: Cancel,
            trigger_call: usize,
        }

        impl Provider for KillAfter {
            fn complete(&self, system: &str, messages: &[Message], tools: &[ToolDef]) -> Result<ChunkStream, ProviderError> {
                let result = self.inner.complete(system, messages, tools);
                if self.inner.call_count() == self.trigger_call {
                    self.cancel.store(true, Ordering::SeqCst);
                }
                result
            }
        }

        let inner = ScriptedProvider::sequence(VecDeque::from(vec![
            vec![
                Ok(Chunk::Content(ContentBlock::ToolCall {
                    id: "call_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({"path": "probe.txt"}),
                })),
                Ok(Chunk::Terminal {
                    termination: Termination::ToolUse,
                    usage: None,
                }),
            ],
            vec![Err(ProviderError::ContextWindowExceeded {
                overshoot: Some(Overshoot {
                    requested: 1_000_000,
                    maximum: 1,
                }),
            })],
            vec![
                Ok(Chunk::Content(ContentBlock::Text("state text".to_string()))),
                Ok(Chunk::Terminal {
                    termination: Termination::EndTurn,
                    usage: None,
                }),
            ],
        ]));
        let cancel = unset_cancel();
        let provider = KillAfter {
            inner,
            cancel: cancel.clone(),
            trigger_call: 3,
        };
        let mut trace = open_trace(&spec, "h1");
        let mut window = Window::new("read the probe");
        let host = LocalHost { cwd: spec.cwd.clone() };
        let mut gate = allow_all_gate();
        let steer_rx = unset_steer();
        let (delivery_tx, delivery_rx) = mpsc::channel();

        let outcome = run_brief(
            &provider,
            &spec,
            &"h1".to_string(),
            &mut trace,
            &cancel,
            &mut window,
            &host,
            &mut gate,
            &steer_rx,
            &delivery_tx,
        );

        assert!(matches!(outcome, Outcome::Killed), "expected Killed");
        assert_eq!(provider.inner.call_count(), 3);
        assert!(delivery_rx.try_recv().is_err());
    }
}
