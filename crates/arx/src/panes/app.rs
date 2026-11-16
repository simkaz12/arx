use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::Line;
use unicode_width::UnicodeWidthChar;

use crate::events::Event;
use crate::panes::clipboard::{Clip, Clipboard, WlPasteClipboard};
use crate::panes::keys::{self, KeyMap};
use crate::panes::palette::Palette;
use crate::panes::state::{self, Outcome, SessionState, Stop};
use crate::panes::trace;
use crate::terminal::{Attachment, Control, RosterModel, TerminalMsg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelId {
    Session,
    Main,
    Reviewers,
    Hands,
}

impl PanelId {
    pub fn number(self) -> u8 {
        match self {
            PanelId::Session => 1,
            PanelId::Main => 2,
            PanelId::Reviewers => 3,
            PanelId::Hands => 4,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PanelId::Session => "session",
            PanelId::Main => "main",
            PanelId::Reviewers => "reviewers",
            PanelId::Hands => "hands",
        }
    }
}

pub const PANEL_ORDER: [PanelId; 4] = [PanelId::Session, PanelId::Main, PanelId::Reviewers, PanelId::Hands];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Panel(PanelId),
    BigView(PanelId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartyKey {
    Session,
    Main(usize),
    Reviewer(usize),
    Hand(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pane {
    Single,
    Brief,
    Turns,
    Doc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneKey(pub PartyKey, pub Pane);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    Brief,
    Turns,
}

impl Half {
    fn swapped(self) -> Half {
        match self {
            Half::Brief => Half::Turns,
            Half::Turns => Half::Brief,
        }
    }

    fn to_pane(self) -> Pane {
        match self {
            Half::Brief => Pane::Brief,
            Half::Turns => Pane::Turns,
        }
    }
}

pub fn is_split_party(party: PartyKey) -> bool {
    matches!(party, PartyKey::Reviewer(_) | PartyKey::Hand(_))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollState {
    Following,
    Anchored { row: usize, rows_at_anchor: usize },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PaneMetrics {
    pub total_rows: usize,
    pub visible: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct RowsCache {
    width: u16,
    source_len: usize,
    rows: usize,
    initialized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkdownCacheKey {
    MainTurn { main: usize, turn: usize },
    Doc,
}

#[derive(Debug, Clone)]
pub enum Modal {
    ModelPicker { models: Vec<RosterModel>, tabs: Vec<ModelPickerTab>, tab: usize, main_family: String },
    Confirm { text: String, input: String, action: ConfirmAction },
}

/// A tab's editable state in the modal — `cursor` is the pending choice (0 =
/// the `from file` row, `i` = `model_ids[i - 1]`), `opened_cursor` is where it
/// started so Enter can tell a changed tab from an unvisited one.
#[derive(Debug, Clone)]
pub struct ModelPickerTab {
    pub position: String,
    pub fallback_id: String,
    pub model_ids: Vec<String>,
    pub opened_cursor: usize,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    Reset,
    Kill { panel: PanelId, id: String },
}

#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub start: usize,
    pub end: usize,
}

fn char_byte_offset(text: &str, char_idx: usize) -> usize {
    text.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(text.len())
}

pub fn wrap_input(text: &str, width: usize) -> Vec<Row> {
    let width = width.max(1);
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![Row { start: 0, end: 0 }];
    }

    let mut rows = Vec::new();
    let mut row_start = 0usize;
    let mut col = 0usize;
    let mut last_space: Option<usize> = None;

    for i in 0..chars.len() {
        let c = chars[i];
        if c == '\n' {
            rows.push(Row { start: row_start, end: i });
            row_start = i + 1;
            col = 0;
            last_space = None;
            continue;
        }
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if col + w > width && i > row_start {
            let (break_at, next_start) = match last_space {
                Some(space_idx) => (space_idx, space_idx + 1),
                None => (i, i),
            };
            rows.push(Row { start: row_start, end: break_at });
            row_start = next_start;
            col = chars[row_start..i].iter().map(|c2| UnicodeWidthChar::width(*c2).unwrap_or(0)).sum::<usize>() + w;
            last_space = None;
        } else {
            col += w;
        }
        if c == ' ' {
            last_space = Some(i);
        }
    }
    rows.push(Row { start: row_start, end: chars.len() });
    rows
}

fn normalize_pasted(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn next_image_number(dir: &Path) -> u32 {
    let mut max = 0u32;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                if let Ok(n) = stem.parse::<u32>() {
                    max = max.max(n);
                }
            }
        }
    }
    max + 1
}

fn is_intercepted_command(line: &str) -> bool {
    let trimmed = line.trim();
    for prefix in ["/model", "/kill"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                return true;
            }
        }
    }
    false
}

pub fn cursor_row(rows: &[Row], cursor: usize) -> usize {
    let mut idx = 0;
    for (i, row) in rows.iter().enumerate() {
        if row.start <= cursor {
            idx = i;
        } else {
            break;
        }
    }
    idx
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainCursor {
    Following,
    At(usize),
}

#[derive(Debug, Clone)]
pub struct Doc {
    pub title: String,
    pub body: String,
}

pub struct App {
    pub state: SessionState,
    pub input: String,
    pub cursor: usize,
    pub history: Vec<String>,
    pub history_idx: Option<usize>,
    pub mode: Mode,
    pub focus: FocusTarget,
    pub main_selected: usize,
    pub reviewer_selected: usize,
    pub hand_selected: usize,
    pub half_focus: Half,
    pub modal: Option<Modal>,
    pub help_visible: bool,
    pub pending_g: Option<Instant>,
    pub quit: bool,
    pub keys: KeyMap,
    pub last_big_view: Rect,
    pub last_panels: [Rect; 4],
    pub outbox: VecDeque<String>,
    outbox_images: VecDeque<Vec<Attachment>>,
    pub answer_outbox: VecDeque<String>,
    pub control: VecDeque<Control>,
    pub queue: VecDeque<String>,
    queue_images: VecDeque<Vec<Attachment>>,
    pub attachments: Vec<Attachment>,
    clipboard: Box<dyn Clipboard>,
    images_dir: PathBuf,
    pub palette: Palette,
    pub doc: Option<Doc>,
    pub transient_hint: Option<String>,
    scroll_states: HashMap<PaneKey, ScrollState>,
    pane_metrics: HashMap<PaneKey, PaneMetrics>,
    row_caches: HashMap<PaneKey, RowsCache>,
    main_cursors: HashMap<usize, MainCursor>,
    main_view_offset: HashMap<usize, usize>,
    main_stop_lines: HashMap<usize, Vec<(usize, usize)>>,
    markdown_cache: HashMap<(MarkdownCacheKey, u16), Vec<Line<'static>>>,
    input_width: usize,
    input_view_top: usize,
}

impl App {
    pub fn new(keys_path: Option<&Path>) -> Self {
        App {
            state: SessionState::new(),
            input: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_idx: None,
            mode: Mode::Normal,
            focus: FocusTarget::Panel(PanelId::Main),
            main_selected: 0,
            reviewer_selected: 0,
            hand_selected: 0,
            half_focus: Half::Turns,
            modal: None,
            help_visible: false,
            pending_g: None,
            quit: false,
            keys: KeyMap::load(keys_path),
            last_big_view: Rect::default(),
            last_panels: [Rect::default(); 4],
            outbox: VecDeque::new(),
            outbox_images: VecDeque::new(),
            answer_outbox: VecDeque::new(),
            control: VecDeque::new(),
            queue: VecDeque::new(),
            queue_images: VecDeque::new(),
            attachments: Vec::new(),
            clipboard: Box::new(WlPasteClipboard),
            images_dir: PathBuf::from("threads/main.images"),
            palette: Palette::default(),
            doc: None,
            transient_hint: None,
            scroll_states: HashMap::new(),
            pane_metrics: HashMap::new(),
            row_caches: HashMap::new(),
            main_cursors: HashMap::new(),
            main_view_offset: HashMap::new(),
            main_stop_lines: HashMap::new(),
            markdown_cache: HashMap::new(),
            input_width: 0,
            input_view_top: 0,
        }
    }

    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    #[cfg(test)]
    pub fn with_clipboard(mut self, clipboard: Box<dyn Clipboard>) -> Self {
        self.clipboard = clipboard;
        self
    }

    #[cfg(test)]
    pub fn with_images_dir(mut self, dir: PathBuf) -> Self {
        self.images_dir = dir;
        self
    }

    /// The image list riding with the oldest outbox line, popped in lockstep with
    /// `outbox` by `drain_outputs` — empty for every line but a fresh paste-bearing one.
    pub fn pop_outbox_images(&mut self) -> Vec<Attachment> {
        self.outbox_images.pop_front().unwrap_or_default()
    }

    pub fn set_layout_rects(&mut self, big_view: Rect, panels: [Rect; 4]) {
        self.last_big_view = big_view;
        self.last_panels = panels;
    }

    pub fn apply_terminal(&mut self, msg: TerminalMsg) {
        match &msg {
            TerminalMsg::Confirm { text } => {
                self.modal = Some(Modal::Confirm { text: text.clone(), input: String::new(), action: ConfirmAction::Reset });
            }
            TerminalMsg::Roster { models, tabs, main_family } => {
                let picker_tabs = tabs
                    .iter()
                    .map(|t| ModelPickerTab {
                        position: t.position.clone(),
                        fallback_id: t.fallback_id.clone(),
                        model_ids: t.model_ids.clone(),
                        opened_cursor: t.cursor,
                        cursor: t.cursor,
                    })
                    .collect();
                self.modal = Some(Modal::ModelPicker {
                    models: models.clone(),
                    tabs: picker_tabs,
                    tab: 0,
                    main_family: main_family.clone(),
                });
            }
            _ => {}
        }
        let mains_before = self.state.mains.len();
        self.state.apply_terminal(&msg);
        if self.state.mains.len() > mains_before {
            self.main_selected = self.state.mains.len() - 1;
            self.focus = FocusTarget::Panel(PanelId::Main);
        }
    }

    pub fn apply_event(&mut self, ev: Event) {
        self.state.apply_event(&ev);
    }

    /// A line submitted while the harness is mid-turn waits, visibly, in `queue`; the
    /// oldest one is released as its own dispatch the next time the operator loop asks
    /// for a line (`release_queued_if_idle`, run off `Incoming::WantLine`). Otherwise it
    /// reaches the operator lane at once. The attachments still marked `[image n]` in the
    /// line go with it; the rest are deleted from disk here, at submit, per the line the
    /// operator actually sent.
    pub fn submit_line(&mut self, line: String) {
        let images = self.take_referenced_attachments(&line);
        if self.state.is_streaming() {
            self.queue.push_back(line);
            self.queue_images.push_back(images);
            return;
        }
        self.dispatch_line(line, images);
    }

    /// Called once per `PanesFront::read` — the operator loop's own lane, never the
    /// checkpoint ritual's confirm wait (that one runs on `read_answer`) and never a
    /// ritual sub-turn's `TurnEnd`. Releases the oldest queued line only when the harness
    /// is not mid-turn, so an intercepted command that produces no `TurnEnd` (`/kill`,
    /// `/model`) still lets the next queued line go on the following read.
    pub fn release_queued_if_idle(&mut self) {
        if self.state.is_streaming() {
            return;
        }
        if let Some(line) = self.queue.pop_front() {
            let images = self.queue_images.pop_front().unwrap_or_default();
            self.dispatch_line(line, images);
        }
    }

    fn take_referenced_attachments(&mut self, text: &str) -> Vec<Attachment> {
        let (referenced, orphaned): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.attachments).into_iter().partition(|a| text.contains(&format!("[image {}]", a.n)));
        for orphan in orphaned {
            let _ = std::fs::remove_file(&orphan.path);
        }
        referenced
    }

    fn dispatch_line(&mut self, line: String, images: Vec<Attachment>) {
        self.history.push(line.clone());
        self.history_idx = None;
        let idx = self.state.current_main;
        let existing_turn = self.state.mains[idx].turn_started;
        self.state.push_operator_line(&line);
        if is_intercepted_command(&line) {
            self.state.mains[idx].turn_started = existing_turn;
        } else {
            self.state.waiting_since = Some(Instant::now());
        }
        self.outbox.push_back(line);
        self.outbox_images.push_back(images);
    }

    /// Reaches the operator lane at once when the harness is idle, or waits in `queue`
    /// otherwise — the same rule `submit_line` follows, minus the history/page side effects
    /// a modal-triggered line (a kill confirm) never carried. Never touches attachments —
    /// a synthetic line like `/kill h1` carries none of its own and must not steal the
    /// operator's still-pending paste.
    fn queue_or_send(&mut self, line: String) {
        if self.state.is_streaming() {
            self.queue.push_back(line);
            self.queue_images.push_back(Vec::new());
        } else {
            self.outbox.push_back(line);
            self.outbox_images.push_back(Vec::new());
        }
    }

    fn confirm_model_pick(&mut self, id: String) {
        self.modal = None;
        self.queue_or_send(format!("/model {id}"));
    }

    /// Enter applies every tab at once: a tab whose cursor moved from where it
    /// opened writes its pin (or clears it, for the `from file` row) through
    /// the same `/model` line the operator could have typed; a tab never
    /// visited cannot change, since its cursor never left `opened_cursor`.
    fn confirm_model_picks(&mut self, tabs: Vec<ModelPickerTab>) {
        self.modal = None;
        let mut changed = false;
        for t in &tabs {
            if t.cursor == t.opened_cursor {
                continue;
            }
            changed = true;
            let line = if t.cursor == 0 {
                format!("/model clear {}", t.position)
            } else if t.position == "main" {
                format!("/model {}", t.model_ids[t.cursor - 1])
            } else {
                format!("/model {} {}", t.position, t.model_ids[t.cursor - 1])
            };
            self.queue_or_send(line);
        }
        if !changed {
            self.apply_terminal(TerminalMsg::Notice {
                text: "[arx] model: no changes".to_string(),
            });
        }
    }

    fn confirm_reset(&mut self, line: String) {
        self.modal = None;
        self.answer_outbox.push_back(line);
    }

    fn char_count(&self) -> usize {
        self.input.chars().count()
    }

    pub fn insert_char(&mut self, c: char) {
        let byte = char_byte_offset(&self.input, self.cursor);
        self.input.insert(byte, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let end = char_byte_offset(&self.input, self.cursor);
        let start = char_byte_offset(&self.input, self.cursor - 1);
        self.input.replace_range(start..end, "");
        self.cursor -= 1;
    }

    pub fn delete_forward(&mut self) {
        let len = self.char_count();
        if self.cursor >= len {
            return;
        }
        let start = char_byte_offset(&self.input, self.cursor);
        let end = char_byte_offset(&self.input, self.cursor + 1);
        self.input.replace_range(start..end, "");
    }

    pub fn cursor_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn cursor_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.char_count());
    }

    fn current_wrap_rows(&self) -> Vec<Row> {
        wrap_input(&self.input, self.input_width.max(1))
    }

    fn cursor_row_index(&self) -> usize {
        cursor_row(&self.current_wrap_rows(), self.cursor)
    }

    pub fn cursor_home(&mut self) {
        let rows = self.current_wrap_rows();
        let idx = cursor_row(&rows, self.cursor);
        self.cursor = rows[idx].start;
    }

    pub fn cursor_end(&mut self) {
        let rows = self.current_wrap_rows();
        let idx = cursor_row(&rows, self.cursor);
        self.cursor = rows[idx].end;
    }

    fn cursor_move_row(&mut self, delta: i32) {
        let rows = self.current_wrap_rows();
        let idx = cursor_row(&rows, self.cursor);
        let target = idx as i32 + delta;
        if target < 0 || target as usize >= rows.len() {
            return;
        }
        let target = target as usize;
        let col = self.cursor - rows[idx].start;
        let row_len = rows[target].end - rows[target].start;
        self.cursor = rows[target].start + col.min(row_len);
    }

    pub fn set_input_width(&mut self, width: usize) {
        self.input_width = width;
    }

    pub fn input_view_top(&self) -> usize {
        self.input_view_top
    }

    pub fn set_input_view_top(&mut self, top: usize) {
        self.input_view_top = top;
    }

    pub fn history_up(&mut self) {
        if self.input.is_empty() && !self.queue.is_empty() {
            let line = self.queue.pop_back().expect("just checked non-empty");
            self.attachments.extend(self.queue_images.pop_back().unwrap_or_default());
            self.cursor = line.chars().count();
            self.input = line;
            return;
        }
        if self.cursor_row_index() > 0 {
            self.cursor_move_row(-1);
            return;
        }
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_idx {
            Some(i) if i > 0 => i - 1,
            Some(i) => i,
            None => self.history.len() - 1,
        };
        self.history_idx = Some(idx);
        self.input = self.history[idx].clone();
        self.cursor = self.char_count();
    }

    pub fn history_down(&mut self) {
        let rows = self.current_wrap_rows();
        if self.cursor_row_index() + 1 < rows.len() {
            self.cursor_move_row(1);
            return;
        }
        match self.history_idx {
            Some(i) if i + 1 < self.history.len() => {
                self.history_idx = Some(i + 1);
                self.input = self.history[i + 1].clone();
            }
            Some(_) => {
                self.history_idx = None;
                self.input.clear();
            }
            None => return,
        }
        self.cursor = self.char_count();
    }

    pub fn tab_complete(&mut self) {
        if self.input.is_empty() || !self.input.starts_with('/') {
            return;
        }
        let matches: Vec<&str> =
            ["/reset", "/model", "/checkpoint"].iter().copied().filter(|c| c.starts_with(self.input.as_str())).collect();
        if matches.len() == 1 {
            self.input = matches[0].to_string();
            if self.input == "/model" {
                self.input.push(' ');
            }
            self.cursor = self.char_count();
        }
    }

    fn move_modal_selection(&mut self, delta: i32) {
        match &mut self.modal {
            Some(Modal::ModelPicker { tabs, tab, .. }) => {
                if let Some(current) = tabs.get_mut(*tab) {
                    let len = current.model_ids.len() as i32 + 1;
                    current.cursor = ((current.cursor as i32 + delta).rem_euclid(len)) as usize;
                }
            }
            Some(Modal::Confirm { .. }) | None => {}
        }
    }

    fn move_modal_tab(&mut self, delta: i32) {
        if let Some(Modal::ModelPicker { tabs, tab, .. }) = &mut self.modal {
            let len = tabs.len() as i32;
            if len > 0 {
                *tab = ((*tab as i32 + delta).rem_euclid(len)) as usize;
            }
        }
    }

    fn mark_killing(&mut self, panel: PanelId, id: &str) {
        let rows = match panel {
            PanelId::Hands => &mut self.state.hands,
            PanelId::Reviewers => &mut self.state.reviewers,
            _ => return,
        };
        if let Some(row) = rows.iter_mut().find(|r| r.id == id) {
            row.killing = true;
        }
    }

    fn submit_modal(&mut self) {
        match self.modal.clone() {
            Some(Modal::ModelPicker { tabs, .. }) => {
                self.confirm_model_picks(tabs);
            }
            Some(Modal::Confirm { input, action, .. }) => match action {
                ConfirmAction::Reset => {
                    let line = if input.is_empty() { "ok".to_string() } else { input };
                    self.confirm_reset(line);
                }
                ConfirmAction::Kill { panel, id } => {
                    self.modal = None;
                    self.queue_or_send(format!("/kill {id}"));
                    self.mark_killing(panel, &id);
                }
            },
            None => {}
        }
    }

    fn cancel_modal(&mut self) {
        match &self.modal {
            Some(Modal::ModelPicker { .. }) => self.modal = None,
            Some(Modal::Confirm { action: ConfirmAction::Reset, .. }) => self.confirm_reset(String::new()),
            Some(Modal::Confirm { action: ConfirmAction::Kill { .. }, .. }) => self.modal = None,
            None => {}
        }
    }

    pub fn active_panel(&self) -> PanelId {
        match self.focus {
            FocusTarget::Panel(p) | FocusTarget::BigView(p) => p,
        }
    }

    pub fn current_party(&self) -> PartyKey {
        match self.active_panel() {
            PanelId::Session => PartyKey::Session,
            PanelId::Main => PartyKey::Main(self.main_selected),
            PanelId::Reviewers => PartyKey::Reviewer(self.reviewer_selected),
            PanelId::Hands => PartyKey::Hand(self.hand_selected),
        }
    }

    fn active_pane_key(&self) -> PaneKey {
        let party = self.current_party();
        let pane = if self.doc.is_some() {
            Pane::Doc
        } else if is_split_party(party) {
            self.half_focus.to_pane()
        } else {
            Pane::Single
        };
        PaneKey(party, pane)
    }

    fn enter_split_page(&mut self) {
        if is_split_party(self.current_party()) {
            self.half_focus = Half::Turns;
        }
    }

    pub fn ensure_trace_loaded(&mut self) {
        let party = self.current_party();
        let row = match party {
            PartyKey::Hand(i) => self.state.hands.get_mut(i),
            PartyKey::Reviewer(i) => self.state.reviewers.get_mut(i),
            _ => None,
        };
        let Some(row) = row else {
            return;
        };
        if row.trace_turns.is_some() || row.outcome != Some(Outcome::Verdict) {
            return;
        }
        let Some(handle) = row.trace.clone() else {
            return;
        };
        row.trace_turns = Some(trace::read_trace(Path::new(&handle), &row.body));
    }

    fn main_stops(&self, idx: usize) -> Vec<Stop> {
        self.state.mains.get(idx).map(|m| state::stops(&m.page)).unwrap_or_default()
    }

    pub fn main_cursor_stop(&self, idx: usize) -> usize {
        let stops = self.main_stops(idx);
        if stops.is_empty() {
            return 0;
        }
        match self.main_cursors.get(&idx) {
            Some(MainCursor::At(i)) => (*i).min(stops.len() - 1),
            _ => stops.len() - 1,
        }
    }

    fn set_main_cursor(&mut self, idx: usize, resolved: usize, last: usize) {
        let state = if resolved >= last { MainCursor::Following } else { MainCursor::At(resolved) };
        self.main_cursors.insert(idx, state);
    }

    pub fn record_main_stop_lines(&mut self, idx: usize, ranges: Vec<(usize, usize)>) {
        self.main_stop_lines.insert(idx, ranges);
    }

    pub fn main_view_offset(&self, idx: usize) -> usize {
        self.main_view_offset.get(&idx).copied().unwrap_or(0)
    }

    pub fn set_main_view_offset(&mut self, idx: usize, offset: usize) {
        self.main_view_offset.insert(idx, offset);
    }

    fn main_nav_down(&mut self) {
        let idx = self.main_selected;
        let stops = self.main_stops(idx);
        if stops.is_empty() {
            return;
        }
        let last = stops.len() - 1;
        let next = (self.main_cursor_stop(idx) + 1).min(last);
        self.set_main_cursor(idx, next, last);
    }

    fn main_nav_up(&mut self) {
        let idx = self.main_selected;
        let stops = self.main_stops(idx);
        if stops.is_empty() {
            return;
        }
        let last = stops.len() - 1;
        let next = self.main_cursor_stop(idx).saturating_sub(1);
        self.set_main_cursor(idx, next, last);
    }

    fn main_nav_top(&mut self) {
        let now = Instant::now();
        let fire = matches!(self.pending_g, Some(t) if now.duration_since(t) < Duration::from_millis(600));
        if fire {
            let idx = self.main_selected;
            let stops = self.main_stops(idx);
            if !stops.is_empty() {
                self.set_main_cursor(idx, 0, stops.len() - 1);
            }
            self.pending_g = None;
        } else {
            self.pending_g = Some(now);
        }
    }

    fn main_nav_bottom(&mut self) {
        let idx = self.main_selected;
        let stops = self.main_stops(idx);
        if stops.is_empty() {
            return;
        }
        let last = stops.len() - 1;
        self.set_main_cursor(idx, last, last);
    }

    fn main_nav_page(&mut self, dir: i32) {
        let idx = self.main_selected;
        let key = PaneKey(PartyKey::Main(idx), Pane::Single);
        let metrics = self.pane_metrics.get(&key).copied().unwrap_or_default();
        let visible = metrics.visible.max(1);
        let bottom = metrics.total_rows.saturating_sub(visible);
        let half = (visible / 2).max(1);
        let offset = self.main_view_offset(idx);
        let new_offset = if dir > 0 { (offset + half).min(bottom) } else { offset.saturating_sub(half) };
        self.set_main_view_offset(idx, new_offset);

        let ranges = self.main_stop_lines.get(&idx).cloned().unwrap_or_default();
        if ranges.is_empty() {
            return;
        }
        let last = ranges.len() - 1;
        let view_bottom = new_offset + visible;
        let target = if dir > 0 {
            ranges.iter().position(|(start, end)| *start >= new_offset && *end <= view_bottom).unwrap_or(last)
        } else {
            ranges.iter().rposition(|(start, end)| *start >= new_offset && *end <= view_bottom).unwrap_or(0)
        };
        self.set_main_cursor(idx, target, last);
    }

    fn is_main_big_view(&self) -> bool {
        matches!(self.focus, FocusTarget::BigView(PanelId::Main)) && self.doc.is_none()
    }

    fn open_doc_for_handle(&mut self, handle: String) {
        self.invalidate_doc_markdown_cache();
        match std::fs::read_to_string(&handle) {
            Ok(body) => self.doc = Some(Doc { title: handle, body }),
            Err(err) => {
                self.state.push_notice_on_current_main(&format!("trace unreadable: {handle}: {err}"));
            }
        }
    }

    fn try_main_stop_action(&mut self) -> bool {
        if !self.is_main_big_view() {
            return false;
        }
        let idx = self.main_selected;
        let cursor = self.main_cursor_stop(idx);
        let stop = match self.main_stops(idx).get(cursor).copied() {
            Some(stop) => stop,
            None => return false,
        };
        let Some(turn) = self.state.mains.get_mut(idx).and_then(|m| m.page.get_mut(stop.turn)) else {
            return false;
        };
        match turn {
            state::Turn::Tools { expanded, .. } if stop.call.is_none() => {
                *expanded = !*expanded;
                let new_stops = self.main_stops(idx);
                if !new_stops.is_empty() {
                    self.set_main_cursor(idx, cursor, new_stops.len() - 1);
                }
                true
            }
            state::Turn::Tools { calls, .. } => {
                let Some(call_idx) = stop.call else { return false };
                let Some(handle) = calls.get(call_idx).and_then(|c| c.handle.clone()) else {
                    return false;
                };
                self.open_doc_for_handle(handle);
                true
            }
            _ => false,
        }
    }

    pub fn focus_panel(&mut self, panel: PanelId) {
        self.focus = FocusTarget::Panel(panel);
    }

    pub fn cycle_panel(&mut self, delta: i32) {
        if let FocusTarget::Panel(p) = self.focus {
            let idx = PANEL_ORDER.iter().position(|x| *x == p).unwrap_or(0) as i32;
            let len = PANEL_ORDER.len() as i32;
            let next = (idx + delta).rem_euclid(len) as usize;
            self.focus_panel(PANEL_ORDER[next]);
        }
    }

    pub fn open_big_view(&mut self) {
        self.focus = FocusTarget::BigView(self.active_panel());
        self.enter_split_page();
    }

    pub fn back_to_panel(&mut self) {
        if self.doc.is_some() {
            self.doc = None;
            self.invalidate_doc_markdown_cache();
            return;
        }
        if let FocusTarget::BigView(p) = self.focus {
            self.focus = FocusTarget::Panel(p);
        }
    }

    fn panel_len(&self, panel: PanelId) -> usize {
        match panel {
            PanelId::Session => 0,
            PanelId::Main => self.state.mains.len(),
            PanelId::Reviewers => self.state.reviewers.len(),
            PanelId::Hands => self.state.hands.len(),
        }
    }

    fn selection_mut(&mut self, panel: PanelId) -> Option<&mut usize> {
        match panel {
            PanelId::Session => None,
            PanelId::Main => Some(&mut self.main_selected),
            PanelId::Reviewers => Some(&mut self.reviewer_selected),
            PanelId::Hands => Some(&mut self.hand_selected),
        }
    }

    fn purge_party_ui_state(&mut self, panel: PanelId) {
        let in_panel = |key: PartyKey| {
            matches!(
                (panel, key),
                (PanelId::Hands, PartyKey::Hand(_)) | (PanelId::Reviewers, PartyKey::Reviewer(_)) | (PanelId::Main, PartyKey::Main(_))
            )
        };
        self.scroll_states.retain(|k, _| !in_panel(k.0));
        self.pane_metrics.retain(|k, _| !in_panel(k.0));
        self.row_caches.retain(|k, _| !in_panel(k.0));
        if panel == PanelId::Main {
            self.main_cursors.clear();
            self.main_view_offset.clear();
            self.main_stop_lines.clear();
            self.markdown_cache.retain(|(k, _), _| !matches!(k, MarkdownCacheKey::MainTurn { .. }));
        }
    }

    fn after_row_removed(&mut self, panel: PanelId) {
        self.purge_party_ui_state(panel);
        let len = self.panel_len(panel);
        if let Some(selected) = self.selection_mut(panel) {
            *selected = if len == 0 { 0 } else { (*selected).min(len - 1) };
        }
        if len == 0 && self.active_panel() == panel {
            self.main_selected = self.state.current_main;
            self.focus = match self.focus {
                FocusTarget::BigView(_) => FocusTarget::BigView(PanelId::Main),
                FocusTarget::Panel(_) => FocusTarget::Panel(PanelId::Main),
            };
        }
    }

    fn open_kill_confirm(&mut self, panel: PanelId, id: String) {
        self.modal = Some(Modal::Confirm {
            text: format!("kill {id}?"),
            input: String::new(),
            action: ConfirmAction::Kill { panel, id },
        });
    }

    fn close_party_row(&mut self, panel: PanelId) {
        let settled_now = |row: &state::HandRow| !row.killing && matches!(row.outcome, Some(Outcome::Verdict) | Some(Outcome::Killed));
        let (id, settled) = match panel {
            PanelId::Hands => match self.state.hands.get(self.hand_selected) {
                Some(row) => (row.id.clone(), settled_now(row)),
                None => return,
            },
            PanelId::Reviewers => match self.state.reviewers.get(self.reviewer_selected) {
                Some(row) => (row.id.clone(), settled_now(row)),
                None => return,
            },
            _ => return,
        };
        if settled {
            match panel {
                PanelId::Hands => self.state.remove_hand(&id),
                PanelId::Reviewers => self.state.remove_reviewer(&id),
                _ => {}
            }
            self.after_row_removed(panel);
        } else {
            self.open_kill_confirm(panel, id);
        }
    }

    /// `q`: stop main mid-turn, or kill the hand/reviewer selected in its panel or open in
    /// the big view, or `nothing running` when neither applies.
    fn stop_action(&mut self) {
        let idx = self.state.current_main;
        if self.state.mains.get(idx).is_some_and(|m| m.turn_started.is_some()) {
            self.control.push_back(Control::Stop);
            return;
        }
        let not_settled = |row: &state::HandRow| row.is_running() || row.outcome == Some(Outcome::Stop);
        match self.current_party() {
            PartyKey::Hand(i) => {
                if let Some(row) = self.state.hands.get(i) {
                    if not_settled(row) {
                        let id = row.id.clone();
                        self.open_kill_confirm(PanelId::Hands, id);
                        return;
                    }
                }
            }
            PartyKey::Reviewer(i) => {
                if let Some(row) = self.state.reviewers.get(i) {
                    if not_settled(row) {
                        let id = row.id.clone();
                        self.open_kill_confirm(PanelId::Reviewers, id);
                        return;
                    }
                }
            }
            _ => {}
        }
        self.transient_hint = Some("nothing running".to_string());
    }

    fn close_main_row(&mut self) {
        let idx = self.main_selected;
        match self.state.mains.get(idx) {
            Some(main) if main.ended.is_some() => {
                self.state.remove_main(idx);
                self.after_row_removed(PanelId::Main);
            }
            Some(_) => {
                self.transient_hint = Some("live main: /reset ends it".to_string());
            }
            None => {}
        }
    }

    fn close_selected_row(&mut self) {
        let panel = match self.focus {
            FocusTarget::Panel(p) => p,
            FocusTarget::BigView(_) => return,
        };
        match panel {
            PanelId::Main => self.close_main_row(),
            PanelId::Hands | PanelId::Reviewers => self.close_party_row(panel),
            PanelId::Session => {}
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let panel = self.active_panel();
        let len = self.panel_len(panel);
        if len == 0 {
            return;
        }
        let delta = if panel == PanelId::Main { -delta } else { delta };
        if let Some(selected) = self.selection_mut(panel) {
            let next = (*selected as i32 + delta).clamp(0, len as i32 - 1) as usize;
            *selected = next;
        }
    }

    fn jump_selection(&mut self, to_end: bool) {
        let panel = self.active_panel();
        let len = self.panel_len(panel);
        if len == 0 {
            return;
        }
        let to_end = if panel == PanelId::Main { !to_end } else { to_end };
        let target = if to_end { len - 1 } else { 0 };
        if let Some(selected) = self.selection_mut(panel) {
            *selected = target;
        }
    }

    fn is_live_main_pane(&self, key: PaneKey) -> bool {
        matches!(key.1, Pane::Single)
            && matches!(key.0, PartyKey::Main(i) if self.state.mains.get(i).map(|m| m.ended.is_none()).unwrap_or(false))
    }

    pub fn scroll_state_for(&mut self, key: PaneKey, total_rows_now: usize) -> ScrollState {
        if let Some(state) = self.scroll_states.get(&key) {
            return *state;
        }
        let default = if self.is_live_main_pane(key) {
            ScrollState::Following
        } else {
            ScrollState::Anchored { row: 0, rows_at_anchor: total_rows_now }
        };
        self.scroll_states.insert(key, default);
        default
    }

    pub fn record_pane_metrics(&mut self, key: PaneKey, total_rows: usize, visible: usize) {
        self.pane_metrics.insert(key, PaneMetrics { total_rows, visible });
    }

    /// Renders once per (key, width) and returns the cached lines on every later call at
    /// that same width — a resize changes the key and misses on purpose. Main-turn text is
    /// immutable once a turn is pushed, so this never goes stale for it; the doc view calls
    /// `invalidate_doc_markdown_cache` itself whenever the open document changes identity.
    pub fn cached_markdown(&mut self, key: MarkdownCacheKey, width: u16, compute: impl FnOnce() -> Vec<Line<'static>>) -> Vec<Line<'static>> {
        let full_key = (key, width);
        if let Some(lines) = self.markdown_cache.get(&full_key) {
            return lines.clone();
        }
        let lines = compute();
        // Only the current width is ever read back for a given key; drop any other width
        // left over from an earlier resize instead of letting entries accumulate per width
        // for the life of the session.
        self.markdown_cache.retain(|(k, _), _| *k != key);
        self.markdown_cache.insert(full_key, lines.clone());
        lines
    }

    pub fn invalidate_doc_markdown_cache(&mut self) {
        self.markdown_cache.retain(|(key, _), _| *key != MarkdownCacheKey::Doc);
    }

    pub fn cached_rows(&mut self, key: PaneKey, width: u16, source_len: usize, compute: impl FnOnce() -> usize) -> usize {
        let cache = self.row_caches.entry(key).or_default();
        let reuse = cache.initialized && cache.width == width && cache.source_len == source_len;
        if !reuse {
            let rows = compute();
            let cache = self.row_caches.get_mut(&key).unwrap();
            *cache = RowsCache { width, source_len, rows, initialized: true };
        }
        self.row_caches[&key].rows
    }

    fn pane_scroll_up(&mut self, amount: usize) {
        let key = self.active_pane_key();
        let metrics = self.pane_metrics.get(&key).copied().unwrap_or_default();
        let state = self.scroll_state_for(key, metrics.total_rows);
        let next = match state {
            ScrollState::Following => {
                let bottom = metrics.total_rows.saturating_sub(metrics.visible);
                ScrollState::Anchored { row: bottom.saturating_sub(amount), rows_at_anchor: metrics.total_rows }
            }
            ScrollState::Anchored { row, rows_at_anchor } => {
                ScrollState::Anchored { row: row.saturating_sub(amount), rows_at_anchor }
            }
        };
        self.scroll_states.insert(key, next);
    }

    fn pane_scroll_down(&mut self, amount: usize) {
        let key = self.active_pane_key();
        let metrics = self.pane_metrics.get(&key).copied().unwrap_or_default();
        let state = self.scroll_state_for(key, metrics.total_rows);
        let bottom = metrics.total_rows.saturating_sub(metrics.visible);
        let next = match state {
            ScrollState::Following => ScrollState::Following,
            ScrollState::Anchored { row, rows_at_anchor } => {
                let row = (row + amount).min(bottom);
                if row >= bottom {
                    ScrollState::Following
                } else {
                    ScrollState::Anchored { row, rows_at_anchor }
                }
            }
        };
        self.scroll_states.insert(key, next);
    }

    fn pane_scroll_top(&mut self) {
        let key = self.active_pane_key();
        let metrics = self.pane_metrics.get(&key).copied().unwrap_or_default();
        self.scroll_states.insert(key, ScrollState::Anchored { row: 0, rows_at_anchor: metrics.total_rows });
    }

    fn pane_scroll_bottom(&mut self) {
        let key = self.active_pane_key();
        self.scroll_states.insert(key, ScrollState::Following);
    }

    pub fn nav_down(&mut self) {
        match self.focus {
            FocusTarget::Panel(_) => self.move_selection(1),
            FocusTarget::BigView(_) if self.is_main_big_view() => self.main_nav_down(),
            FocusTarget::BigView(_) => self.pane_scroll_down(1),
        }
    }

    pub fn nav_up(&mut self) {
        match self.focus {
            FocusTarget::Panel(_) => self.move_selection(-1),
            FocusTarget::BigView(_) if self.is_main_big_view() => self.main_nav_up(),
            FocusTarget::BigView(_) => self.pane_scroll_up(1),
        }
    }

    pub fn nav_top(&mut self) {
        match self.focus {
            FocusTarget::Panel(_) => self.jump_selection(false),
            FocusTarget::BigView(_) if self.is_main_big_view() => self.main_nav_top(),
            FocusTarget::BigView(_) => {
                let now = Instant::now();
                let fire = matches!(self.pending_g, Some(t) if now.duration_since(t) < Duration::from_millis(600));
                if fire {
                    self.pane_scroll_top();
                    self.pending_g = None;
                } else {
                    self.pending_g = Some(now);
                }
            }
        }
    }

    pub fn nav_bottom(&mut self) {
        match self.focus {
            FocusTarget::Panel(_) => self.jump_selection(true),
            FocusTarget::BigView(_) if self.is_main_big_view() => self.main_nav_bottom(),
            FocusTarget::BigView(_) => self.pane_scroll_bottom(),
        }
    }

    pub fn nav_page(&mut self, dir: i32) {
        if self.is_main_big_view() {
            self.main_nav_page(dir);
        } else if let FocusTarget::BigView(_) = self.focus {
            if dir > 0 {
                self.pane_scroll_down(10);
            } else {
                self.pane_scroll_up(10);
            }
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.transient_hint = None;
        if modifiers.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c') | KeyCode::Char('C')) {
            self.quit = true;
            return;
        }
        let token = match keys::key_token(code, modifiers) {
            Some(t) => t,
            None => return,
        };

        if self.help_visible {
            self.help_visible = false;
            return;
        }

        if self.modal.is_some() {
            self.handle_modal_key(&token, code);
            return;
        }

        match self.mode {
            Mode::Insert => self.handle_insert_key(&token, code),
            Mode::Normal => self.handle_normal_key(&token),
        }
    }

    fn handle_normal_key(&mut self, token: &str) {
        if matches!(self.focus, FocusTarget::BigView(_))
            && is_split_party(self.current_party())
            && self.keys.split.matches("swap_pane", token)
        {
            self.half_focus = self.half_focus.swapped();
            return;
        }
        if self.keys.normal.matches("quit", token) {
            self.quit = true;
            return;
        }
        if self.keys.normal.matches("help", token) {
            self.help_visible = true;
            return;
        }
        if self.keys.normal.matches("panel_1", token) {
            self.focus_panel(PanelId::Session);
            return;
        }
        if self.keys.normal.matches("panel_2", token) {
            self.focus_panel(PanelId::Main);
            return;
        }
        if self.keys.normal.matches("panel_3", token) {
            self.focus_panel(PanelId::Reviewers);
            return;
        }
        if self.keys.normal.matches("panel_4", token) {
            self.focus_panel(PanelId::Hands);
            return;
        }
        if self.keys.normal.matches("back", token) {
            self.back_to_panel();
            return;
        }
        if self.keys.normal.matches("close", token) {
            self.close_selected_row();
            return;
        }
        if self.keys.normal.matches("stop", token) {
            self.stop_action();
            return;
        }
        if self.keys.normal.matches("expand", token) && self.try_main_stop_action() {
            return;
        }
        let is_open = self.keys.normal.matches("open", token);
        let is_insert = self.keys.normal.matches("insert", token);
        if is_open || is_insert {
            match self.focus {
                FocusTarget::Panel(_) => {
                    if is_open {
                        self.open_big_view();
                    } else if is_insert {
                        self.mode = Mode::Insert;
                    }
                }
                FocusTarget::BigView(_) => {
                    if is_insert {
                        self.mode = Mode::Insert;
                    }
                }
            }
            return;
        }
        if self.keys.normal.matches("next_panel", token) {
            self.cycle_panel(1);
            return;
        }
        if self.keys.normal.matches("prev_panel", token) {
            self.cycle_panel(-1);
            return;
        }
        if self.keys.normal.matches("down", token) {
            self.nav_down();
            return;
        }
        if self.keys.normal.matches("up", token) {
            self.nav_up();
            return;
        }
        if self.keys.normal.matches("top", token) {
            self.nav_top();
            return;
        }
        if self.keys.normal.matches("bottom", token) {
            self.nav_bottom();
            return;
        }
        if self.keys.normal.matches("page_down", token) {
            self.nav_page(1);
            return;
        }
        if self.keys.normal.matches("page_up", token) {
            self.nav_page(-1);
        }
    }

    fn handle_insert_key(&mut self, token: &str, code: KeyCode) {
        if self.keys.insert.matches("normal", token) {
            self.mode = Mode::Normal;
            self.focus = FocusTarget::BigView(PanelId::Main);
            return;
        }
        if self.keys.insert.matches("submit", token) {
            let line = self.input.clone();
            if !line.trim().is_empty() {
                self.input.clear();
                self.cursor = 0;
                self.submit_line(line);
            }
            return;
        }
        if self.keys.insert.matches("newline", token) {
            self.insert_char('\n');
            return;
        }
        if self.keys.insert.matches("left", token) {
            self.cursor_left();
            return;
        }
        if self.keys.insert.matches("right", token) {
            self.cursor_right();
            return;
        }
        if self.keys.insert.matches("home", token) {
            self.cursor_home();
            return;
        }
        if self.keys.insert.matches("end", token) {
            self.cursor_end();
            return;
        }
        if self.keys.insert.matches("delete", token) {
            self.delete_forward();
            return;
        }
        if self.keys.insert.matches("history_up", token) {
            self.history_up();
            return;
        }
        if self.keys.insert.matches("history_down", token) {
            self.history_down();
            return;
        }
        if self.keys.insert.matches("complete", token) {
            self.tab_complete();
            return;
        }
        if self.keys.insert.matches("paste", token) {
            self.paste_action();
            return;
        }
        match code {
            KeyCode::Backspace => self.backspace(),
            KeyCode::Char(c) => self.insert_char(c),
            _ => {}
        }
    }

    /// Bracketed paste: normalized and inserted at the cursor, nothing submitted. A
    /// modal (the help overlay included) drops it; normal mode enters insert first.
    pub fn handle_paste(&mut self, text: &str) {
        if self.modal.is_some() || self.help_visible {
            return;
        }
        if self.mode == Mode::Normal {
            self.mode = Mode::Insert;
        }
        self.insert_text(&normalize_pasted(text));
    }

    fn insert_text(&mut self, text: &str) {
        let byte = char_byte_offset(&self.input, self.cursor);
        self.input.insert_str(byte, text);
        self.cursor += text.chars().count();
    }

    /// `Ctrl-V`: an image on the clipboard is saved under `threads/main.images/` and
    /// marked `[image n]` at the cursor; text is inserted like a paste; an empty or
    /// failing clipboard sets the status line instead of touching the input.
    fn paste_action(&mut self) {
        let clip = self.clipboard.read();
        match clip {
            Ok(Clip::Text(text)) => self.insert_text(&normalize_pasted(&text)),
            Ok(Clip::Image { media_type, bytes }) => self.insert_pasted_image(media_type, bytes),
            Err(err) => self.transient_hint = Some(format!("clipboard: {err}")),
        }
    }

    fn insert_pasted_image(&mut self, media_type: String, bytes: Vec<u8>) {
        const MAX_BYTES: usize = 5 * 1024 * 1024;
        if bytes.len() > MAX_BYTES {
            self.transient_hint = Some("clipboard: image over 5 MB".to_string());
            return;
        }
        let dir = self.images_dir.clone();
        if let Err(err) = std::fs::create_dir_all(&dir) {
            self.transient_hint = Some(format!("clipboard: {err}"));
            return;
        }
        let n = next_image_number(&dir);
        let ext = crate::panes::clipboard::image_extension(&media_type);
        let path = dir.join(format!("{n}.{ext}"));
        if let Err(err) = std::fs::write(&path, &bytes) {
            self.transient_hint = Some(format!("clipboard: {err}"));
            return;
        }
        self.insert_text(&format!("[image {n}]"));
        self.attachments.push(Attachment { n, path, media_type, bytes: bytes.len() as u64 });
    }

    fn handle_modal_key(&mut self, token: &str, code: KeyCode) {
        if matches!(self.modal, Some(Modal::Confirm { .. })) {
            self.handle_confirm_modal_key(token, code);
            return;
        }
        if self.keys.modal.matches("down", token) {
            self.move_modal_selection(1);
            return;
        }
        if self.keys.modal.matches("up", token) {
            self.move_modal_selection(-1);
            return;
        }
        if self.keys.modal.matches("confirm", token) {
            self.submit_modal();
            return;
        }
        if self.keys.modal.matches("cancel", token) {
            self.cancel_modal();
            return;
        }
        if matches!(self.modal, Some(Modal::ModelPicker { .. })) {
            if self.keys.modal.matches("prev_tab", token) {
                self.move_modal_tab(-1);
                return;
            }
            if self.keys.modal.matches("next_tab", token) {
                self.move_modal_tab(1);
                return;
            }
        }
        match &mut self.modal {
            Some(Modal::ModelPicker { tabs, tab, .. }) => {
                if let KeyCode::Char(c) = code {
                    if let Some(n) = c.to_digit(10) {
                        let n = n as usize;
                        if let Some(current) = tabs.get_mut(*tab) {
                            if n >= 1 && n <= current.model_ids.len() {
                                current.cursor = n;
                            }
                        }
                    }
                }
            }
            Some(Modal::Confirm { .. }) | None => {}
        }
    }

    fn handle_confirm_modal_key(&mut self, token: &str, code: KeyCode) {
        if self.keys.modal.matches("confirm", token) {
            self.submit_modal();
            return;
        }
        if self.keys.modal.matches("cancel", token) {
            self.cancel_modal();
            return;
        }
        if let Some(Modal::Confirm { input, action: ConfirmAction::Reset, .. }) = &mut self.modal {
            match code {
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) => input.push(c),
                _ => {}
            }
        }
    }

    pub fn handle_mouse_scroll(&mut self, panel_under_pointer: Option<PanelId>, big_view_under_pointer: bool, amount: i32) {
        if big_view_under_pointer {
            if amount > 0 {
                self.pane_scroll_down(amount as usize);
            } else {
                self.pane_scroll_up((-amount) as usize);
            }
            return;
        }
        if let Some(panel) = panel_under_pointer {
            let was = self.focus;
            self.focus = FocusTarget::Panel(panel);
            self.move_selection(amount);
            if !matches!(was, FocusTarget::Panel(p) if p == panel) {
                self.focus = was;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panes::clipboard;

    #[test]
    fn a_pane_s_scroll_state_survives_leaving_the_party_and_returning() {
        // main's own big view now carries a stop cursor, not a ScrollState (scope 3);
        // a split party's turns pane is where ScrollState persistence still lives.
        let mut app = App::new(None);
        app.state.apply_event(&Event::Spawn { hand: "h1".to_string(), agent: "hand-do".to_string(), role: "hand".to_string() });
        app.focus_panel(PanelId::Hands);
        app.open_big_view();
        let key = PaneKey(PartyKey::Hand(0), Pane::Turns);
        app.record_pane_metrics(key, 100, 10);
        app.nav_up();
        let anchored = app.scroll_state_for(key, 100);
        assert!(matches!(anchored, ScrollState::Anchored { .. }), "state: {anchored:?}");

        app.back_to_panel();
        app.focus_panel(PanelId::Main);
        app.focus_panel(PanelId::Hands);
        app.open_big_view();

        let restored = app.scroll_state_for(key, 100);
        assert_eq!(restored, anchored);
    }

    #[test]
    fn tab_swaps_pane_focus_on_a_split_page_and_next_panel_still_cycles_panels_elsewhere() {
        let mut app = App::new(None);
        app.state.apply_event(&Event::Spawn { hand: "h1".to_string(), agent: "hand-do".to_string(), role: "hand".to_string() });
        app.focus_panel(PanelId::Hands);
        app.open_big_view();
        assert_eq!(app.half_focus, Half::Turns);

        app.handle_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.half_focus, Half::Brief);

        app.back_to_panel();
        app.focus_panel(PanelId::Session);
        app.handle_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.active_panel(), PanelId::Main);
    }

    fn folded_tools(calls: &[&str]) -> state::Turn {
        state::Turn::Tools {
            calls: calls
                .iter()
                .map(|tool| state::ToolCall { tool: tool.to_string(), subject: None, outcome: None, bytes: None, handle: None })
                .collect(),
            expanded: false,
        }
    }

    #[test]
    fn j_and_k_move_the_cursor_over_stops_and_enter_expands_a_fold_onto_its_first_call() {
        let mut app = App::new(None);
        app.state.mains[0].page =
            vec![state::Turn::Operator("hi".to_string()), state::Turn::Main("hello".to_string()), folded_tools(&["read_file", "list_files", "save"])];
        app.focus = FocusTarget::BigView(PanelId::Main);

        assert_eq!(app.main_cursor_stop(0), 2, "a fresh page follows the last stop");

        app.nav_up();
        assert_eq!(app.main_cursor_stop(0), 1);
        app.nav_up();
        assert_eq!(app.main_cursor_stop(0), 0);
        app.nav_up();
        assert_eq!(app.main_cursor_stop(0), 0, "k from the first stop stays");

        app.nav_down();
        app.nav_down();
        assert_eq!(app.main_cursor_stop(0), 2);

        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(matches!(&app.state.mains[0].page[2], state::Turn::Tools { expanded: true, .. }));
        assert_eq!(app.main_stops(0).len(), 5, "2 turns before the fold plus 3 calls");
        assert_eq!(app.main_cursor_stop(0), 2, "cursor lands on the first call");
    }

    #[test]
    fn g_arms_following_so_a_new_turn_arriving_moves_the_cursor_to_it() {
        let mut app = App::new(None);
        app.state.mains[0].page = vec![state::Turn::Operator("hi".to_string()), state::Turn::Main("hello".to_string())];
        app.focus = FocusTarget::BigView(PanelId::Main);

        app.nav_up();
        assert_eq!(app.main_cursor_stop(0), 0);
        app.nav_bottom();
        assert_eq!(app.main_cursor_stop(0), 1);

        app.state.mains[0].page.push(state::Turn::Notice("· dispatched h1".to_string()));
        assert_eq!(app.main_cursor_stop(0), 2, "a cursor following the tail tracks new turns");
    }

    #[test]
    fn esc_from_insert_focuses_the_big_view_on_main() {
        let mut app = App::new(None);
        app.focus_panel(PanelId::Hands);
        app.mode = Mode::Insert;

        app.handle_key(KeyCode::Esc, KeyModifiers::NONE);

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.focus, FocusTarget::BigView(PanelId::Main));
    }

    #[test]
    fn cached_markdown_skips_recompute_at_the_same_key_and_width_but_not_after_a_resize() {
        let mut app = App::new(None);
        let calls = std::cell::Cell::new(0);
        let compute = |calls: &std::cell::Cell<u32>| {
            calls.set(calls.get() + 1);
            vec![Line::from("rendered")]
        };

        let key = MarkdownCacheKey::Doc;
        let first = app.cached_markdown(key, 60, || compute(&calls));
        let second = app.cached_markdown(key, 60, || compute(&calls));
        assert_eq!(calls.get(), 1, "a hit at the same (key, width) must not recompute");
        assert_eq!(first.len(), second.len());

        app.cached_markdown(key, 80, || compute(&calls));
        assert_eq!(calls.get(), 2, "a resize changes the key on purpose, so this is a miss");

        app.invalidate_doc_markdown_cache();
        app.cached_markdown(key, 60, || compute(&calls));
        assert_eq!(calls.get(), 3, "invalidation forces a recompute even at a previously cached width");
    }

    #[test]
    fn resizing_through_several_widths_never_retains_more_than_one_entry_per_key() {
        let mut app = App::new(None);
        let key = MarkdownCacheKey::MainTurn { main: 0, turn: 0 };
        for width in [60u16, 61, 62, 80, 45, 110] {
            app.cached_markdown(key, width, || vec![Line::from("x")]);
        }
        let entries_for_key = app.markdown_cache.keys().filter(|(k, _)| *k == key).count();
        assert_eq!(entries_for_key, 1, "only the current width should survive repeated resizes, not one per width ever seen");
    }

    fn row_ranges(rows: &[Row]) -> Vec<(usize, usize)> {
        rows.iter().map(|r| (r.start, r.end)).collect()
    }

    #[test]
    fn wrap_input_on_an_empty_string_is_one_empty_row() {
        assert_eq!(row_ranges(&wrap_input("", 10)), vec![(0, 0)]);
    }

    #[test]
    fn wrap_input_shorter_than_the_width_is_one_row() {
        assert_eq!(row_ranges(&wrap_input("abc", 10)), vec![(0, 3)]);
    }

    #[test]
    fn wrap_input_wraps_a_three_word_string_once_at_a_space() {
        let rows = wrap_input("aaa bbb ccc", 7);
        assert_eq!(row_ranges(&rows), vec![(0, 3), (4, 11)], "rows: {rows:?}");
    }

    #[test]
    fn wrap_input_hard_breaks_a_word_longer_than_the_width() {
        let word = "x".repeat(40);
        let rows = wrap_input(&word, 10);
        assert_eq!(row_ranges(&rows), vec![(0, 10), (10, 20), (20, 30), (30, 40)], "rows: {rows:?}");
    }

    #[test]
    fn wrap_input_treats_a_newline_as_an_undrawn_break() {
        assert_eq!(row_ranges(&wrap_input("a\nb", 10)), vec![(0, 1), (2, 3)]);
    }

    #[test]
    fn typing_left_twice_and_inserting_a_char_lands_it_mid_word() {
        let mut app = App::new(None);
        app.insert_char('a');
        app.insert_char('b');
        app.insert_char('c');
        assert_eq!(app.input, "abc");
        assert_eq!(app.cursor, 3);

        app.cursor_left();
        app.cursor_left();
        assert_eq!(app.cursor, 1);

        app.insert_char('X');
        assert_eq!(app.input, "aXbc");
        assert_eq!(app.cursor, 2);
    }

    #[test]
    fn home_and_end_move_to_the_bounds_of_the_cursor_s_own_wrapped_row() {
        let mut app = App::new(None);
        app.set_input_width(7);
        app.input = "aaa bbb ccc".to_string();
        app.cursor = 8;

        app.cursor_home();
        assert_eq!(app.cursor, 4, "start of the second row");

        app.cursor_end();
        assert_eq!(app.cursor, 11, "end of the second row");
    }

    #[test]
    fn up_on_a_wrapped_row_moves_the_cursor_up_a_row_instead_of_recalling_history() {
        let mut app = App::new(None);
        app.history.push("earlier line".to_string());
        app.set_input_width(7);
        app.input = "aaa bbb ccc".to_string();
        app.cursor = 8;

        app.history_up();
        assert_eq!(app.cursor, 3, "clipped to the shorter first row's length");
        assert_eq!(app.input, "aaa bbb ccc", "the row move must not touch history");
    }

    #[test]
    fn down_crosses_consecutive_hard_wrap_seams_from_column_0_to_column_0() {
        let mut app = App::new(None);
        app.set_input_width(3);
        app.input = "abcdefghi".to_string();
        app.cursor = 0;

        assert_eq!(cursor_row(&wrap_input(&app.input, 3), app.cursor), 0);

        app.history_down();
        assert_eq!(app.cursor, 3, "Down from column 0 lands at column 0 of the next hard-wrapped row");
        assert_eq!(cursor_row(&wrap_input(&app.input, 3), app.cursor), 1, "the cursor must now be owned by the second row");

        app.history_down();
        assert_eq!(app.cursor, 6, "a second Down crosses the next seam the same way");
        assert_eq!(cursor_row(&wrap_input(&app.input, 3), app.cursor), 2, "and now the third row owns it");
    }

    #[test]
    fn up_on_the_first_row_recalls_history() {
        let mut app = App::new(None);
        app.history.push("earlier line".to_string());
        app.set_input_width(40);
        app.input = "hi".to_string();
        app.cursor = 1;

        app.history_up();
        assert_eq!(app.input, "earlier line");
        assert_eq!(app.cursor, "earlier line".chars().count());
    }

    #[test]
    fn backspace_at_the_start_and_delete_at_the_end_are_no_ops() {
        let mut app = App::new(None);
        app.backspace();
        assert_eq!(app.input, "");
        assert_eq!(app.cursor, 0);

        app.input = "abc".to_string();
        app.cursor = 3;
        app.delete_forward();
        assert_eq!(app.input, "abc");
        assert_eq!(app.cursor, 3);
    }

    fn spawn_hand_row(app: &mut App, id: &str) {
        app.apply_event(Event::Spawn { hand: id.to_string(), agent: "hand-do".to_string(), role: "hand".to_string() });
    }

    fn deliver_hand(app: &mut App, id: &str) {
        app.apply_terminal(TerminalMsg::HandDelivered {
            hand: id.to_string(),
            outcome: "verdict".to_string(),
            text: Some("done".to_string()),
            trace: None,
            bytes: Some(1),
            elapsed_ms: 10,
            lag_ms: 1,
            schema: None,
            diff: None,
        });
    }

    #[test]
    fn x_on_a_delivered_hand_removes_it_and_selects_the_row_after_it() {
        let mut app = App::new(None);
        spawn_hand_row(&mut app, "h1");
        spawn_hand_row(&mut app, "h2");
        spawn_hand_row(&mut app, "h3");
        deliver_hand(&mut app, "h2");
        app.focus_panel(PanelId::Hands);
        app.hand_selected = 1;

        app.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);

        assert_eq!(app.state.hands.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), vec!["h1", "h3"]);
        assert_eq!(app.hand_selected, 1, "the row that was after h2 (h3) is now at index 1");
    }

    #[test]
    fn x_on_the_last_remaining_row_leaves_the_panel_empty_and_the_party_on_the_live_main() {
        let mut app = App::new(None);
        spawn_hand_row(&mut app, "h1");
        deliver_hand(&mut app, "h1");
        app.focus_panel(PanelId::Hands);
        app.hand_selected = 0;

        app.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);

        assert!(app.state.hands.is_empty());
        assert_eq!(app.active_panel(), PanelId::Main);
        assert_eq!(app.current_party(), PartyKey::Main(app.state.current_main));
    }

    #[test]
    fn x_on_a_running_hand_opens_a_confirm_modal_and_enter_pushes_exactly_one_kill_line() {
        let mut app = App::new(None);
        spawn_hand_row(&mut app, "h1");
        app.focus_panel(PanelId::Hands);
        app.hand_selected = 0;

        app.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(
            matches!(&app.modal, Some(Modal::Confirm { action: ConfirmAction::Kill { id, .. }, .. }) if id == "h1"),
            "modal: {:?}",
            app.modal
        );

        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.outbox.len(), 1, "outbox: {:?}", app.outbox);
        assert_eq!(app.outbox.pop_front(), Some("/kill h1".to_string()));
        assert!(app.modal.is_none());
        assert_eq!(app.state.hands[0].status_cell(), "killing");
    }

    #[test]
    fn x_on_a_running_hand_then_esc_pushes_nothing() {
        let mut app = App::new(None);
        spawn_hand_row(&mut app, "h1");
        app.focus_panel(PanelId::Hands);
        app.hand_selected = 0;

        app.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Esc, KeyModifiers::NONE);

        assert!(app.outbox.is_empty(), "outbox: {:?}", app.outbox);
        assert!(app.modal.is_none());
        assert_ne!(app.state.hands[0].status_cell(), "killing");
    }

    #[test]
    fn x_on_an_ended_main_removes_it() {
        let mut app = App::new(None);
        app.apply_event(Event::Reset { phase: "respawned".to_string(), check: Some("cp1".to_string()) });
        assert_eq!(app.state.mains.len(), 2);
        app.focus_panel(PanelId::Main);
        app.main_selected = 0;

        app.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);

        assert_eq!(app.state.mains.len(), 1);
        assert_eq!(app.state.mains[0].label, "main #2");
    }

    #[test]
    fn x_on_the_live_main_removes_nothing_and_sets_the_status_line() {
        let mut app = App::new(None);
        app.focus_panel(PanelId::Main);
        app.main_selected = 0;

        app.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);

        assert_eq!(app.state.mains.len(), 1);
        assert_eq!(app.transient_hint.as_deref(), Some("live main: /reset ends it"));

        app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert!(app.transient_hint.is_none(), "the hint clears on the next key");
    }

    #[test]
    fn a_late_delivery_for_a_removed_hand_id_changes_nothing() {
        let mut app = App::new(None);
        spawn_hand_row(&mut app, "h1");
        deliver_hand(&mut app, "h1");
        app.focus_panel(PanelId::Hands);
        app.hand_selected = 0;
        app.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(app.state.hands.is_empty());

        deliver_hand(&mut app, "h1");

        assert!(app.state.hands.is_empty(), "a late delivery for a removed id must never re-create the row");
    }

    #[test]
    fn a_manually_typed_kill_or_model_line_never_leaves_the_main_clock_stuck_running() {
        let mut app = App::new(None);
        app.submit_line("/kill h3".to_string());
        assert!(
            app.state.mains[0].turn_started.is_none(),
            "the harness intercepts /kill before it becomes a turn, so no clock should start"
        );

        app.submit_line("/model".to_string());
        assert!(app.state.mains[0].turn_started.is_none(), "the harness intercepts /model the same way");

        app.submit_line("plain prompt".to_string());
        assert!(app.state.mains[0].turn_started.is_some(), "a real operator line still opens the clock");
    }

    #[test]
    fn an_intercepted_command_typed_mid_turn_preserves_the_running_clock() {
        let mut app = App::new(None);
        app.submit_line("plain prompt".to_string());
        let started = app.state.mains[0].turn_started.expect("the real turn opened the clock");

        app.submit_line("/kill h3".to_string());

        assert_eq!(
            app.state.mains[0].turn_started,
            Some(started),
            "an intercepted command submitted while a real turn is in flight must not touch that turn's clock"
        );
    }

    #[test]
    fn a_line_submitted_while_streaming_lands_in_the_queue_and_not_in_outbox() {
        let mut app = App::new(None);
        app.submit_line("first turn".to_string());
        assert_eq!(app.outbox.len(), 1, "outbox: {:?}", app.outbox);
        app.outbox.clear();

        app.submit_line("typed while main is still thinking".to_string());
        assert!(app.outbox.is_empty(), "outbox: {:?}", app.outbox);
        assert_eq!(app.queue.len(), 1);
        assert_eq!(app.queue[0], "typed while main is still thinking");
    }

    #[test]
    fn turn_end_alone_releases_nothing_only_the_next_read_request_does() {
        let mut app = App::new(None);
        app.submit_line("first turn".to_string());
        app.outbox.clear();
        app.submit_line("queued one".to_string());
        app.submit_line("queued two".to_string());
        assert_eq!(app.queue.len(), 2);

        app.apply_terminal(TerminalMsg::TurnEnd { text: "reply".to_string() });
        assert_eq!(app.queue.len(), 2, "a TurnEnd on its own must not release — a ritual sub-turn also ends this way");
        assert!(app.outbox.is_empty());

        app.release_queued_if_idle();

        assert_eq!(app.queue.len(), 1, "queue: {:?}", app.queue);
        assert_eq!(app.queue[0], "queued two");
        assert_eq!(app.outbox.pop_front(), Some("queued one".to_string()));
        assert!(
            app.state.mains[0].page.iter().any(|t| matches!(t, state::Turn::Operator(text) if text == "queued one")),
            "the released line becomes its own operator turn: {:?}",
            app.state.mains[0].page
        );
    }

    #[test]
    fn release_queued_if_idle_is_a_no_op_while_a_turn_is_still_open() {
        let mut app = App::new(None);
        app.submit_line("first turn".to_string());
        app.outbox.clear();
        app.submit_line("queued while streaming".to_string());
        assert!(app.state.is_streaming());

        app.release_queued_if_idle();

        assert_eq!(app.queue.len(), 1, "still mid-turn, nothing should release");
        assert!(app.outbox.is_empty());
    }

    #[test]
    fn an_intercepted_command_produces_no_turn_end_but_the_next_read_request_still_releases_behind_it() {
        let mut app = App::new(None);
        app.submit_line("first turn".to_string());
        app.outbox.clear();
        app.submit_line("/kill h3".to_string());
        app.submit_line("second queued line".to_string());
        assert_eq!(app.queue.len(), 2);

        app.apply_terminal(TerminalMsg::TurnEnd { text: "reply".to_string() });
        app.release_queued_if_idle();
        assert_eq!(app.outbox.pop_front(), Some("/kill h3".to_string()));
        assert!(app.state.mains[0].turn_started.is_none(), "an intercepted command must not open the clock");

        app.release_queued_if_idle();
        assert_eq!(
            app.outbox.pop_front(),
            Some("second queued line".to_string()),
            "the line behind an intercepted command releases on the very next read request, with no TurnEnd of its own to wait for"
        );
    }

    #[test]
    fn up_with_an_empty_input_pulls_the_newest_queued_line_back_and_out_of_the_queue() {
        let mut app = App::new(None);
        app.submit_line("first turn".to_string());
        app.submit_line("queued one".to_string());
        app.submit_line("queued two".to_string());

        app.history_up();

        assert_eq!(app.input, "queued two");
        assert_eq!(app.cursor, "queued two".chars().count());
        assert_eq!(app.queue.len(), 1);
        assert_eq!(app.queue[0], "queued one");
    }

    #[test]
    fn q_with_main_s_turn_open_sends_control_stop_regardless_of_focus() {
        let mut app = App::new(None);
        app.submit_line("hi".to_string());
        app.focus_panel(PanelId::Hands);

        app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);

        assert_eq!(app.control.pop_front(), Some(Control::Stop));
    }

    #[test]
    fn q_on_a_running_hand_opens_the_same_kill_confirm_x_opens() {
        let mut app = App::new(None);
        spawn_hand_row(&mut app, "h1");
        app.focus_panel(PanelId::Hands);
        app.hand_selected = 0;

        app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);

        assert!(
            matches!(&app.modal, Some(Modal::Confirm { action: ConfirmAction::Kill { id, .. }, .. }) if id == "h1"),
            "modal: {:?}",
            app.modal
        );
    }

    #[test]
    fn q_with_nothing_running_sets_the_status_line() {
        let mut app = App::new(None);

        app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);

        assert_eq!(app.transient_hint.as_deref(), Some("nothing running"));
    }

    #[test]
    fn a_bracketed_paste_in_insert_mode_inserts_at_the_cursor_and_submits_nothing() {
        let mut app = App::new(None);
        app.mode = Mode::Insert;
        app.input = "ab".to_string();
        app.cursor = 1;

        app.handle_paste("a\r\nb");

        assert_eq!(app.input, "aa\nbb");
        assert_eq!(app.cursor, 4);
        assert!(app.outbox.is_empty());
    }

    #[test]
    fn a_bracketed_paste_in_normal_mode_enters_insert_first() {
        let mut app = App::new(None);
        assert_eq!(app.mode, Mode::Normal);

        app.handle_paste("pasted");

        assert_eq!(app.mode, Mode::Insert);
        assert_eq!(app.input, "pasted");
    }

    #[test]
    fn a_bracketed_paste_is_dropped_while_a_modal_is_open() {
        let mut app = App::new(None);
        app.apply_terminal(TerminalMsg::Confirm { text: "reset?".to_string() });

        app.handle_paste("should not land");

        assert_eq!(app.input, "");
    }

    #[test]
    fn ctrl_v_with_a_png_on_the_clipboard_writes_it_under_images_dir_and_marks_the_input() {
        let dir = tempfile::tempdir().unwrap();
        let images_dir = dir.path().join("threads/main.images");
        let mut app = App::new(None)
            .with_clipboard(Box::new(clipboard::FakeClipboard(Ok(clipboard::Clip::Image {
                media_type: "image/png".to_string(),
                bytes: vec![0x89, b'P', b'N', b'G'],
            }))))
            .with_images_dir(images_dir.clone());
        app.mode = Mode::Insert;

        app.handle_key(KeyCode::Char('v'), KeyModifiers::CONTROL);

        assert_eq!(app.input, "[image 1]");
        assert!(std::fs::read(images_dir.join("1.png")).unwrap() == vec![0x89, b'P', b'N', b'G']);
        assert_eq!(app.attachments.len(), 1);
        assert_eq!(app.attachments[0].n, 1);
        assert_eq!(app.attachments[0].media_type, "image/png");
    }

    #[test]
    fn ctrl_v_with_text_on_the_clipboard_inserts_it() {
        let mut app = App::new(None).with_clipboard(Box::new(clipboard::FakeClipboard(Ok(clipboard::Clip::Text("pasted text".to_string())))));
        app.mode = Mode::Insert;

        app.handle_key(KeyCode::Char('v'), KeyModifiers::CONTROL);

        assert_eq!(app.input, "pasted text");
        assert!(app.attachments.is_empty());
    }

    #[test]
    fn ctrl_v_with_an_oversized_image_refuses_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let images_dir = dir.path().join("threads/main.images");
        let mut app = App::new(None)
            .with_clipboard(Box::new(clipboard::FakeClipboard(Ok(clipboard::Clip::Image {
                media_type: "image/png".to_string(),
                bytes: vec![0u8; 6 * 1024 * 1024],
            }))))
            .with_images_dir(images_dir.clone());
        app.mode = Mode::Insert;

        app.handle_key(KeyCode::Char('v'), KeyModifiers::CONTROL);

        assert_eq!(app.input, "");
        assert_eq!(app.transient_hint.as_deref(), Some("clipboard: image over 5 MB"));
        assert!(!images_dir.exists() || std::fs::read_dir(&images_dir).unwrap().next().is_none());
        assert!(app.attachments.is_empty());
    }

    #[test]
    fn submit_with_the_image_marker_deleted_sends_a_plain_line_and_removes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let images_dir = dir.path().join("threads/main.images");
        let mut app = App::new(None)
            .with_clipboard(Box::new(clipboard::FakeClipboard(Ok(clipboard::Clip::Image {
                media_type: "image/png".to_string(),
                bytes: vec![1, 2, 3],
            }))))
            .with_images_dir(images_dir.clone());
        app.mode = Mode::Insert;
        app.handle_key(KeyCode::Char('v'), KeyModifiers::CONTROL);
        assert_eq!(app.input, "[image 1]");
        let image_path = images_dir.join("1.png");
        assert!(image_path.exists());

        app.input = "never mind".to_string();
        app.cursor = app.input.chars().count();
        app.submit_line(app.input.clone());

        assert_eq!(app.outbox.pop_front(), Some("never mind".to_string()));
        assert!(app.pop_outbox_images().is_empty());
        assert!(!image_path.exists(), "the orphaned attachment must be deleted from disk");
    }

    #[test]
    fn confirming_a_queued_model_pick_does_not_delete_a_still_pending_draft_s_image() {
        let dir = tempfile::tempdir().unwrap();
        let images_dir = dir.path().join("threads/main.images");
        let mut app = App::new(None)
            .with_clipboard(Box::new(clipboard::FakeClipboard(Ok(clipboard::Clip::Image {
                media_type: "image/png".to_string(),
                bytes: vec![1, 2, 3],
            }))))
            .with_images_dir(images_dir.clone());
        app.mode = Mode::Insert;
        app.submit_line("first turn".to_string());
        app.outbox.clear();
        app.submit_line("/model".to_string());

        app.handle_key(KeyCode::Char('v'), KeyModifiers::CONTROL);
        assert_eq!(app.input, "[image 1]");
        assert_eq!(app.attachments.len(), 1);

        app.apply_terminal(TerminalMsg::Roster {
            models: vec![RosterModel { id: "m1".to_string(), body: "claude-code".to_string(), family: "claude".to_string() }],
            tabs: vec![crate::terminal::RosterTab {
                position: "main".to_string(),
                fallback_id: "m1".to_string(),
                effective_id: "m1".to_string(),
                effective_body: "claude-code".to_string(),
                pinned: false,
                model_ids: vec!["m1".to_string()],
                cursor: 0,
            }],
            main_family: "claude".to_string(),
        });
        app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);

        assert!(images_dir.join("1.png").exists(), "the model pick must not delete the still-pending draft's image");
        assert_eq!(app.attachments.len(), 1, "attachments: {:?}", app.attachments);
    }

    #[test]
    fn modal_key_handling_tabs_over_to_a_hand_and_back_and_applies_only_the_changed_tab() {
        let mut app = App::new(None);
        app.apply_terminal(TerminalMsg::Roster {
            models: vec![
                RosterModel { id: "m1".to_string(), body: "claude-code".to_string(), family: "claude".to_string() },
                RosterModel { id: "m2".to_string(), body: "codex".to_string(), family: "gpt".to_string() },
                RosterModel { id: "m3".to_string(), body: "codex".to_string(), family: "gpt".to_string() },
            ],
            tabs: vec![
                crate::terminal::RosterTab {
                    position: "main".to_string(),
                    fallback_id: "m1".to_string(),
                    effective_id: "m1".to_string(),
                    effective_body: "claude-code".to_string(),
                    pinned: false,
                    model_ids: vec!["m1".to_string(), "m2".to_string(), "m3".to_string()],
                    cursor: 0,
                },
                crate::terminal::RosterTab {
                    position: "hand-ex".to_string(),
                    fallback_id: "m1".to_string(),
                    effective_id: "m1".to_string(),
                    effective_body: "claude-code".to_string(),
                    pinned: false,
                    model_ids: vec!["m1".to_string(), "m2".to_string(), "m3".to_string()],
                    cursor: 0,
                },
            ],
            main_family: "claude".to_string(),
        });

        app.handle_key(KeyCode::Char('l'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('h'), KeyModifiers::NONE);

        match &app.modal {
            Some(Modal::ModelPicker { tab, tabs, .. }) => {
                assert_eq!(*tab, 0, "h must have walked back to main's tab");
                assert_eq!(tabs[0].cursor, tabs[0].opened_cursor, "main's cursor was never moved");
                assert_eq!(tabs[1].cursor, 2, "two downs from the from-file row select the second registry model");
            }
            other => panic!("expected the model picker still open, got {other:?}"),
        }

        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);

        assert_eq!(app.outbox.len(), 1, "only the changed tab writes a line: {:?}", app.outbox);
        assert_eq!(app.outbox.pop_front(), Some("/model hand-ex m2".to_string()));
        assert!(app.modal.is_none());
    }

    #[test]
    fn model_picker_hides_native_from_runner_but_keeps_it_for_reviewer() {
        let mut app = App::new(None);
        app.apply_terminal(TerminalMsg::Roster {
            models: vec![
                RosterModel { id: "claude".to_string(), body: "claude-code".to_string(), family: "claude".to_string() },
                RosterModel { id: "deepseek".to_string(), body: "native".to_string(), family: "deepseek".to_string() },
                RosterModel { id: "codex".to_string(), body: "codex".to_string(), family: "gpt".to_string() },
            ],
            tabs: vec![
                crate::terminal::RosterTab {
                    position: "runner".to_string(),
                    fallback_id: "codex".to_string(),
                    effective_id: "codex".to_string(),
                    effective_body: "codex".to_string(),
                    pinned: false,
                    model_ids: vec!["claude".to_string(), "codex".to_string()],
                    cursor: 0,
                },
                crate::terminal::RosterTab {
                    position: "reviewer".to_string(),
                    fallback_id: "codex".to_string(),
                    effective_id: "codex".to_string(),
                    effective_body: "codex".to_string(),
                    pinned: false,
                    model_ids: vec!["claude".to_string(), "deepseek".to_string(), "codex".to_string()],
                    cursor: 0,
                },
            ],
            main_family: "gpt".to_string(),
        });

        app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('l'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);

        assert_eq!(app.outbox.pop_front(), Some("/model runner codex".to_string()));
        assert_eq!(app.outbox.pop_front(), Some("/model reviewer deepseek".to_string()));
        assert!(app.outbox.is_empty());
    }

    #[test]
    fn a_reset_confirm_reaches_the_answer_lane_never_the_operator_queue() {
        let mut app = App::new(None);
        app.submit_line("first turn".to_string());
        app.outbox.clear();
        app.submit_line("queued behind the reset".to_string());
        app.apply_terminal(TerminalMsg::Confirm { text: "reset?".to_string() });

        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);

        assert_eq!(app.answer_outbox.pop_front(), Some("ok".to_string()));
        assert!(app.outbox.is_empty());
        assert_eq!(app.queue.len(), 1, "the queued line must survive the reset confirm untouched");
    }

    #[test]
    fn submit_with_the_image_marker_present_sends_a_turn_carrying_the_attachment() {
        let dir = tempfile::tempdir().unwrap();
        let images_dir = dir.path().join("threads/main.images");
        let mut app = App::new(None)
            .with_clipboard(Box::new(clipboard::FakeClipboard(Ok(clipboard::Clip::Image {
                media_type: "image/png".to_string(),
                bytes: vec![1, 2, 3],
            }))))
            .with_images_dir(images_dir.clone());
        app.mode = Mode::Insert;
        app.handle_key(KeyCode::Char('v'), KeyModifiers::CONTROL);
        app.insert_char('!');
        assert_eq!(app.input, "[image 1]!");

        app.submit_line(app.input.clone());

        assert_eq!(app.outbox.pop_front(), Some("[image 1]!".to_string()));
        let images = app.pop_outbox_images();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].n, 1);
        assert!(images_dir.join("1.png").exists());
    }

    #[test]
    fn is_intercepted_command_matches_the_harness_s_whitespace_grammar() {
        assert!(is_intercepted_command("/model"));
        assert!(is_intercepted_command("/model claude-opus-5"));
        assert!(is_intercepted_command("/model\nclaude-opus-5"));
        assert!(is_intercepted_command("/kill"));
        assert!(is_intercepted_command("/kill\th3"));
        assert!(!is_intercepted_command("/modelclaude-opus-5"), "no whitespace after the command name is not a match");
        assert!(!is_intercepted_command("/killer h3"), "a longer word sharing the prefix is not a match");
    }
}
