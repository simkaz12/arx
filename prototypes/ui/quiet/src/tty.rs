use std::io::{stdout, Stdout, Write};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event as CEvent, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use crossterm::{cursor, execute};

use crate::driver;
use crate::editor::{Editor, EditorOutcome};
use crate::format::{admission_lines, ask_subject_display, delivery_line, dispatch_line, markdown_line, model_list_line, role_label, tool_call_line};
use crate::scenario::{self, Event, ModelListReason, OperatorInput};
use crate::state::{SessionState, MODELS};

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";
const AMBER: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const WIDTH: usize = 100;

struct RawGuard;

impl RawGuard {
    fn enable() -> Self {
        let _ = enable_raw_mode();
        RawGuard
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

enum Turn {
    Idle,
    Waiting(Instant),
    Streaming,
}

struct AskState {
    tool: String,
    subject: String,
    typed: String,
}

struct PickerState {
    current: String,
    selected: usize,
}

struct Ui {
    state: SessionState,
    editor: Editor,
    turn: Turn,
    ask: Option<AskState>,
    confirm: Option<String>,
    picker: Option<PickerState>,
    pending_tool: Option<(String, String)>,
    rule_pending: bool,
    prompt_lines: u16,
    line_buf: String,
    in_fence: bool,
}

fn wl(out: &mut Stdout, text: &str) {
    let _ = write!(out, "{text}\r\n");
}

pub fn run(auto: bool, fast: bool) {
    let (rx, tx) = scenario::spawn(fast);
    let (echo_tx, echo_rx) = mpsc::channel::<String>();
    let forward = if auto {
        Some(driver::spawn_auto_driver_with_echo(fast, true, tx.clone(), Some(echo_tx)))
    } else {
        None
    };

    let _raw = RawGuard::enable();
    let mut out = stdout();
    let mut ui = Ui {
        state: SessionState::new(),
        editor: Editor::new(),
        turn: Turn::Idle,
        ask: None,
        confirm: None,
        picker: None,
        pending_tool: None,
        rule_pending: false,
        prompt_lines: 0,
        line_buf: String::new(),
        in_fence: false,
    };

    draw_prompt(&mut out, &mut ui);

    let mut disconnected = false;
    loop {
        if disconnected {
            break;
        }
        match rx.try_recv() {
            Ok(event) => {
                clear_prompt(&mut out, &ui);
                let for_driver = event.clone();
                on_engine_event(&mut out, &mut ui, event);
                if let Some(fwd) = &forward {
                    let _ = fwd.send(for_driver);
                }
                draw_prompt(&mut out, &mut ui);
                continue;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                disconnected = true;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        let mut dirty = matches!(ui.turn, Turn::Waiting(_));
        let mut key_to_handle = None;
        if event::poll(Duration::from_millis(0)).unwrap_or(false) {
            match event::read() {
                Ok(CEvent::Key(key)) => {
                    key_to_handle = Some(key);
                    dirty = true;
                }
                Ok(CEvent::Resize(_, _)) => {
                    dirty = true;
                }
                _ => {}
            }
        }
        let mut echoed = None;
        if auto {
            if let Ok(line) = echo_rx.try_recv() {
                echoed = Some(line);
                dirty = true;
            }
        }
        if dirty {
            clear_prompt(&mut out, &ui);
        }
        let mut quit = false;
        if let Some(key) = key_to_handle {
            if !handle_key(&mut out, &mut ui, key, &tx) {
                quit = true;
            }
        }
        if let Some(line) = echoed {
            reflect_echo(&mut out, &mut ui, &line, &tx);
        }
        if dirty {
            draw_prompt(&mut out, &mut ui);
        }
        if quit {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn submit_line(out: &mut Stdout, ui: &mut Ui, text: &str, tx: &Sender<OperatorInput>) {
    wl(out, &format!("› {text}"));
    ui.turn = Turn::Waiting(Instant::now());
    let _ = tx.send(OperatorInput::Line(text.to_string()));
}

fn reflect_echo(out: &mut Stdout, ui: &mut Ui, line: &str, tx: &Sender<OperatorInput>) {
    if let Some(ask) = ui.ask.take() {
        let header = format!("[arx] ask: {} {}", ask.tool, ask_subject_display(&ask.subject));
        wl(out, &header);
        wl(
            out,
            &format!("a allow once · A allow always · d deny once · D deny always · a note may follow the key: {line}"),
        );
        ui.rule_pending = true;
    } else if ui.confirm.take().is_some() {
        wl(out, &format!("confirm› {line}"));
    } else {
        ui.picker = None;
        wl(out, &format!("› {line}"));
    }
    ui.turn = Turn::Waiting(Instant::now());
    let _ = tx.send(OperatorInput::Line(line.to_string()));
}

fn handle_key(out: &mut Stdout, ui: &mut Ui, key: crossterm::event::KeyEvent, tx: &Sender<OperatorInput>) -> bool {
    if ui.ask.is_some() {
        return handle_ask_key(out, ui, key, tx);
    }
    if ui.confirm.is_some() {
        return handle_confirm_key(out, ui, key, tx);
    }
    if ui.picker.is_some() {
        return handle_picker_key(out, ui, key, tx);
    }
    match ui.turn {
        Turn::Idle => match ui.editor.handle_key(key) {
            EditorOutcome::Submit(text) => {
                submit_line(out, ui, &text, tx);
                true
            }
            EditorOutcome::Quit => false,
            EditorOutcome::Cleared | EditorOutcome::Continue => true,
        },
        Turn::Waiting(_) | Turn::Streaming => {
            if key.code == KeyCode::Esc {
                let _ = tx.send(OperatorInput::Abort);
            } else if let KeyCode::Char('c') = key.code {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    return false;
                }
            }
            true
        }
    }
}

fn handle_ask_key(out: &mut Stdout, ui: &mut Ui, key: crossterm::event::KeyEvent, tx: &Sender<OperatorInput>) -> bool {
    let ask = ui.ask.as_mut().unwrap();
    match key.code {
        KeyCode::Enter => {
            let line = ask.typed.clone();
            let header = format!("[arx] ask: {} {}", ask.tool, ask_subject_display(&ask.subject));
            let prompt = format!(
                "a allow once · A allow always · d deny once · D deny always · a note may follow the key: {line}"
            );
            wl(out, &header);
            wl(out, &prompt);
            ui.ask = None;
            ui.rule_pending = true;
            ui.turn = Turn::Waiting(Instant::now());
            let _ = tx.send(OperatorInput::Line(line));
        }
        KeyCode::Esc => {
            let header = format!("[arx] ask: {} {}", ask.tool, ask_subject_display(&ask.subject));
            wl(out, &header);
            wl(
                out,
                "a allow once · A allow always · d deny once · D deny always · a note may follow the key: d (esc)",
            );
            ui.ask = None;
            ui.turn = Turn::Waiting(Instant::now());
            let _ = tx.send(OperatorInput::Line("d".to_string()));
        }
        KeyCode::Backspace => {
            ask.typed.pop();
        }
        KeyCode::Char(c) => ask.typed.push(c),
        _ => {}
    }
    true
}

fn handle_confirm_key(out: &mut Stdout, ui: &mut Ui, key: crossterm::event::KeyEvent, tx: &Sender<OperatorInput>) -> bool {
    let typed = ui.confirm.as_mut().unwrap();
    match key.code {
        KeyCode::Enter => {
            let line = typed.clone();
            wl(out, &format!("confirm› {line}"));
            ui.confirm = None;
            ui.turn = Turn::Waiting(Instant::now());
            let _ = tx.send(OperatorInput::Line(line));
        }
        KeyCode::Backspace => {
            typed.pop();
        }
        KeyCode::Char(c) => typed.push(c),
        _ => {}
    }
    true
}

fn handle_picker_key(_out: &mut Stdout, ui: &mut Ui, key: crossterm::event::KeyEvent, tx: &Sender<OperatorInput>) -> bool {
    let picker = ui.picker.as_mut().unwrap();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if picker.selected > 0 {
                picker.selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if picker.selected + 1 < MODELS.len() {
                picker.selected += 1;
            }
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            if let Some(d) = c.to_digit(10) {
                let i = d as usize;
                if i >= 1 && i <= MODELS.len() {
                    picker.selected = i - 1;
                    confirm_pick(ui, tx);
                }
            }
        }
        KeyCode::Enter => confirm_pick(ui, tx),
        KeyCode::Esc => {
            ui.picker = None;
            ui.turn = Turn::Idle;
        }
        _ => {}
    }
    true
}

fn confirm_pick(ui: &mut Ui, tx: &Sender<OperatorInput>) {
    let picker = ui.picker.take().unwrap();
    let chosen = MODELS[picker.selected];
    ui.turn = Turn::Waiting(Instant::now());
    let _ = tx.send(OperatorInput::Line(format!("/model {chosen}")));
}

fn on_engine_event(out: &mut Stdout, ui: &mut Ui, event: Event) {
    match event {
        Event::Chunk(text) => {
            if !matches!(ui.turn, Turn::Streaming) {
                ui.turn = Turn::Streaming;
            }
            ui.line_buf.push_str(&text);
            while let Some(pos) = ui.line_buf.find('\n') {
                let line: String = ui.line_buf[..pos].to_string();
                ui.line_buf.drain(..=pos);
                for formatted in markdown_line(&line, WIDTH, &mut ui.in_fence) {
                    wl(out, &format!("  {formatted}"));
                }
            }
        }
        Event::TurnEnd(_) => {
            flush_line_buf(out, ui);
            wl(out, "");
            ui.turn = Turn::Idle;
        }
        Event::ToolCall { tool, subject } => {
            ui.pending_tool = Some((tool, subject));
        }
        Event::GateAsk { tool, subject } => {
            ui.ask = Some(AskState { tool, subject, typed: String::new() });
        }
        Event::GateVerdict { rule, .. } => {
            if ui.rule_pending {
                if let Some(r) = rule {
                    wl(out, &format!("{AMBER}  rule appended: {r} -> .arx/gate.toml{RESET}"));
                }
                ui.rule_pending = false;
            }
        }
        Event::ToolResult { admission } => {
            if let Some((tool, subject)) = ui.pending_tool.take() {
                let (summary, preview) = admission_lines(&admission);
                wl(out, &format!("{DIM}  · {}   {summary}{RESET}", tool_call_line(&tool, &subject)));
                if let Some(p) = preview {
                    wl(out, &format!("{DIM}    {p}{RESET}"));
                }
            }
        }
        Event::Spawn { hand_id, agent, role, body, model, .. } => {
            ui.state.hands_in_flight += 1;
            wl(out, &format!("{DIM}  ⇢ {} ({}){RESET}", dispatch_line(&hand_id, &agent, &body, &model), role_label(role)));
        }
        Event::Delivered { hand_id, outcome, text, trace, elapsed_ms, schema_ok, .. } => {
            ui.state.hands_in_flight -= 1;
            wl(out, &format!("{DIM}  ⇠ {}{RESET}", delivery_line(&hand_id, outcome, elapsed_ms, &trace)));
            wl(out, &format!("    {text}"));
            if let Some(ok) = schema_ok {
                wl(out, &format!("{DIM}    schema: {}{RESET}", if ok { "ok" } else { "malformed" }));
            }
        }
        Event::Notice { text, level } => {
            let color = match level {
                crate::scenario::NoticeLevel::Routine => DIM,
                crate::scenario::NoticeLevel::Nudge => AMBER,
                crate::scenario::NoticeLevel::Error => RED,
            };
            wl(out, &format!("{color}[arx] {text}{RESET}"));
            ui.turn = Turn::Idle;
        }
        Event::Usage { input, output, elapsed_ms, occupancy_after, .. } => {
            ui.state.occupancy = occupancy_after;
            ui.state.last_tokens = Some(input + output);
            ui.state.last_elapsed_ms = Some(elapsed_ms);
            wl(
                out,
                &format!("{DIM}  {:.1}k tok · {:.1}s{RESET}", (input + output) as f64 / 1000.0, elapsed_ms as f64 / 1000.0),
            );
        }
        Event::Checkpoint { number, path, delta } => {
            ui.state.checkpoint = number;
            let previous = number.saturating_sub(1);
            wl(out, &format!("{DIM}[arx] checkpoint {number} written: {path}, delta +{delta} since checkpoint {previous}{RESET}"));
            ui.turn = Turn::Idle;
        }
        Event::ResetArmed { occupancy, window, reserve } => {
            ui.state.occupancy = occupancy;
            wl(out, &format!("{AMBER}[arx] window at {occupancy} of {window}, reserve {reserve}; /reset to reset now{RESET}"));
            ui.turn = Turn::Idle;
        }
        Event::ResetConfirm { body } => {
            for line in body.split('\n') {
                wl(out, &format!("  {line}"));
            }
            wl(
                out,
                "ok to tear down and respawn; any other line rides into the new main's first turn as an operator note",
            );
            ui.confirm = Some(String::new());
        }
        Event::Orientation { text: _, verdict, occupancy_after } => {
            flush_line_buf(out, ui);
            wl(out, "");
            wl(out, &format!("{DIM}[arx] orientation: {verdict}{RESET}"));
            ui.state.occupancy = occupancy_after;
            ui.state.model = crate::state::INITIAL_MODEL.to_string();
            ui.turn = Turn::Idle;
        }
        Event::ModelList { models, current, reason, attempted } => match reason {
            ModelListReason::Unlisted => {
                wl(out, &format!("{DIM}[arx] {} is not a known model{RESET}", attempted.unwrap_or_default()));
                wl(out, &format!("{DIM}[arx] models: {}{RESET}", model_list_line(&models, &current)));
                ui.turn = Turn::Idle;
            }
            ModelListReason::Requested => {
                let selected = MODELS.iter().position(|m| *m == current).unwrap_or(0);
                ui.picker = Some(PickerState { current, selected });
                ui.turn = Turn::Idle;
            }
        },
        Event::ModelSwitched { from, to } => {
            wl(out, &format!("{DIM}[arx] model: {from} -> {to}{RESET}"));
            ui.state.model = to;
            ui.turn = Turn::Idle;
        }
        Event::ProviderError { message, recoverable } => {
            wl(out, &format!("{RED}[arx] {message}{}{RESET}", if recoverable { "; retrying" } else { "" }));
        }
        Event::Aborted => {
            if !ui.line_buf.is_empty() {
                let remaining = std::mem::take(&mut ui.line_buf);
                wl(out, &format!("  {remaining}"));
            }
            wl(out, &format!("{DIM}(aborted; partial text above stays in the transcript){RESET}"));
            ui.turn = Turn::Idle;
        }
    }
}

fn flush_line_buf(out: &mut Stdout, ui: &mut Ui) {
    if !ui.line_buf.is_empty() {
        let remaining = std::mem::take(&mut ui.line_buf);
        for formatted in markdown_line(&remaining, WIDTH, &mut ui.in_fence) {
            wl(out, &format!("  {formatted}"));
        }
    }
}

fn clear_prompt(out: &mut Stdout, ui: &Ui) {
    if ui.prompt_lines == 0 {
        return;
    }
    let up = ui.prompt_lines - 1;
    if up > 0 {
        let _ = execute!(out, cursor::MoveUp(up));
    }
    let _ = execute!(out, cursor::MoveToColumn(0), Clear(ClearType::FromCursorDown));
}

fn draw_prompt(out: &mut Stdout, ui: &mut Ui) {
    if let Some(ask) = &ui.ask {
        let header = format!("[arx] ask: {} {}", ask.tool, ask_subject_display(&ask.subject));
        let _ = write!(out, "{header}\r\n");
        let _ = write!(
            out,
            "a allow once · A allow always · d deny once · D deny always · a note may follow the key: {}",
            ask.typed
        );
        ui.prompt_lines = 2;
        let _ = out.flush();
        return;
    }
    if let Some(typed) = &ui.confirm {
        let _ = write!(out, "confirm› {typed}");
        ui.prompt_lines = 1;
        let _ = out.flush();
        return;
    }
    if let Some(picker) = &ui.picker {
        for (i, m) in MODELS.iter().enumerate() {
            let marker = if *m == picker.current { "●" } else { " " };
            let cursor_mark = if i == picker.selected { "›" } else { " " };
            let _ = write!(out, "{cursor_mark} {marker} {}. {m}", i + 1);
            if i + 1 < MODELS.len() {
                let _ = write!(out, "\r\n");
            }
        }
        ui.prompt_lines = MODELS.len() as u16;
        let _ = out.flush();
        return;
    }
    match &ui.turn {
        Turn::Streaming => {
            ui.prompt_lines = 0;
        }
        Turn::Waiting(since) => {
            let elapsed = since.elapsed().as_secs_f64();
            let _ = write!(out, "{DIM}… thinking {elapsed:.1}s  (esc to abort){RESET}");
            ui.prompt_lines = 1;
            let _ = out.flush();
        }
        Turn::Idle => {
            let status = ui.state.status_line();
            let _ = write!(out, "{DIM}{status}{RESET}\r\n");
            let buf = ui.editor.text();
            let printable = buf.replace('\n', "\r\n  ");
            let _ = write!(out, "› {printable}");
            ui.prompt_lines = 2 + buf.matches('\n').count() as u16;
            let _ = out.flush();
        }
    }
}
