mod app;
mod keys;
mod scenario;
mod state;
mod ui;

use std::io::stdout;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::layout::Rect;
use ratatui::Terminal;

use app::{App, AutoDriver, FocusTarget, Modal, PanelId, PANEL_ORDER};
use scenario::{Event, Input, Timing};

#[derive(Parser, Debug)]
#[command(about = "panes: full-screen ratatui front for the arx scenario")]
struct Args {
    #[arg(long)]
    auto: bool,
    #[arg(long)]
    fast: bool,
    #[arg(long)]
    snapshot: bool,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    if args.snapshot {
        run_snapshot();
        return Ok(());
    }
    run_interactive(args.auto, args.fast)
}

fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

fn run_interactive(auto: bool, fast: bool) -> std::io::Result<()> {
    let (input_tx, input_rx) = mpsc::channel::<Input>();
    let (event_tx, event_rx) = mpsc::channel::<Event>();
    let timing = Timing { fast };
    thread::spawn(move || scenario::run(event_tx, input_rx, timing));

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(input_tx, event_rx);
    let mut driver = if auto { Some(AutoDriver::new()) } else { None };

    let result = (|| -> std::io::Result<()> {
        loop {
            app.drain_events();
            if let Some(d) = &mut driver {
                d.tick(&mut app, fast);
            }
            terminal.draw(|f| ui::draw(f, &mut app))?;
            if app.quit {
                break;
            }
            if event::poll(Duration::from_millis(30))? {
                match event::read()? {
                    CEvent::Key(key) => {
                        if key.kind == KeyEventKind::Press {
                            app.handle_key(key.code, key.modifiers);
                        }
                    }
                    CEvent::Mouse(m) => {
                        let panel = PANEL_ORDER
                            .iter()
                            .zip(app.last_panels.iter())
                            .find(|(_, r)| rect_contains(**r, m.column, m.row))
                            .map(|(p, _)| *p);
                        let big_view = panel.is_none() && rect_contains(app.last_big_view, m.column, m.row);
                        match m.kind {
                            MouseEventKind::ScrollUp => app.handle_mouse_scroll(panel, big_view, -3),
                            MouseEventKind::ScrollDown => app.handle_mouse_scroll(panel, big_view, 3),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    result
}

const SCENE_BOUNDARIES: [&str; 7] = [
    "scene 1: a plain reply",
    "scene 2: tool calls, an ask, admission",
    "scene 3: a dispatch and a judge, delivery while idle",
    "scene 4: /model",
    "scene 5: nudge, checkpoint, reset",
    "scene 6: abort and a non-fatal error",
    "scene 7: an unknown command",
];

fn run_snapshot() {
    let (input_tx, input_rx) = mpsc::channel::<Input>();
    let (event_tx, event_rx) = mpsc::channel::<Event>();
    thread::spawn(move || scenario::run(event_tx, input_rx, Timing { fast: true }));

    let backend = TestBackend::new(110, 36);
    let mut terminal = Terminal::new(backend).expect("test backend");
    let mut app = App::new(input_tx, event_rx);

    let capture = |app: &mut App, terminal: &mut Terminal<TestBackend>, label: &str| {
        terminal.draw(|f| ui::draw(f, app)).ok();
        println!("=== {label} ===");
        print_buffer(terminal);
        println!();
    };

    app.submit_line("why did the store move".to_string());
    scroll_up_mid_stream(&mut app, 3, 2);
    capture(&mut app, &mut terminal, "main scrolled up with k while scene 1 streams, showing the n new lines hint");
    app.handle_key(KeyCode::Char('G'), KeyModifiers::NONE);
    drain_quiet(&mut app);
    capture(&mut app, &mut terminal, SCENE_BOUNDARIES[0]);
    app.snapshot_select(FocusTarget::Panel(PanelId::Main), app.state.current_main);
    app.help_visible = true;
    capture(&mut app, &mut terminal, "extra: ? overlay");
    app.help_visible = false;

    app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    app.handle_key(KeyCode::Char('k'), KeyModifiers::NONE);
    app.handle_key(KeyCode::Char('k'), KeyModifiers::NONE);
    app.handle_key(KeyCode::Char('4'), KeyModifiers::NONE);
    app.handle_key(KeyCode::Char('2'), KeyModifiers::NONE);
    capture(&mut app, &mut terminal, "main's page after leaving for panel 4 and coming back with 2, same anchored position");

    send_and_drain(&mut app, "what does terminal.rs pin down");
    resolve_ask(&mut app, 'A', Some("routine test run"));
    drain_quiet(&mut app);
    capture(&mut app, &mut terminal, SCENE_BOUNDARIES[1]);
    app.snapshot_select(app::FocusTarget::BigView(PanelId::Session), 0);
    capture(&mut app, &mut terminal, "extra: panel 1 session page");
    app.snapshot_select(app::FocusTarget::Panel(PanelId::Main), app.state.current_main);

    app.submit_line("go ahead and dispatch the cleanup".to_string());
    drain_until(&mut app, |e| matches!(e, Event::TurnEnd { .. }));
    app.snapshot_select(app::FocusTarget::BigView(PanelId::Hands), 0);
    capture(&mut app, &mut terminal, "extra: hand page h-0007 running, before delivery");
    capture(&mut app, &mut terminal, "h-0007 split page while running (scene 3, before delivery), bottom focused");
    app.snapshot_select(app::FocusTarget::Panel(PanelId::Main), app.state.current_main);
    drain_quiet(&mut app);
    capture(&mut app, &mut terminal, SCENE_BOUNDARIES[2]);
    app.snapshot_select(app::FocusTarget::BigView(PanelId::Hands), 0);
    capture(&mut app, &mut terminal, "extra: hand page h-0007 after delivery");
    capture(&mut app, &mut terminal, "h-0007 split page after delivery, bottom focused, at the top of its turns");
    app.handle_key(KeyCode::Char('G'), KeyModifiers::NONE);
    capture(&mut app, &mut terminal, "h-0007 split page after G, showing the verdict block");
    app.handle_key(KeyCode::Tab, KeyModifiers::NONE);
    capture(&mut app, &mut terminal, "h-0007 split page after Tab, top pane focused");
    app.snapshot_select(app::FocusTarget::BigView(PanelId::Judges), 0);
    capture(&mut app, &mut terminal, "extra: judge page h-0008 after delivery");
    app.handle_key(KeyCode::Char('G'), KeyModifiers::NONE);
    capture(&mut app, &mut terminal, "h-0008 split page after delivery, bottom focused, after G");
    app.snapshot_select(app::FocusTarget::Panel(PanelId::Main), app.state.current_main);

    send_and_drain(&mut app, "what's the word from both of them");
    send_and_drain(&mut app, "/model");
    resolve_model_pick(&mut app, "claude-sonnet-5");
    drain_quiet(&mut app);
    send_and_drain(&mut app, "/model gpt-5");
    capture(&mut app, &mut terminal, SCENE_BOUNDARIES[3]);

    send_and_drain(&mut app, "keep going with the polishing pass");
    send_and_drain(&mut app, "/checkpoint");
    send_and_drain(&mut app, "/reset");
    send_and_drain(&mut app, "ok");
    capture(&mut app, &mut terminal, SCENE_BOUNDARIES[4]);
    app.snapshot_select(app::FocusTarget::BigView(PanelId::Main), 0);
    capture(&mut app, &mut terminal, "extra: main #1 frozen page after the reset");
    app.snapshot_select(app::FocusTarget::Panel(PanelId::Main), app.state.current_main);

    app.submit_line("tell me about the pass so far".to_string());
    abort_after_chunks(&mut app, 6);
    drain_quiet(&mut app);
    send_and_drain(&mut app, "continue");
    capture(&mut app, &mut terminal, SCENE_BOUNDARIES[5]);

    app.submit_line("/reste".to_string());
    capture(&mut app, &mut terminal, SCENE_BOUNDARIES[6]);
}

fn send_and_drain(app: &mut App, line: &str) {
    app.submit_line(line.to_string());
    drain_quiet(app);
}

fn drain_quiet(app: &mut App) {
    loop {
        match app.rx_recv_timeout() {
            Some(event) => app.apply_scenario_event(event),
            None => break,
        }
    }
}

fn drain_until(app: &mut App, stop_after: impl Fn(&Event) -> bool) {
    loop {
        match app.rx_recv_timeout() {
            Some(event) => {
                let stop = stop_after(&event);
                app.apply_scenario_event(event);
                if stop {
                    break;
                }
            }
            None => break,
        }
    }
}

fn resolve_ask(app: &mut App, key: char, note: Option<&str>) {
    drain_quiet(app);
    if matches!(app.modal, Some(Modal::Ask { .. })) {
        app.answer_ask(key, note.map(|s| s.to_string()));
    }
}

fn resolve_model_pick(app: &mut App, id: &str) {
    drain_quiet(app);
    if matches!(app.modal, Some(Modal::ModelPicker { .. })) {
        app.confirm_model_pick(id.to_string());
    }
}

fn scroll_up_mid_stream(app: &mut App, n_chunks: usize, k_presses: usize) {
    app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    let mut seen = 0;
    loop {
        match app.rx_recv_timeout() {
            Some(event) => {
                let is_chunk = matches!(event, Event::Chunk { .. });
                app.apply_scenario_event(event);
                if is_chunk {
                    seen += 1;
                    if seen >= n_chunks {
                        break;
                    }
                }
            }
            None => break,
        }
    }
    for _ in 0..k_presses {
        app.handle_key(KeyCode::Char('k'), KeyModifiers::NONE);
    }
}

fn abort_after_chunks(app: &mut App, n: usize) {
    let mut seen = 0;
    loop {
        match app.rx_recv_timeout() {
            Some(event) => {
                let is_chunk = matches!(event, Event::Chunk { .. });
                app.apply_scenario_event(event);
                if is_chunk {
                    seen += 1;
                    if seen >= n {
                        app.send_abort();
                    }
                }
            }
            None => break,
        }
    }
}

fn print_buffer(terminal: &Terminal<TestBackend>) {
    let buffer = terminal.backend().buffer();
    let area = buffer.area;
    for y in 0..area.height {
        let mut row = String::new();
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            row.push_str(cell.symbol());
        }
        println!("{}", row.trim_end());
    }
}
