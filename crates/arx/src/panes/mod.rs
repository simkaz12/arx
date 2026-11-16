mod app;
pub(crate) mod clipboard;
mod keys;
pub(crate) mod palette;
mod state;
mod trace;
mod ui;

use std::io::{self, stdout};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event as CEvent, KeyEventKind,
    MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal as RatatuiTerminal;

use crate::events::Event;
use crate::terminal::{Attachment, Control, Front, OperatorMsg, Poll, Terminal, TerminalMsg};

use app::{App, PANEL_ORDER};
use palette::Palette;

const TICK: Duration = Duration::from_millis(33);

enum Incoming {
    Term(TerminalMsg),
    Evt(Event),
    WantLine,
}

enum UiOut {
    Line(String),
    Turn { text: String, images: Vec<Attachment> },
    Eof,
    Failed(String),
}

pub struct PanesFront {
    render_tx: Option<Sender<Incoming>>,
    poll_rx: Receiver<UiOut>,
    answer_rx: Receiver<UiOut>,
    control_rx: Receiver<Control>,
    ui_thread: Option<JoinHandle<()>>,
    use_fallback: bool,
    fallback: Option<Terminal<io::Stdout>>,
    terminal_outcome: Option<Result<(), String>>,
}

impl PanesFront {
    pub fn new(keys_path: Option<&Path>, palette: Palette) -> io::Result<Self> {
        let (render_tx, render_rx) = mpsc::channel::<Incoming>();
        let (poll_tx, poll_rx) = mpsc::channel::<UiOut>();
        let (answer_tx, answer_rx) = mpsc::channel::<UiOut>();
        let (control_tx, control_rx) = mpsc::channel::<Control>();
        let keys_path = keys_path.map(Path::to_path_buf);
        let ui_thread = thread::Builder::new()
            .name("arx-panes-ui".to_string())
            .spawn(move || run_ui(render_rx, poll_tx, answer_tx, control_tx, keys_path, palette))
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        Ok(PanesFront {
            render_tx: Some(render_tx),
            poll_rx,
            answer_rx,
            control_rx,
            ui_thread: Some(ui_thread),
            use_fallback: false,
            fallback: None,
            terminal_outcome: None,
        })
    }

    fn fallback_render(&mut self, msg: &TerminalMsg) -> io::Result<()> {
        self.fallback.get_or_insert_with(|| Terminal::new(io::stdout())).render(msg)
    }

    /// Shared by `read` and `read_answer`: both lanes carry the same `UiOut` shape and
    /// report the same terminal outcome once the UI thread ends, so one poll drives either.
    fn poll_lane(rx: &Receiver<UiOut>, wait: Option<Duration>, terminal_outcome: &mut Option<Result<(), String>>) -> io::Result<Poll> {
        if let Some(outcome) = terminal_outcome {
            return match outcome {
                Ok(()) => Ok(Poll::Eof),
                Err(reason) => Err(io::Error::new(io::ErrorKind::Other, reason.clone())),
            };
        }
        let outcome = match wait {
            Some(duration) => rx.recv_timeout(duration),
            None => rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };
        match outcome {
            Ok(UiOut::Line(line)) => Ok(Poll::Line(OperatorMsg::Line(line))),
            Ok(UiOut::Turn { text, images }) => Ok(Poll::Line(OperatorMsg::Turn { text, images })),
            Ok(UiOut::Eof) => {
                *terminal_outcome = Some(Ok(()));
                Ok(Poll::Eof)
            }
            Ok(UiOut::Failed(reason)) => {
                *terminal_outcome = Some(Err(reason.clone()));
                Err(io::Error::new(io::ErrorKind::Other, reason))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(Poll::Idle),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let reason = "panes UI thread ended without a clean quit or a reported error".to_string();
                *terminal_outcome = Some(Err(reason.clone()));
                Err(io::Error::new(io::ErrorKind::Other, reason))
            }
        }
    }
}

impl Front for PanesFront {
    fn render(&mut self, msg: &TerminalMsg) -> io::Result<()> {
        if self.use_fallback {
            return self.fallback_render(msg);
        }
        let sent = self.render_tx.as_ref().map(|tx| tx.send(Incoming::Term(msg.clone())).is_ok()).unwrap_or(false);
        if !sent {
            self.use_fallback = true;
            return self.fallback_render(msg);
        }
        Ok(())
    }

    fn event(&mut self, event: &Event) -> io::Result<()> {
        if self.use_fallback {
            return Ok(());
        }
        let sent = self.render_tx.as_ref().map(|tx| tx.send(Incoming::Evt(event.clone())).is_ok()).unwrap_or(false);
        if !sent {
            self.use_fallback = true;
        }
        Ok(())
    }

    fn read(&mut self, wait: Option<Duration>) -> io::Result<Poll> {
        if let Some(tx) = &self.render_tx {
            let _ = tx.send(Incoming::WantLine);
        }
        Self::poll_lane(&self.poll_rx, wait, &mut self.terminal_outcome)
    }

    fn read_answer(&mut self, wait: Option<Duration>) -> io::Result<Poll> {
        Self::poll_lane(&self.answer_rx, wait, &mut self.terminal_outcome)
    }

    fn poll_control(&mut self) -> Option<Control> {
        self.control_rx.try_recv().ok()
    }
}

impl Drop for PanesFront {
    fn drop(&mut self) {
        self.render_tx = None;
        if let Some(handle) = self.ui_thread.take() {
            let _ = handle.join();
        }
    }
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture, DisableBracketedPaste);
        default_hook(info);
    }));
}

fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

fn run_ui(
    render_rx: Receiver<Incoming>,
    poll_tx: Sender<UiOut>,
    answer_tx: Sender<UiOut>,
    control_tx: Sender<Control>,
    keys_path: Option<PathBuf>,
    palette: Palette,
) {
    install_panic_hook();
    let result = run_ui_inner(&render_rx, &poll_tx, &answer_tx, &control_tx, keys_path.as_deref(), palette);
    drop(render_rx);
    match result {
        Ok(()) => {
            let _ = poll_tx.send(UiOut::Eof);
            let _ = answer_tx.send(UiOut::Eof);
        }
        Err(err) => {
            let _ = disable_raw_mode();
            let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
            eprintln!("error: {err}");
            let _ = poll_tx.send(UiOut::Failed(err.to_string()));
            let _ = answer_tx.send(UiOut::Failed(err.to_string()));
        }
    }
}

fn run_ui_inner(
    render_rx: &Receiver<Incoming>,
    poll_tx: &Sender<UiOut>,
    answer_tx: &Sender<UiOut>,
    control_tx: &Sender<Control>,
    keys_path: Option<&Path>,
    palette: Palette,
) -> io::Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = RatatuiTerminal::new(backend)?;

    let mut app = App::new(keys_path).with_palette(palette);

    let result = (|| -> io::Result<()> {
        loop {
            drain_incoming(&mut app, render_rx);
            drain_outputs(&mut app, poll_tx, answer_tx, control_tx);
            terminal.draw(|f| ui::draw(f, &mut app))?;
            if app.quit {
                break;
            }
            if event::poll(TICK)? {
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
                    CEvent::Paste(text) => app.handle_paste(&text),
                    _ => {}
                }
            }
            drain_outputs(&mut app, poll_tx, answer_tx, control_tx);
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture, DisableBracketedPaste)?;
    terminal.show_cursor()?;
    result
}

fn drain_incoming(app: &mut App, rx: &Receiver<Incoming>) {
    loop {
        match rx.try_recv() {
            Ok(Incoming::Term(msg)) => app.apply_terminal(msg),
            Ok(Incoming::Evt(ev)) => app.apply_event(ev),
            Ok(Incoming::WantLine) => app.release_queued_if_idle(),
            Err(mpsc::TryRecvError::Empty) => break,
            Err(mpsc::TryRecvError::Disconnected) => {
                app.quit = true;
                break;
            }
        }
    }
}

fn drain_outputs(app: &mut App, poll_tx: &Sender<UiOut>, answer_tx: &Sender<UiOut>, control_tx: &Sender<Control>) {
    while let Some(line) = app.outbox.pop_front() {
        let images = app.pop_outbox_images();
        let out = if images.is_empty() { UiOut::Line(line) } else { UiOut::Turn { text: line, images } };
        if poll_tx.send(out).is_err() {
            app.quit = true;
        }
    }
    while let Some(line) = app.answer_outbox.pop_front() {
        if answer_tx.send(UiOut::Line(line)).is_err() {
            app.quit = true;
        }
    }
    while let Some(control) = app.control.pop_front() {
        let _ = control_tx.send(control);
    }
}

#[cfg(test)]
impl PanesFront {
    fn for_test(poll_rx: Receiver<UiOut>) -> Self {
        let (render_tx, _render_rx) = mpsc::channel();
        let (_answer_tx, answer_rx) = mpsc::channel();
        let (_control_tx, control_rx) = mpsc::channel();
        PanesFront {
            render_tx: Some(render_tx),
            poll_rx,
            answer_rx,
            control_rx,
            ui_thread: None,
            use_fallback: false,
            fallback: None,
            terminal_outcome: None,
        }
    }

    fn for_test_with_answer_channel(answer_rx: Receiver<UiOut>) -> Self {
        let (render_tx, _render_rx) = mpsc::channel();
        let (_poll_tx, poll_rx) = mpsc::channel();
        let (_control_tx, control_rx) = mpsc::channel();
        PanesFront {
            render_tx: Some(render_tx),
            poll_rx,
            answer_rx,
            control_rx,
            ui_thread: None,
            use_fallback: false,
            fallback: None,
            terminal_outcome: None,
        }
    }

    fn for_test_with_control_channel(control_rx: Receiver<Control>) -> Self {
        let (render_tx, _render_rx) = mpsc::channel();
        let (_poll_tx, poll_rx) = mpsc::channel();
        let (_answer_tx, answer_rx) = mpsc::channel();
        PanesFront {
            render_tx: Some(render_tx),
            poll_rx,
            answer_rx,
            control_rx,
            ui_thread: None,
            use_fallback: false,
            fallback: None,
            terminal_outcome: None,
        }
    }

    fn for_test_with_dead_render_channel(poll_rx: Receiver<UiOut>) -> Self {
        let (render_tx, render_rx) = mpsc::channel();
        drop(render_rx);
        let (_answer_tx, answer_rx) = mpsc::channel();
        let (_control_tx, control_rx) = mpsc::channel();
        PanesFront {
            render_tx: Some(render_tx),
            poll_rx,
            answer_rx,
            control_rx,
            ui_thread: None,
            use_fallback: false,
            fallback: None,
            terminal_outcome: None,
        }
    }

    fn for_test_with_live_render_channel(poll_rx: Receiver<UiOut>) -> (Self, Receiver<Incoming>) {
        let (render_tx, render_rx) = mpsc::channel();
        let (_answer_tx, answer_rx) = mpsc::channel();
        let (_control_tx, control_rx) = mpsc::channel();
        let front = PanesFront {
            render_tx: Some(render_tx),
            poll_rx,
            answer_rx,
            control_rx,
            ui_thread: None,
            use_fallback: false,
            fallback: None,
            terminal_outcome: None,
        };
        (front, render_rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_answers_idle_on_timeout_and_eof_after_a_clean_quit_closes_the_channel() {
        let (poll_tx, poll_rx) = mpsc::channel();
        let mut front = PanesFront::for_test(poll_rx);

        match front.read(Some(Duration::from_millis(20))).unwrap() {
            Poll::Idle => {}
            _ => panic!("expected Idle on timeout"),
        }

        poll_tx.send(UiOut::Eof).unwrap();
        drop(poll_tx);
        match front.read(Some(Duration::from_millis(20))).unwrap() {
            Poll::Eof => {}
            _ => panic!("expected Eof after a clean quit closed the channel"),
        }
    }

    #[test]
    fn read_errors_when_the_ui_thread_disappears_without_a_clean_quit_or_a_reported_failure() {
        let (poll_tx, poll_rx) = mpsc::channel();
        let mut front = PanesFront::for_test(poll_rx);
        drop(poll_tx);

        let err = front.read(Some(Duration::from_millis(20))).unwrap_err();
        assert!(err.to_string().contains("without a clean quit"), "err: {err}");
    }

    #[test]
    fn read_errors_when_the_ui_thread_reports_a_failure() {
        let (poll_tx, poll_rx) = mpsc::channel();
        let mut front = PanesFront::for_test(poll_rx);
        poll_tx.send(UiOut::Failed("raw mode setup failed".to_string())).unwrap();

        let err = front.read(Some(Duration::from_millis(20))).unwrap_err();
        assert!(err.to_string().contains("raw mode setup failed"), "err: {err}");
    }

    #[test]
    fn a_render_that_finds_the_channel_already_dead_still_lets_read_report_the_real_failure() {
        let (poll_tx, poll_rx) = mpsc::channel();
        let mut front = PanesFront::for_test_with_dead_render_channel(poll_rx);

        front.render(&TerminalMsg::Notice { text: "hello".to_string() }).unwrap();
        assert!(front.use_fallback);

        poll_tx.send(UiOut::Failed("raw mode setup failed".to_string())).unwrap();
        let err = front.read(Some(Duration::from_millis(20))).unwrap_err();
        assert!(err.to_string().contains("raw mode setup failed"), "err: {err}");
    }

    #[test]
    fn a_render_that_finds_the_channel_already_dead_still_reports_eof_on_a_clean_quit() {
        let (poll_tx, poll_rx) = mpsc::channel();
        let mut front = PanesFront::for_test_with_dead_render_channel(poll_rx);

        front.render(&TerminalMsg::Notice { text: "hello".to_string() }).unwrap();
        assert!(front.use_fallback);

        poll_tx.send(UiOut::Eof).unwrap();
        match front.read(Some(Duration::from_millis(20))).unwrap() {
            Poll::Eof => {}
            _ => panic!("expected Eof"),
        }
    }

    #[test]
    fn a_render_after_eof_falls_back_instead_of_vanishing_into_a_dead_ui_thread() {
        let (poll_tx, poll_rx) = mpsc::channel();
        let (mut front, render_rx) = PanesFront::for_test_with_live_render_channel(poll_rx);

        drop(render_rx);
        poll_tx.send(UiOut::Eof).unwrap();

        match front.read(Some(Duration::from_millis(20))).unwrap() {
            Poll::Eof => {}
            _ => panic!("expected Eof"),
        }

        front.render(&TerminalMsg::Notice { text: "delivery notice".to_string() }).unwrap();
        assert!(front.use_fallback, "a post-Eof render must fall back, not vanish into a dead channel");
    }

    #[test]
    fn read_returns_a_line_the_ui_thread_submitted() {
        let (poll_tx, poll_rx) = mpsc::channel();
        let mut front = PanesFront::for_test(poll_rx);
        poll_tx.send(UiOut::Line("hello".to_string())).unwrap();

        match front.read(Some(Duration::from_millis(20))).unwrap() {
            Poll::Line(OperatorMsg::Line(line)) => assert_eq!(line, "hello"),
            _ => panic!("expected a submitted line"),
        }
    }
}
