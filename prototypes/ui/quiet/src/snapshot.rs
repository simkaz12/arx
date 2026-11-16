use std::sync::mpsc::{Receiver, Sender};

use crate::format::{admission_lines, ask_subject_display, dispatch_line, delivery_line, markdown_lite, model_list_line, role_label, strip_ansi, tool_call_line};
use crate::scenario::{self, Admission, Event, ModelListReason, NoticeLevel, OperatorInput};
use crate::state::SessionState;

const WIDTH: usize = 100;

struct Pending {
    tool: Option<(String, String)>,
    from_ask: bool,
    chunk_buf: String,
}

pub fn run(fast: bool) {
    let (rx, tx) = scenario::spawn(fast);
    let mut state = SessionState::new();
    let mut pending = Pending { tool: None, from_ask: false, chunk_buf: String::new() };

    println!("=== boot ===");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::Notice { .. }));

    scene(&rx, &tx, &mut state, &mut pending, 1, "a plain reply", "What changed with the store root?", |e| matches!(e, Event::Usage { .. }));

    scene(&rx, &tx, &mut state, &mut pending, 2, "tool calls, an ask, admission", "Can you check what still pins down the piped shape?", |_| false);
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::GateAsk { .. }));
    send(&tx, "A polishing pass gate");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::Usage { .. }));

    println!();
    println!("=== Scene 3: dispatch, judge, delivery while idle ===");
    send(&tx, "Go ahead and dispatch someone to clean that up.");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::TurnEnd(_)));
    println!("  (operator idle; deliveries land unprompted)");
    drain_until_n(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::Delivered { .. }), 2);
    send(&tx, "ok");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::TurnEnd(_)));

    println!();
    println!("=== Scene 4: /model picker, direct switch, unlisted, already ===");
    send(&tx, "/model gpt-5");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::ModelList { .. }));
    send(&tx, "/model");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::ModelList { .. }));
    send(&tx, "/model claude-sonnet-5");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| {
        matches!(e, Event::ModelSwitched { .. }) || matches!(e, Event::Notice { .. })
    });

    println!();
    println!("=== Scene 5: nudge, checkpoint, reset ===");
    send(&tx, "How's context usage looking?");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::ResetArmed { .. }));
    send(&tx, "/checkpoint");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::Checkpoint { .. }));
    send(&tx, "/reset");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::ResetConfirm { .. }));
    send(&tx, "ok");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::Orientation { .. }));

    println!();
    println!("=== Scene 6: abort and a recoverable error ===");
    send(&tx, "Tell me more about the polishing backlog.");
    let mut chunk_count = 0u32;
    loop {
        match rx.recv() {
            Ok(e) => {
                render(&e, &mut state, &mut pending);
                if let Event::Chunk(_) = e {
                    chunk_count += 1;
                    if chunk_count == 3 {
                        println!("  (esc pressed)");
                        let _ = tx.send(OperatorInput::Abort);
                    }
                }
                if matches!(e, Event::Aborted) || matches!(e, Event::TurnEnd(_)) {
                    break;
                }
            }
            Err(_) => return,
        }
    }
    send(&tx, "Try again?");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::TurnEnd(_)));

    println!();
    println!("=== Scene 7: an unknown command ===");
    send(&tx, "/reste");
    drain_until(&rx, &tx, &mut state, &mut pending, |e| matches!(e, Event::Notice { .. }));
}

fn scene(
    rx: &Receiver<Event>,
    tx: &Sender<OperatorInput>,
    state: &mut SessionState,
    pending: &mut Pending,
    n: u32,
    label: &str,
    line: &str,
    stop: impl Fn(&Event) -> bool,
) {
    println!();
    println!("=== Scene {n}: {label} ===");
    send(tx, line);
    if n != 2 {
        drain_until(rx, tx, state, pending, stop);
    }
}

fn send(tx: &Sender<OperatorInput>, line: &str) {
    println!("› {line}");
    let _ = tx.send(OperatorInput::Line(line.to_string()));
}

fn drain_until(rx: &Receiver<Event>, _tx: &Sender<OperatorInput>, state: &mut SessionState, pending: &mut Pending, stop: impl Fn(&Event) -> bool) {
    loop {
        match rx.recv() {
            Ok(e) => {
                let done = stop(&e);
                render(&e, state, pending);
                if done {
                    return;
                }
            }
            Err(_) => return,
        }
    }
}

fn drain_until_n(rx: &Receiver<Event>, _tx: &Sender<OperatorInput>, state: &mut SessionState, pending: &mut Pending, stop: impl Fn(&Event) -> bool, n: u32) {
    let mut seen = 0;
    while seen < n {
        match rx.recv() {
            Ok(e) => {
                if stop(&e) {
                    seen += 1;
                }
                render(&e, state, pending);
            }
            Err(_) => return,
        }
    }
}

fn render(event: &Event, state: &mut SessionState, pending: &mut Pending) {
    match event {
        Event::Chunk(text) => {
            pending.chunk_buf.push_str(text);
        }
        Event::TurnEnd(text) => {
            pending.chunk_buf.clear();
            for line in markdown_lite(text, WIDTH) {
                println!("  {}", strip_ansi(&line));
            }
        }
        Event::ToolCall { tool, subject } => {
            pending.tool = Some((tool.clone(), subject.clone()));
        }
        Event::GateAsk { tool, subject } => {
            println!("[arx] ask: {tool} {}", ask_subject_display(subject));
            println!(
                "a allow once · A allow always · d deny once · D deny always · a note may follow the key: A polishing pass gate"
            );
            pending.from_ask = true;
        }
        Event::GateVerdict { rule, .. } => {
            if pending.from_ask {
                if let Some(r) = rule {
                    println!("  rule appended: {r} -> .arx/gate.toml");
                }
                pending.from_ask = false;
            }
        }
        Event::ToolResult { admission } => {
            if let Some((tool, subject)) = pending.tool.take() {
                let (summary, preview) = admission_lines(admission);
                println!("  · {}   {summary}", tool_call_line(&tool, &subject));
                if let Some(p) = preview {
                    println!("    {p}");
                }
            } else if let Admission::Externalized { bytes: _, handle, preview } = admission {
                println!("  · externalized → {handle}");
                println!("    {preview}");
            }
        }
        Event::Spawn { hand_id, agent, role, body, model, .. } => {
            state.hands_in_flight += 1;
            println!("  ⇢ {} ({})", dispatch_line(hand_id, agent, body, model), role_label(*role));
        }
        Event::Delivered { hand_id, outcome, text, trace, elapsed_ms, schema_ok, .. } => {
            state.hands_in_flight -= 1;
            println!("  ⇠ {}", delivery_line(hand_id, *outcome, *elapsed_ms, trace));
            println!("    {text}");
            if let Some(ok) = schema_ok {
                println!("    schema: {}", if *ok { "ok" } else { "malformed" });
            }
        }
        Event::Notice { text, level } => {
            let tag = match level {
                NoticeLevel::Routine => "[arx]",
                NoticeLevel::Nudge => "[arx] !",
                NoticeLevel::Error => "[arx] !!",
            };
            println!("{tag} {text}");
        }
        Event::Usage { input, output, elapsed_ms, occupancy_after, .. } => {
            state.occupancy = *occupancy_after;
            state.last_tokens = Some(input + output);
            state.last_elapsed_ms = Some(*elapsed_ms);
            println!(
                "  (turn: {:.1}k tok, {:.1}s, occupancy now {occupancy_after})",
                (*input + *output) as f64 / 1000.0,
                *elapsed_ms as f64 / 1000.0
            );
        }
        Event::Checkpoint { number, path, delta } => {
            state.checkpoint = *number;
            let previous = number.saturating_sub(1);
            println!("[arx] checkpoint {number} written: {path}, delta +{delta} since checkpoint {previous}");
        }
        Event::ResetArmed { occupancy, window, reserve } => {
            state.occupancy = *occupancy;
            println!("[arx] ! window at {occupancy} of {window}, reserve {reserve}; /reset to reset now");
        }
        Event::ResetConfirm { body } => {
            for line in body.split('\n') {
                println!("  {}", strip_ansi(&line));
            }
            println!("ok to tear down and respawn; any other line rides into the new main's first turn as an operator note");
        }
        Event::Orientation { text, verdict, occupancy_after } => {
            pending.chunk_buf.clear();
            for line in markdown_lite(text, WIDTH) {
                println!("  {}", strip_ansi(&line));
            }
            println!("[arx] orientation: {verdict}");
            state.occupancy = *occupancy_after;
            state.model = crate::state::INITIAL_MODEL.to_string();
        }
        Event::ModelList { models, current, reason, attempted } => {
            if *reason == ModelListReason::Unlisted {
                println!("[arx] {} is not a known model", attempted.clone().unwrap_or_default());
            }
            println!("[arx] models: {}", model_list_line(&models, &current));
        }
        Event::ModelSwitched { from, to } => {
            state.model = to.clone();
            println!("[arx] model: {from} -> {to}");
        }
        Event::ProviderError { message, recoverable } => {
            println!("[arx] !! {message}{}", if *recoverable { "; retrying" } else { "" });
        }
        Event::Aborted => {
            if !pending.chunk_buf.is_empty() {
                let partial = std::mem::take(&mut pending.chunk_buf);
                for line in markdown_lite(&partial, WIDTH) {
                    println!("  {}", strip_ansi(&line));
                }
            }
            println!("  (stream aborted; partial text above stays in the transcript)");
        }
    }
}
