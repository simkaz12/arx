use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::scenario::{Event, OperatorInput};

pub fn spawn_auto_driver(fast: bool, with_picker: bool, operator_tx: Sender<OperatorInput>) -> Sender<Event> {
    spawn_auto_driver_with_echo(fast, with_picker, operator_tx, None)
}

pub fn spawn_auto_driver_with_echo(
    fast: bool,
    with_picker: bool,
    operator_tx: Sender<OperatorInput>,
    echo: Option<Sender<String>>,
) -> Sender<Event> {
    let (fwd_tx, fwd_rx) = mpsc::channel::<Event>();
    thread::spawn(move || run_driver(fwd_rx, operator_tx, fast, with_picker, echo));
    fwd_tx
}

// When an echo channel exists (interactive tty), the UI owns the actual send: it prints
// the line first and only then forwards it to the engine, exactly as it does for a real
// keystroke, so the operator's line can never render after the engine's own response to
// it. Without an echo channel (piped mode has no visible echo at all) the driver sends
// straight to the engine.
fn send_line(tx: &Sender<OperatorInput>, echo: &Option<Sender<String>>, line: &str, fast: bool) {
    if !fast {
        thread::sleep(Duration::from_millis(600));
    }
    match echo {
        Some(e) => {
            let _ = e.send(line.to_string());
        }
        None => {
            let _ = tx.send(OperatorInput::Line(line.to_string()));
        }
    }
}

fn send_reactive(tx: &Sender<OperatorInput>, echo: &Option<Sender<String>>, line: &str) {
    match echo {
        Some(e) => {
            let _ = e.send(line.to_string());
        }
        None => {
            let _ = tx.send(OperatorInput::Line(line.to_string()));
        }
    }
}

fn wait_for(rx: &Receiver<Event>, pred: impl Fn(&Event) -> bool) -> bool {
    loop {
        match rx.recv() {
            Ok(e) if pred(&e) => return true,
            Ok(_) => continue,
            Err(_) => return false,
        }
    }
}

fn wait_for_n(rx: &Receiver<Event>, pred: impl Fn(&Event) -> bool, n: u32) -> bool {
    let mut seen = 0;
    while seen < n {
        match rx.recv() {
            Ok(e) if pred(&e) => seen += 1,
            Ok(_) => continue,
            Err(_) => return false,
        }
    }
    true
}

fn is_turn_end(e: &Event) -> bool {
    matches!(e, Event::TurnEnd(_))
}

fn run_driver(rx: Receiver<Event>, tx: Sender<OperatorInput>, fast: bool, with_picker: bool, echo: Option<Sender<String>>) {
    send_line(&tx, &echo, "What changed with the store root?", fast);
    if !wait_for(&rx, is_turn_end) {
        return;
    }

    send_line(&tx, &echo, "Can you check what still pins down the piped shape?", fast);
    if !wait_for(&rx, |e| matches!(e, Event::GateAsk { .. })) {
        return;
    }
    send_reactive(&tx, &echo, "A polishing pass gate");
    if !wait_for(&rx, is_turn_end) {
        return;
    }

    send_line(&tx, &echo, "Go ahead and dispatch someone to clean that up.", fast);
    if !wait_for(&rx, is_turn_end) {
        return;
    }
    if !wait_for_n(&rx, |e| matches!(e, Event::Delivered { .. }), 2) {
        return;
    }

    send_line(&tx, &echo, "ok", fast);
    if !wait_for(&rx, is_turn_end) {
        return;
    }

    send_line(&tx, &echo, "/model gpt-5", fast);
    if !wait_for(&rx, |e| matches!(e, Event::ModelList { .. })) {
        return;
    }

    send_line(&tx, &echo, "/model", fast);
    if !wait_for(&rx, |e| matches!(e, Event::ModelList { .. })) {
        return;
    }
    if with_picker {
        if !fast {
            thread::sleep(Duration::from_millis(500));
        }
        send_reactive(&tx, &echo, "/model claude-sonnet-5");
        if !wait_for(&rx, |e| matches!(e, Event::ModelSwitched { .. })) {
            return;
        }
    }

    send_line(&tx, &echo, "/model claude-sonnet-5", fast);
    if !wait_for(&rx, |e| matches!(e, Event::ModelSwitched { .. }) || matches!(e, Event::Notice { .. })) {
        return;
    }

    send_line(&tx, &echo, "How's context usage looking?", fast);
    if !wait_for(&rx, |e| matches!(e, Event::ResetArmed { .. })) {
        return;
    }

    send_line(&tx, &echo, "/checkpoint", fast);
    if !wait_for(&rx, |e| matches!(e, Event::Checkpoint { .. })) {
        return;
    }

    send_line(&tx, &echo, "/reset", fast);
    if !wait_for(&rx, |e| matches!(e, Event::ResetConfirm { .. })) {
        return;
    }
    send_reactive(&tx, &echo, "ok");
    if !wait_for(&rx, |e| matches!(e, Event::Orientation { .. })) {
        return;
    }

    send_line(&tx, &echo, "Tell me more about the polishing backlog.", fast);
    let mut chunk_count = 0u32;
    let mut abort_sent = false;
    loop {
        match rx.recv() {
            Ok(Event::Chunk(_)) => {
                chunk_count += 1;
                if chunk_count >= 3 && !abort_sent {
                    let _ = tx.send(OperatorInput::Abort);
                    abort_sent = true;
                }
            }
            Ok(Event::Aborted) | Ok(Event::TurnEnd(_)) => break,
            Ok(_) => {}
            Err(_) => return,
        }
    }

    send_line(&tx, &echo, "Try again?", fast);
    if !wait_for(&rx, is_turn_end) {
        return;
    }

    send_line(&tx, &echo, "/reste", fast);
    let _ = wait_for(&rx, |e| matches!(e, Event::Notice { .. }));
}
