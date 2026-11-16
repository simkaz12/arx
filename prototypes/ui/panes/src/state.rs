use std::collections::VecDeque;
use std::time::Instant;

use crate::scenario::{Admission, Event, ModelSource, NoticeKind, Outcome, Role, TraceTurn};

const GATE_RECENT_LEN: usize = 5;
const HANDS_KEPT: usize = 50;

pub const WINDOW: u32 = 200_000;
pub const RESERVE: u32 = 40_000;
pub const NUDGE_THRESHOLD: u32 = 120_000;
pub const SCOPE: &str = "arx";
pub const STORE_ROOT: &str = "~/.local/share/arx/store";
pub const GROUNDING: &str = "hot 4 claims \u{b7} 2 embers";
pub const MODELS: [&str; 3] = ["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"];
pub const SLASH_COMMANDS: [&str; 4] = ["/reset", "/model", "/model <id>", "/checkpoint"];

#[derive(Debug, Clone)]
pub enum Line {
    Operator(String),
    Main(String),
    ToolSummary(String),
    DispatchSummary(String),
    Delivery { id: String, text: String },
    ArxPrompt(String),
    Notice { kind: NoticeKind, text: String },
    AbortedPartial(String),
    ResetConfirmBlock(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaugeAlert {
    Normal,
    Nudge,
    ResetArmed,
}

#[derive(Debug, Clone)]
pub struct MainIncarnation {
    pub label: String,
    pub model: String,
    pub ended: Option<String>,
    pub transcript: Vec<Line>,
}

impl MainIncarnation {
    fn new(label: String, model: String) -> Self {
        MainIncarnation { label, model, ended: None, transcript: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub struct HandRow {
    pub id: String,
    pub agent: String,
    pub role: Role,
    pub body: String,
    pub container: String,
    pub model: String,
    pub brief: String,
    pub spawned_at: Instant,
    pub outcome: Option<Outcome>,
    pub verdict_text: Option<String>,
    pub trace: Option<String>,
    pub bytes: Option<u32>,
    pub elapsed_ms: Option<u32>,
    pub lag_ms: Option<u32>,
    pub schema_ok: Option<bool>,
    pub turns: Vec<TraceTurn>,
}

impl HandRow {
    pub fn status_cell(&self) -> String {
        match self.outcome {
            None => format!("{:.1}s", self.spawned_at.elapsed().as_secs_f32()),
            Some(Outcome::Verdict) => format!("\u{2713} {:.1}s", self.elapsed_ms.unwrap_or(0) as f32 / 1000.0),
            Some(Outcome::Stop) => "stop".to_string(),
            Some(Outcome::Killed) => "killed".to_string(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.outcome.is_none()
    }
}

#[derive(Debug, Clone)]
pub struct GateEntry {
    pub allowed: bool,
    pub rule: String,
}

#[derive(Debug, Default)]
pub struct GateStats {
    pub allow: u32,
    pub deny: u32,
    pub ask: u32,
    pub recent: VecDeque<GateEntry>,
    pub durable_rules: Vec<String>,
    pub pending: Option<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct LastUsage {
    pub input: u32,
    pub output: u32,
    pub cache_read: u32,
    pub model: String,
    pub source: ModelSource,
    pub elapsed_ms: u32,
}

pub struct SessionState {
    pub session_started: Instant,
    pub model: String,
    pub occupancy: u32,
    pub checkpoint_number: u32,
    pub checkpoint_delta: i32,
    pub turn_count: u32,
    pub current_main: usize,
    pub mains: Vec<MainIncarnation>,
    pub judges: Vec<HandRow>,
    pub hands: Vec<HandRow>,
    pub gate: GateStats,
    pub last_usage: Option<LastUsage>,
    pub pending_tool: Option<(String, String)>,
    pub streaming_buffer: Option<String>,
}

impl SessionState {
    pub fn new() -> Self {
        let mut first_main = MainIncarnation::new("main #1".to_string(), "claude-opus-5".to_string());
        first_main.transcript.push(Line::Notice { kind: NoticeKind::Info, text: format!("grounding: {GROUNDING}") });
        SessionState {
            session_started: Instant::now(),
            model: "claude-opus-5".to_string(),
            occupancy: 61_412,
            checkpoint_number: 2,
            checkpoint_delta: 0,
            turn_count: 0,
            current_main: 0,
            mains: vec![first_main],
            judges: Vec::new(),
            hands: Vec::new(),
            gate: GateStats::default(),
            last_usage: None,
            pending_tool: None,
            streaming_buffer: None,
        }
    }

    fn transcript_mut(&mut self) -> &mut Vec<Line> {
        &mut self.mains[self.current_main].transcript
    }

    fn flush_stream(&mut self) -> bool {
        if let Some(buf) = self.streaming_buffer.take() {
            if !buf.is_empty() {
                self.transcript_mut().push(Line::Main(buf));
                return true;
            }
        }
        false
    }

    pub fn gauge_alert(&self) -> GaugeAlert {
        if self.occupancy >= WINDOW - RESERVE {
            GaugeAlert::ResetArmed
        } else if self.occupancy >= NUDGE_THRESHOLD {
            GaugeAlert::Nudge
        } else {
            GaugeAlert::Normal
        }
    }

    pub fn hands_in_flight(&self) -> usize {
        self.hands.iter().chain(self.judges.iter()).filter(|h| h.is_running()).count()
    }

    pub fn push_operator_line(&mut self, text: &str) {
        self.transcript_mut().push(Line::Operator(text.to_string()));
    }

    pub fn push_unknown_command_notice(&mut self) {
        self.transcript_mut().push(Line::Notice {
            kind: NoticeKind::Info,
            text: format!("unknown command; try one of: {}", SLASH_COMMANDS.join(", ")),
        });
    }

    fn find_row_mut(&mut self, id: &str) -> Option<&mut HandRow> {
        self.hands.iter_mut().chain(self.judges.iter_mut()).find(|h| h.id == id)
    }

    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Chunk { text } => match &mut self.streaming_buffer {
                Some(buf) => buf.push_str(&text),
                None => self.streaming_buffer = Some(text),
            },
            Event::TurnEnd { text } => {
                if !self.flush_stream() {
                    self.transcript_mut().push(Line::Main(text));
                }
                self.turn_count += 1;
            }
            Event::ExchangeEnd => {}
            Event::ToolCall { tool, subject } => {
                self.flush_stream();
                self.pending_tool = Some((tool, subject));
            }
            Event::ToolResult { admission } => {
                if let Some((tool, subject)) = self.pending_tool.take() {
                    let short_subject = subject.chars().take(60).collect::<String>();
                    let line = match admission {
                        Admission::Inline { bytes } => {
                            format!("\u{b7} {tool} {short_subject} \u{2192} inline {:.1} KB", bytes as f32 / 1024.0)
                        }
                        Admission::Externalized { bytes, handle, preview } => format!(
                            "\u{b7} {tool} {short_subject} \u{2192} externalized {:.1} KB \u{2192} {handle} \u{2192} \"{preview}\"",
                            bytes as f32 / 1024.0
                        ),
                    };
                    self.transcript_mut().push(Line::ToolSummary(line));
                }
            }
            Event::GateAsk { tool, subject } => {
                self.gate.ask += 1;
                self.gate.pending = Some((tool, subject));
            }
            Event::GateVerdict { allowed, rule } => {
                self.gate.pending = None;
                if allowed {
                    self.gate.allow += 1;
                } else {
                    self.gate.deny += 1;
                }
                self.gate.recent.push_back(GateEntry { allowed, rule });
                while self.gate.recent.len() > GATE_RECENT_LEN {
                    self.gate.recent.pop_front();
                }
            }
            Event::Spawn { id, agent, role, body, container, model, brief } => {
                self.transcript_mut().push(Line::DispatchSummary(format!("\u{b7} dispatched {id} {agent}")));
                let row = HandRow {
                    id,
                    agent,
                    role,
                    body,
                    container,
                    model,
                    brief,
                    spawned_at: Instant::now(),
                    outcome: None,
                    verdict_text: None,
                    trace: None,
                    bytes: None,
                    elapsed_ms: None,
                    lag_ms: None,
                    schema_ok: None,
                    turns: Vec::new(),
                };
                let bucket = match role {
                    Role::Hand => &mut self.hands,
                    Role::Judge => &mut self.judges,
                };
                bucket.push(row);
                while bucket.len() > HANDS_KEPT {
                    bucket.remove(0);
                }
            }
            Event::Delivered { id, outcome, text, trace, bytes, elapsed_ms, lag_ms, schema_ok, turns } => {
                self.transcript_mut().push(Line::Delivery { id: id.clone(), text: text.clone() });
                if let Some(row) = self.find_row_mut(&id) {
                    row.outcome = Some(outcome);
                    row.verdict_text = Some(text);
                    row.trace = Some(trace);
                    row.bytes = Some(bytes);
                    row.elapsed_ms = Some(elapsed_ms);
                    row.lag_ms = Some(lag_ms);
                    row.schema_ok = schema_ok;
                    row.turns = turns;
                }
            }
            Event::Notice { kind, text } => {
                if let Some(rule) = text.strip_prefix("durable rule appended: ").and_then(|s| s.split(" -> ").next()) {
                    self.gate.durable_rules.push(rule.to_string());
                }
                if text == "respawning main from checkpoint 3" {
                    let reason = format!("reset from checkpoint {}", self.checkpoint_number);
                    self.mains[self.current_main].ended = Some(reason);
                    self.mains.push(MainIncarnation::new(
                        format!("main #{}", self.mains.len() + 1),
                        self.model.clone(),
                    ));
                    self.current_main = self.mains.len() - 1;
                }
                match kind {
                    NoticeKind::Info
                        if text.starts_with("Write the state object") || text.starts_with("Check every claim") =>
                    {
                        self.transcript_mut().push(Line::ArxPrompt(text));
                    }
                    _ => self.transcript_mut().push(Line::Notice { kind, text }),
                }
            }
            Event::Usage { input, output, cache_read, model, source, elapsed_ms, occupancy_after } => {
                self.occupancy = occupancy_after;
                self.last_usage = Some(LastUsage { input, output, cache_read, model, source, elapsed_ms });
            }
            Event::Checkpoint { number, path, delta } => {
                self.checkpoint_delta = delta;
                self.checkpoint_number = number;
                self.transcript_mut().push(Line::Notice {
                    kind: NoticeKind::Info,
                    text: format!("checkpoint {number} written: {path}"),
                });
            }
            Event::ResetArmed { occupancy, window, reserve } => {
                self.transcript_mut().push(Line::Notice {
                    kind: NoticeKind::ResetArmed,
                    text: format!("window at {occupancy} of {window}, reserve {reserve}; /reset to reset now"),
                });
            }
            Event::ResetConfirm { state_object } => {
                self.transcript_mut().push(Line::ResetConfirmBlock(state_object));
                self.transcript_mut().push(Line::Notice {
                    kind: NoticeKind::Info,
                    text: "ok to tear down and respawn; any other line rides into the new main's first turn as an operator note".to_string(),
                });
            }
            Event::Orientation { text, verdict, occupancy_after } => {
                self.occupancy = occupancy_after;
                self.turn_count += 1;
                if !self.flush_stream() {
                    self.transcript_mut().push(Line::Main(text));
                }
                self.transcript_mut().push(Line::Notice { kind: NoticeKind::Info, text: format!("orientation: {verdict}") });
            }
            Event::ModelList { .. } => {}
            Event::ModelSwitched { from, to } => {
                self.model = to.clone();
                self.transcript_mut().push(Line::Notice { kind: NoticeKind::Info, text: format!("model: {from} -> {to}") });
            }
            Event::ProviderError { text } => {
                self.transcript_mut().push(Line::Notice { kind: NoticeKind::Error, text });
            }
            Event::Aborted { partial } => {
                self.streaming_buffer = None;
                self.transcript_mut().push(Line::AbortedPartial(partial));
            }
        }
    }
}
