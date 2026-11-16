# Terminal, phase B: the panes front

Class: polishing brief under the owner's working-mode ruling (store decision `b51158b9`): one bounded hand-do brief, the diff left uncommitted for the owner to commit and use. Not a card. `gate_run: yes`.

Phase A (`2026-09-08-terminal-a-front-seam.md`) put a `Front` trait in `crates/arx/src/terminal.rs` with `render`, `event` and `read(wait)`, a `LineFront` that is today's terminal byte for byte, and fanned every run-path event out to the front. This phase adds the second adapter: `PanesFront`, the full-screen ratatui front the owner chose (store decisions `6d3744ae` and `ea96b3ac`), lifted from the prototype under `prototypes/ui/panes/` and driven by the harness instead of the scripted scenario. Read the prototype's `README.md`, `briefs/panes-v2.md` and `briefs/panes-v3.md` (round three landed: split page, per-pane scroll memory, a `[split]` key context) before the code: they say what each key and pane does, and that stands unless this brief says otherwise.

The owner's rulings that bound this phase: the front shows nothing the harness does not already hold; a delivery shows the moment it lands while the operator is idle; Esc-to-abort stays out, and the bottom line advertises only what works; no editor inside the terminal.

## Selecting the front

Add a `[terminal]` section to config with two optional keys, `front = "line" | "panes"` (default `line`) and `keys = "<path>"` (a `keys.toml` overriding the embedded defaults; absent means defaults only). The parser refuses unknown fields everywhere, so the section is a real struct with `#[serde(default)]`; `check-config` prints nothing new for it. `cmd_run` builds `PanesFront` only when `front = "panes"` and both stdin and stdout are terminals; otherwise `LineFront`, silently. Every integration test runs without a `[terminal]` section and stays on the line front.

Add `[terminal]\nfront = "panes"` to the repo's `arx.toml` so the owner's next `arx` lands in panes.

## The adapter

`PanesFront` lives in `crates/arx/src/panes/` (`mod.rs` with the adapter, plus the prototype's `ui.rs`, `keys.rs`, `state.rs`, `app.rs` lifted and trimmed; `scenario.rs` does not come along). It runs the UI on its own thread, which owns the terminal: raw mode, alternate screen, mouse capture, crossterm events polled at a short tick. The harness thread talks to it over two channels.

- `render(msg)` sends the message to the UI thread. `event(ev)` sends the event. Both clone; derive `Clone` where missing.
- `read(wait)` waits on the channel from the UI: a submitted line becomes `Poll::Line`, the timeout `Poll::Idle`, and the UI thread quitting `Poll::Eof`.
- `q` in normal mode and Ctrl-C anywhere quit the UI thread, which restores the terminal before it exits; the harness then sees `Eof` and runs today's drain and shutdown on the normal screen. In raw mode Ctrl-C is a key, not a signal, so the drain and `adapter.shutdown` run, which they do not today under Ctrl-C. Say so in the README, nowhere else.
- Any error exit while the front is up must leave the terminal restored and the error text readable on the normal screen: restore first, then print. Install a panic hook that restores the terminal.

Three additions to `TerminalMsg`, each rendered by `LineFront` exactly as the text it replaces so the line output does not move:

- `Confirm { text }` for the reset confirm that `run_reset` sends before its read (today a `Notice`). The line front renders it as the notice. The panes front shows a modal: Enter sends `ok`, Esc sends an empty line (both confirm today), and typing a line sends that line, which rides into the new main's first turn as today.
- `ModelList { models, current }` for `/model` with no argument (today the list is formatted into a `Notice`). The line front formats it as today; move that formatting into the line front. The panes front shows the picker; choosing sends `/model <id>` as a line, which the loop parses as today, so the success and failure notices come back unchanged.
- `Status { model, occupancy, window, reserve, nudge, checkpoint, turn }` sent after every main turn, after a `/model` switch and after a respawn. The line front ignores it. The values come from what `cmd_run` already holds: `current_model`, `checkpoint.last_occupancy`, `reserve.window`, `reserve.reserve`, `nudge.threshold`, the last checkpoint number with its delta if any, and `last_handle.turn`.

## Finish the fan-out

Phase A fanned out the 26 `append_event` sites in `cmd_run`, `run_turn_cycle`, `run_reset` and `gate_call` through `log_event`. Eight sites on the same run path stayed raw because A's list named them by function: `run_checkpoint_ritual` (2, checkpoint pass and fail), `on_hand_call` (3, tap, steer, kill), `dispatch_and_record` (2, family, spawn), `on_store_drain` (1, drain). Route all eight through `log_event` first; the hands panel cannot exist without `spawn`. After that, `grep -n "append_event(" crates/arx/src/main.rs` shows no raw call on the `arx run` path; if one remains that you cannot reach, stop and report it.

## The run loop

`cmd_run` sends `Prompt`, then loops on `front.read(Some(wait))` where `wait` is 200 ms: `Idle` runs `pump_deliveries!()` and reads again without a second prompt; `Line` is handled as today; `Eof` breaks to the drain. The line front never answers `Idle`, so piped and line-tty behaviour do not change.

## What the UI shows

The prototype's layout, modes and keys, with these mappings from harness messages to the state reducer:

- `Chunk` appends to the live main's streaming turn. `TurnEnd` reconciles the way `Terminal::render` does, except that on a mismatch the streamed text is replaced by the canonical text in place rather than printed twice; the front owns the screen, so there is nothing to reprint.
- The operator's submitted line is appended to main's page as the `>` line at submit time.
- `Notice` is a dim line on main's page. The reset state-object body arrives as a notice and renders as the block the prototype shows on `/reset`.
- `Ask` opens the gate modal: `a`, `A`, `d`, `D`, with the note field; the answer is sent as the one line the ask loop reads today, key first, note after. While the modal is open the input box takes no keys. An answer the loop rejects comes back as the `[arx] answer with a, A, d or D` notice; show it and keep the modal open.
- Events become the dim one-liners main's page shows in the prototype: `gate` and `admission` as tool-call lines with their verdict or bytes, `spawn` as a dispatch line, `delivered` as a delivery line, `model`, `checkpoint`, `reset`, `nudge`, `miss`, `floor`, `schema`. Events the prototype never showed (`pull`, `echo`, `rewind`, `orientation`, `family`, `drain`, `wall`, `tap`, `steer`, `kill`) get one dim line each in the same style; no special rendering.
- `spawn` adds a row to the hands or judges panel by role, with the elapsed timer ticking on the UI thread's clock; `delivered` settles it to the outcome and elapsed; `kill` marks it killed. Selecting a row shows the prototype's round-two party page (header and, after delivery, the verdict block). The split page and the brief come in phase C; until then the page says nothing about tap.
- `reset` with phase `respawned` freezes the current main page with an `ended: reset` header and starts the next incarnation's page, selected. The main panel lists incarnations newest first with model and `live`/`ended`, as in the prototype.
- `Status` fills the session panel and the session page: model, occupancy gauge against the window with the reserve ceiling and the nudge tick, checkpoint number and delta, turn count. Gate counts (allow, deny, ask) come from `gate` events with `by: operator`; a pending ask shows while the modal is open. The session page also lists the operator model list from the last `ModelList` seen, or the current model alone before any.
- The slash popup completes `/model`, `/checkpoint`, `/reset`. Any other line, slash or not, goes to main as today; no hint.
- Abort is out: drop the `abort` binding from the embedded defaults and every `Esc abort` fragment from the bottom line, so Esc in normal mode always means back and never waits on a stream (the prototype's round-three verdict found the global abort binding swallowing Esc whenever a turn was in flight). Esc in insert mode leaves insert mode and does nothing else.
- Mouse wheel scrolls whatever is under the pointer, as in the prototype.

Keys, `keys.toml`, the `?` overlay, the markdown-lite renderer, the gauge, history, the modal contrast and the panel numbering are the prototype's. The embedded defaults are the prototype's `keys.toml`; `[terminal] keys` overrides by name.

## Bounds

- New files under `crates/arx/src/panes/`; edits to `crates/arx/src/terminal.rs`, `main.rs`, `config.rs`, `events.rs` (a `Clone` derive), `crates/arx/Cargo.toml` (ratatui and crossterm at the prototype's versions; the workspace `Cargo.lock` moves accordingly, and that is expected here), `arx.toml` (the `[terminal]` section), and a short section in `README.md` or wherever the repo documents `arx.toml` keys.
- Do not modify anything under `crates/arx/tests/`, `sidecar/`, `prompts/`, `docs/`, or the prototypes. If a test literal has to change because of a new config field, stop and report; `#[serde(default)]` plus `Default` should make that unnecessary.
- The working tree carries uncommitted changes from earlier briefs, phase A among them. Build on them. Never `git stash`, never `git checkout --` a file, never revert a hunk you did not write.
- No test module inside a file that any integration test `#[path]`-includes (see `CLAUDE.md`); `grep -rn "#\[path" crates/arx/tests` tells you which. Unit tests for the panes code go in the panes module files if none is included, else in `main.rs`'s test module.

## Verify

1. Before any edit: `cargo test -p arx` green; record totals per binary.
2. After: `cargo test -p arx` green with the previous totals unchanged plus the new unit tests, which you list; `cargo test --workspace`; `cargo run -p arx -- check-config ./arx.toml` shows the same lines as before plus nothing.
3. Unit tests through ratatui's `TestBackend` at 110x36: a scripted message sequence (prompt, chunk, chunk, turn end, a gate ask and its answer, a spawn and its delivery, a status) renders frames that match fixed expectations; the `Confirm` and `ModelList` line renderings equal the strings they replace; `read` answers `Idle` on timeout and `Eof` after the UI channel closes.
4. Live under tmux, `tmux new -d -s panesb -x 110 -y 36`, running the real binary against the stub sidecar the integration tests use (see `crates/arx/tests/common/mod.rs` for how they configure it) with `front = "panes"`: type a line in insert mode and watch the stub's reply stream, trigger an ask if the stub can, run `/model` and pick, press `?`, press `q`, and confirm the shell prompt is back with the screen restored. Paste the captures. Kill the tmux session when done.
5. Piped mode: `printf 'hello\n' | cargo run -p arx -- run` against the same stub config with `front = "panes"` in it prints exactly what it printed before this phase.

## Verdict

Return as text: what was built, the file list with line counts, the `TerminalMsg` and `Front` as landed, the config struct, the test totals before and after per binary, the last 10 lines of `cargo test -p arx`, the tmux captures fenced, the piped-mode diff (empty), `git status --short`, three things you would change next, and the codex gate outcome with dispositions and log path. Fence field: `gate_run: yes`.
