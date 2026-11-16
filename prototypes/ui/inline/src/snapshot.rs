use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;

use crate::app::App;
use crate::auto::{Action, SCENE_BOUNDARY_AFTER, SCRIPT};
use crate::event::{Event as EngineEvent, UiEvent};
use crate::markdown::plain_line;
use crate::scenario;

const WIDTH: u16 = 110;
const HEIGHT: u16 = 36;

fn drain_quiet(app: &mut App, erx: &mpsc::Receiver<EngineEvent>) {
    loop {
        match erx.recv_timeout(Duration::from_millis(120)) {
            Ok(ev) => app.on_engine_event(ev),
            Err(_) => break,
        }
    }
}

fn buffer_lines(buf: &Buffer) -> Vec<String> {
    let area = buf.area;
    (0..area.height)
        .map(|y| {
            let mut row = String::new();
            for x in 0..area.width {
                row.push_str(buf[(x, y)].symbol());
            }
            row.trim_end().to_string()
        })
        .collect()
}

fn capture(app: &App, label: &str, out: &mut String) {
    let live_h = app.live_height(WIDTH).min(HEIGHT);
    let scrollback_h = HEIGHT - live_h;
    let sb = &app.scrollback;
    let start = sb.len().saturating_sub(scrollback_h as usize);

    let backend = TestBackend::new(WIDTH, live_h);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            app.render(frame, area);
        })
        .expect("draw live area");
    let live_lines = buffer_lines(terminal.backend().buffer());

    out.push_str(&format!("=== {label} ===\n"));
    for l in &sb[start..] {
        out.push_str(&plain_line(l));
        out.push('\n');
    }
    for l in live_lines {
        out.push_str(&l);
        out.push('\n');
    }
}

pub fn run() {
    let (etx, erx) = mpsc::channel::<EngineEvent>();
    let (utx, urx) = mpsc::channel::<UiEvent>();
    thread::spawn(move || scenario::run(etx, urx, true));

    let mut app = App::new();
    let mut out = String::new();
    let mut scene_no = 0usize;

    for (i, action) in SCRIPT.iter().enumerate() {
        match action {
            Action::Line(text) => {
                app.input.buffer = text.to_string();
                if let Some(ev) = app.submit_buffer() {
                    let _ = utx.send(ev);
                }
            }
            Action::LineWithAbort(text) => {
                app.input.buffer = text.to_string();
                if let Some(ev) = app.submit_buffer() {
                    let _ = utx.send(ev);
                }
                thread::sleep(Duration::from_millis(20));
                let _ = utx.send(UiEvent::Abort);
            }
            Action::AskAnswer(key) => {
                app.ask_select(*key);
                if let Some(ev) = app.ask_confirm() {
                    let _ = utx.send(ev);
                }
            }
            Action::ModelPick(id) => {
                app.input.buffer = "/model".to_string();
                let _ = app.submit_buffer();
                capture(&app, &format!("(mid-scene, action {i}, picker open)"), &mut out);
                if let Some(ev) = app.picker_confirm_by_id(id) {
                    let _ = utx.send(ev);
                }
            }
            Action::Confirm(line) => {
                if let Some(ev) = app.confirm_submit_with(line) {
                    let _ = utx.send(ev);
                }
            }
        }
        drain_quiet(&mut app, &erx);

        if SCENE_BOUNDARY_AFTER.contains(&i) {
            scene_no += 1;
            capture(&app, &format!("Scene {scene_no}"), &mut out);
        } else {
            capture(&app, &format!("(mid-scene, action {i})"), &mut out);
        }
    }

    print!("{out}");
}
