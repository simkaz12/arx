use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Hand,
    Judge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Verdict,
    #[allow(dead_code)]
    Stop,
    #[allow(dead_code)]
    Killed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSource {
    Sdk,
    #[allow(dead_code)]
    Assumed,
}

#[derive(Debug, Clone)]
pub enum Admission {
    Inline { bytes: u32 },
    Externalized { bytes: u32, handle: String, preview: String },
}

#[derive(Debug, Clone)]
pub enum TraceTurn {
    Text(String),
    ToolCall { tool: String, args: String },
    ToolResult(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Nudge,
    ResetArmed,
    Error,
}

#[derive(Debug, Clone)]
pub enum Event {
    Chunk { text: String },
    TurnEnd { text: String },
    ExchangeEnd,
    ToolCall { tool: String, subject: String },
    ToolResult { admission: Admission },
    GateAsk { tool: String, subject: String },
    GateVerdict { allowed: bool, rule: String },
    Spawn { id: String, agent: String, role: Role, body: String, container: String, model: String, brief: String },
    Delivered {
        id: String,
        outcome: Outcome,
        text: String,
        trace: String,
        bytes: u32,
        elapsed_ms: u32,
        lag_ms: u32,
        schema_ok: Option<bool>,
        turns: Vec<TraceTurn>,
    },
    Notice { kind: NoticeKind, text: String },
    Usage {
        input: u32,
        output: u32,
        cache_read: u32,
        model: String,
        source: ModelSource,
        elapsed_ms: u32,
        occupancy_after: u32,
    },
    Checkpoint { number: u32, path: String, delta: i32 },
    ResetArmed { occupancy: u32, window: u32, reserve: u32 },
    ResetConfirm { state_object: String },
    Orientation { text: String, verdict: String, occupancy_after: u32 },
    ModelList { models: Vec<String>, current: String },
    ModelSwitched { from: String, to: String },
    ProviderError { text: String },
    Aborted { partial: String },
}

#[derive(Debug, Clone)]
pub enum Input {
    Line(String),
    AskAnswer { key: char, note: Option<String> },
    Abort,
}

#[derive(Debug, Clone, Copy)]
pub struct Timing {
    pub fast: bool,
}

impl Timing {
    fn ms(&self, millis: u64) -> Duration {
        if self.fast {
            Duration::from_millis(0)
        } else {
            Duration::from_millis(millis)
        }
    }
}

pub const MODELS: [&str; 3] = ["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"];

fn tokenize_with_whitespace(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_ws = false;
    for c in text.chars() {
        let is_ws = c.is_whitespace();
        if current.is_empty() {
            current.push(c);
            in_ws = is_ws;
        } else if is_ws == in_ws {
            current.push(c);
        } else {
            tokens.push(std::mem::take(&mut current));
            current.push(c);
            in_ws = is_ws;
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn stream_units(text: &str) -> Vec<String> {
    let tokens = tokenize_with_whitespace(text);
    let mut units = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let mut unit = tokens[i].clone();
        i += 1;
        if i < tokens.len() && tokens[i].chars().all(char::is_whitespace) {
            unit.push_str(&tokens[i]);
            i += 1;
        }
        units.push(unit);
    }
    units
}

fn stream_plain(tx: &Sender<Event>, text: &str, timing: &Timing) {
    let units = stream_units(text);
    let mut idx = 0;
    while idx < units.len() {
        let take: Vec<String> = units[idx..].iter().take(4).cloned().collect();
        idx += take.len();
        if tx.send(Event::Chunk { text: take.concat() }).is_err() {
            return;
        }
        thread::sleep(timing.ms(90));
    }
}

enum StreamOutcome {
    Completed(String),
    Aborted(String),
}

fn stream_abortable(tx: &Sender<Event>, rx: &Receiver<Input>, text: &str, timing: &Timing) -> StreamOutcome {
    let units = stream_units(text);
    let mut acc = String::new();
    let mut idx = 0;
    while idx < units.len() {
        let take: Vec<String> = units[idx..].iter().take(4).cloned().collect();
        idx += take.len();
        let chunk = take.concat();
        acc.push_str(&chunk);
        if tx.send(Event::Chunk { text: chunk }).is_err() {
            return StreamOutcome::Completed(acc);
        }
        match rx.recv_timeout(timing.ms(90).max(Duration::from_millis(15))) {
            Ok(Input::Abort) => return StreamOutcome::Aborted(acc),
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return StreamOutcome::Completed(acc),
        }
    }
    StreamOutcome::Completed(acc)
}

fn expect_line(rx: &Receiver<Input>) -> Option<String> {
    loop {
        match rx.recv() {
            Ok(Input::Line(l)) => return Some(l),
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

fn expect_ask_answer(rx: &Receiver<Input>) -> Option<(char, Option<String>)> {
    loop {
        match rx.recv() {
            Ok(Input::AskAnswer { key, note }) => return Some((key, note)),
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

fn expect_command(
    tx: &Sender<Event>,
    rx: &Receiver<Input>,
    timing: &Timing,
    matches: impl Fn(&str) -> bool,
) -> Option<String> {
    loop {
        let line = expect_line(rx)?;
        let trimmed = line.trim().to_string();
        if matches(&trimmed) {
            return Some(trimmed);
        }
        stream_plain(tx, "Not scripted at this step; the next scene needs the exact command shown in the hint bar.", timing);
        tx.send(Event::TurnEnd {
            text: "Not scripted at this step; the next scene needs the exact command shown in the hint bar."
                .to_string(),
        })
        .ok();
        tx.send(Event::ExchangeEnd).ok();
    }
}

fn gate_verdict_for_answer(key: char) -> (bool, &'static str, bool) {
    match key {
        'a' => (true, "operator: allow once", false),
        'A' => (true, "Bash cargo test -p arx*", true),
        'd' => (false, "operator: deny once", false),
        'D' => (false, "Bash cargo test -p arx*", true),
        _ => (true, "operator: allow once", false),
    }
}

const SCENE1_REPLY: &str = "The store root moved out of the repo in L3-05, so a fresh clone has no `store/` at all. Two things follow:\n\n- `arx run` refuses an unregistered scope, and registration is still a hand-typed `mkdir` under `~/.local/share/arx/store`.\n- The tracked `pid.txt` is a leftover of the L1-close session. Nothing writes it now.\n\nI'd delete `pid.txt` in the polishing pass and leave the store registration as it is.";

const SCENE2_PRE: &str = "terminal.rs is still the L1-04 adapter; let me see what the tests pin down.";
const SCENE2_POST: &str = "Eleven integration tests pin the byte shape of piped output, so a richer terminal has to keep piped mode byte-identical and add the new look only when stdin is a tty.";

const SCENE3_TURN: &str = "Dispatched hand-do to delete `pid.txt` and drop its mention from CLAUDE.md, and the codex judge to review that diff once it lands. I'll fold both when they report.";
const SCENE3_FOLD: &str = "Both back. The hand's diff is in the working tree, uncommitted, and the judge found nothing. Your call whether to commit it with the rest of the polishing work.";

const H0007_BRIEF: &str = "delete `pid.txt` and drop its mention from CLAUDE.md";
const H0008_BRIEF: &str = "review the diff h-0007 landed against its brief; findings or none";

fn h0007_turns() -> Vec<TraceTurn> {
    vec![
        TraceTurn::Text(
            "Plan: read the hazard paragraph, delete pid.txt, reword the line to match, then run the suite."
                .to_string(),
        ),
        TraceTurn::ToolCall { tool: "Read".to_string(), args: "CLAUDE.md".to_string() },
        TraceTurn::ToolResult(
            "pid.txt at the root is a leftover of the L1-close session, still tracked; nothing removes it yet."
                .to_string(),
        ),
        TraceTurn::ToolCall { tool: "Bash".to_string(), args: "rm pid.txt".to_string() },
        TraceTurn::ToolCall {
            tool: "Edit".to_string(),
            args: "CLAUDE.md \u{2014} reword the hazard line to say nothing writes pid.txt now".to_string(),
        },
        TraceTurn::ToolCall { tool: "Bash".to_string(), args: "cargo test -p arx".to_string() },
        TraceTurn::ToolResult("test result: ok. 23 passed; 0 failed".to_string()),
    ]
}

fn h0008_turns() -> Vec<TraceTurn> {
    vec![
        TraceTurn::ToolCall { tool: "Bash".to_string(), args: "git diff".to_string() },
        TraceTurn::ToolResult(
            "-pid.txt\nCLAUDE.md | 2 +-\n2 files changed, 1 insertion(+), 2 deletions(-)".to_string(),
        ),
        TraceTurn::ToolCall { tool: "Read".to_string(), args: "CLAUDE.md".to_string() },
        TraceTurn::ToolResult(
            "pid.txt at the root is a tracked leftover of the L1-close session: nothing writes it now, and no change to it is staged."
                .to_string(),
        ),
        TraceTurn::Text(
            "The diff removes pid.txt and the hazard paragraph now says nothing writes it, matching the tree. Nothing else in the diff touches state the brief didn't name."
                .to_string(),
        ),
    ]
}

const SCENE5_REPLY: &str = "On it. Working through the rest of the list now.";

const CHECKPOINT_STATE: &str = "position   polishing pass, day 1; pid.txt removed, /model landed, doubled reply fixed\ntarget     arx usable as the daily driver for arx work (L3-04)\ndrift      none; terminal look still unruled (44afcdc8)\nrelies on  580e402e, 57aa480a";

const ORIENTATION_TEXT: &str = "Resuming from checkpoint 3. Position and target are clear, the drift line is empty, both decision ids resolve. Ready.";

const SCENE6_FILLER: &str = "The polishing pass has touched five files so far, all of them small: the store root move, the reset floor, the admission trace path, and now the terminal front. None of them changed a public seam, so the L1-04 contract still holds for every adapter that reads it.\n\nWhat's left is judgment calls, not code: whether pid.txt's removal rides in its own commit or with the rest of the pass, whether the gate file gets a durable rule for cargo test now or waits until it's asked twice more, and whether the reset floor needs a test of its own or the existing coverage already pins it.";

const SCENE6_RETRY_REPLY: &str = "Back. Where were we?";

pub fn run(tx: Sender<Event>, rx: Receiver<Input>, timing: Timing) {
    let mut current_model = MODELS[0].to_string();

    macro_rules! give_up {
        ($opt:expr) => {
            match $opt {
                Some(v) => v,
                None => return,
            }
        };
    }

    give_up!(expect_line(&rx));
    thread::sleep(timing.ms(700));
    stream_plain(&tx, SCENE1_REPLY, &timing);
    tx.send(Event::TurnEnd { text: SCENE1_REPLY.to_string() }).ok();
    tx.send(Event::Usage {
        input: 18_240,
        output: 142,
        cache_read: 16_900,
        model: current_model.clone(),
        source: ModelSource::Sdk,
        elapsed_ms: 3_180,
        occupancy_after: 63_900,
    })
    .ok();
    tx.send(Event::ExchangeEnd).ok();

    give_up!(expect_line(&rx));
    tx.send(Event::ToolCall {
        tool: "Read".to_string(),
        subject: "crates/arx/src/terminal.rs".to_string(),
    })
    .ok();
    tx.send(Event::GateVerdict { allowed: true, rule: "Read *".to_string() }).ok();
    tx.send(Event::ToolResult { admission: Admission::Inline { bytes: 6_312 } }).ok();
    stream_plain(&tx, SCENE2_PRE, &timing);
    tx.send(Event::ToolCall {
        tool: "Bash".to_string(),
        subject: "cargo test -p arx --test claude_main 2>&1 | tail -40".to_string(),
    })
    .ok();
    tx.send(Event::GateAsk {
        tool: "Bash".to_string(),
        subject: "cargo test -p arx --test claude_main 2>&1 | tail -40".to_string(),
    })
    .ok();
    let (key, _note) = give_up!(expect_ask_answer(&rx));
    let (allowed, rule, durable) = gate_verdict_for_answer(key);
    tx.send(Event::GateVerdict { allowed, rule: rule.to_string() }).ok();
    if durable {
        tx.send(Event::Notice {
            kind: NoticeKind::Info,
            text: "durable rule appended: Bash cargo test -p arx* -> .arx/gate.toml".to_string(),
        })
        .ok();
    }
    tx.send(Event::ToolResult {
        admission: Admission::Externalized {
            bytes: 14_803,
            handle: "trace/2026-09-08/t07-bash-3.txt".to_string(),
            preview: "test result: ok. 11 passed; 0 failed".to_string(),
        },
    })
    .ok();
    stream_plain(&tx, SCENE2_POST, &timing);
    let scene2_full = format!("{SCENE2_PRE}\n\n{SCENE2_POST}");
    tx.send(Event::TurnEnd { text: scene2_full }).ok();
    tx.send(Event::Usage {
        input: 33_110,
        output: 96,
        cache_read: 18_240,
        model: current_model.clone(),
        source: ModelSource::Sdk,
        elapsed_ms: 9_640,
        occupancy_after: 79_300,
    })
    .ok();
    tx.send(Event::ExchangeEnd).ok();

    give_up!(expect_line(&rx));
    tx.send(Event::Spawn {
        id: "h-0007".to_string(),
        agent: "hand-do".to_string(),
        role: Role::Hand,
        body: "claude-code".to_string(),
        container: "arx-hand-claude:2.1.258".to_string(),
        model: "claude-sonnet-5".to_string(),
        brief: H0007_BRIEF.to_string(),
    })
    .ok();
    tx.send(Event::GateVerdict { allowed: true, rule: "dispatch *".to_string() }).ok();
    tx.send(Event::Spawn {
        id: "h-0008".to_string(),
        agent: "codex-judge".to_string(),
        role: Role::Judge,
        body: "codex".to_string(),
        container: "arx-hand-codex:0.152.1".to_string(),
        model: "gpt-5.4".to_string(),
        brief: H0008_BRIEF.to_string(),
    })
    .ok();
    stream_plain(&tx, SCENE3_TURN, &timing);
    tx.send(Event::TurnEnd { text: SCENE3_TURN.to_string() }).ok();
    tx.send(Event::ExchangeEnd).ok();
    thread::sleep(timing.ms(6_000));
    tx.send(Event::Delivered {
        id: "h-0007".to_string(),
        outcome: Outcome::Verdict,
        text: "PASS: pid.txt removed, CLAUDE.md hazard line rewritten, 23 tests green".to_string(),
        trace: "trace/2026-09-08/h-0007.md".to_string(),
        bytes: 2_118,
        elapsed_ms: 5_940,
        lag_ms: 12,
        schema_ok: None,
        turns: h0007_turns(),
    })
    .ok();
    thread::sleep(timing.ms(3_000));
    tx.send(Event::Delivered {
        id: "h-0008".to_string(),
        outcome: Outcome::Verdict,
        text: "no findings; the hazard paragraph now says nothing writes pid.txt, which matches the tree"
            .to_string(),
        trace: "trace/2026-09-08/h-0008.md".to_string(),
        bytes: 611,
        elapsed_ms: 8_870,
        lag_ms: 8,
        schema_ok: Some(true),
        turns: h0008_turns(),
    })
    .ok();

    give_up!(expect_line(&rx));
    stream_plain(&tx, SCENE3_FOLD, &timing);
    tx.send(Event::TurnEnd { text: SCENE3_FOLD.to_string() }).ok();
    tx.send(Event::ExchangeEnd).ok();

    give_up!(expect_command(&tx, &rx, &timing, |l| l == "/model"));
    tx.send(Event::ModelList { models: MODELS.iter().map(|s| s.to_string()).collect(), current: current_model.clone() }).ok();
    let pick = give_up!(expect_command(&tx, &rx, &timing, |l| l.starts_with("/model ")));
    let id = pick.trim_start_matches("/model ").trim().to_string();
    if id == current_model {
        tx.send(Event::Notice { kind: NoticeKind::Info, text: format!("model: already {id}") }).ok();
    } else if MODELS.contains(&id.as_str()) {
        tx.send(Event::ModelSwitched { from: current_model.clone(), to: id.clone() }).ok();
        current_model = id;
    } else {
        tx.send(Event::Notice {
            kind: NoticeKind::Error,
            text: format!("model: {id} not listed; models are {}", MODELS.join(", ")),
        })
        .ok();
    }
    let pick2 = give_up!(expect_command(&tx, &rx, &timing, |l| l.starts_with("/model ")));
    let id2 = pick2.trim_start_matches("/model ").trim().to_string();
    if id2 == current_model {
        tx.send(Event::Notice { kind: NoticeKind::Info, text: format!("model: already {id2}") }).ok();
    } else if MODELS.contains(&id2.as_str()) {
        tx.send(Event::ModelSwitched { from: current_model.clone(), to: id2.clone() }).ok();
        current_model = id2;
    } else {
        tx.send(Event::Notice {
            kind: NoticeKind::Error,
            text: format!("model: {id2} not listed; models are {}", MODELS.join(", ")),
        })
        .ok();
    }

    give_up!(expect_line(&rx));
    stream_plain(&tx, SCENE5_REPLY, &timing);
    tx.send(Event::TurnEnd { text: SCENE5_REPLY.to_string() }).ok();
    tx.send(Event::Usage {
        input: 4_400,
        output: 220,
        cache_read: 300,
        model: current_model.clone(),
        source: ModelSource::Sdk,
        elapsed_ms: 2_100,
        occupancy_after: 121_800,
    })
    .ok();
    tx.send(Event::Notice { kind: NoticeKind::Nudge, text: "main at 121800 tokens, past 120000".to_string() }).ok();
    tx.send(Event::ResetArmed { occupancy: 161_200, window: 200_000, reserve: 40_000 }).ok();
    tx.send(Event::ExchangeEnd).ok();

    give_up!(expect_command(&tx, &rx, &timing, |l| l == "/checkpoint"));
    tx.send(Event::Notice {
        kind: NoticeKind::Info,
        text: "Write the state object: position, target, drift. Cite decision ids you rely on.".to_string(),
    })
    .ok();
    stream_plain(&tx, CHECKPOINT_STATE, &timing);
    tx.send(Event::TurnEnd { text: CHECKPOINT_STATE.to_string() }).ok();
    tx.send(Event::Notice {
        kind: NoticeKind::Info,
        text: "Check every claim above against the tree and the store.".to_string(),
    })
    .ok();
    stream_plain(&tx, "all four hold", &timing);
    tx.send(Event::TurnEnd { text: "all four hold".to_string() }).ok();
    tx.send(Event::Checkpoint {
        number: 3,
        path: "threads/main.checkpoints/3.md".to_string(),
        delta: 58_400,
    })
    .ok();
    tx.send(Event::ExchangeEnd).ok();

    give_up!(expect_command(&tx, &rx, &timing, |l| l == "/reset"));
    tx.send(Event::ResetConfirm { state_object: CHECKPOINT_STATE.to_string() }).ok();

    give_up!(expect_command(&tx, &rx, &timing, |l| l == "ok"));
    tx.send(Event::Notice { kind: NoticeKind::Info, text: "tearing down main".to_string() }).ok();
    thread::sleep(timing.ms(600));
    tx.send(Event::Notice { kind: NoticeKind::Info, text: "respawning main from checkpoint 3".to_string() }).ok();
    thread::sleep(timing.ms(600));
    stream_plain(&tx, ORIENTATION_TEXT, &timing);
    tx.send(Event::Orientation {
        text: ORIENTATION_TEXT.to_string(),
        verdict: "ready".to_string(),
        occupancy_after: 9_120,
    })
    .ok();
    tx.send(Event::ExchangeEnd).ok();

    give_up!(expect_line(&rx));
    match stream_abortable(&tx, &rx, SCENE6_FILLER, &timing) {
        StreamOutcome::Aborted(partial) => {
            tx.send(Event::Aborted { partial }).ok();
        }
        StreamOutcome::Completed(text) => {
            tx.send(Event::TurnEnd { text }).ok();
            tx.send(Event::ExchangeEnd).ok();
        }
    }

    give_up!(expect_line(&rx));
    tx.send(Event::ProviderError { text: "overloaded_error: retry after 2 s".to_string() }).ok();
    thread::sleep(timing.ms(2_000));
    stream_plain(&tx, SCENE6_RETRY_REPLY, &timing);
    tx.send(Event::TurnEnd { text: SCENE6_RETRY_REPLY.to_string() }).ok();
    tx.send(Event::ExchangeEnd).ok();

    loop {
        if expect_line(&rx).is_none() {
            return;
        }
        let text = "The scripted session ends here.";
        stream_plain(&tx, text, &timing);
        tx.send(Event::TurnEnd { text: text.to_string() }).ok();
        tx.send(Event::ExchangeEnd).ok();
    }
}
