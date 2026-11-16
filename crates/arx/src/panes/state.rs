use std::time::Instant;

use crate::events::Event;
use crate::panes::trace::TraceTurn;
use crate::terminal::TerminalMsg;

pub const SPINNER: [char; 4] = ['-', '\\', '|', '/'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Hand,
    Reviewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Verdict,
    Stop,
    Killed,
}

impl Outcome {
    fn parse(text: &str) -> Option<Outcome> {
        match text {
            "verdict" => Some(Outcome::Verdict),
            "stop" => Some(Outcome::Stop),
            "killed" => Some(Outcome::Killed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub tool: String,
    pub subject: Option<String>,
    pub outcome: Option<String>,
    pub bytes: Option<u64>,
    pub handle: Option<String>,
}

impl ToolCall {
    fn new(tool: String) -> Self {
        ToolCall {
            tool,
            subject: None,
            outcome: None,
            bytes: None,
            handle: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Turn {
    Operator(String),
    Main(String),
    Notice(String),
    Tools { calls: Vec<ToolCall>, expanded: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stop {
    pub turn: usize,
    pub call: Option<usize>,
}

pub fn stops(page: &[Turn]) -> Vec<Stop> {
    let mut out = Vec::new();
    for (i, turn) in page.iter().enumerate() {
        match turn {
            Turn::Tools { calls, expanded: true } => {
                for j in 0..calls.len() {
                    out.push(Stop { turn: i, call: Some(j) });
                }
            }
            _ => out.push(Stop { turn: i, call: None }),
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct MainIncarnation {
    pub label: String,
    pub model: String,
    pub ended: Option<String>,
    pub page: Vec<Turn>,
    pub started: Instant,
    pub turn_started: Option<Instant>,
    pub tokens: u64,
}

impl MainIncarnation {
    fn new(label: String, model: String) -> Self {
        MainIncarnation {
            label,
            model,
            ended: None,
            page: Vec::new(),
            started: Instant::now(),
            turn_started: None,
            tokens: 0,
        }
    }

    pub fn status_cell(&self) -> String {
        let tok = format_tokens(self.tokens);
        if self.ended.is_some() {
            format!("ended \u{b7} {tok}")
        } else if let Some(turn_started) = self.turn_started {
            format!("running \u{b7} {:.1}s \u{b7} {tok}", turn_started.elapsed().as_secs_f32())
        } else {
            format!("live \u{b7} {tok}")
        }
    }
}

fn format_tokens(n: u64) -> String {
    if n < 1000 {
        format!("{n} tok")
    } else if n < 1_000_000 {
        format!("{:.1}k tok", n as f64 / 1000.0)
    } else {
        format!("{:.1}M tok", n as f64 / 1_000_000.0)
    }
}

#[derive(Debug, Clone)]
pub struct HandRow {
    pub id: String,
    pub agent: String,
    pub role: Role,
    pub body: String,
    pub image: String,
    pub model: String,
    pub brief: String,
    pub spawned_at: Instant,
    pub outcome: Option<Outcome>,
    pub verdict_text: Option<String>,
    pub trace: Option<String>,
    pub bytes: Option<u64>,
    pub diff: Option<String>,
    pub elapsed_ms: Option<u64>,
    pub lag_ms: Option<u64>,
    pub schema_ok: Option<bool>,
    pub trace_turns: Option<Result<Vec<TraceTurn>, String>>,
    pub killing: bool,
}

impl HandRow {
    fn new(id: String, agent: String, role: Role) -> Self {
        HandRow {
            id,
            agent,
            role,
            body: String::new(),
            image: String::new(),
            model: String::new(),
            brief: String::new(),
            spawned_at: Instant::now(),
            outcome: None,
            verdict_text: None,
            trace: None,
            bytes: None,
            diff: None,
            elapsed_ms: None,
            lag_ms: None,
            schema_ok: None,
            trace_turns: None,
            killing: false,
        }
    }

    pub fn status_cell(&self) -> String {
        if self.killing {
            return "killing".to_string();
        }
        match self.outcome {
            None => format!("{:.1}s", self.spawned_at.elapsed().as_secs_f32()),
            Some(Outcome::Verdict) if self.role == Role::Reviewer && self.schema_ok == Some(false) => "\u{2717}".to_string(),
            Some(Outcome::Verdict) => format!("\u{2713} {:.1}s", self.elapsed_ms.unwrap_or(0) as f32 / 1000.0),
            Some(Outcome::Stop) => "stop".to_string(),
            Some(Outcome::Killed) => "killed".to_string(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.outcome.is_none()
    }
}

#[derive(Debug, Default)]
pub struct GateStats {
    pub allow: u32,
    pub deny: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Status {
    pub model: String,
    pub occupancy: u64,
    pub window: u64,
    pub reserve: u64,
    pub nudge: u64,
    pub checkpoint: Option<(u64, u64)>,
    pub turn: u64,
}

pub struct SessionState {
    pub status: Status,
    pub session_started: Instant,
    pub current_main: usize,
    pub mains: Vec<MainIncarnation>,
    pub reviewers: Vec<HandRow>,
    pub hands: Vec<HandRow>,
    pub gate: GateStats,
    pub last_models: Vec<String>,
    pub streaming_buffer: Option<String>,
    pub waiting_since: Option<Instant>,
}

impl SessionState {
    pub fn new() -> Self {
        SessionState {
            status: Status::default(),
            session_started: Instant::now(),
            current_main: 0,
            mains: vec![MainIncarnation::new("main #1".to_string(), String::new())],
            reviewers: Vec::new(),
            hands: Vec::new(),
            gate: GateStats::default(),
            last_models: Vec::new(),
            streaming_buffer: None,
            waiting_since: None,
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.waiting_since.is_some()
    }

    pub fn hands_in_flight(&self) -> usize {
        self.hands.iter().chain(self.reviewers.iter()).filter(|h| h.is_running()).count()
    }

    fn page_mut(&mut self) -> &mut Vec<Turn> {
        &mut self.mains[self.current_main].page
    }

    pub fn push_operator_line(&mut self, text: &str) {
        let idx = self.current_main;
        self.mains[idx].turn_started = Some(Instant::now());
        self.page_mut().push(Turn::Operator(text.to_string()));
    }

    pub fn push_notice_on_current_main(&mut self, text: &str) {
        self.page_mut().push(Turn::Notice(text.to_string()));
    }

    fn find_row_mut(&mut self, id: &str) -> Option<&mut HandRow> {
        self.hands.iter_mut().chain(self.reviewers.iter_mut()).find(|h| h.id == id)
    }

    fn flush_stream(&mut self) {
        self.streaming_buffer = None;
    }

    pub fn remove_hand(&mut self, id: &str) {
        self.hands.retain(|h| h.id != id);
    }

    pub fn remove_reviewer(&mut self, id: &str) {
        self.reviewers.retain(|h| h.id != id);
    }

    pub fn remove_main(&mut self, index: usize) {
        if index >= self.mains.len() {
            return;
        }
        self.mains.remove(index);
        if self.current_main > index {
            self.current_main -= 1;
        }
    }

    pub fn apply_terminal(&mut self, msg: &TerminalMsg) {
        match msg {
            TerminalMsg::Chunk { text } => {
                let idx = self.current_main;
                if self.mains[idx].turn_started.is_none() {
                    self.mains[idx].turn_started = Some(Instant::now());
                }
                match &mut self.streaming_buffer {
                    Some(buf) => buf.push_str(text),
                    None => self.streaming_buffer = Some(text.clone()),
                }
            }
            TerminalMsg::TurnEnd { text } => {
                self.flush_stream();
                self.waiting_since = None;
                let idx = self.current_main;
                self.mains[idx].turn_started = None;
                // A tool-only turn (the model closed with no text of its own) leaves
                // `text` empty; the fold already speaks for that turn, so skip adding an
                // invisible stop no cursor could ever land on with anything to show.
                if !text.trim().is_empty() {
                    self.page_mut().push(Turn::Main(text.clone()));
                }
            }
            TerminalMsg::Notice { text } => {
                self.page_mut().push(Turn::Notice(text.clone()));
            }
            TerminalMsg::Confirm { .. } => {}
            TerminalMsg::Roster { models, .. } => {
                self.last_models = models.iter().map(|m| m.id.clone()).collect();
            }
            TerminalMsg::Status { model, occupancy, window, reserve, nudge, checkpoint, turn } => {
                self.status = Status {
                    model: model.clone(),
                    occupancy: *occupancy,
                    window: *window,
                    reserve: *reserve,
                    nudge: *nudge,
                    checkpoint: *checkpoint,
                    turn: *turn,
                };
            }
            TerminalMsg::HandSpawned { hand, body, image, model, brief, .. } => {
                if let Some(row) = self.find_row_mut(hand) {
                    row.body = body.clone();
                    row.image = image.clone();
                    row.model = model.clone();
                    row.brief = brief.clone();
                    row.spawned_at = Instant::now();
                }
            }
            TerminalMsg::HandStopped { hand, question } => {
                if let Some(row) = self.find_row_mut(hand) {
                    row.outcome = Some(Outcome::Stop);
                    row.verdict_text = Some(question.clone());
                }
            }
            TerminalMsg::HandDelivered { hand, outcome, text, trace, bytes, elapsed_ms, lag_ms, schema, diff } => {
                if let Some(row) = self.find_row_mut(hand) {
                    row.outcome = Outcome::parse(outcome);
                    row.verdict_text = text.clone();
                    row.trace = trace.clone();
                    row.bytes = *bytes;
                    row.diff = diff.clone();
                    row.elapsed_ms = Some(*elapsed_ms);
                    row.lag_ms = Some(*lag_ms);
                    row.schema_ok = schema.as_deref().map(|s| s == "ok");
                    row.killing = false;
                }
            }
            TerminalMsg::ToolCall { tool, subject } => {
                self.fill_subject(tool, subject.clone());
            }
            TerminalMsg::Usage { input, output, cache_creation, cache_read } => {
                let idx = self.current_main;
                self.mains[idx].tokens += input + output + cache_creation + cache_read;
            }
            TerminalMsg::Stopped => {
                self.page_mut().push(Turn::Notice("stopped by operator".to_string()));
            }
            TerminalMsg::Prompt => {}
        }
    }

    pub fn apply_event(&mut self, ev: &Event) {
        match ev {
            Event::Gate { verdict, .. } => match verdict.as_str() {
                "allow" => self.gate.allow += 1,
                "deny" => self.gate.deny += 1,
                _ => {}
            },
            Event::Spawn { hand, agent, role } => {
                let is_reviewer = role == "reviewer";
                let row = HandRow::new(hand.clone(), agent.clone(), if is_reviewer { Role::Reviewer } else { Role::Hand });
                if is_reviewer {
                    self.reviewers.push(row);
                } else {
                    self.hands.push(row);
                }
            }
            Event::Delivered { hand, outcome, trace, bytes, elapsed_ms, lag_ms } => {
                if let Some(row) = self.find_row_mut(hand) {
                    row.outcome = Outcome::parse(outcome);
                    row.trace = trace.clone();
                    row.bytes = *bytes;
                    row.elapsed_ms = *elapsed_ms;
                    row.lag_ms = *lag_ms;
                    row.killing = false;
                }
            }
            Event::Schema { hand, outcome, .. } => {
                if let Some(row) = self.find_row_mut(hand) {
                    row.schema_ok = Some(outcome == "ok");
                }
            }
            Event::Kill { hand } => {
                if let Some(row) = self.find_row_mut(hand) {
                    row.outcome = Some(Outcome::Killed);
                }
            }
            Event::Model { position, to, .. } if position == "main" => {
                self.status.model = to.clone();
            }
            Event::Reset { phase, check } if phase == "respawned" => {
                let reason = format!("reset from {}", check.clone().unwrap_or_default());
                self.mains[self.current_main].ended = Some(reason);
                self.mains[self.current_main].turn_started = None;
                let model = self.status.model.clone();
                self.mains.push(MainIncarnation::new(format!("main #{}", self.mains.len() + 1), model));
                self.current_main = self.mains.len() - 1;
            }
            _ => {}
        }

        if let Event::Gate { tool, verdict, .. } = ev {
            if verdict == "allow" {
                if let Some(bare) = tool.strip_prefix("mcp__arx__") {
                    self.push_tool_call(bare.to_string());
                    return;
                }
            }
        }
        if let Event::Admission { tool, outcome, bytes, handle } = ev {
            if self.fill_admission(tool, outcome, *bytes, handle.clone()) {
                return;
            }
        }

        let line = event_line(ev);
        self.page_mut().push(Turn::Notice(line));
    }

    fn push_tool_call(&mut self, tool: String) {
        let call = ToolCall::new(tool);
        let page = self.page_mut();
        match page.last_mut() {
            Some(Turn::Tools { calls, expanded: false }) => calls.push(call),
            _ => page.push(Turn::Tools { calls: vec![call], expanded: false }),
        }
    }

    fn fill_subject(&mut self, tool: &str, subject: Option<String>) {
        for turn in self.page_mut().iter_mut().rev() {
            if let Turn::Tools { calls, .. } = turn {
                if let Some(call) = calls.iter_mut().rev().find(|c| c.tool == tool && c.subject.is_none()) {
                    call.subject = subject;
                    return;
                }
            }
        }
    }

    fn fill_admission(&mut self, tool: &str, outcome: &str, bytes: u64, handle: Option<String>) -> bool {
        for turn in self.page_mut().iter_mut().rev() {
            if let Turn::Tools { calls, .. } = turn {
                if let Some(call) = calls.iter_mut().rev().find(|c| c.tool == tool && c.outcome.is_none()) {
                    call.outcome = Some(outcome.to_string());
                    call.bytes = Some(bytes);
                    call.handle = handle;
                    return true;
                }
            }
        }
        false
    }
}

fn event_line(ev: &Event) -> String {
    match ev {
        Event::Gate { tool, verdict, by, reason } => {
            let who = by.as_deref().unwrap_or("rule");
            match reason {
                Some(reason) => format!("\u{b7} {tool} \u{2192} {verdict} ({who}: {reason})"),
                None => format!("\u{b7} {tool} \u{2192} {verdict} ({who})"),
            }
        }
        Event::Admission { tool, outcome, bytes, handle } => match handle {
            Some(handle) => format!("\u{b7} {tool} \u{2192} {outcome} {bytes}b \u{2192} {handle}"),
            None => format!("\u{b7} {tool} \u{2192} {outcome} {bytes}b"),
        },
        Event::Spawn { hand, agent, role } => format!("\u{b7} dispatched {hand} {agent} ({role})"),
        Event::Delivered { hand, outcome, .. } => format!("\u{b7} {hand} delivered: {outcome}"),
        Event::Model { position, from, to } => format!("\u{b7} model: {position} {from} -> {to}"),
        Event::Checkpoint { check, delta } => format!("\u{b7} checkpoint {check} (delta {delta})"),
        Event::Reset { phase, check } => match check {
            Some(check) => format!("\u{b7} reset {phase} ({check})"),
            None => format!("\u{b7} reset {phase}"),
        },
        Event::Nudge { seat, hand, occupancy, threshold } => match hand {
            Some(hand) => format!("\u{b7} nudge {seat}/{hand} at {occupancy}, past {threshold}"),
            None => format!("\u{b7} nudge {seat} at {occupancy}, past {threshold}"),
        },
        Event::Miss { size } => format!("\u{b7} miss {size}b"),
        Event::Floor { overshoot } => format!("\u{b7} floor overshoot {overshoot}"),
        Event::Schema { hand, outcome, reason } => match reason {
            Some(reason) => format!("\u{b7} schema {hand} {outcome}: {reason}"),
            None => format!("\u{b7} schema {hand} {outcome}"),
        },
        Event::Pull { handle, path } => format!("\u{b7} pull {handle} {path}"),
        Event::Echo { handle } => format!("\u{b7} echo {handle}"),
        Event::Rewind { tokens_cut } => format!("\u{b7} rewind {tokens_cut} tokens"),
        Event::Orientation { verdict } => format!("\u{b7} orientation {verdict}"),
        Event::Family { agent, family } => format!("\u{b7} family {agent} {family}"),
        Event::Drain { hand, promoted, dropped, overrides } => {
            format!("\u{b7} drain {hand} +{promoted} -{dropped} ~{overrides}")
        }
        Event::Wall { hand, tool } => format!("\u{b7} wall {hand} {tool}"),
        Event::Tap { hand } => format!("\u{b7} tap {hand}"),
        Event::Steer { hand } => format!("\u{b7} steer {hand}"),
        Event::Kill { hand } => format!("\u{b7} kill {hand}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_malformed_reviewer_verdict_shows_a_cross_not_a_checkmark() {
        let mut state = SessionState::new();
        state.apply_event(&Event::Spawn { hand: "h1".to_string(), agent: "codex-reviewer".to_string(), role: "reviewer".to_string() });
        state.apply_terminal(&TerminalMsg::HandDelivered {
            hand: "h1".to_string(),
            outcome: "verdict".to_string(),
            text: Some("not json".to_string()),
            trace: None,
            bytes: Some(2),
            elapsed_ms: 100,
            lag_ms: 5,
            schema: Some("malformed".to_string()),
            diff: None,
        });

        let row = state.reviewers.iter().find(|h| h.id == "h1").expect("reviewer row");
        assert_eq!(row.status_cell(), "\u{2717}");
    }

    #[test]
    fn a_valid_reviewer_verdict_still_shows_a_checkmark_with_elapsed() {
        let mut state = SessionState::new();
        state.apply_event(&Event::Spawn { hand: "h1".to_string(), agent: "codex-reviewer".to_string(), role: "reviewer".to_string() });
        state.apply_terminal(&TerminalMsg::HandDelivered {
            hand: "h1".to_string(),
            outcome: "verdict".to_string(),
            text: Some("{}".to_string()),
            trace: None,
            bytes: Some(2),
            elapsed_ms: 5900,
            lag_ms: 5,
            schema: Some("ok".to_string()),
            diff: None,
        });

        let row = state.reviewers.iter().find(|h| h.id == "h1").expect("reviewer row");
        assert_eq!(row.status_cell(), "\u{2713} 5.9s");
    }

    #[test]
    fn a_hand_s_verdict_is_never_marked_by_schema_since_hands_carry_none() {
        let mut state = SessionState::new();
        state.apply_event(&Event::Spawn { hand: "h1".to_string(), agent: "hand-do".to_string(), role: "hand".to_string() });
        state.apply_terminal(&TerminalMsg::HandDelivered {
            hand: "h1".to_string(),
            outcome: "verdict".to_string(),
            text: Some("done".to_string()),
            trace: None,
            bytes: Some(2),
            elapsed_ms: 1000,
            lag_ms: 5,
            schema: None,
            diff: None,
        });

        let row = state.hands.iter().find(|h| h.id == "h1").expect("hand row");
        assert_eq!(row.status_cell(), "\u{2713} 1.0s");
    }

    #[test]
    fn the_killing_cell_survives_the_early_kill_event_until_the_real_delivery_lands() {
        let mut state = SessionState::new();
        state.apply_event(&Event::Spawn { hand: "h1".to_string(), agent: "hand-do".to_string(), role: "hand".to_string() });
        state.hands.iter_mut().find(|h| h.id == "h1").unwrap().killing = true;

        state.apply_event(&Event::Kill { hand: "h1".to_string() });
        let row = state.hands.iter().find(|h| h.id == "h1").expect("hand row");
        assert_eq!(row.outcome, Some(Outcome::Killed), "the early Kill event still marks the outcome");
        assert_eq!(row.status_cell(), "killing", "but the cell must not flip until the real delivery settles it");

        state.apply_terminal(&TerminalMsg::HandDelivered {
            hand: "h1".to_string(),
            outcome: "killed".to_string(),
            text: None,
            trace: None,
            bytes: None,
            elapsed_ms: 100,
            lag_ms: 5,
            schema: None,
            diff: None,
        });
        let row = state.hands.iter().find(|h| h.id == "h1").expect("hand row");
        assert_eq!(row.status_cell(), "killed", "the real delivery settles it");
    }

    fn allow_gate(tool: &str) -> Event {
        Event::Gate { tool: tool.to_string(), verdict: "allow".to_string(), by: None, reason: None }
    }

    fn admission(tool: &str, outcome: &str, bytes: u64, handle: Option<&str>) -> Event {
        Event::Admission { tool: tool.to_string(), outcome: outcome.to_string(), bytes, handle: handle.map(str::to_string) }
    }

    #[test]
    fn three_allow_gates_with_their_admissions_fold_into_one_tools_turn() {
        let mut state = SessionState::new();
        state.apply_event(&allow_gate("mcp__arx__read_file"));
        state.apply_event(&admission("read_file", "inline", 11, None));
        state.apply_event(&allow_gate("mcp__arx__list_files"));
        state.apply_event(&admission("list_files", "inline", 40, None));
        state.apply_event(&allow_gate("mcp__arx__save"));
        state.apply_event(&admission("save", "externalized", 900, Some("threads/main.traces/1-1.md")));

        let page = &state.mains[0].page;
        assert_eq!(page.len(), 1, "page: {page:?}");
        match &page[0] {
            Turn::Tools { calls, expanded } => {
                assert!(!expanded);
                assert_eq!(
                    calls,
                    &vec![
                        ToolCall { tool: "read_file".to_string(), subject: None, outcome: Some("inline".to_string()), bytes: Some(11), handle: None },
                        ToolCall { tool: "list_files".to_string(), subject: None, outcome: Some("inline".to_string()), bytes: Some(40), handle: None },
                        ToolCall {
                            tool: "save".to_string(),
                            subject: None,
                            outcome: Some("externalized".to_string()),
                            bytes: Some(900),
                            handle: Some("threads/main.traces/1-1.md".to_string()),
                        },
                    ]
                );
            }
            other => panic!("expected a Tools turn, got {other:?}"),
        }
    }

    #[test]
    fn gate_counters_count_every_allow_and_deny_regardless_of_who_decided() {
        let mut state = SessionState::new();
        state.apply_event(&Event::Gate { tool: "Bash".to_string(), verdict: "allow".to_string(), by: None, reason: None });
        state.apply_event(&Event::Gate {
            tool: "Bash".to_string(),
            verdict: "deny".to_string(),
            by: Some("rule".to_string()),
            reason: None,
        });
        state.apply_event(&Event::Gate {
            tool: "Bash".to_string(),
            verdict: "allow".to_string(),
            by: Some("hook".to_string()),
            reason: None,
        });
        state.apply_event(&Event::Gate {
            tool: "Bash".to_string(),
            verdict: "deny".to_string(),
            by: Some("operator".to_string()),
            reason: None,
        });

        assert_eq!(state.gate.allow, 2);
        assert_eq!(state.gate.deny, 2);
    }

    #[test]
    fn a_deny_gate_between_two_allows_splits_the_run_with_a_notice_between() {
        let mut state = SessionState::new();
        state.apply_event(&allow_gate("mcp__arx__read_file"));
        state.apply_event(&admission("read_file", "inline", 5, None));
        state.apply_event(&Event::Gate { tool: "Bash".to_string(), verdict: "deny".to_string(), by: None, reason: None });
        state.apply_event(&allow_gate("mcp__arx__list_files"));
        state.apply_event(&admission("list_files", "inline", 6, None));

        let page = &state.mains[0].page;
        assert_eq!(page.len(), 3, "page: {page:?}");
        assert!(matches!(&page[0], Turn::Tools { calls, .. } if calls.len() == 1));
        assert!(matches!(&page[1], Turn::Notice(_)));
        assert!(matches!(&page[2], Turn::Tools { calls, .. } if calls.len() == 1));
    }

    #[test]
    fn an_admission_with_no_open_call_becomes_a_notice() {
        let mut state = SessionState::new();
        state.apply_event(&admission("read_file", "inline", 5, None));

        let page = &state.mains[0].page;
        assert_eq!(page.len(), 1, "page: {page:?}");
        assert!(matches!(&page[0], Turn::Notice(text) if text.contains("read_file")), "page: {page:?}");
    }

    #[test]
    fn a_tool_only_turn_ending_with_no_text_leaves_no_invisible_main_stop() {
        let mut state = SessionState::new();
        state.apply_event(&allow_gate("mcp__arx__read_file"));
        state.apply_event(&admission("read_file", "inline", 5, None));
        state.apply_terminal(&TerminalMsg::TurnEnd { text: String::new() });

        let page = &state.mains[0].page;
        assert_eq!(page.len(), 1, "an empty TurnEnd must not add its own turn: {page:?}");
        assert!(matches!(&page[0], Turn::Tools { .. }), "page: {page:?}");
    }

    #[test]
    fn an_operator_turn_then_turn_end_opens_and_closes_the_turn() {
        let mut state = SessionState::new();
        assert!(state.mains[0].turn_started.is_none());

        state.push_operator_line("hi");
        assert!(state.mains[0].turn_started.is_some());

        state.apply_terminal(&TerminalMsg::TurnEnd { text: "hello".to_string() });
        assert!(state.mains[0].turn_started.is_none());
    }

    #[test]
    fn a_chunk_on_an_idle_incarnation_opens_a_turn() {
        let mut state = SessionState::new();
        assert!(state.mains[0].turn_started.is_none());

        state.apply_terminal(&TerminalMsg::Chunk { text: "partial".to_string() });
        assert!(state.mains[0].turn_started.is_some());
    }

    #[test]
    fn a_usage_message_sums_into_the_incarnation_s_token_total() {
        let mut state = SessionState::new();
        state.apply_terminal(&TerminalMsg::Usage { input: 1000, output: 200, cache_creation: 0, cache_read: 300 });
        assert_eq!(state.mains[0].tokens, 1500);
        assert_eq!(state.mains[0].status_cell(), "live \u{b7} 1.5k tok");
    }

    #[test]
    fn two_usage_messages_sum() {
        let mut state = SessionState::new();
        state.apply_terminal(&TerminalMsg::Usage { input: 100, output: 0, cache_creation: 0, cache_read: 0 });
        state.apply_terminal(&TerminalMsg::Usage { input: 50, output: 0, cache_creation: 0, cache_read: 0 });
        assert_eq!(state.mains[0].tokens, 150);
    }

    #[test]
    fn a_respawning_reset_leaves_the_old_incarnation_ended_with_its_tokens_and_the_new_one_live_at_zero() {
        let mut state = SessionState::new();
        state.apply_terminal(&TerminalMsg::Usage { input: 100, output: 0, cache_creation: 0, cache_read: 0 });
        state.apply_event(&Event::Reset { phase: "respawned".to_string(), check: None });

        assert_eq!(state.mains[0].status_cell(), "ended \u{b7} 100 tok");
        assert_eq!(state.mains[1].status_cell(), "live \u{b7} 0 tok");
    }

    #[test]
    fn a_stopped_message_after_two_chunks_leaves_the_main_turn_and_adds_the_notice() {
        let mut state = SessionState::new();
        state.apply_terminal(&TerminalMsg::Chunk { text: "first ".to_string() });
        state.apply_terminal(&TerminalMsg::Chunk { text: "part".to_string() });
        state.apply_terminal(&TerminalMsg::TurnEnd { text: "first part".to_string() });
        state.apply_terminal(&TerminalMsg::Stopped);

        let page = &state.mains[0].page;
        assert!(matches!(&page[0], Turn::Main(text) if text == "first part"), "page: {page:?}");
        assert!(matches!(&page[1], Turn::Notice(text) if text == "stopped by operator"), "page: {page:?}");
    }

    #[test]
    fn the_token_formatter_crosses_each_unit_boundary() {
        assert_eq!(format_tokens(999), "999 tok");
        assert_eq!(format_tokens(1000), "1.0k tok");
        assert_eq!(format_tokens(38200), "38.2k tok");
        assert_eq!(format_tokens(1_200_000), "1.2M tok");
    }
}
