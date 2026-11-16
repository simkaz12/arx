use std::io::{self, BufRead, Write};
use std::sync::mpsc::Sender;

use crate::driver;
use crate::format::{ask_subject_display, model_list_line};
use crate::scenario::{self, Event, ModelListReason, OperatorInput};

pub fn run(auto: bool, fast: bool) {
    let (event_rx, operator_tx) = scenario::spawn(fast);
    let forward = if auto {
        Some(driver::spawn_auto_driver(fast, false, operator_tx))
    } else {
        spawn_stdin_forwarder(operator_tx);
        None
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut shown = String::new();

    while let Ok(event) = event_rx.recv() {
        render(&mut out, &mut shown, &event);
        if let Some(fwd) = &forward {
            if fwd.send(event).is_err() {
                break;
            }
        }
    }
}

fn spawn_stdin_forwarder(tx: Sender<OperatorInput>) {
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => {
                    if tx.send(OperatorInput::Line(l)).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn render(out: &mut impl Write, shown: &mut String, event: &Event) {
    match event {
        Event::Chunk(text) => {
            let _ = write!(out, "{text}");
            let _ = out.flush();
            shown.push_str(text);
        }
        Event::TurnEnd(text) => {
            if shown == text {
                let _ = writeln!(out);
            } else if shown.is_empty() {
                let _ = writeln!(out, "{text}");
                let _ = writeln!(out);
            } else {
                let _ = writeln!(out);
                let _ = writeln!(out, "{text}");
                let _ = writeln!(out);
            }
            shown.clear();
        }
        Event::GateAsk { tool, subject } => {
            let _ = writeln!(out, "[arx] ask: {tool} {}", ask_subject_display(subject));
            let _ = write!(
                out,
                "a allow once · A allow always · d deny once · D deny always · a note may follow the key: "
            );
            let _ = out.flush();
        }
        Event::Notice { text, .. } => {
            let _ = writeln!(out, "{text}");
        }
        Event::ResetArmed { occupancy, window, reserve } => {
            let _ = writeln!(out, "window at {occupancy} of {window}, reserve {reserve}; /reset to reset now");
        }
        Event::ResetConfirm { body } => {
            for line in body.split('\n') {
                let _ = writeln!(out, "  {line}");
            }
            let _ = writeln!(
                out,
                "ok to tear down and respawn; any other line rides into the new main's first turn as an operator note"
            );
        }
        Event::Checkpoint { number, path, delta } => {
            let previous = number.saturating_sub(1);
            let _ = writeln!(out, "checkpoint {number} written: {path}, delta +{delta} since checkpoint {previous}");
        }
        Event::ModelList { models, current, reason, attempted } => {
            match reason {
                ModelListReason::Unlisted => {
                    let id = attempted.clone().unwrap_or_default();
                    let _ = writeln!(out, "{id} is not a known model");
                }
                ModelListReason::Requested => {}
            }
            let _ = writeln!(out, "models: {}", model_list_line(&models, &current));
        }
        Event::ModelSwitched { from, to } => {
            let _ = writeln!(out, "model: {from} -> {to}");
        }
        Event::Orientation { text, verdict, .. } => {
            if shown == text {
                let _ = writeln!(out);
            } else if shown.is_empty() {
                let _ = writeln!(out, "{text}");
                let _ = writeln!(out);
            } else {
                let _ = writeln!(out);
                let _ = writeln!(out, "{text}");
                let _ = writeln!(out);
            }
            shown.clear();
            let _ = writeln!(out, "orientation: {verdict}");
        }
        Event::ProviderError { message, recoverable } => {
            if *recoverable {
                let _ = writeln!(out, "{message}; retrying");
            } else {
                let _ = writeln!(out, "error: {message}");
            }
        }
        Event::Aborted => {
            if !shown.is_empty() {
                let _ = writeln!(out);
                shown.clear();
            }
            let _ = writeln!(out, "(aborted)");
        }
        Event::ToolCall { .. }
        | Event::ToolResult { .. }
        | Event::GateVerdict { .. }
        | Event::Spawn { .. }
        | Event::Delivered { .. }
        | Event::Usage { .. } => {}
    }
}
