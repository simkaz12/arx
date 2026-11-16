# Terminal, phase A: the front seam

Class: polishing brief under the owner's working-mode ruling (store decision `b51158b9`): one bounded hand-do brief, the diff left uncommitted for the owner to commit. Not a card; the arx-v0 map is untouched. `gate_run: yes`, the codex round runs on the diff as a whole.

The owner ruled on 2026-09-08 that the `panes` terminal (prototype under `prototypes/ui/panes`, store decision `6d3744ae`) goes into the real binary and gets polished in use, not demoed further. The real `arx run` is a blocking line loop: one stdin lock, `read_operator` at three sites, `terminal.render` for everything the operator sees, deliveries pumped only after an operator line. A full-screen front needs its own thread and a channel in each direction. Phase A names the seam the two fronts share and moves today's behaviour behind it unchanged. Phase B plugs the panes front in. Phase C fills the party panels from the ledger and the swept traces.

Nothing visible changes in this phase. Every integration test passes with the same totals, and piped and tty output stay byte for byte what they are.

## The seam

Today `crates/arx/src/terminal.rs` has `TerminalMsg` (`Chunk`, `TurnEnd`, `Notice`, `Ask`), `Terminal<W: Write>` with `render`, `OperatorMsg::Line`, and the free function `read_operator(&mut impl BufRead)`. `cmd_run` in `main.rs` builds `Terminal::new(io::stdout())`, locks stdin once, prints `> ` itself when stdin is a tty, and threads the terminal and the stdin lock through `gate_call`, `run_turn_cycle`, `run_reset` and the `cycle!`, `check_reserve!` and `pump_deliveries!` macros.

Introduce one trait, in `terminal.rs`:

```rust
pub trait Front {
    fn render(&mut self, msg: &TerminalMsg) -> io::Result<()>;
    fn event(&mut self, event: &Event) -> io::Result<()>;
    fn read(&mut self, wait: Option<Duration>) -> io::Result<Poll>;
}

pub enum Poll {
    Line(OperatorMsg),
    Eof,
    Idle,
}
```

`render` is today's render. `event` receives a copy of every `Event` the run path appends to the transcript log, so a front can keep panels without tailing the file; the line front ignores it. `read` blocks for the next operator line; `wait` is the longest it may block before answering `Idle`, `None` meaning forever. The line front ignores `wait` and never answers `Idle`, since stdin is not pollable and today's loop blocks; the panes front in phase B answers `Idle` so the loop can pump deliveries while the operator is idle (owner ruling 2026-09-08: a delivery shows the moment it lands).

Add `TerminalMsg::Prompt`. The line front renders it as `> ` plus a flush when stdin is a tty and as nothing otherwise, which is what `cmd_run` prints inline today at the top of its loop; `cmd_run` sends the message instead. No other site prints a prompt today (the gate ask and the reset confirm read without one), so nothing else changes.

`LineFront<W: Write, R: BufRead>` wraps today's `Terminal<W>`, the reader, and the tty flag. Its `read` calls `read_operator` and maps `None` to `Eof`. `read_operator` and the `Terminal` unit tests stay as they are.

`cmd_run` builds the line front once and passes `&mut dyn Front` (or a generic bound, your call) where it passed the terminal and the stdin lock. `gate_call`, `run_turn_cycle`, `run_reset` and the macros take the front. Errors keep going to stderr through `eprintln!` as today.

## Event fan-out

Every `log.append_event(...)` on the `arx run` path (`cmd_run` and the functions and macros it calls: `run_turn_cycle`, `run_reset`, `gate_call`, `judge_verdict_event`, `check_reserve!`, `pump_deliveries!`, the drain loop after stdin closes) is followed by `front.event(&ev)` with the same event. Do it through one helper so no site is missed. Before you start, count the `append_event(` calls on that path and write the number down; after, every one of them has its fan-out, and the count of fan-out calls equals it. If the numbers differ, stop and report which site is unpaired rather than adjusting either number.

## Bounds

- Edit only `crates/arx/src/terminal.rs` and `crates/arx/src/main.rs`. If the fan-out needs `Event` visible in `terminal.rs`, a `use` line is enough.
- Do not modify anything under `crates/arx/tests/`, `sidecar/`, `prompts/`, `docs/` (other than nothing; this brief is already written), `arx.toml`, or the prototypes.
- The working tree carries uncommitted changes in `main.rs`, `claude_code.rs`, `config.rs`, `events.rs`, the tests and the sidecar from earlier polishing briefs. Build on top of them. Never `git stash`, never `git checkout --` a file, never revert a hunk you did not write.
- No new crates, no new dependencies, no `Cargo.toml` change.
- `prompts/main.md` stays: main experiences nothing different.

## Verify

1. Before any edit: `cargo test -p arx` (needs the Docker daemon; the stub images build on first run). Record the totals per test binary. If it is not green before you touch anything, stop and report the failures; the working tree is the owner's and may already carry a red test.
2. After: `cargo test -p arx` with identical totals and all green; `cargo test --workspace`; `cargo run -p arx -- check-config ./arx.toml` unchanged.
3. `grep -n "print!\|println!" crates/arx/src/main.rs` shows no stdout write left on the run path outside `LineFront`: the `> ` prompt has moved.
4. `git status --short` shows only the two files changed by you plus what was already modified before you started; list both sets.

## Verdict

Return as text: what was built, the trait and `LineFront` signatures as landed, the append-event count before and the fan-out count after, the test totals before and after per binary, the last 10 lines of `cargo test -p arx`, the `git status --short` listing, and the codex gate outcome with dispositions and log path. Fence fields: `gate_run: yes`.
