use std::io::{BufRead, Write};
use std::time::Duration;

use crate::events::Event;

pub use crate::main_body::{Attachment, Control};

/// One `[[models]]` entry as it appears in the roster the operator picks from —
/// carried whole (id, wire-format body, family) so neither front needs the
/// provider table to render it.
#[derive(Debug, Clone, PartialEq)]
pub struct RosterModel {
    pub id: String,
    pub body: String,
    pub family: String,
}

/// One position's row in the roster: where it falls back to, what it is bound
/// to right now, the models this position accepts, and the cursor a picker
/// opens on (0 = the `from file` row, `i` = `model_ids[i - 1]`).
#[derive(Debug, Clone, PartialEq)]
pub struct RosterTab {
    pub position: String,
    pub fallback_id: String,
    pub effective_id: String,
    pub effective_body: String,
    pub pinned: bool,
    pub model_ids: Vec<String>,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub enum TerminalMsg {
    Chunk { text: String },
    TurnEnd { text: String },
    Notice { text: String },
    ToolCall { tool: String, subject: Option<String> },
    Usage { input: u64, output: u64, cache_creation: u64, cache_read: u64 },
    Prompt,
    Confirm { text: String },
    Roster { models: Vec<RosterModel>, tabs: Vec<RosterTab>, main_family: String },
    Status {
        model: String,
        occupancy: u64,
        window: u64,
        reserve: u64,
        nudge: u64,
        checkpoint: Option<(u64, u64)>,
        turn: u64,
    },
    HandSpawned {
        hand: String,
        agent: String,
        role: String,
        body: String,
        image: String,
        model: String,
        brief: String,
    },
    HandStopped {
        hand: String,
        question: String,
    },
    HandDelivered {
        hand: String,
        outcome: String,
        text: Option<String>,
        trace: Option<String>,
        bytes: Option<u64>,
        elapsed_ms: u64,
        lag_ms: u64,
        schema: Option<String>,
        diff: Option<String>,
    },
    Stopped,
}

fn render_roster(tabs: &[RosterTab]) -> String {
    let mut text = "[arx] model: roster".to_string();
    for tab in tabs {
        let source = if tab.pinned { "pinned" } else { "from file" };
        text.push('\n');
        text.push_str(&format!("{}: {} ({}) — {source}", tab.position, tab.effective_id, tab.effective_body));
    }
    text
}

pub trait Front {
    fn render(&mut self, msg: &TerminalMsg) -> std::io::Result<()>;
    fn event(&mut self, event: &Event) -> std::io::Result<()>;
    fn read(&mut self, wait: Option<Duration>) -> std::io::Result<Poll>;

    /// The gate-ask lane: `gate_call`'s answer loop calls this and nothing else. The
    /// default answers exactly as `read` does, which is what a line-input front wants —
    /// piped mode has no separate answer channel.
    fn read_answer(&mut self, wait: Option<Duration>) -> std::io::Result<Poll> {
        self.read(wait)
    }

    /// The control lane a running turn polls between reads, non-blocking. The default
    /// never signals a stop — a line-input front has no key to send one from.
    fn poll_control(&mut self) -> Option<Control> {
        None
    }
}

#[derive(Debug)]
pub enum Poll {
    Line(OperatorMsg),
    Eof,
    Idle,
}

#[derive(Debug, PartialEq)]
pub enum OperatorMsg {
    Line(String),
    Turn { text: String, images: Vec<Attachment> },
}

pub struct Terminal<W: Write> {
    writer: W,
    shown: String,
}

impl<W: Write> Terminal<W> {
    pub fn new(writer: W) -> Self {
        Terminal {
            writer,
            shown: String::new(),
        }
    }

    pub fn render(&mut self, msg: &TerminalMsg) -> std::io::Result<()> {
        match msg {
            TerminalMsg::Chunk { text } => {
                write!(self.writer, "{text}")?;
                self.writer.flush()?;
                self.shown.push_str(text);
            }
            TerminalMsg::TurnEnd { text } => {
                if self.shown == *text {
                    writeln!(self.writer)?;
                } else if self.shown.is_empty() {
                    writeln!(self.writer, "{text}")?;
                    writeln!(self.writer)?;
                } else {
                    writeln!(self.writer)?;
                    writeln!(self.writer, "{text}")?;
                    writeln!(self.writer)?;
                }
                self.shown.clear();
            }
            TerminalMsg::Notice { text } => {
                writeln!(self.writer, "{text}")?;
            }
            TerminalMsg::Prompt => {
                write!(self.writer, "> ")?;
                let _ = self.writer.flush();
            }
            TerminalMsg::Confirm { text } => {
                writeln!(self.writer, "{text}")?;
            }
            TerminalMsg::Roster { tabs, .. } => {
                let text = render_roster(tabs);
                writeln!(self.writer, "{text}")?;
            }
            TerminalMsg::Status { .. }
            | TerminalMsg::HandSpawned { .. }
            | TerminalMsg::HandStopped { .. }
            | TerminalMsg::HandDelivered { .. }
            | TerminalMsg::ToolCall { .. }
            | TerminalMsg::Usage { .. }
            | TerminalMsg::Stopped => {}
        }
        Ok(())
    }
}

pub struct LineFront<W: Write, R: BufRead> {
    terminal: Terminal<W>,
    reader: R,
    tty: bool,
}

impl<W: Write, R: BufRead> LineFront<W, R> {
    pub fn new(writer: W, reader: R, tty: bool) -> Self {
        LineFront {
            terminal: Terminal::new(writer),
            reader,
            tty,
        }
    }
}

impl<W: Write, R: BufRead> Front for LineFront<W, R> {
    fn render(&mut self, msg: &TerminalMsg) -> std::io::Result<()> {
        if matches!(msg, TerminalMsg::Prompt) && !self.tty {
            return Ok(());
        }
        self.terminal.render(msg)
    }

    fn event(&mut self, _event: &Event) -> std::io::Result<()> {
        Ok(())
    }

    fn read(&mut self, _wait: Option<Duration>) -> std::io::Result<Poll> {
        match read_operator(&mut self.reader)? {
            Some(msg) => Ok(Poll::Line(msg)),
            None => Ok(Poll::Eof),
        }
    }
}

pub fn read_operator(input: &mut impl BufRead) -> std::io::Result<Option<OperatorMsg>> {
    let mut line = String::new();
    let bytes_read = input.read_line(&mut line)?;
    if bytes_read == 0 {
        return Ok(None);
    }
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    Ok(Some(OperatorMsg::Line(line)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn chunks_matching_canonical_close_with_separator_only() {
        let mut terminal = Terminal::new(Vec::new());
        terminal
            .render(&TerminalMsg::Chunk {
                text: "stub:".to_string(),
            })
            .unwrap();
        terminal
            .render(&TerminalMsg::Chunk {
                text: "hello".to_string(),
            })
            .unwrap();
        terminal
            .render(&TerminalMsg::TurnEnd {
                text: "stub:hello".to_string(),
            })
            .unwrap();

        let output = String::from_utf8(terminal.writer).unwrap();
        assert_eq!(output, "stub:hello\n");
        assert_eq!(output.matches("stub:hello").count(), 1);
    }

    #[test]
    fn turn_end_without_chunks_prints_canonical() {
        let mut terminal = Terminal::new(Vec::new());
        terminal
            .render(&TerminalMsg::TurnEnd {
                text: "stub:hello".to_string(),
            })
            .unwrap();

        let output = String::from_utf8(terminal.writer).unwrap();
        assert_eq!(output, "stub:hello\n\n");
    }

    #[test]
    fn mismatched_chunks_are_reconciled_to_canonical() {
        let mut terminal = Terminal::new(Vec::new());
        terminal
            .render(&TerminalMsg::Chunk {
                text: "stub:htesting".to_string(),
            })
            .unwrap();
        terminal
            .render(&TerminalMsg::TurnEnd {
                text: "stub:hello".to_string(),
            })
            .unwrap();

        let output = String::from_utf8(terminal.writer).unwrap();
        assert_eq!(output, "stub:htesting\nstub:hello\n\n");
    }

    #[test]
    fn a_second_message_kind_renders_through_the_same_channel() {
        let mut terminal = Terminal::new(Vec::new());
        terminal
            .render(&TerminalMsg::Notice {
                text: "reset armed".to_string(),
            })
            .unwrap();

        let output = String::from_utf8(terminal.writer).unwrap();
        assert_eq!(output, "reset armed\n");
    }

    #[test]
    fn confirm_renders_exactly_as_the_notice_it_replaces() {
        let text = "[arx] state object 1 above. ok to tear down and respawn; any other line rides into the new main's first turn as an operator note.".to_string();

        let mut notice_terminal = Terminal::new(Vec::new());
        notice_terminal.render(&TerminalMsg::Notice { text: text.clone() }).unwrap();
        let notice_output = String::from_utf8(notice_terminal.writer).unwrap();

        let mut confirm_terminal = Terminal::new(Vec::new());
        confirm_terminal.render(&TerminalMsg::Confirm { text }).unwrap();
        let confirm_output = String::from_utf8(confirm_terminal.writer).unwrap();

        assert_eq!(confirm_output, notice_output);
    }

    #[test]
    fn roster_renders_exactly_as_the_notice_it_replaces() {
        let tabs = vec![
            RosterTab {
                position: "main".to_string(),
                fallback_id: "stub-model".to_string(),
                effective_id: "stub-model".to_string(),
                effective_body: "claude-code".to_string(),
                pinned: false,
                model_ids: vec!["stub-model".to_string()],
                cursor: 0,
            },
            RosterTab {
                position: "hand-ex".to_string(),
                fallback_id: "stub-model".to_string(),
                effective_id: "stub-model-2".to_string(),
                effective_body: "codex".to_string(),
                pinned: true,
                model_ids: vec!["stub-model-2".to_string()],
                cursor: 1,
            },
        ];
        let text = render_roster(&tabs);

        let mut notice_terminal = Terminal::new(Vec::new());
        notice_terminal.render(&TerminalMsg::Notice { text: text.clone() }).unwrap();
        let notice_output = String::from_utf8(notice_terminal.writer).unwrap();
        assert_eq!(
            notice_output,
            "[arx] model: roster\nmain: stub-model (claude-code) — from file\nhand-ex: stub-model-2 (codex) — pinned\n"
        );

        let mut roster_terminal = Terminal::new(Vec::new());
        roster_terminal
            .render(&TerminalMsg::Roster { models: Vec::new(), tabs, main_family: "claude".to_string() })
            .unwrap();
        let roster_output = String::from_utf8(roster_terminal.writer).unwrap();

        assert_eq!(roster_output, notice_output);
    }

    #[test]
    fn status_renders_nothing() {
        let mut terminal = Terminal::new(Vec::new());
        terminal
            .render(&TerminalMsg::Status {
                model: "claude-sonnet-5".to_string(),
                occupancy: 1000,
                window: 200_000,
                reserve: 16_000,
                nudge: 120_000,
                checkpoint: Some((2, 500)),
                turn: 4,
            })
            .unwrap();

        let output = String::from_utf8(terminal.writer).unwrap();
        assert_eq!(output, "");
    }

    #[test]
    fn hand_spawned_stopped_and_delivered_render_nothing() {
        let mut terminal = Terminal::new(Vec::new());
        terminal
            .render(&TerminalMsg::HandSpawned {
                hand: "h1".to_string(),
                agent: "hand-do".to_string(),
                role: "hand".to_string(),
                body: "claude-code".to_string(),
                image: "arx-hand-claude:2.1.258".to_string(),
                model: "claude-sonnet-5".to_string(),
                brief: "do the thing".to_string(),
            })
            .unwrap();
        terminal
            .render(&TerminalMsg::HandStopped {
                hand: "h1".to_string(),
                question: "which db?".to_string(),
            })
            .unwrap();
        terminal
            .render(&TerminalMsg::HandDelivered {
                hand: "h1".to_string(),
                outcome: "verdict".to_string(),
                text: Some("done".to_string()),
                trace: Some("threads/main.h1".to_string()),
                bytes: Some(512),
                elapsed_ms: 4200,
                lag_ms: 30,
                schema: None,
                diff: None,
            })
            .unwrap();

        let output = String::from_utf8(terminal.writer).unwrap();
        assert_eq!(output, "");
    }

    #[test]
    fn usage_renders_nothing() {
        let mut terminal = Terminal::new(Vec::new());
        terminal
            .render(&TerminalMsg::Usage { input: 100, output: 20, cache_creation: 0, cache_read: 5 })
            .unwrap();

        let output = String::from_utf8(terminal.writer).unwrap();
        assert_eq!(output, "");
    }

    #[test]
    fn stopped_renders_nothing() {
        let mut terminal = Terminal::new(Vec::new());
        terminal.render(&TerminalMsg::Stopped).unwrap();

        let output = String::from_utf8(terminal.writer).unwrap();
        assert_eq!(output, "");
    }

    #[test]
    fn line_front_s_read_answer_and_poll_control_use_the_trait_defaults() {
        let mut front = LineFront::new(Vec::new(), Cursor::new(b"hello\n".to_vec()), false);
        assert!(front.poll_control().is_none());
        match front.read_answer(None).unwrap() {
            Poll::Line(OperatorMsg::Line(line)) => assert_eq!(line, "hello"),
            other => panic!("expected the same line read gives: {other:?}"),
        }
    }

    #[test]
    fn read_operator_returns_line_then_none_at_eof() {
        let mut input = Cursor::new(b"hello\n".to_vec());

        let first = read_operator(&mut input).unwrap();
        assert_eq!(first, Some(OperatorMsg::Line("hello".to_string())));

        let second = read_operator(&mut input).unwrap();
        assert_eq!(second, None);
    }
}
