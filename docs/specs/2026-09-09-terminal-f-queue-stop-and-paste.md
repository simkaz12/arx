# Terminal, phase F: the queue, the stop key, paste

Class: polishing brief under the owner's working-mode ruling (store decision `b51158b9`), the same class as phases A to E: one bounded hand-do brief, the diff left uncommitted for the owner to commit and use. Not a card. `gate_run: yes`.

Phase E (`2026-09-09-terminal-e-input-close-and-main-clock.md`) gave the input a cursor and a growing box, `x` to close a row with `/kill <id>` behind it, and a clock on main's row. The owner's next report, 2026-09-09, named three things: a line typed during a turn is ignored, nothing stops main mid-turn, and paste of text and images. The owner ruled on 2026-09-09: `q` is the stop key and no longer quits (Ctrl-c quits); a line typed during a turn waits, visibly, and goes as the next turn; steering for now is stop plus a queued line; Ctrl-V pastes text and images. The stop reopens phase B's "no Esc abort" ruling, superseded in the store. Vocabulary is `CONTEXT.md`'s.

## Problem / Solution

The front already queues a second line: it sits in the channel behind `PanesFront::read` (panes/mod.rs) until the harness calls `read` again at the top of the operator loop (main.rs, the `'operator` loop). It is lost when a gate ask fires during the turn: `gate_call`'s answer loop calls the same `read`, gets the queued line instead of `a` or `d`, prints the "answer with a, A, d or D" notice and drops it. Nothing shows what is queued. The turn cycle blocks in `run_turn` (claude_code.rs) on the sidecar's channel with no way in; the sidecar has no interrupt op though the SDK's query has `interrupt()`. Bracketed paste is off, so a pasted block submits a turn per line; `Ctrl-V` reaches the front as nothing, and an image never arrives at all because no terminal passes one.

The fix: two lanes at the front seam, one for operator lines and one for gate answers, with the queue held in the front and drawn under the big view; a control lane the turn cycle polls, `q` sending `stop`, the sidecar calling `interrupt()`; bracketed paste inserting at the cursor, `Ctrl-V` reading the Wayland clipboard through `wl-paste`, images saved under `threads/main.images/` and sent to the sidecar as content blocks.

## Decisions locked

- **Two read lanes.** `Front` gains `fn read_answer(&mut self, wait: Option<Duration>) -> io::Result<Poll>` beside `read`. `gate_call`'s ask loop calls `read_answer` and nothing else; the operator loop calls `read` and nothing else. `LineFront::read_answer` reads a line exactly as `read` does today, so piped mode and every integration test are unchanged. `PanesFront::read_answer` returns only what the ask modal produces; `PanesFront::read` returns only submitted operator lines.
- **The queue lives in the front.** A line submitted while the harness is not waiting at the operator loop stays in `App.queue: VecDeque<String>`; the UI thread hands over the oldest line only when the harness's `read` asks for one. The strip between the big view and the input: `queued 2 · ↑ edits the last` on the first row, then one dim row per line cut at the width, at most four rows; more lines than fit collapse into `… and N more` on the last row. Nothing drawn when the queue is empty. Each released line becomes its own operator turn, oldest first.
- **Up with an empty input** and a non-empty queue pulls the newest queued line into the input and out of the queue; otherwise phase E's row and history rule applies.
- **`q` stops, Ctrl-c quits.** `[normal]` becomes `stop = ["q"]`, `quit = ["Ctrl-c"]`. `q` while main's turn is open, focus anywhere, sends `Control::Stop`. `q` with a running or stopped hand or judge selected in its panel, or its page in the big view, opens phase E's kill confirm for that party, the same modal `x` opens. `q` with nothing running sets the status line to `nothing running`.
- **The control lane.** `Front` gains `fn poll_control(&mut self) -> Option<Control>`, non-blocking; `LineFront` returns `None`. `Control` has one variant, `Stop`. `run_turn` takes a `&mut dyn FnMut() -> Option<Control>` and its read loop uses `recv_timeout` of 50 ms instead of `recv`; on `Stop` it writes `{"op":"interrupt"}` to the sidecar once and keeps reading until `turn_end`. The harness passes a closure over `front.poll_control()`.
- **The sidecar's interrupt.** `op === 'interrupt'` calls `activeQuery.interrupt()`. The SDK then finishes the query's current turn on its own; the sidecar emits `turn` with the text it has, `usage` if the result carries one, and `turn_end` with `stopped: true`. The exact result message the SDK emits after `interrupt()` (subtype, `is_error`) is unknown before the live probe below; the verdict records it verbatim.
- **The stop on the record.** `TurnOutcome` gains `stopped: bool`. The harness pushes `TerminalMsg::Stopped` and the page shows the notice `stopped by operator` under the text main streamed; that text stays as a main turn. The turn event gains `stopped: bool` serialized only when true, so no existing event line changes and the kind count stays. The transcript's turn row carries the same flag the way the harness already writes turn metadata; main's next turn sees the flag through whatever the transcript projection already shows it, and nothing is added to `prompts/main.md` for the stop.
- **Bracketed paste.** `EnableBracketedPaste` at setup, `DisableBracketedPaste` at teardown, both beside the alternate screen calls. `Event::Paste(s)` with `\r\n` and `\r` normalized to `\n`: in insert mode inserts at the cursor, nothing submitted; in normal mode enters insert then inserts; in a modal it is dropped.
- **`Ctrl-V` reads the clipboard.** `[insert]` gains `paste = ["Ctrl-v"]`. The action runs `wl-paste --list-types`; with an `image/png`, `image/jpeg`, `image/gif` or `image/webp` type present (png preferred) it reads that type to `threads/main.images/<n>.<ext>` relative to the run's cwd, `n` one above the highest number already there, inserts `[image n]` at the cursor and records `Attachment { n, path, media_type, bytes }` on `App`; with text only it inserts the text as a paste; empty clipboard or a failing `wl-paste` sets the status line to `clipboard: <error>`. Over 5 MB is refused with `clipboard: image over 5 MB`. The clipboard is behind a `Clipboard` trait with the `wl-paste` adapter and a test fake; the command runs synchronously on the UI thread.
- **Submit with images.** At submit, the attachments whose `[image n]` marker is still in the text go with the line and the rest are deleted from disk. `OperatorMsg` gains `Turn { text: String, images: Vec<Attachment> }`; the panes front sends `Line` when there are no images, so nothing else changes. The harness writes `{"op":"prompt","text":…,"images":[{"path":…,"media_type":…}]}`; without images the op is what it is today. The sidecar reads each file, base64-encodes it and builds `content` as an array: one `text` block then one `image` block per file, the SDK's own block shapes; with no images `content` stays the string it is today, so the stub sidecar and every integration test see the same op. The operator turn on the page shows the text as typed, markers included, and renders no image. The transcript's operator row records the paths beside the text.
- **`prompts/main.md`** gains one sentence: an operator line may carry images, and `[image n]` in the text is that image. Main experiences the image, so the prompt says so.
- **Mouse** stays as it is; drag-to-select with capture on is parked (Shift-drag bypasses capture in most emulators).

## Execution plan

The working tree carries uncommitted phases A to E and earlier polishing diffs. Tripwire before any edit: `grep -n 'close' crates/arx/src/panes/keys.toml` must show phase E's `close = ["x"]`; if it does not, STOP and report, phase E has not landed. Scopes run in this order; each leaves `cargo build -p arx` green.

### 0. Main panel rows never wrap

- **Files:** Modify `crates/arx/src/panes/ui.rs`.
- **Build:** Phase E's verdict found that `draw_right_column` budgets one row per incarnation while the new status cell (`running · 12.4s · 38.2k tok`) can wrap a main row to two lines at 110 columns, pushing the next incarnation off the panel though it stays selectable by index. Rule: a main panel row never wraps. When the row is wider than the panel, drop the model cell first, then cut the label, and keep the status cell whole; the hands and judges rows follow the same rule if they can wrap today.
- **Verify:** `TestBackend` at 110x36 with two incarnations, one running with a six-figure token count: both rows visible on one line each; at 140x36 the model cell is back. `cargo test -p arx --bin arx panes::` green.

### 1. The two lanes and the visible queue

- **Files:** Modify `crates/arx/src/terminal.rs`, `crates/arx/src/panes/mod.rs`, `crates/arx/src/panes/app.rs`, `crates/arx/src/panes/ui.rs`, `crates/arx/src/main.rs`.
- **Build:** `read_answer` on `Front` and both fronts; the queue on `App`; the request-driven handover between the UI thread and `PanesFront::read`; the strip; the Up rule; `gate_call`'s loop on `read_answer`.
- **Steps:**
  1. Trace how the ask modal's answer travels today (app.rs `handle_modal_key` to `outbox` to `mod.rs` `drain_outbox` to `read`) and split it: answers go on their own channel to `read_answer`; operator lines go to the queue.
  2. `PanesFront::read` sends a want-line request to the UI thread and waits; the UI thread answers with the oldest queued line or, when the queue is empty, the next submitted line. Keep `Poll::Eof` on quit as today.
  3. The strip in ui.rs between the big view and the input, inside the area phase E's height rule already shares; the input cap still counts from the remaining area.
  4. `gate_call` (main.rs, the ask loop) calls `read_answer`; the notice for a bad answer stays for piped mode.
- **Verify:** unit tests: a line submitted while `waiting_since` is set lands in `queue` and not in `outbox`; a want-line request releases the oldest; a modal `a` while two lines are queued reaches `read_answer` and leaves the queue at two; Up with an empty input pulls the newest queued line back; the strip frame through `TestBackend` at 110x36 with two queued lines, and with six. Harness test in `main.rs`'s own module: `gate_call` with a recording front whose `read_answer` yields `a` and whose `read` would yield an operator line allows the call and the operator line is still unread. `cargo test -p arx --bin arx panes::` green.

### 2. The stop

- **Files:** Modify `crates/arx/src/terminal.rs`, `crates/arx/src/claude_code.rs`, `crates/arx/src/main.rs`, `crates/arx/src/events.rs` (one optional field), `crates/arx/src/panes/app.rs`, `crates/arx/src/panes/state.rs`, `crates/arx/src/panes/keys.toml`, `crates/arx/src/panes/keys.rs`, `sidecar/claude-main.mjs`, `CONTEXT.md` (the operator surface section: `q` stops, Ctrl-c quits).
- **Build:** `Control`, `poll_control`, the `run_turn` loop on `recv_timeout` with the closure, the interrupt op in the sidecar, `stopped` on `TurnOutcome`, `TerminalMsg::Stopped`, the notice, the event field, the `stop` and `quit` bindings, `q`'s three cases.
- **Steps:**
  1. Read the SDK's `Query` type in `sidecar/node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts` for `interrupt()` and what the result message says after it; quote it in the verdict.
  2. The sidecar op; then the Rust side. Every caller of `run_turn` passes the closure; the fake and stub bodies are not touched.
  3. `q` in app.rs: resolve focus as phase E's `close` does; main running sends `Control::Stop` over the control channel; a hand or judge row opens the kill confirm; else the status line.
  4. If `sidecar/test/` has a runner, add a case for the interrupt op; if it does not, say so.
- **Verify:** unit tests: `q` with a turn open pushes `Control::Stop`; `q` on a running hand row opens the kill modal; `q` idle sets the status line; a `TerminalMsg::Stopped` after two chunks leaves the main turn and adds the notice. Harness test in `main.rs`'s module: `run_turn` against a scripted sidecar process (the same technique the existing adapter tests use) with the closure yielding `Stop` on the second poll writes exactly one `{"op":"interrupt"}` and returns `stopped: true` when the script answers with `turn_end` carrying it. The event line for a stopped turn contains `"stopped":true` and an unstopped one is byte-identical to today's. Live probe against the real sidecar and the owner's login from a temp dir as the Bounds describe: a prompt that streams for a while, `q` after a few lines, the result message the SDK emitted pasted into the verdict, the notice on the page, and a following turn that works and shows main saw the cut. `cargo test -p arx --bin arx panes::` green.

### 3. Paste

- **Files:** Modify `crates/arx/src/panes/mod.rs`, `crates/arx/src/panes/app.rs`, `crates/arx/src/panes/keys.toml`, `crates/arx/src/panes/keys.rs`, `crates/arx/src/terminal.rs`, `crates/arx/src/claude_code.rs`, `crates/arx/src/main.rs`, `sidecar/claude-main.mjs`, `prompts/main.md` (one sentence), `CONTEXT.md` (the operator surface section: Ctrl-V, images). Create `crates/arx/src/panes/clipboard.rs`.
- **Build:** bracketed paste on and handled; the `Clipboard` trait, the `wl-paste` adapter, the fake; the `paste` action; `Attachment`; the marker filter at submit; `OperatorMsg::Turn`; the prompt op with `images`; the sidecar's content array; the transcript row.
- **Steps:**
  1. Bracketed paste first, with its unit test, before the clipboard.
  2. `clipboard.rs`: `trait Clipboard { fn read(&self) -> Result<Clip, String> }` with `Clip::Text(String)` and `Clip::Image { media_type, bytes }`; the adapter shells `wl-paste --list-types` then `wl-paste --type <mime>`; the fake returns what a test sets.
  3. The action, the file write, the marker, the attachment list; the filter at submit.
  4. The seam, the op, the sidecar. Check the SDK's user message content block types in `sdk.d.ts` and use them.
  5. The `prompts/main.md` sentence and the `CONTEXT.md` lines.
- **Verify:** unit tests: `Event::Paste("a\r\nb")` in insert inserts `a\nb` at the cursor and submits nothing; a paste in normal mode enters insert; `Ctrl-V` with the fake returning a png writes `threads/main.images/1.png` under a temp cwd, inserts `[image 1]` and records the attachment; the fake returning text inserts it; the fake returning a 6 MB image refuses and writes nothing; submit with `[image 1]` deleted from the text sends `Line` and the file is gone; submit with it present sends `Turn` with one attachment. Harness test in `main.rs`'s module: a `Turn` with one image writes a prompt op whose `images` carries the path and media type; a `Line` writes the op it writes today, byte-identical. Live probe from the temp dir: `wl-copy < some.png`, `Ctrl-V` in the front, submit `what is in this image`, the answer pasted into the verdict. `cargo test -p arx --bin arx panes::` green.

## Out of scope

- Mid-turn delivery of a queued line; a queued line always waits for the turn to end.
- Mouse selection and copy out of the panes; mouse capture stays on.
- Stopping a hand or judge other than through phase E's kill path; a stopped hand's partial work is what the kill path leaves.
- Images anywhere but the operator line to main: no images to hands, no image rendering on the page, no image in the trace document.
- X11 clipboards; `wl-paste` is the one adapter, and a machine without it sees the `clipboard:` note.
- `crates/arx/tests/`, the stub sidecar under `crates/arx/tests/fixtures/`, `docs/` other than this brief, `prototypes/`, `arx.toml`: untouched.

Done means: the three scopes' unit tests pass, the totals of `cargo test -p arx` before and after differ only by the tests added here, `cargo test --workspace` is green, `check-config` on the repo's `arx.toml` is byte-identical, piped mode with `front = "panes"` prints what it printed before, the two live probes are pasted, and the tmux capture shows the queue strip, a stop, and an image marker in an operator turn.

## Bounds

- Edits under `crates/arx/src/panes/`, plus `terminal.rs`, `claude_code.rs`, `main.rs`, `events.rs` (the one field), `sidecar/claude-main.mjs` and `sidecar/test/` if a runner exists there, one sentence in `prompts/main.md`, the operator surface lines in `CONTEXT.md`. Nothing else. No new Rust dependency; `base64` is the sidecar's `Buffer`.
- Never `git stash`, never `git checkout --` a file, never revert a hunk you did not write. The owner keeps a live `arx run` open in this repo; never run `arx run` from the repo root and never write `threads/main.jsonl` there. A live check builds a temp dir outside the repo holding a copy of `arx.toml` with `sidecar`, `instructions` and `agents` pointed at the repo by absolute path, keeps `[store] root`, and removes the dir after. The two live probes use the real sidecar and the owner's login; do not substitute the stub for them.
- No inline `#[cfg(test)]` module in a file an integration test `#[path]`-includes (`claude_code.rs`, `container.rs`, `events.rs`, `gate.rs`, `state_object.rs`, `transcript.rs`). Harness tests live in `main.rs`'s module.
- Stub images: after any edit under `crates/arx/tests/fixtures/*-stub/` (none expected) `docker rmi` the tag first. `cargo test -p arx` needs the Docker daemon.

## Verify

1. Before any edit: the phase E tripwire; `cargo test -p arx` green; record totals per binary.
2. After every scope: `cargo build -p arx` and `cargo test -p arx --bin arx panes::`.
3. At the end: `cargo test -p arx` with the same totals plus the added tests, listed by name; `cargo test --workspace`; `check-config` diff empty.
4. Live under tmux against the real sidecar from the temp dir: type a line, then a second and a third while the first runs and watch the strip; Up to pull the third back; `q` mid-turn and read the notice; watch the queued line go as the next turn; `wl-copy` a png, `Ctrl-V`, submit, read the answer; paste a three-line block with the terminal's own paste and see one input with three rows. Paste the captures. Kill the tmux session when done.
5. Piped mode against the stub sidecar prints exactly what it printed before this phase.

## Verdict

Return as text: what was built per scope, the file list with line counts, the `Front` and `OperatorMsg` additions as landed, the SDK's `interrupt()` contract quoted and the result message observed live, the `sidecar/test/` outcome, the test totals before and after per binary, the last 10 lines of `cargo test -p arx`, the tmux captures fenced, the two live probe transcripts, the piped-mode diff (empty), `git status --short`, three things you would change next, and the codex gate outcome with dispositions and log path. Fence field: `gate_run: yes`.

## Review handoff

Polishing class, as phases A to E: dispatch hand-do on this brief after phase E's verdict is in; it self-runs the codex gate on the behaviour-change axis and reports dispositions with the log path. The diff stays uncommitted for the owner.
