use std::collections::HashMap;
use std::time::Instant;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::event::{Admission, AskAnswer, DispatchInfo, Delivery, Event, Key, Usage, UiEvent};
use crate::markdown::{
    amber_style, arrival_style, arx_role_style, dim_style, operator_style, red_style,
    render_markdown_lite, tool_head_style,
};

const SPINNER: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
const VERBS: [&str; 4] = ["Thinking", "Reading", "Composing", "Working"];
pub const COMMANDS: [(&str, &str); 4] = [
    ("/reset", "tear down and respawn main from the last checkpoint"),
    ("/model", "open the model picker"),
    ("/model <id>", "switch directly, no picker"),
    ("/checkpoint", "write a state object checkpoint"),
];

#[derive(Debug, Clone, Copy)]
pub enum NoticeKind {
    Dim,
    Amber,
    Red,
}

fn notice_style(kind: NoticeKind) -> Style {
    match kind {
        NoticeKind::Dim => dim_style(),
        NoticeKind::Amber => amber_style(),
        NoticeKind::Red => red_style(),
    }
}

pub struct ToolLive {
    pub tool: String,
    pub subject: String,
    pub started_at: Instant,
    pub ask_open: bool,
    pub result: Option<(Admission, String)>,
}

pub struct DispatchLive {
    pub info: DispatchInfo,
    pub started_at: Instant,
    pub delivered: Option<Delivery>,
}

pub enum Segment {
    Text(String),
    Tool(ToolLive),
    Dispatch(DispatchLive),
    Notice(String, NoticeKind),
}

pub struct TurnLive {
    pub started_at: Instant,
    pub verb: &'static str,
    pub got_first_token: bool,
    pub segments: Vec<Segment>,
}

impl TurnLive {
    fn new(turn_no: u32) -> Self {
        TurnLive {
            started_at: Instant::now(),
            verb: VERBS[turn_no as usize % VERBS.len()],
            got_first_token: false,
            segments: Vec::new(),
        }
    }

    fn push_text(&mut self, chunk: String) {
        match self.segments.last_mut() {
            Some(Segment::Text(s)) => s.push_str(&chunk),
            _ => self.segments.push(Segment::Text(chunk)),
        }
    }

    fn last_open_tool(&mut self) -> Option<&mut ToolLive> {
        self.segments.iter_mut().rev().find_map(|s| match s {
            Segment::Tool(t) if t.result.is_none() => Some(t),
            _ => None,
        })
    }

    fn open_dispatch(&mut self, hand_id: &str) -> Option<&mut DispatchLive> {
        self.segments.iter_mut().rev().find_map(|s| match s {
            Segment::Dispatch(d) if d.info.hand_id == hand_id && d.delivered.is_none() => Some(d),
            _ => None,
        })
    }
}

pub enum Overlay {
    None,
    Ask {
        tool: String,
        subject: String,
        selected: usize,
        note: String,
    },
    ModelPicker {
        selected: usize,
    },
    ResetConfirm {
        body: String,
        input: String,
    },
}

pub struct Input {
    pub buffer: String,
    pub history: Vec<String>,
    pub history_idx: Option<usize>,
}

impl Input {
    fn new() -> Self {
        Input {
            buffer: String::new(),
            history: Vec::new(),
            history_idx: None,
        }
    }
}

pub struct App {
    pub scope: String,
    pub store_root: String,
    pub model: String,
    pub models: Vec<String>,
    pub occupancy: u64,
    pub window: u64,
    pub reserve: u64,
    pub checkpoint_number: u32,
    pub hands_in_flight: u32,
    pub last_usage: Option<Usage>,
    pub dispatches: HashMap<String, DispatchInfo>,
    pub scrollback: Vec<Line<'static>>,
    pub committed_len: usize,
    pub input: Input,
    pub live: Option<TurnLive>,
    pub overlay: Overlay,
    pub turn_no: u32,
    pub quit: bool,
}

impl App {
    pub fn new() -> Self {
        App {
            scope: "arx".to_string(),
            store_root: "~/.local/share/arx/store".to_string(),
            model: "claude-opus-5".to_string(),
            models: vec![
                "claude-opus-5".to_string(),
                "claude-sonnet-5".to_string(),
                "claude-haiku-4-5".to_string(),
            ],
            occupancy: 0,
            window: 200_000,
            reserve: 40_000,
            checkpoint_number: 2,
            hands_in_flight: 0,
            last_usage: None,
            dispatches: HashMap::new(),
            scrollback: Vec::new(),
            committed_len: 0,
            input: Input::new(),
            live: None,
            overlay: Overlay::None,
            turn_no: 0,
            quit: false,
        }
    }

    pub fn take_new_scrollback(&mut self) -> Vec<Line<'static>> {
        let fresh = self.scrollback[self.committed_len..].to_vec();
        self.committed_len = self.scrollback.len();
        fresh
    }

    fn push_scrollback(&mut self, lines: Vec<Line<'static>>) {
        self.scrollback.extend(lines);
    }

    fn commit_notice(&mut self, text: String, kind: NoticeKind) {
        self.push_scrollback(vec![Line::styled(
            format!("[arx] {text}"),
            notice_style(kind),
        )]);
    }

    pub fn commit_operator_line(&mut self, text: &str) {
        self.push_scrollback(vec![Line::styled(
            format!("> {text}"),
            operator_style(),
        )]);
    }

    fn commit_arx_line(&mut self, text: String) {
        self.push_scrollback(vec![Line::styled(format!("» {text}"), arx_role_style())]);
    }

    fn commit_arrival(&mut self, delivery: Delivery) {
        let agent = self
            .dispatches
            .get(&delivery.hand_id)
            .map(|i| i.agent.clone())
            .unwrap_or_else(|| delivery.hand_id.clone());
        let mut lines = vec![Line::styled(
            format!(
                "↳ delivered {} · {} · verdict in {:.1}s → {}",
                delivery.hand_id,
                agent,
                delivery.elapsed_ms as f32 / 1000.0,
                delivery.trace
            ),
            arrival_style(),
        )];
        lines.push(Line::styled(format!("  {}", delivery.text), dim_style()));
        self.push_scrollback(lines);
    }

    fn segment_lines(seg: Segment, now: Instant) -> Vec<Line<'static>> {
        match seg {
            Segment::Text(s) => render_markdown_lite(&s),
            Segment::Notice(text, kind) => {
                vec![Line::styled(format!("[arx] {text}"), notice_style(kind))]
            }
            Segment::Tool(t) => {
                let head = format!("⏺ {} {}", t.tool, t.subject);
                let mut lines = vec![Line::styled(head, tool_head_style())];
                match t.result {
                    Some((Admission::Inline { bytes }, preview)) => {
                        lines.push(Line::styled(
                            format!("  ⎿ inline · {}", fmt_kb(bytes)),
                            dim_style(),
                        ));
                        if !preview.is_empty() {
                            lines.push(Line::styled(format!("      {preview}"), dim_style()));
                        }
                    }
                    Some((Admission::Externalized { bytes, trace }, preview)) => {
                        lines.push(Line::styled(
                            format!("  ⎿ externalized {} → {}", fmt_kb(bytes), trace),
                            dim_style(),
                        ));
                        if !preview.is_empty() {
                            lines.push(Line::styled(format!("      {preview}"), dim_style()));
                        }
                    }
                    None => {
                        let elapsed = now.duration_since(t.started_at).as_secs_f32();
                        lines.push(Line::styled(
                            format!("  ⎿ running {elapsed:.1}s"),
                            dim_style(),
                        ));
                    }
                }
                lines
            }
            Segment::Dispatch(d) => {
                let head = format!(
                    "⏺ dispatch {} · {} · {} · {}",
                    d.info.agent, d.info.body, d.info.model, d.info.hand_id
                );
                let mut lines = vec![Line::styled(head, tool_head_style())];
                match d.delivered {
                    Some(delivery) => {
                        lines.push(Line::styled(
                            format!(
                                "  ⎿ verdict in {:.1}s → {}",
                                delivery.elapsed_ms as f32 / 1000.0,
                                delivery.trace
                            ),
                            dim_style(),
                        ));
                        lines.push(Line::styled(format!("      {}", delivery.text), dim_style()));
                    }
                    None => {
                        let elapsed = now.duration_since(d.started_at).as_secs_f32();
                        lines.push(Line::styled(
                            format!("  ⎿ running {elapsed:.1}s"),
                            dim_style(),
                        ));
                    }
                }
                lines
            }
        }
    }

    fn commit_turn(&mut self, turn: TurnLive) {
        let now = Instant::now();
        let mut lines = Vec::new();
        for seg in turn.segments {
            lines.extend(Self::segment_lines(seg, now));
        }
        self.push_scrollback(lines);
    }

    pub fn on_engine_event(&mut self, ev: Event) {
        match ev {
            Event::Notice(text) => self.commit_notice(text, NoticeKind::Dim),
            Event::Amber(text) => self.commit_notice(text, NoticeKind::Amber),
            Event::Error(text) => self.commit_notice(text, NoticeKind::Red),
            Event::ArxLine(text) => self.commit_arx_line(text),
            Event::TurnStart => {
                self.live = Some(TurnLive::new(self.turn_no));
                self.turn_no += 1;
            }
            Event::Chunk(text) => {
                if let Some(turn) = &mut self.live {
                    turn.got_first_token = true;
                    turn.push_text(text);
                }
            }
            Event::ToolCall { tool, subject } => {
                if let Some(turn) = &mut self.live {
                    turn.segments.push(Segment::Tool(ToolLive {
                        tool,
                        subject,
                        started_at: Instant::now(),
                        ask_open: false,
                        result: None,
                    }));
                }
            }
            Event::GateAsk { tool, subject } => {
                if let Some(turn) = &mut self.live {
                    if let Some(t) = turn.last_open_tool() {
                        t.ask_open = true;
                    }
                }
                self.overlay = Overlay::Ask {
                    tool,
                    subject,
                    selected: 0,
                    note: String::new(),
                };
            }
            Event::GateVerdict { tool, .. } => {
                if tool != "dispatch" {
                    if let Some(turn) = &mut self.live {
                        if let Some(t) = turn.last_open_tool() {
                            t.ask_open = false;
                        }
                    }
                    self.overlay = Overlay::None;
                }
            }
            Event::ToolResult { admission, preview } => {
                if let Some(turn) = &mut self.live {
                    if let Some(t) = turn.last_open_tool() {
                        t.result = Some((admission, preview));
                    }
                }
            }
            Event::Spawn(info) => {
                self.dispatches.insert(info.hand_id.clone(), info.clone());
                self.hands_in_flight += 1;
                if let Some(turn) = &mut self.live {
                    turn.segments.push(Segment::Dispatch(DispatchLive {
                        info,
                        started_at: Instant::now(),
                        delivered: None,
                    }));
                }
            }
            Event::Delivered(delivery) => {
                let mut settled_live = false;
                if let Some(turn) = &mut self.live {
                    if let Some(d) = turn.open_dispatch(&delivery.hand_id) {
                        d.delivered = Some(delivery.clone());
                        settled_live = true;
                    }
                }
                if !settled_live {
                    self.commit_arrival(delivery);
                }
                self.hands_in_flight = self.hands_in_flight.saturating_sub(1);
            }
            Event::Usage(u) => {
                self.model = u.model.clone();
                self.occupancy = u.occupancy_after;
                self.last_usage = Some(u);
            }
            Event::TurnEnd(canonical) => match self.live.take() {
                Some(turn) => self.commit_turn(turn),
                None => self.push_scrollback(render_markdown_lite(&canonical)),
            },
            Event::CheckpointWritten {
                number,
                path,
                delta,
                since,
            } => {
                self.checkpoint_number = number;
                self.commit_notice(format!("checkpoint {number} written: {path}"), NoticeKind::Dim);
                self.commit_notice(
                    format!("delta +{delta} since checkpoint {since}"),
                    NoticeKind::Dim,
                );
            }
            Event::ResetConfirm { body } => {
                self.overlay = Overlay::ResetConfirm {
                    body,
                    input: String::new(),
                };
            }
            Event::Orientation { verdict, .. } => {
                self.commit_notice(format!("orientation: {verdict}"), NoticeKind::Dim);
            }
            Event::ModelSwitched { from, to } => {
                self.model = to.clone();
                self.commit_notice(format!("model: {from} -> {to}"), NoticeKind::Dim);
                self.overlay = Overlay::None;
            }
            Event::ModelAlready(m) => {
                self.commit_notice(format!("already on {m}"), NoticeKind::Dim);
                self.overlay = Overlay::None;
            }
            Event::ModelUnlisted { requested, models } => {
                self.commit_notice(
                    format!("unlisted model {requested}; choices: {}", models.join(", ")),
                    NoticeKind::Amber,
                );
                self.overlay = Overlay::None;
            }
            Event::ProviderError(text) => {
                if let Some(turn) = &mut self.live {
                    turn.segments
                        .push(Segment::Notice(text, NoticeKind::Red));
                } else {
                    self.commit_notice(text, NoticeKind::Red);
                }
            }
            Event::RetryOk => {
                if let Some(turn) = &mut self.live {
                    turn.segments
                        .push(Segment::Notice("retrying now".to_string(), NoticeKind::Dim));
                }
            }
            Event::Aborted => {
                if let Some(mut turn) = self.live.take() {
                    turn.segments
                        .push(Segment::Notice("aborted".to_string(), NoticeKind::Amber));
                    self.commit_turn(turn);
                }
            }
            Event::OccupancyReset(v) => self.occupancy = v,
        }
    }
}

pub fn fmt_kb(bytes: u64) -> String {
    format!("{:.1} KB", bytes as f64 / 1000.0)
}

pub fn fmt_num(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut parts = Vec::new();
    while n > 0 {
        parts.push(format!("{:03}", n % 1000));
        n /= 1000;
    }
    parts.reverse();
    let joined = parts.join(",");
    joined.trim_start_matches('0').to_string()
}

impl App {
    pub fn live_height(&self, width: u16) -> u16 {
        let turn_h = self.live_turn_height(width);
        let overlay_h = self.overlay_height();
        let input_h = if matches!(self.overlay, Overlay::None) {
            self.input_box_height()
        } else {
            0
        };
        (turn_h + overlay_h + input_h + 1).clamp(4, 28)
    }

    fn live_turn_height(&self, width: u16) -> u16 {
        match &self.live {
            None => 0,
            Some(turn) => {
                if !turn.got_first_token && turn.segments.is_empty() {
                    1
                } else {
                    let mut h = 0u16;
                    for seg in &turn.segments {
                        h += Self::segment_height(seg, width);
                    }
                    h.max(1)
                }
            }
        }
    }

    fn segment_height(seg: &Segment, width: u16) -> u16 {
        match seg {
            Segment::Text(s) => wrap_height(s, width).max(1),
            Segment::Notice(_, _) => 1,
            Segment::Tool(t) => {
                if t.result.is_some() {
                    3
                } else {
                    2
                }
            }
            Segment::Dispatch(d) => {
                if d.delivered.is_some() {
                    3
                } else {
                    2
                }
            }
        }
    }

    fn overlay_height(&self) -> u16 {
        match &self.overlay {
            Overlay::None => 0,
            Overlay::Ask { .. } => 9,
            Overlay::ModelPicker { .. } => 7,
            Overlay::ResetConfirm { body, .. } => body.lines().count() as u16 + 6,
        }
    }

    fn input_box_height(&self) -> u16 {
        let wrapped = self.input.buffer.lines().count().max(1);
        (wrapped as u16 + 2).clamp(3, 7)
    }
}

fn wrap_height(text: &str, width: u16) -> u16 {
    let w = width.max(10) as usize;
    text.lines()
        .map(|l| (l.chars().count().max(1) + w - 1) / w)
        .sum::<usize>()
        .max(1) as u16
}

impl App {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let turn_h = self.live_turn_height(area.width);
        let overlay_h = self.overlay_height();
        let input_h = if matches!(self.overlay, Overlay::None) {
            self.input_box_height()
        } else {
            0
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(turn_h),
                Constraint::Length(overlay_h),
                Constraint::Length(input_h),
                Constraint::Length(1),
            ])
            .split(area);

        self.render_turn(frame, chunks[0]);
        self.render_overlay(frame, chunks[1]);
        if matches!(self.overlay, Overlay::None) {
            self.render_input(frame, chunks[2]);
        }
        self.render_status(frame, chunks[3]);
    }

    fn render_turn(&self, frame: &mut Frame, area: Rect) {
        if area.height == 0 {
            return;
        }
        let Some(turn) = &self.live else { return };
        if !turn.got_first_token && turn.segments.is_empty() {
            let elapsed = turn.started_at.elapsed();
            let frame_idx = (elapsed.as_millis() / 90) as usize % SPINNER.len();
            let line = format!(
                "{} {}… ({:.1}s · esc to abort)",
                SPINNER[frame_idx],
                turn.verb,
                elapsed.as_secs_f32()
            );
            frame.render_widget(Paragraph::new(line), area);
            return;
        }
        let now = Instant::now();
        let mut lines: Vec<Line<'static>> = Vec::new();
        for seg in &turn.segments {
            lines.extend(Self::preview_segment_lines(seg, now));
        }
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
    }

    fn preview_segment_lines(seg: &Segment, now: Instant) -> Vec<Line<'static>> {
        match seg {
            Segment::Text(s) => render_markdown_lite(s),
            Segment::Notice(text, kind) => {
                vec![Line::styled(format!("[arx] {text}"), notice_style(*kind))]
            }
            Segment::Tool(t) => {
                let head = format!("⏺ {} {}", t.tool, t.subject);
                let mut lines = vec![Line::styled(head, tool_head_style())];
                match &t.result {
                    Some((Admission::Inline { bytes }, preview)) => {
                        lines.push(Line::styled(
                            format!("  ⎿ inline · {}", fmt_kb(*bytes)),
                            dim_style(),
                        ));
                        if !preview.is_empty() {
                            lines.push(Line::styled(format!("      {preview}"), dim_style()));
                        }
                    }
                    Some((Admission::Externalized { bytes, trace }, preview)) => {
                        lines.push(Line::styled(
                            format!("  ⎿ externalized {} → {}", fmt_kb(*bytes), trace),
                            dim_style(),
                        ));
                        if !preview.is_empty() {
                            lines.push(Line::styled(format!("      {preview}"), dim_style()));
                        }
                    }
                    None if t.ask_open => {
                        lines.push(Line::styled("  ⎿ waiting on gate ask".to_string(), amber_style()));
                    }
                    None => {
                        let elapsed = now.duration_since(t.started_at).as_secs_f32();
                        lines.push(Line::styled(
                            format!("  ⎿ running {elapsed:.1}s"),
                            dim_style(),
                        ));
                    }
                }
                lines
            }
            Segment::Dispatch(d) => {
                let head = format!(
                    "⏺ dispatch {} · {} · {} · {}",
                    d.info.agent, d.info.body, d.info.model, d.info.hand_id
                );
                let mut lines = vec![Line::styled(head, tool_head_style())];
                match &d.delivered {
                    Some(delivery) => {
                        lines.push(Line::styled(
                            format!(
                                "  ⎿ verdict in {:.1}s → {}",
                                delivery.elapsed_ms as f32 / 1000.0,
                                delivery.trace
                            ),
                            dim_style(),
                        ));
                        lines.push(Line::styled(format!("      {}", delivery.text), dim_style()));
                    }
                    None => {
                        let elapsed = now.duration_since(d.started_at).as_secs_f32();
                        lines.push(Line::styled(
                            format!("  ⎿ running {elapsed:.1}s"),
                            dim_style(),
                        ));
                    }
                }
                lines
            }
        }
    }

    fn render_overlay(&self, frame: &mut Frame, area: Rect) {
        if area.height == 0 {
            return;
        }
        match &self.overlay {
            Overlay::None => {}
            Overlay::Ask {
                tool,
                subject,
                selected,
                note,
            } => {
                let choices = [
                    ("a", "allow once"),
                    ("A", "allow always"),
                    ("d", "deny once"),
                    ("D", "deny always"),
                ];
                let mut lines = vec![
                    Line::styled(format!("ask: {tool} {subject}"), amber_style()),
                    Line::raw(""),
                ];
                for (i, (key, label)) in choices.iter().enumerate() {
                    let marker = if i == *selected { "› " } else { "  " };
                    let style = if i == *selected {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        dim_style()
                    };
                    lines.push(Line::styled(format!("{marker}{key}  {label}"), style));
                }
                lines.push(Line::raw(""));
                lines.push(Line::styled(format!("note: {note}"), dim_style()));
                let block = Block::default().borders(Borders::ALL).title("gate ask");
                frame.render_widget(Paragraph::new(lines).block(block), area);
            }
            Overlay::ModelPicker { selected } => {
                let items: Vec<ListItem> = self
                    .models
                    .iter()
                    .enumerate()
                    .map(|(i, m)| {
                        let current = if *m == self.model { " (current)" } else { "" };
                        let marker = if i == *selected { "› " } else { "  " };
                        ListItem::new(format!("{marker}{}. {m}{current}", i + 1))
                    })
                    .collect();
                let block = Block::default().borders(Borders::ALL).title("/model");
                frame.render_widget(List::new(items).block(block), area);
            }
            Overlay::ResetConfirm { body, input } => {
                let mut lines: Vec<Line<'static>> = Vec::new();
                for row in body.lines() {
                    lines.push(Line::styled(row.to_string(), dim_style()));
                }
                lines.push(Line::raw(""));
                lines.push(Line::raw(
                    "ok to tear down and respawn; any other line rides into the new main's first turn as an operator note",
                ));
                lines.push(Line::styled(format!("> {input}"), operator_style()));
                let block = Block::default().borders(Borders::ALL).title("/reset");
                frame.render_widget(Paragraph::new(lines).block(block), area);
            }
        }
    }

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL);
        let text = format!("> {}", self.input.buffer);
        frame.render_widget(Paragraph::new(text).block(block).wrap(Wrap { trim: false }), area);

        if self.input.buffer.starts_with('/') && !self.input.buffer.contains(' ') {
            let matches: Vec<&(&str, &str)> = COMMANDS
                .iter()
                .filter(|(name, _)| name.starts_with(self.input.buffer.as_str()) || self.input.buffer == "/")
                .collect();
            if !matches.is_empty() && area.y >= matches.len() as u16 + 1 {
                let hint_h = matches.len() as u16 + 2;
                let hint_area = Rect {
                    x: area.x,
                    y: area.y.saturating_sub(hint_h),
                    width: area.width,
                    height: hint_h,
                };
                let items: Vec<ListItem> = matches
                    .iter()
                    .map(|(name, desc)| ListItem::new(format!("{name}  {desc}")))
                    .collect();
                let block = Block::default().borders(Borders::ALL).title("commands");
                frame.render_widget(List::new(items).block(block), hint_area);
            }
        }
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let mut parts = vec![
            self.model.clone(),
            self.scope.clone(),
            format!(
                "{}/{} (reserve {})",
                fmt_num(self.occupancy),
                fmt_num(self.window),
                fmt_num(self.reserve),
            ),
            format!("cp {}", self.checkpoint_number),
            format!("hands {}", self.hands_in_flight),
        ];
        if let Some(u) = &self.last_usage {
            parts.push(format!(
                "last {}+{} tok · {:.2}s",
                fmt_num(u.input as u64),
                fmt_num(u.output as u64),
                u.elapsed_ms as f32 / 1000.0
            ));
        }
        let line = Line::styled(parts.join(" · "), dim_style());
        frame.render_widget(Paragraph::new(line), area);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn ask_answer_from(selected: usize, note: &str) -> AskAnswer {
    let key = match selected {
        0 => 'a',
        1 => 'A',
        2 => 'd',
        _ => 'D',
    };
    AskAnswer {
        key,
        note: if note.is_empty() { None } else { Some(note.to_string()) },
    }
}

impl App {
    pub fn parse_line(&mut self, line: &str) -> Option<UiEvent> {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            return None;
        }
        self.input.history.push(trimmed.to_string());
        self.input.history_idx = None;
        if let Some(rest) = trimmed.strip_prefix("/model") {
            let id = rest.trim();
            self.commit_operator_line(trimmed);
            if id.is_empty() {
                self.overlay = Overlay::ModelPicker {
                    selected: self.models.iter().position(|m| *m == self.model).unwrap_or(0),
                };
                return None;
            }
            return Some(UiEvent::ModelPick(id.to_string()));
        }
        if trimmed == "/checkpoint" {
            self.commit_operator_line(trimmed);
            return Some(UiEvent::Checkpoint);
        }
        if trimmed == "/reset" {
            self.commit_operator_line(trimmed);
            return Some(UiEvent::Reset);
        }
        if let Some(cmd) = trimmed.strip_prefix('/') {
            let _ = cmd;
            self.input.history.pop();
            self.commit_notice(
                "unknown command; try /reset, /model, /model <id>, /checkpoint".to_string(),
                NoticeKind::Amber,
            );
            return None;
        }
        self.commit_operator_line(trimmed);
        Some(UiEvent::Chat(trimmed.to_string()))
    }

    pub fn submit_buffer(&mut self) -> Option<UiEvent> {
        let line = std::mem::take(&mut self.input.buffer);
        self.parse_line(&line)
    }

    pub fn ask_select(&mut self, key: char) {
        if let Overlay::Ask { selected, .. } = &mut self.overlay {
            *selected = match key {
                'a' => 0,
                'A' => 1,
                'd' => 2,
                'D' => 3,
                _ => *selected,
            };
        }
    }

    pub fn ask_move(&mut self, delta: i32) {
        if let Overlay::Ask { selected, .. } = &mut self.overlay {
            let n = 4i32;
            *selected = ((*selected as i32 + delta).rem_euclid(n)) as usize;
        }
    }

    pub fn ask_type(&mut self, c: char) {
        if let Overlay::Ask { note, .. } = &mut self.overlay {
            note.push(c);
        }
    }

    pub fn ask_backspace(&mut self) {
        if let Overlay::Ask { note, .. } = &mut self.overlay {
            note.pop();
        }
    }

    pub fn ask_confirm(&mut self) -> Option<UiEvent> {
        if let Overlay::Ask { selected, note, .. } = &self.overlay {
            let answer = ask_answer_from(*selected, note);
            self.overlay = Overlay::None;
            return Some(UiEvent::AskAnswer(answer));
        }
        None
    }

    pub fn picker_move(&mut self, delta: i32) {
        if let Overlay::ModelPicker { selected } = &mut self.overlay {
            let n = self.models.len() as i32;
            *selected = ((*selected as i32 + delta).rem_euclid(n)) as usize;
        }
    }

    pub fn picker_confirm(&mut self) -> Option<UiEvent> {
        if let Overlay::ModelPicker { selected } = &self.overlay {
            let id = self.models[*selected].clone();
            self.overlay = Overlay::None;
            return Some(UiEvent::ModelPick(id));
        }
        None
    }

    pub fn picker_pick_index(&mut self, idx: usize) -> Option<UiEvent> {
        if idx < self.models.len() {
            if let Overlay::ModelPicker { selected } = &mut self.overlay {
                *selected = idx;
            }
            return self.picker_confirm();
        }
        None
    }

    pub fn picker_confirm_by_id(&mut self, id: &str) -> Option<UiEvent> {
        let idx = self.models.iter().position(|m| m == id)?;
        self.picker_pick_index(idx)
    }

    pub fn confirm_type(&mut self, c: char) {
        if let Overlay::ResetConfirm { input, .. } = &mut self.overlay {
            input.push(c);
        }
    }

    pub fn confirm_backspace(&mut self) {
        if let Overlay::ResetConfirm { input, .. } = &mut self.overlay {
            input.pop();
        }
    }

    pub fn confirm_submit(&mut self) -> Option<UiEvent> {
        if let Overlay::ResetConfirm { input, .. } = &self.overlay {
            let line = input.clone();
            self.overlay = Overlay::None;
            return Some(UiEvent::ConfirmLine(line));
        }
        None
    }

    pub fn confirm_submit_with(&mut self, line: &str) -> Option<UiEvent> {
        if let Overlay::ResetConfirm { input, .. } = &mut self.overlay {
            *input = line.to_string();
        }
        self.confirm_submit()
    }

    fn tab_complete(&mut self) {
        if !self.input.buffer.starts_with('/') {
            return;
        }
        let names = ["/reset", "/model", "/checkpoint"];
        let matches: Vec<&&str> = names
            .iter()
            .filter(|n| n.starts_with(self.input.buffer.as_str()))
            .collect();
        if matches.len() == 1 {
            self.input.buffer = matches[0].to_string();
        }
    }

    fn history_prev(&mut self) {
        if self.input.history.is_empty() {
            return;
        }
        let idx = match self.input.history_idx {
            None => self.input.history.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.input.history_idx = Some(idx);
        self.input.buffer = self.input.history[idx].clone();
    }

    fn history_next(&mut self) {
        match self.input.history_idx {
            None => {}
            Some(i) if i + 1 < self.input.history.len() => {
                self.input.history_idx = Some(i + 1);
                self.input.buffer = self.input.history[i + 1].clone();
            }
            Some(_) => {
                self.input.history_idx = None;
                self.input.buffer.clear();
            }
        }
    }

    pub fn on_key(&mut self, key: Key) -> Option<UiEvent> {
        if key == Key::CtrlC {
            self.quit = true;
            return None;
        }
        match &self.overlay {
            Overlay::Ask { .. } => match key {
                Key::Char(c @ ('a' | 'A' | 'd' | 'D')) => {
                    self.ask_select(c);
                    None
                }
                Key::Up => {
                    self.ask_move(-1);
                    None
                }
                Key::Down => {
                    self.ask_move(1);
                    None
                }
                Key::Backspace => {
                    self.ask_backspace();
                    None
                }
                Key::Enter => self.ask_confirm(),
                Key::Char(c) => {
                    self.ask_type(c);
                    None
                }
                _ => None,
            },
            Overlay::ModelPicker { .. } => match key {
                Key::Up | Key::Char('k') => {
                    self.picker_move(-1);
                    None
                }
                Key::Down | Key::Char('j') => {
                    self.picker_move(1);
                    None
                }
                Key::Char(c) if c.is_ascii_digit() && c != '0' => {
                    let idx = c.to_digit(10).unwrap() as usize - 1;
                    self.picker_pick_index(idx)
                }
                Key::Enter => self.picker_confirm(),
                Key::Esc => {
                    self.overlay = Overlay::None;
                    None
                }
                _ => None,
            },
            Overlay::ResetConfirm { .. } => match key {
                Key::Enter => self.confirm_submit(),
                Key::Backspace => {
                    self.confirm_backspace();
                    None
                }
                Key::Char(c) => {
                    self.confirm_type(c);
                    None
                }
                Key::Esc => {
                    self.overlay = Overlay::None;
                    None
                }
                _ => None,
            },
            Overlay::None => match key {
                Key::Esc => {
                    if self.live.is_some() {
                        Some(UiEvent::Abort)
                    } else {
                        None
                    }
                }
                Key::Enter => self.submit_buffer(),
                Key::Tab => {
                    self.tab_complete();
                    None
                }
                Key::Backspace => {
                    self.input.buffer.pop();
                    None
                }
                Key::Up => {
                    if self.input.buffer.is_empty() {
                        self.history_prev();
                    }
                    None
                }
                Key::Down => {
                    if self.input.buffer.is_empty() || self.input.history_idx.is_some() {
                        self.history_next();
                    }
                    None
                }
                Key::Char(c) => {
                    self.input.buffer.push(c);
                    None
                }
                Key::CtrlC => unreachable!(),
            },
        }
    }
}
