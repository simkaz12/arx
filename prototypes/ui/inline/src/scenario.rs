use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::event::{
    AskAnswer, Admission, DispatchInfo, Delivery, Event, Usage, UiEvent,
};

const MODELS: [&str; 3] = ["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"];

fn wait_ms(ms: u64, fast: bool) {
    if !fast && ms > 0 {
        thread::sleep(Duration::from_millis(ms));
    }
}

fn chunk_tokens(text: &str, group: usize) -> Vec<String> {
    let tokens: Vec<&str> = text.split_inclusive(char::is_whitespace).collect();
    if tokens.is_empty() {
        return vec![text.to_string()];
    }
    tokens
        .chunks(group.max(1))
        .map(|c| c.concat())
        .collect()
}

fn stream(tx: &Sender<Event>, rx: &Receiver<UiEvent>, text: &str, total_ms: u64, fast: bool) -> bool {
    let chunks = chunk_tokens(text, 3);
    let per = if fast {
        3
    } else if chunks.is_empty() {
        0
    } else {
        total_ms / chunks.len() as u64
    };
    for chunk in chunks {
        if let Ok(UiEvent::Abort) = rx.try_recv() {
            return false;
        }
        let _ = tx.send(Event::Chunk(chunk));
        thread::sleep(Duration::from_millis(per));
    }
    true
}

fn recv_ask(rx: &Receiver<UiEvent>) -> AskAnswer {
    loop {
        if let Ok(UiEvent::AskAnswer(a)) = rx.recv() {
            return a;
        }
    }
}

fn recv_pick(rx: &Receiver<UiEvent>) -> String {
    loop {
        if let Ok(UiEvent::ModelPick(id)) = rx.recv() {
            return id;
        }
    }
}

fn recv_checkpoint(rx: &Receiver<UiEvent>) {
    loop {
        if let Ok(UiEvent::Checkpoint) = rx.recv() {
            return;
        }
    }
}

fn recv_reset(rx: &Receiver<UiEvent>) {
    loop {
        if let Ok(UiEvent::Reset) = rx.recv() {
            return;
        }
    }
}

fn recv_confirm(rx: &Receiver<UiEvent>) -> String {
    loop {
        if let Ok(UiEvent::ConfirmLine(line)) = rx.recv() {
            return line;
        }
    }
}

fn recv_any_line(rx: &Receiver<UiEvent>) {
    let _ = rx.recv();
}

const CHECKPOINT_STATE: &str = "position   polishing pass, day 1; pid.txt removed, /model landed, doubled reply fixed\ntarget     arx usable as the daily driver for arx work (L3-04)\ndrift      none; terminal look still unruled (44afcdc8)\nrelies on  580e402e, 57aa480a";

pub fn run(tx: Sender<Event>, rx: Receiver<UiEvent>, fast: bool) {
    let mut current_model = "claude-opus-5".to_string();
    let _ = tx.send(Event::Notice("grounding: hot 4 claims · 2 embers".to_string()));

    // Scene 1: a plain reply.
    recv_any_line(&rx);
    let _ = tx.send(Event::TurnStart);
    wait_ms(700, fast);
    let scene1 = "The store root moved out of the repo in L3-05, so a fresh clone has no `store/` at all. Two things follow:\n\n- `arx run` refuses an unregistered scope, and registration is still a hand-typed `mkdir` under `~/.local/share/arx/store`.\n- The tracked `pid.txt` is a leftover of the L1-close session. Nothing writes it now.\n\nI'd delete `pid.txt` in the polishing pass and leave the store registration as it is.";
    stream(&tx, &rx, scene1, 2000, fast);
    let _ = tx.send(Event::TurnEnd(scene1.to_string()));
    let _ = tx.send(Event::Usage(Usage {
        input: 18240,
        output: 142,
        cache_read: 16900,
        model: "claude-opus-5".to_string(),
        model_source: "sdk",
        elapsed_ms: 3180,
        occupancy_after: 63900,
    }));

    // Scene 2: tool calls, an ask, admission.
    recv_any_line(&rx);
    let _ = tx.send(Event::TurnStart);
    wait_ms(500, fast);
    let _ = tx.send(Event::ToolCall {
        tool: "Read".to_string(),
        subject: "crates/arx/src/terminal.rs".to_string(),
    });
    wait_ms(200, fast);
    let _ = tx.send(Event::GateVerdict {
        tool: "Read".to_string(),
        subject: "crates/arx/src/terminal.rs".to_string(),
        allowed: true,
        rule: "Read *".to_string(),
    });
    wait_ms(150, fast);
    let _ = tx.send(Event::ToolResult {
        admission: Admission::Inline { bytes: 6312 },
        preview: "pub enum TerminalMsg { Chunk, TurnEnd, Notice, Ask }".to_string(),
    });
    wait_ms(300, fast);
    let pretool = "terminal.rs is still the L1-04 adapter; let me see what the tests pin down.";
    stream(&tx, &rx, pretool, 700, fast);

    let _ = tx.send(Event::ToolCall {
        tool: "Bash".to_string(),
        subject: "cargo test -p arx --test claude_main 2>&1 | tail -40".to_string(),
    });
    wait_ms(200, fast);
    let _ = tx.send(Event::GateAsk {
        tool: "Bash".to_string(),
        subject: "cargo test -p arx --test claude_main 2>&1 | tail -40".to_string(),
    });
    let answer = recv_ask(&rx);
    let (allowed, rule) = match answer.key {
        'a' => (true, "allow once".to_string()),
        'A' => (true, "Bash cargo test -p arx*".to_string()),
        'd' => (false, "deny once".to_string()),
        _ => (false, "Bash cargo test -p arx* (deny)".to_string()),
    };
    let _ = tx.send(Event::GateVerdict {
        tool: "Bash".to_string(),
        subject: "cargo test -p arx --test claude_main 2>&1 | tail -40".to_string(),
        allowed,
        rule: rule.clone(),
    });
    if answer.key == 'A' || answer.key == 'D' {
        let _ = tx.send(Event::Notice(format!(
            "rule appended: `{rule}` -> .arx/gate.toml"
        )));
    }
    wait_ms(200, fast);
    let _ = tx.send(Event::ToolResult {
        admission: Admission::Externalized {
            bytes: 14803,
            trace: "trace/2026-09-08/t07-bash-3.txt".to_string(),
        },
        preview: "test result: ok. 11 passed; 0 failed".to_string(),
    });
    wait_ms(300, fast);
    let posttool = "Eleven integration tests pin the byte shape of piped output, so a richer terminal has to keep piped mode byte-identical and add the new look only when stdin is a tty.";
    stream(&tx, &rx, posttool, 1600, fast);
    let _ = tx.send(Event::TurnEnd(format!("{pretool}\n\n{posttool}")));
    let _ = tx.send(Event::Usage(Usage {
        input: 33110,
        output: 96,
        cache_read: 18240,
        model: "claude-opus-5".to_string(),
        model_source: "sdk",
        elapsed_ms: 9640,
        occupancy_after: 79300,
    }));

    // Scene 3: a dispatch and a judge, delivery while idle.
    recv_any_line(&rx);
    let _ = tx.send(Event::TurnStart);
    wait_ms(400, fast);
    let _ = tx.send(Event::Spawn(DispatchInfo {
        hand_id: "h-0007".to_string(),
        agent: "hand-do".to_string(),
        role: "hand",
        body: "claude-code",
        model: "claude-sonnet-5".to_string(),
    }));
    let _ = tx.send(Event::GateVerdict {
        tool: "dispatch".to_string(),
        subject: "hand-do".to_string(),
        allowed: true,
        rule: "dispatch *".to_string(),
    });
    wait_ms(200, fast);
    let _ = tx.send(Event::Spawn(DispatchInfo {
        hand_id: "h-0008".to_string(),
        agent: "codex-judge".to_string(),
        role: "judge",
        body: "codex",
        model: "gpt-5.4".to_string(),
    }));
    let _ = tx.send(Event::GateVerdict {
        tool: "dispatch".to_string(),
        subject: "codex-judge".to_string(),
        allowed: true,
        rule: "dispatch *".to_string(),
    });
    wait_ms(300, fast);
    let dispatched = "Dispatched hand-do to delete `pid.txt` and drop its mention from CLAUDE.md, and the codex judge to review that diff once it lands. I'll fold both when they report.";
    stream(&tx, &rx, dispatched, 1500, fast);
    let _ = tx.send(Event::TurnEnd(dispatched.to_string()));

    {
        let tx1 = tx.clone();
        thread::spawn(move || {
            if fast {
                thread::sleep(Duration::from_millis(5));
            } else {
                wait_ms(6000, fast);
            }
            let _ = tx1.send(Event::Delivered(Delivery {
                hand_id: "h-0007".to_string(),
                outcome: "verdict",
                text: "PASS: pid.txt removed, CLAUDE.md hazard line rewritten, 23 tests green"
                    .to_string(),
                trace: "trace/2026-09-08/h-0007.md".to_string(),
                bytes: 2118,
                elapsed_ms: 5940,
                lag_ms: 12,
                schema_ok: None,
            }));
        });
    }
    {
        let tx2 = tx.clone();
        thread::spawn(move || {
            if fast {
                thread::sleep(Duration::from_millis(15));
            } else {
                wait_ms(9000, fast);
            }
            let _ = tx2.send(Event::Delivered(Delivery {
                hand_id: "h-0008".to_string(),
                outcome: "verdict",
                text: "no findings; the hazard paragraph now says nothing writes pid.txt, which matches the tree"
                    .to_string(),
                trace: "trace/2026-09-08/h-0008.md".to_string(),
                bytes: 611,
                elapsed_ms: 8870,
                lag_ms: 8,
                schema_ok: Some(true),
            }));
        });
    }

    recv_any_line(&rx);
    let _ = tx.send(Event::TurnStart);
    wait_ms(400, fast);
    let both_back = "Both back. The hand's diff is in the working tree, uncommitted, and the judge found nothing. Your call whether to commit it with the rest of the polishing work.";
    stream(&tx, &rx, both_back, 1400, fast);
    let _ = tx.send(Event::TurnEnd(both_back.to_string()));

    // Scene 4: /model
    let picked = recv_pick(&rx);
    if picked == current_model {
        let _ = tx.send(Event::ModelAlready(picked));
    } else if MODELS.contains(&picked.as_str()) {
        let _ = tx.send(Event::ModelSwitched {
            from: current_model.clone(),
            to: picked.clone(),
        });
        current_model = picked;
    } else {
        let _ = tx.send(Event::ModelUnlisted {
            requested: picked,
            models: MODELS.iter().map(|m| m.to_string()).collect(),
        });
    }

    // Scene 5: nudge, checkpoint, reset.
    recv_any_line(&rx);
    let _ = tx.send(Event::TurnStart);
    wait_ms(300, fast);
    let _ = tx.send(Event::Usage(Usage {
        input: 40200,
        output: 88,
        cache_read: 20100,
        model: "claude-sonnet-5".to_string(),
        model_source: "sdk",
        elapsed_ms: 4100,
        occupancy_after: 121800,
    }));
    let _ = tx.send(Event::Amber("main at 121800 tokens, past 120000".to_string()));
    wait_ms(200, fast);
    let _ = tx.send(Event::Amber(
        "window at 161200 of 200000, reserve 40000; /reset to reset now".to_string(),
    ));
    let _ = tx.send(Event::TurnEnd(String::new()));

    recv_checkpoint(&rx);
    let _ = tx.send(Event::ArxLine(
        "Write the state object: position, target, drift. Cite decision ids you rely on."
            .to_string(),
    ));
    let _ = tx.send(Event::TurnStart);
    wait_ms(400, fast);
    let fenced_state = format!("```\n{CHECKPOINT_STATE}\n```");
    stream(&tx, &rx, &fenced_state, 1500, fast);
    let _ = tx.send(Event::TurnEnd(fenced_state));
    let _ = tx.send(Event::ArxLine(
        "Check every claim above against the tree and the store.".to_string(),
    ));
    let _ = tx.send(Event::TurnStart);
    wait_ms(400, fast);
    stream(&tx, &rx, "all four hold", 400, fast);
    let _ = tx.send(Event::TurnEnd("all four hold".to_string()));
    let _ = tx.send(Event::CheckpointWritten {
        number: 3,
        path: "threads/main.checkpoints/3.md".to_string(),
        delta: 58400,
        since: 2,
    });

    // /reset
    recv_reset(&rx);
    let _ = tx.send(Event::ResetConfirm {
        body: CHECKPOINT_STATE.to_string(),
    });
    let line = recv_confirm(&rx);
    if line.trim().eq_ignore_ascii_case("ok") {
        let _ = tx.send(Event::TurnStart);
        wait_ms(600, fast);
        let _ = tx.send(Event::Notice("tearing down main".to_string()));
        wait_ms(500, fast);
        let _ = tx.send(Event::Notice("respawned main from checkpoint 3".to_string()));
        wait_ms(300, fast);
        let orientation = "Resuming from checkpoint 3. Position and target are clear, the drift line is empty, both decision ids resolve. Ready.";
        stream(&tx, &rx, orientation, 1200, fast);
        let _ = tx.send(Event::TurnEnd(orientation.to_string()));
        let _ = tx.send(Event::Orientation {
            text: orientation.to_string(),
            verdict: "ready",
        });
        let _ = tx.send(Event::OccupancyReset(9120));
    } else {
        let _ = tx.send(Event::Notice("noted; staying up".to_string()));
    }

    // Scene 6: abort and a non-fatal error.
    recv_any_line(&rx);
    let _ = tx.send(Event::TurnStart);
    wait_ms(500, fast);
    let filler = "Walking through the polishing list end to end: `pid.txt` is gone, the store registration note in CLAUDE.md matches the tree, and the doubled-reply fix from the sidecar's assembly change has a pinned regression test under `sidecar/test/`.\n\nWhat is left is the terminal look itself, which is why we are running these three prototypes side by side before ruling on a shape.";
    let completed = stream(&tx, &rx, filler, 3000, fast);
    if !completed {
        let _ = tx.send(Event::Aborted);
    } else {
        let _ = tx.send(Event::TurnEnd(filler.to_string()));
    }

    recv_any_line(&rx);
    let _ = tx.send(Event::TurnStart);
    wait_ms(400, fast);
    let _ = tx.send(Event::ProviderError(
        "overloaded_error: retry after 2 s".to_string(),
    ));
    wait_ms(2000, fast);
    let _ = tx.send(Event::RetryOk);
    let back = "Back. Where were we?";
    stream(&tx, &rx, back, 500, fast);
    let _ = tx.send(Event::TurnEnd(back.to_string()));

    loop {
        match rx.recv() {
            Ok(_) => {
                let _ = tx.send(Event::Notice(
                    "end of the scripted session; nothing further to show".to_string(),
                ));
            }
            Err(_) => break,
        }
    }
}
