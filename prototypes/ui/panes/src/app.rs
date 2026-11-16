use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;

use crate::keys::{self, KeyMap};
use crate::scenario::{Event, Input};
use crate::state::SessionState;

const ASK_CHOICES: [(char, &str); 4] =
    [('a', "allow once"), ('A', "allow always"), ('d', "deny once"), ('D', "deny always")];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelId {
    Session,
    Main,
    Judges,
    Hands,
}

impl PanelId {
    pub fn number(self) -> u8 {
        match self {
            PanelId::Session => 1,
            PanelId::Main => 2,
            PanelId::Judges => 3,
            PanelId::Hands => 4,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PanelId::Session => "session",
            PanelId::Main => "main",
            PanelId::Judges => "judges",
            PanelId::Hands => "hands",
        }
    }
}

pub const PANEL_ORDER: [PanelId; 4] = [PanelId::Session, PanelId::Main, PanelId::Judges, PanelId::Hands];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Panel(PanelId),
    BigView(PanelId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartyKey {
    Session,
    Main(usize),
    Judge(usize),
    Hand(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pane {
    Single,
    Brief,
    Turns,
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
    matches!(party, PartyKey::Judge(_) | PartyKey::Hand(_))
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

#[derive(Debug, Clone)]
pub enum Modal {
    Ask { tool: String, subject: String, selected: usize, note: String },
    ModelPicker { models: Vec<String>, current: String, selected: usize },
}

pub struct App {
    pub state: SessionState,
    pub input: String,
    pub history: Vec<String>,
    pub history_idx: Option<usize>,
    pub mode: Mode,
    pub focus: FocusTarget,
    pub main_selected: usize,
    pub judge_selected: usize,
    pub hand_selected: usize,
    pub half_focus: Half,
    pub modal: Option<Modal>,
    pub help_visible: bool,
    pub pending_g: Option<Instant>,
    pub waiting_since: Option<Instant>,
    pub quit: bool,
    pub keys: KeyMap,
    pub last_big_view: Rect,
    pub last_panels: [Rect; 4],
    scroll_states: HashMap<PaneKey, ScrollState>,
    pane_metrics: HashMap<PaneKey, PaneMetrics>,
    row_caches: HashMap<PaneKey, RowsCache>,
    tx: Sender<Input>,
    rx: Receiver<Event>,
}

impl App {
    pub fn new(tx: Sender<Input>, rx: Receiver<Event>) -> Self {
        App {
            state: SessionState::new(),
            input: String::new(),
            history: Vec::new(),
            history_idx: None,
            mode: Mode::Normal,
            focus: FocusTarget::Panel(PanelId::Main),
            main_selected: 0,
            judge_selected: 0,
            hand_selected: 0,
            half_focus: Half::Turns,
            modal: None,
            help_visible: false,
            pending_g: None,
            waiting_since: None,
            quit: false,
            keys: KeyMap::load(Path::new("keys.toml")),
            last_big_view: Rect::default(),
            last_panels: [Rect::default(); 4],
            scroll_states: HashMap::new(),
            pane_metrics: HashMap::new(),
            row_caches: HashMap::new(),
            tx,
            rx,
        }
    }

    pub fn set_layout_rects(&mut self, big_view: Rect, panels: [Rect; 4]) {
        self.last_big_view = big_view;
        self.last_panels = panels;
    }

    pub fn ready_for_input(&self) -> bool {
        self.modal.is_none() && self.waiting_since.is_none()
    }

    pub fn is_streaming(&self) -> bool {
        self.waiting_since.is_some()
    }

    pub fn drain_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            self.apply_scenario_event(event);
        }
    }

    pub fn rx_recv_timeout(&self) -> Option<Event> {
        self.rx.recv_timeout(Duration::from_millis(5)).ok()
    }

    pub fn apply_scenario_event(&mut self, event: Event) {
        match &event {
            Event::GateAsk { tool, subject } => {
                self.modal = Some(Modal::Ask {
                    tool: tool.clone(),
                    subject: subject.clone(),
                    selected: 0,
                    note: String::new(),
                });
            }
            Event::ModelList { models, current } => {
                let selected = models.iter().position(|m| m == current).unwrap_or(0);
                self.modal = Some(Modal::ModelPicker { models: models.clone(), current: current.clone(), selected });
            }
            Event::ExchangeEnd | Event::Aborted { .. } => {
                self.waiting_since = None;
            }
            _ => {}
        }
        let mains_before = self.state.mains.len();
        self.state.apply(event);
        if self.state.mains.len() > mains_before {
            self.main_selected = self.state.mains.len() - 1;
            self.focus = FocusTarget::Panel(PanelId::Main);
        }
    }

    fn known_command(line: &str) -> bool {
        line == "/reset" || line == "/checkpoint" || line == "/model" || line.starts_with("/model ")
    }

    pub fn submit_line(&mut self, line: String) {
        if line.starts_with('/') && !Self::known_command(&line) {
            self.state.push_operator_line(&line);
            self.state.push_unknown_command_notice();
            return;
        }
        self.history.push(line.clone());
        self.history_idx = None;
        self.state.push_operator_line(&line);
        let instant_reply = line == "/reset" || line == "/model" || line.starts_with("/model ");
        if !instant_reply {
            self.waiting_since = Some(Instant::now());
        }
        self.tx.send(Input::Line(line)).ok();
    }

    pub fn answer_ask(&mut self, key: char, note: Option<String>) {
        self.modal = None;
        self.tx.send(Input::AskAnswer { key, note }).ok();
    }

    pub fn confirm_model_pick(&mut self, id: String) {
        self.modal = None;
        self.submit_line(format!("/model {id}"));
    }

    pub fn send_abort(&mut self) {
        self.tx.send(Input::Abort).ok();
    }

    pub fn backspace(&mut self) {
        self.input.pop();
    }

    pub fn history_up(&mut self) {
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
    }

    pub fn history_down(&mut self) {
        match self.history_idx {
            Some(i) if i + 1 < self.history.len() => {
                self.history_idx = Some(i + 1);
                self.input = self.history[i + 1].clone();
            }
            Some(_) => {
                self.history_idx = None;
                self.input.clear();
            }
            None => {}
        }
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
        }
    }

    pub fn move_modal_selection(&mut self, delta: i32) {
        match &mut self.modal {
            Some(Modal::Ask { selected, .. }) => {
                let len = ASK_CHOICES.len() as i32;
                *selected = ((*selected as i32 + delta).rem_euclid(len)) as usize;
            }
            Some(Modal::ModelPicker { selected, models, .. }) => {
                let len = models.len() as i32;
                *selected = ((*selected as i32 + delta).rem_euclid(len)) as usize;
            }
            None => {}
        }
    }

    pub fn submit_modal(&mut self) {
        match self.modal.clone() {
            Some(Modal::Ask { selected, note, .. }) => {
                let (key, _) = ASK_CHOICES[selected];
                let note = if note.is_empty() { None } else { Some(note) };
                self.answer_ask(key, note);
            }
            Some(Modal::ModelPicker { models, selected, .. }) => {
                let id = models[selected].clone();
                self.confirm_model_pick(id);
            }
            None => {}
        }
    }

    pub fn cancel_modal(&mut self) {
        if matches!(self.modal, Some(Modal::ModelPicker { .. })) {
            self.modal = None;
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
            PanelId::Judges => PartyKey::Judge(self.judge_selected),
            PanelId::Hands => PartyKey::Hand(self.hand_selected),
        }
    }

    fn active_pane_key(&self) -> PaneKey {
        let party = self.current_party();
        let pane = if is_split_party(party) { self.half_focus.to_pane() } else { Pane::Single };
        PaneKey(party, pane)
    }

    fn enter_split_page(&mut self) {
        if is_split_party(self.current_party()) {
            self.half_focus = Half::Turns;
        }
    }

    pub fn focus_panel(&mut self, panel: PanelId) {
        self.focus = FocusTarget::Panel(panel);
    }

    pub fn snapshot_select(&mut self, focus: FocusTarget, index: usize) {
        let panel = match focus {
            FocusTarget::Panel(p) | FocusTarget::BigView(p) => p,
        };
        match panel {
            PanelId::Session => {}
            PanelId::Main => self.main_selected = index,
            PanelId::Judges => self.judge_selected = index,
            PanelId::Hands => self.hand_selected = index,
        }
        self.focus = focus;
        if matches!(focus, FocusTarget::BigView(_)) {
            self.enter_split_page();
        }
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
        if let FocusTarget::BigView(p) = self.focus {
            self.focus = FocusTarget::Panel(p);
        }
    }

    fn panel_len(&self, panel: PanelId) -> usize {
        match panel {
            PanelId::Session => 0,
            PanelId::Main => self.state.mains.len(),
            PanelId::Judges => self.state.judges.len(),
            PanelId::Hands => self.state.hands.len(),
        }
    }

    fn selection_mut(&mut self, panel: PanelId) -> Option<&mut usize> {
        match panel {
            PanelId::Session => None,
            PanelId::Main => Some(&mut self.main_selected),
            PanelId::Judges => Some(&mut self.judge_selected),
            PanelId::Hands => Some(&mut self.hand_selected),
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
            FocusTarget::BigView(_) => self.pane_scroll_down(1),
        }
    }

    pub fn nav_up(&mut self) {
        match self.focus {
            FocusTarget::Panel(_) => self.move_selection(-1),
            FocusTarget::BigView(_) => self.pane_scroll_up(1),
        }
    }

    pub fn nav_top(&mut self) {
        match self.focus {
            FocusTarget::Panel(_) => self.jump_selection(false),
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
            FocusTarget::BigView(_) => self.pane_scroll_bottom(),
        }
    }

    pub fn nav_page(&mut self, dir: i32) {
        if let FocusTarget::BigView(_) = self.focus {
            if dir > 0 {
                self.pane_scroll_down(10);
            } else {
                self.pane_scroll_up(10);
            }
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
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

        if self.is_streaming() && self.keys.normal.matches("abort", &token) {
            self.send_abort();
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
            self.focus_panel(PanelId::Judges);
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
            return;
        }
        if self.keys.insert.matches("submit", token) {
            let line = self.input.clone();
            if !line.trim().is_empty() {
                self.input.clear();
                self.submit_line(line);
            }
            return;
        }
        if self.keys.insert.matches("newline", token) {
            self.input.push('\n');
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
        match code {
            KeyCode::Backspace => self.backspace(),
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
    }

    fn handle_modal_key(&mut self, token: &str, code: KeyCode) {
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
        match &mut self.modal {
            Some(Modal::Ask { note, selected, .. }) => match code {
                KeyCode::Backspace => {
                    note.pop();
                }
                KeyCode::Char(c) => {
                    if let Some(idx) = ASK_CHOICES.iter().position(|(k, _)| *k == c) {
                        *selected = idx;
                    } else {
                        note.push(c);
                    }
                }
                _ => {}
            },
            Some(Modal::ModelPicker { models, selected, .. }) => {
                if let KeyCode::Char(c) = code {
                    if let Some(n) = c.to_digit(10) {
                        let n = n as usize;
                        if n >= 1 && n <= models.len() {
                            *selected = n - 1;
                        }
                    }
                }
            }
            None => {}
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

#[derive(Debug, Clone)]
pub enum AutoAction {
    Line(&'static str),
    AskAnswer(char, Option<&'static str>),
    PickModel(&'static str),
    AbortWhenStreaming,
}

pub fn auto_script() -> Vec<AutoAction> {
    vec![
        AutoAction::Line("why did the store move out of the repo"),
        AutoAction::Line("what does terminal.rs pin down today"),
        AutoAction::AskAnswer('A', Some("routine test run")),
        AutoAction::Line("go ahead and dispatch the cleanup"),
        AutoAction::Line("what's the word from both of them"),
        AutoAction::Line("/model"),
        AutoAction::PickModel("claude-sonnet-5"),
        AutoAction::Line("/model gpt-5"),
        AutoAction::Line("keep going with the polishing pass"),
        AutoAction::Line("/checkpoint"),
        AutoAction::Line("/reset"),
        AutoAction::Line("ok"),
        AutoAction::Line("tell me about the pass so far"),
        AutoAction::AbortWhenStreaming,
        AutoAction::Line("continue"),
    ]
}

pub struct AutoDriver {
    script: Vec<AutoAction>,
    idx: usize,
    last_fire: Instant,
}

impl AutoDriver {
    pub fn new() -> Self {
        AutoDriver { script: auto_script(), idx: 0, last_fire: Instant::now() }
    }

    pub fn done(&self) -> bool {
        self.idx >= self.script.len()
    }

    pub fn tick(&mut self, app: &mut App, fast: bool) {
        if self.done() {
            return;
        }
        let pace = if fast { Duration::from_millis(0) } else { Duration::from_millis(900) };
        let action = self.script[self.idx].clone();
        match action {
            AutoAction::Line(text) => {
                if app.ready_for_input() && self.last_fire.elapsed() >= pace {
                    app.submit_line(text.to_string());
                    self.idx += 1;
                    self.last_fire = Instant::now();
                }
            }
            AutoAction::AskAnswer(key, note) => {
                if matches!(app.modal, Some(Modal::Ask { .. })) && self.last_fire.elapsed() >= pace {
                    app.answer_ask(key, note.map(|s| s.to_string()));
                    self.idx += 1;
                    self.last_fire = Instant::now();
                }
            }
            AutoAction::PickModel(id) => {
                if matches!(app.modal, Some(Modal::ModelPicker { .. })) && self.last_fire.elapsed() >= pace {
                    app.confirm_model_pick(id.to_string());
                    self.idx += 1;
                    self.last_fire = Instant::now();
                }
            }
            AutoAction::AbortWhenStreaming => {
                if let Some(buf) = &app.state.streaming_buffer {
                    if buf.split_whitespace().count() >= 8 {
                        app.send_abort();
                        self.idx += 1;
                        self.last_fire = Instant::now();
                    }
                }
            }
        }
    }
}
