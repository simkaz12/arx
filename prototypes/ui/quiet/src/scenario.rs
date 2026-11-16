use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::state::{INITIAL_CHECKPOINT, INITIAL_MODEL, MODELS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Hand,
    Judge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryOutcome {
    Verdict,
    Stop,
    Killed,
}

impl DeliveryOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            DeliveryOutcome::Verdict => "verdict",
            DeliveryOutcome::Stop => "stop",
            DeliveryOutcome::Killed => "killed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeLevel {
    Routine,
    Nudge,
    Error,
}

#[derive(Debug, Clone)]
pub enum Admission {
    Inline { bytes: u64 },
    Externalized { bytes: u64, handle: String, preview: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelListReason {
    Requested,
    Unlisted,
}

#[derive(Debug, Clone)]
pub enum Event {
    Chunk(String),
    TurnEnd(String),
    ToolCall { tool: String, subject: String },
    ToolResult { admission: Admission },
    GateAsk { tool: String, subject: String },
    GateVerdict { decision: GateDecision, rule: Option<String> },
    Spawn { hand_id: String, agent: String, role: Role, body: String, container: String, model: String },
    Delivered {
        hand_id: String,
        outcome: DeliveryOutcome,
        text: String,
        trace: String,
        bytes: u64,
        elapsed_ms: u64,
        lag_ms: u64,
        schema_ok: Option<bool>,
    },
    Notice { text: String, level: NoticeLevel },
    Usage {
        input: u64,
        output: u64,
        cache_read: u64,
        model: String,
        model_source: String,
        elapsed_ms: u64,
        occupancy_after: u64,
    },
    Checkpoint { number: u32, path: String, delta: i64 },
    ResetArmed { occupancy: u64, window: u64, reserve: u64 },
    ResetConfirm { body: String },
    Orientation { text: String, verdict: String, occupancy_after: u64 },
    ModelList { models: Vec<String>, current: String, reason: ModelListReason, attempted: Option<String> },
    ModelSwitched { from: String, to: String },
    ProviderError { message: String, recoverable: bool },
    Aborted,
}

#[derive(Debug, Clone)]
pub enum OperatorInput {
    Line(String),
    Abort,
}

pub fn spawn(fast: bool) -> (Receiver<Event>, Sender<OperatorInput>) {
    let (event_tx, event_rx) = mpsc::channel::<Event>();
    let (operator_tx, operator_rx) = mpsc::channel::<OperatorInput>();
    thread::spawn(move || run(fast, event_tx, operator_rx));
    (event_rx, operator_tx)
}

fn sleep(fast: bool, ms: u64) {
    if !fast && ms > 0 {
        thread::sleep(Duration::from_millis(ms));
    }
}

fn tokenize(text: &str, per_chunk: usize) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if ch == ' ' || ch == '\n' {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
        .chunks(per_chunk.max(1))
        .map(|group| group.concat())
        .collect()
}

fn stream(tx: &Sender<Event>, rx: &Receiver<OperatorInput>, text: &str, fast: bool) -> bool {
    sleep(fast, 700);
    let delay_ms: u64 = if fast { 1 } else { 140 };
    for piece in tokenize(text, 6) {
        let _ = tx.send(Event::Chunk(piece));
        if let Ok(OperatorInput::Abort) = rx.recv_timeout(Duration::from_millis(delay_ms)) {
            return false;
        }
    }
    true
}

fn notice(tx: &Sender<Event>, text: &str, level: NoticeLevel) {
    let _ = tx.send(Event::Notice { text: text.to_string(), level });
}

const CHECKPOINT_BODY: &str = "position   polishing pass, day 1; pid.txt removed, /model landed, doubled reply fixed\ntarget     arx usable as the daily driver for arx work (L3-04)\ndrift      none; terminal look still unruled (44afcdc8)\nrelies on  580e402e, 57aa480a";

fn run(fast: bool, tx: Sender<Event>, rx: Receiver<OperatorInput>) {
    notice(
        &tx,
        &format!(
            "scope {}, store {}; grounding: {}",
            crate::state::SCOPE,
            crate::state::STORE_ROOT,
            crate::state::GROUNDING
        ),
        NoticeLevel::Routine,
    );

    let mut current_model = INITIAL_MODEL.to_string();
    let mut checkpoint_number = INITIAL_CHECKPOINT;
    let mut step: u32 = 0;

    loop {
        let input = match rx.recv() {
            Ok(msg) => msg,
            Err(_) => break,
        };
        match input {
            OperatorInput::Line(line) => {
                let trimmed = line.trim();
                if trimmed == "/model" {
                    let _ = tx.send(Event::ModelList {
                        models: MODELS.iter().map(|s| s.to_string()).collect(),
                        current: current_model.clone(),
                        reason: ModelListReason::Requested,
                        attempted: None,
                    });
                } else if let Some(id) = trimmed.strip_prefix("/model ") {
                    apply_model_target(id.trim(), &mut current_model, &tx);
                } else if trimmed == "/checkpoint" {
                    scene_checkpoint(&mut checkpoint_number, &tx, &rx, fast);
                } else if trimmed == "/reset" {
                    scene_reset(&tx, &rx, fast, checkpoint_number, &mut current_model);
                } else if trimmed.starts_with('/') {
                    notice(&tx, "unknown command; try /reset, /model, /checkpoint", NoticeLevel::Routine);
                } else {
                    step += 1;
                    if !run_step(step, &tx, &rx, fast, &mut current_model) {
                        break;
                    }
                }
            }
            OperatorInput::Abort => {}
        }
    }
}

fn apply_model_target(id: &str, current: &mut String, tx: &Sender<Event>) {
    if !MODELS.contains(&id) {
        let _ = tx.send(Event::ModelList {
            models: MODELS.iter().map(|s| s.to_string()).collect(),
            current: current.clone(),
            reason: ModelListReason::Unlisted,
            attempted: Some(id.to_string()),
        });
    } else if id == current {
        notice(tx, &format!("already {id}"), NoticeLevel::Routine);
    } else {
        let from = current.clone();
        *current = id.to_string();
        let _ = tx.send(Event::ModelSwitched { from, to: id.to_string() });
    }
}

fn run_step(step: u32, tx: &Sender<Event>, rx: &Receiver<OperatorInput>, fast: bool, current_model: &mut String) -> bool {
    match step {
        1 => scene1(tx, rx, fast, current_model),
        2 => scene2(tx, rx, fast, current_model),
        3 => scene3_dispatch(tx, rx, fast),
        4 => scene3_fold(tx, rx, fast),
        5 => scene5_nudge(tx),
        6 => scene6_abort(tx, rx, fast),
        7 => scene6_retry(tx, rx, fast),
        _ => {
            notice(tx, "session script exhausted; nothing left to play", NoticeLevel::Routine);
        }
    }
    true
}

const SCENE1_TEXT: &str = "The store root moved out of the repo in L3-05, so a fresh clone has no `store/` at all. Two things follow:\n\n- `arx run` refuses an unregistered scope, and registration is still a hand-typed `mkdir` under `~/.local/share/arx/store`.\n- The tracked `pid.txt` is a leftover of the L1-close session. Nothing writes it now.\n\nI'd delete `pid.txt` in the polishing pass and leave the store registration as it is.";

fn scene1(tx: &Sender<Event>, rx: &Receiver<OperatorInput>, fast: bool, current_model: &str) {
    if !stream(tx, rx, SCENE1_TEXT, fast) {
        let _ = tx.send(Event::Aborted);
        return;
    }
    let _ = tx.send(Event::TurnEnd(SCENE1_TEXT.to_string()));
    let _ = tx.send(Event::Usage {
        input: 18_240,
        output: 142,
        cache_read: 16_900,
        model: current_model.to_string(),
        model_source: "sdk".to_string(),
        elapsed_ms: 3_180,
        occupancy_after: 63_900,
    });
}

const SCENE2_PRE: &str = "terminal.rs is still the L1-04 adapter; let me see what the tests pin down.";
const SCENE2_POST: &str = "Eleven integration tests pin the byte shape of piped output, so a richer terminal has to keep piped mode byte-identical and add the new look only when stdin is a tty.";
const BASH_SUBJECT: &str = "cargo test -p arx --test claude_main 2>&1 | tail -40";

fn scene2(tx: &Sender<Event>, rx: &Receiver<OperatorInput>, fast: bool, current_model: &str) {
    let _ = tx.send(Event::ToolCall { tool: "Read".to_string(), subject: "crates/arx/src/terminal.rs".to_string() });
    let _ = tx.send(Event::GateVerdict { decision: GateDecision::Allow, rule: Some("Read *".to_string()) });
    let _ = tx.send(Event::ToolResult { admission: Admission::Inline { bytes: 6_312 } });

    if !stream(tx, rx, SCENE2_PRE, fast) {
        let _ = tx.send(Event::Aborted);
        return;
    }

    let _ = tx.send(Event::ToolCall { tool: "Bash".to_string(), subject: BASH_SUBJECT.to_string() });
    let _ = tx.send(Event::GateAsk { tool: "Bash".to_string(), subject: BASH_SUBJECT.to_string() });
    let key = loop {
        match rx.recv() {
            Ok(OperatorInput::Line(l)) => {
                break l.trim_start().chars().next().unwrap_or('d');
            }
            Ok(OperatorInput::Abort) => break 'd',
            Err(_) => return,
        }
    };
    let rule = if key == 'A' || key == 'D' {
        Some("Bash cargo test -p arx*".to_string())
    } else {
        None
    };
    let _ = tx.send(Event::GateVerdict { decision: GateDecision::Allow, rule });
    let _ = tx.send(Event::ToolResult {
        admission: Admission::Externalized {
            bytes: 14_803,
            handle: "trace/2026-09-08/t07-bash-3.txt".to_string(),
            preview: "test result: ok. 11 passed; 0 failed".to_string(),
        },
    });

    if !stream(tx, rx, SCENE2_POST, fast) {
        let _ = tx.send(Event::Aborted);
        return;
    }
    let canonical = format!("{SCENE2_PRE}\n\n{SCENE2_POST}");
    let _ = tx.send(Event::TurnEnd(canonical));
    let _ = tx.send(Event::Usage {
        input: 33_110,
        output: 96,
        cache_read: 18_240,
        model: current_model.to_string(),
        model_source: "sdk".to_string(),
        elapsed_ms: 9_640,
        occupancy_after: 79_300,
    });
}

const SCENE3_DISPATCH_TEXT: &str = "Dispatched hand-do to delete `pid.txt` and drop its mention from CLAUDE.md, and the codex judge to review that diff once it lands. I'll fold both when they report.";
const SCENE3_FOLD_TEXT: &str = "Both back. The hand's diff is in the working tree, uncommitted, and the judge found nothing. Your call whether to commit it with the rest of the polishing work.";

fn scene3_dispatch(tx: &Sender<Event>, rx: &Receiver<OperatorInput>, fast: bool) {
    let _ = tx.send(Event::Spawn {
        hand_id: "h-0007".to_string(),
        agent: "hand-do".to_string(),
        role: Role::Hand,
        body: "claude-code".to_string(),
        container: "arx-hand-claude:2.1.258".to_string(),
        model: "claude-sonnet-5".to_string(),
    });
    let _ = tx.send(Event::GateVerdict { decision: GateDecision::Allow, rule: Some("dispatch *".to_string()) });
    let _ = tx.send(Event::Spawn {
        hand_id: "h-0008".to_string(),
        agent: "codex-judge".to_string(),
        role: Role::Judge,
        body: "codex".to_string(),
        container: "arx-hand-codex:0.152.1".to_string(),
        model: "gpt-5.4".to_string(),
    });
    let _ = tx.send(Event::GateVerdict { decision: GateDecision::Allow, rule: Some("dispatch *".to_string()) });

    if stream(tx, rx, SCENE3_DISPATCH_TEXT, fast) {
        let _ = tx.send(Event::TurnEnd(SCENE3_DISPATCH_TEXT.to_string()));
    } else {
        let _ = tx.send(Event::Aborted);
        return;
    }

    let tx1 = tx.clone();
    let fast1 = fast;
    thread::spawn(move || {
        sleep(fast1, 6_000);
        let _ = tx1.send(Event::Delivered {
            hand_id: "h-0007".to_string(),
            outcome: DeliveryOutcome::Verdict,
            text: "PASS: pid.txt removed, CLAUDE.md hazard line rewritten, 23 tests green".to_string(),
            trace: "trace/2026-09-08/h-0007.md".to_string(),
            bytes: 2_118,
            elapsed_ms: 5_940,
            lag_ms: 12,
            schema_ok: None,
        });
    });
    let tx2 = tx.clone();
    thread::spawn(move || {
        sleep(fast, 9_000);
        let _ = tx2.send(Event::Delivered {
            hand_id: "h-0008".to_string(),
            outcome: DeliveryOutcome::Verdict,
            text: "no findings; the hazard paragraph now says nothing writes pid.txt, which matches the tree".to_string(),
            trace: "trace/2026-09-08/h-0008.md".to_string(),
            bytes: 611,
            elapsed_ms: 8_870,
            lag_ms: 8,
            schema_ok: Some(true),
        });
    });
}

fn scene3_fold(tx: &Sender<Event>, rx: &Receiver<OperatorInput>, fast: bool) {
    if !stream(tx, rx, SCENE3_FOLD_TEXT, fast) {
        let _ = tx.send(Event::Aborted);
        return;
    }
    let _ = tx.send(Event::TurnEnd(SCENE3_FOLD_TEXT.to_string()));
}

fn scene5_nudge(tx: &Sender<Event>) {
    notice(
        tx,
        &format!("main at 121800 tokens, past {}", crate::state::NUDGE_THRESHOLD),
        NoticeLevel::Nudge,
    );
    let _ = tx.send(Event::ResetArmed { occupancy: 161_200, window: 200_000, reserve: 40_000 });
}

fn scene_checkpoint(checkpoint_number: &mut u32, tx: &Sender<Event>, rx: &Receiver<OperatorInput>, fast: bool) {
    notice(tx, "Write the state object: position, target, drift. Cite decision ids you rely on.", NoticeLevel::Routine);
    if !stream(tx, rx, CHECKPOINT_BODY, fast) {
        let _ = tx.send(Event::Aborted);
        return;
    }
    let _ = tx.send(Event::TurnEnd(CHECKPOINT_BODY.to_string()));

    notice(tx, "Check every claim above against the tree and the store.", NoticeLevel::Routine);
    if !stream(tx, rx, "all four hold", fast) {
        let _ = tx.send(Event::Aborted);
        return;
    }
    let _ = tx.send(Event::TurnEnd("all four hold".to_string()));

    *checkpoint_number += 1;
    let _ = tx.send(Event::Checkpoint {
        number: *checkpoint_number,
        path: format!("threads/main.checkpoints/{}.md", checkpoint_number),
        delta: 58_400,
    });
}

const ORIENTATION_TEXT: &str = "Resuming from checkpoint 3. Position and target are clear, the drift line is empty, both decision ids resolve. Ready.";

fn scene_reset(tx: &Sender<Event>, rx: &Receiver<OperatorInput>, fast: bool, _checkpoint_number: u32, current_model: &mut String) {
    let _ = tx.send(Event::ResetConfirm { body: CHECKPOINT_BODY.to_string() });
    let _confirm = match rx.recv() {
        Ok(OperatorInput::Line(l)) => l,
        _ => return,
    };

    notice(tx, "tearing down main", NoticeLevel::Routine);
    sleep(fast, 400);
    notice(tx, "respawning main", NoticeLevel::Routine);
    sleep(fast, 400);

    *current_model = INITIAL_MODEL.to_string();
    if !stream(tx, rx, ORIENTATION_TEXT, fast) {
        let _ = tx.send(Event::Aborted);
        return;
    }
    let _ = tx.send(Event::Orientation {
        text: ORIENTATION_TEXT.to_string(),
        verdict: "ready".to_string(),
        occupancy_after: 9_120,
    });
}

fn scene6_abort(tx: &Sender<Event>, rx: &Receiver<OperatorInput>, fast: bool) {
    let filler = "Here's the shape of the remaining polishing backlog, roughly in the order I'd tackle it. None of this is urgent, and all of it can wait for a calmer session.\n\nFirst the terminal look, since that's what you're staring at every turn; then the gate file's growing rule list, which could use a pass to collapse near-duplicates; then a look at whether the checkpoint cadence still matches how fast occupancy actually grows in practice.";
    if !stream(tx, rx, filler, fast) {
        let _ = tx.send(Event::Aborted);
        return;
    }
    let _ = tx.send(Event::TurnEnd(filler.to_string()));
}

fn scene6_retry(tx: &Sender<Event>, rx: &Receiver<OperatorInput>, fast: bool) {
    let _ = tx.send(Event::ProviderError {
        message: "overloaded_error: retry after 2 s".to_string(),
        recoverable: true,
    });
    sleep(fast, 2_000);
    let text = "Back. Where were we?";
    if !stream(tx, rx, text, fast) {
        let _ = tx.send(Event::Aborted);
        return;
    }
    let _ = tx.send(Event::TurnEnd(text.to_string()));
}
