mod app;
mod auto;
mod event;
mod markdown;
mod scenario;
mod snapshot;

use std::io::{self, IsTerminal, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{
    self as cevent, Event as CEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::{Terminal, TerminalOptions, Viewport};

use app::App;
use auto::{Action, SCRIPT};
use event::{Event as EngineEvent, Key, UiEvent};
use markdown::plain_line;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let auto = args.iter().any(|a| a == "--auto");
    let fast = args.iter().any(|a| a == "--fast");
    let snap = args.iter().any(|a| a == "--snapshot");

    if snap {
        snapshot::run();
        return;
    }

    let (etx, erx) = mpsc::channel::<EngineEvent>();
    let (utx, urx) = mpsc::channel::<UiEvent>();
    thread::spawn(move || scenario::run(etx, urx, fast));

    if io::stdout().is_terminal() {
        run_tty(erx, utx, auto, fast).expect("tty session failed");
    } else {
        run_headless(erx, utx, auto, fast);
    }
}

struct Driver {
    idx: usize,
    abort_at: Option<Instant>,
}

impl Driver {
    fn new() -> Self {
        Driver { idx: 0, abort_at: None }
    }

    fn tick(&mut self, app: &mut App, utx: &Sender<UiEvent>, fast: bool) -> bool {
        if let Some(at) = self.abort_at {
            if Instant::now() >= at {
                if app.live.is_some() {
                    let _ = utx.send(UiEvent::Abort);
                }
                self.abort_at = None;
            }
            return self.idx < SCRIPT.len();
        }
        if self.idx >= SCRIPT.len() {
            return false;
        }
        let idle = app.live.is_none();
        match &SCRIPT[self.idx] {
            Action::Line(text) => {
                let waits_on_hands = *text == "Anything back yet?";
                if idle
                    && matches!(app.overlay, app::Overlay::None)
                    && (!waits_on_hands || app.hands_in_flight == 0)
                {
                    app.input.buffer = text.to_string();
                    if let Some(ev) = app.submit_buffer() {
                        let _ = utx.send(ev);
                    }
                    self.idx += 1;
                }
            }
            Action::LineWithAbort(text) => {
                if idle && matches!(app.overlay, app::Overlay::None) {
                    app.input.buffer = text.to_string();
                    if let Some(ev) = app.submit_buffer() {
                        let _ = utx.send(ev);
                    }
                    self.abort_at = Some(
                        Instant::now()
                            + if fast {
                                Duration::from_millis(15)
                            } else {
                                Duration::from_millis(900)
                            },
                    );
                    self.idx += 1;
                }
            }
            Action::AskAnswer(key) => {
                if matches!(app.overlay, app::Overlay::Ask { .. }) {
                    app.ask_select(*key);
                    if let Some(ev) = app.ask_confirm() {
                        let _ = utx.send(ev);
                    }
                    self.idx += 1;
                }
            }
            Action::ModelPick(id) => {
                if matches!(app.overlay, app::Overlay::ModelPicker { .. }) {
                    if let Some(ev) = app.picker_confirm_by_id(id) {
                        let _ = utx.send(ev);
                    }
                    self.idx += 1;
                } else if idle && matches!(app.overlay, app::Overlay::None) {
                    app.input.buffer = "/model".to_string();
                    let _ = app.submit_buffer();
                }
            }
            Action::Confirm(line) => {
                if matches!(app.overlay, app::Overlay::ResetConfirm { .. }) {
                    if let Some(ev) = app.confirm_submit_with(line) {
                        let _ = utx.send(ev);
                    }
                    self.idx += 1;
                }
            }
        }
        true
    }
}

fn translate_key(k: KeyEvent) -> Option<Key> {
    if k.kind != KeyEventKind::Press {
        return None;
    }
    if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
        return Some(Key::CtrlC);
    }
    match k.code {
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Tab => Some(Key::Tab),
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        KeyCode::Esc => Some(Key::Esc),
        KeyCode::Char(c) => Some(Key::Char(c)),
        _ => None,
    }
}

fn run_tty(
    erx: Receiver<EngineEvent>,
    utx: Sender<UiEvent>,
    auto: bool,
    fast: bool,
) -> io::Result<()> {
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(6),
        },
    )?;

    let mut app = App::new();
    let mut driver = if auto { Some(Driver::new()) } else { None };
    let tick = if fast { Duration::from_millis(10) } else { Duration::from_millis(33) };

    loop {
        while let Ok(ev) = erx.try_recv() {
            app.on_engine_event(ev);
        }
        if let Some(d) = &mut driver {
            d.tick(&mut app, &utx, fast);
        }
        if app.quit {
            break;
        }

        let width = terminal.size()?.width;
        let height = app.live_height(width).max(4);
        let area = Rect::new(0, 0, width, height);
        if terminal.get_frame().area() != area {
            terminal.resize(area)?;
        }

        let fresh = app.take_new_scrollback();
        if !fresh.is_empty() {
            let h = fresh.len() as u16;
            terminal.insert_before(h, |buf| {
                render_lines_into(buf, &fresh);
            })?;
        }

        terminal.draw(|frame| {
            let area = frame.area();
            app.render(frame, area);
        })?;

        if !auto && cevent::poll(tick)? {
            match cevent::read()? {
                CEvent::Key(k) => {
                    if let Some(key) = translate_key(k) {
                        if let Some(ui) = app.on_key(key) {
                            let _ = utx.send(ui);
                        }
                    }
                }
                CEvent::Resize(_, _) => {}
                _ => {}
            }
        } else {
            thread::sleep(tick);
        }
    }

    disable_raw_mode()?;
    Ok(())
}

fn render_lines_into(buf: &mut ratatui::buffer::Buffer, lines: &[Line<'static>]) {
    use ratatui::widgets::{Paragraph, Widget};
    Paragraph::new(lines.to_vec()).render(buf.area, buf);
}

fn run_headless(erx: Receiver<EngineEvent>, utx: Sender<UiEvent>, auto: bool, fast: bool) {
    let mut app = App::new();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if auto {
        let mut driver = Driver::new();
        loop {
            while let Ok(ev) = erx.try_recv() {
                app.on_engine_event(ev);
            }
            for line in app.take_new_scrollback() {
                let _ = writeln!(out, "{}", plain_line(&line));
            }
            if app.quit {
                break;
            }
            let more = driver.tick(&mut app, &utx, fast);
            if !more && app.live.is_none() && matches!(app.overlay, app::Overlay::None) {
                thread::sleep(Duration::from_millis(50));
                while let Ok(ev) = erx.try_recv() {
                    app.on_engine_event(ev);
                }
                for line in app.take_new_scrollback() {
                    let _ = writeln!(out, "{}", plain_line(&line));
                }
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let _ = writeln!(out, "{}", status_line(&app));
        return;
    }

    let stdin = io::stdin();
    for line in stdin.lines() {
        let Ok(line) = line else { break };
        if let Some(ev) = app.parse_line(&line) {
            let _ = utx.send(ev);
        }
        loop {
            match erx.recv_timeout(Duration::from_millis(200)) {
                Ok(ev) => app.on_engine_event(ev),
                Err(_) => break,
            }
        }
        for l in app.take_new_scrollback() {
            let _ = writeln!(out, "{}", plain_line(&l));
        }
    }
}

fn status_line(app: &App) -> String {
    format!(
        "[status] model={} scope={} occupancy={} checkpoint={} hands={}",
        app.model, app.scope, app.occupancy, app.checkpoint_number, app.hands_in_flight
    )
}
