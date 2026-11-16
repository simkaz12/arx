# L1-04 — the terminal, thin

Card: `docs/wayfinder/maps/arx-v0/L1-04-terminal-thin.md`. Ruled by D16 (`705e4df2`), D15 (`18fec68a`, `d65f8fe1`, `253334ad`), D02 chunk shape (`72bfc6e9`). A scope that lands against any of them stops and reopens the ruling in the store first.

## Problem / Solution

`arx run` already carries a session — stdin lines in, streamed deltas out, transcript rows written — but the operator surface has no seam. `cmd_run` prints straight into the loop: delta text here, a blank separator there, errors on stderr. The design ruled five surfaces, and three of them arrive with later cards (the gate's ask prompt with L2-02, the checkpoint command with L2-06, the reset-armed notice and orientation report with L3-02). Without a typed channel, each of those lands as another print statement threaded through the loop.

This card fixes the seam: one typed channel from arx to the terminal and one back. A message from arx to the operator is a `TerminalMsg` kind rendered through a single entry point; a message from the operator is an `OperatorMsg` kind read through a single entry point. A later surface is a new kind on an existing channel, never a new channel. The channel is the typed call seam, not a thread — a later async surface can put a queue behind the same types without moving the interface.

## Decisions locked

- arx owns the operator surface under every body; the terminal is arx's front, never the CLI body's TUI (`705e4df2`).
- Stream items crossing a seam are semantically complete chunks assembled inside the adapter (`72bfc6e9`). The existing sidecar `delta` events already conform — text fragments, wire bookkeeping kept sidecar-side. The terminal renders chunks as they arrive; it never sees SDK stream events.
- The canonical reply text is the result-message text (L1-03 codex ruling, already shipped). The terminal reconciles its streamed display to canonical at turn end, so what the terminal showed always matches the transcript's `turn` row.
- Three kinds ship now: `Chunk` (streamed partial of main's reply), `TurnEnd` (canonical text, closes the display), `Notice` (freestanding operator-facing line). `Notice` is the proof kind for the done-when: a kind beyond the reply stream that renders through the untouched channels. The later surfaces land as new variants: gate ask (L2-02), checkpoint command (L2-06), reset-armed notice and orientation report (L3-02) per `18fec68a`/`d65f8fe1`/`253334ad`.
- Input side is typed the same way: `OperatorMsg::Line(String)` is the only kind now; the checkpoint command (L2-06) arrives as a second variant.
- Piped-mode stdout stays byte-identical to L1-03's output. The existing `claude_main.rs` integration tests are the proof — they run unchanged.
- Interactive affordance is a `> ` prompt marker, written only when stdin is a terminal (`std::io::IsTerminal`, stable). Piped runs never see it.
- The renderer writes to `impl Write`, so unit tests render into a buffer. No TUI library, no readline, no history.
- Sidecar protocol, `claude_code.rs`, and `transcript.rs` are untouched. This card consumes their existing interfaces.
- Transcript appends stay before the TurnEnd render (`506f4dd2`) — the durable record wins over display fidelity when stdout breaks. Render-first would invert the failure into shown-but-never-logged, worse for a harness whose transcript is the source of truth. Residual risk accepted: chunk-render failures fail before any append, so the only mismatch window is a TurnEnd render failure after successful or absent streaming, and that path exits 1.

## Execution plan

### Scope 1 — the terminal module

- **Files:** Create `crates/arx/src/terminal.rs`. Modify `crates/arx/src/main.rs` (add `mod terminal;` only, in this scope).
- **Build:** The seam's types and both entry points, with unit tests, nothing wired yet.
- **Steps:**
  1. Define the outbound kinds:
     ```rust
     pub enum TerminalMsg {
         Chunk { text: String },
         TurnEnd { text: String },
         Notice { text: String },
     }
     ```
  2. Define the inbound kind: `pub enum OperatorMsg { Line(String) }`.
  3. Define the renderer: `pub struct Terminal<W: Write>` holding the writer and the text shown for the in-flight turn. One entry point: `pub fn render(&mut self, msg: &TerminalMsg) -> std::io::Result<()>`.
     - `Chunk`: write the text with no newline, flush, accumulate it into the shown-text buffer.
     - `TurnEnd`: if the accumulated shown text equals the canonical text, terminate the line and write the blank turn separator — reproducing `cmd_run`'s current bytes exactly. If nothing streamed, write the canonical text, newline, separator. If the shown text differs from canonical, terminate the line, write the canonical text on its own line, then the separator. Clear the accumulator in every arm.
     - `Notice`: write the text and a newline.
  4. Define the reader: `pub fn read_operator(input: &mut impl BufRead) -> std::io::Result<Option<OperatorMsg>>` — one line read, `None` on EOF, trailing newline stripped, no other interpretation.
  5. Unit tests in-module, rendering into a `Vec<u8>`:
     - `chunks_matching_canonical_close_with_separator_only` — chunks concatenating to the canonical text produce chunk bytes + newline + blank line, canonical printed once.
     - `turn_end_without_chunks_prints_canonical` — no chunks, `TurnEnd` prints text + newline + separator.
     - `mismatched_chunks_are_reconciled_to_canonical` — divergent chunks end with the canonical text on its own line before the separator.
     - `a_second_message_kind_renders_through_the_same_channel` — a `Notice` pushed through the same `render` renders its line; this is the card's done-when proof, and the test may touch nothing but the public seam.
     - `read_operator_returns_line_then_none_at_eof`.
- **Verify:** `cargo test -p arx` — the five new unit tests listed above green; all pre-existing tests green and unchanged. Count them before starting: the suite stands at 67 (26 unit, 6 check_config, 10 claude_main, 25 transcript_log). If it is not 67 at baseline, STOP and report rather than adjusting expectations.

### Scope 2 — cmd_run rewires through the seam

- **Files:** Modify `crates/arx/src/main.rs`.
- **Build:** Every byte `cmd_run` writes to stdout goes through `Terminal::render`; every operator line arrives through `read_operator`; a TTY session gets the prompt marker.
- **Steps:**
  1. In `cmd_run`, construct `Terminal` over stdout once, before the loop.
  2. Replace the stdin `lines()` loop's read with `read_operator`; `None` keeps the current EOF path (shutdown, `ExitCode::SUCCESS`).
  3. Route the `run_turn` `on_delta` callback into `render(&TerminalMsg::Chunk { .. })` (the closure and the `Terminal` need to share access — a `RefCell` or splitting the borrow is fine; the adapter's callback signature is `impl FnMut(&str)` and does not change).
  4. On a completed turn, replace the direct prints with `render(&TerminalMsg::TurnEnd { text: outcome.text.clone() })`. Transcript appends (`append_turn_as("operator", ..)`, `append_turn_as("main", ..)`, `append_usage`) are untouched and keep their current order relative to the render calls.
  5. Before each read, when `std::io::stdin().is_terminal()`, write `> ` to stdout and flush — not through a `TerminalMsg` kind; the prompt is the input channel's affordance, not an arx→operator message.
  6. Errors keep their exact current shape: `eprintln!("error: {err}")`, `adapter.kill()`, `ExitCode::FAILURE`. Stderr is not the terminal channel's business in this card.
- **Verify:** `cargo test -p arx --test claude_main` — all 10 pre-existing integration tests green with no edits to their assertions (the byte-identical proof). `printf 'reply with exactly the word turquoise\n' | cargo run -q -p arx -- run` against the root `arx.toml` prints turquoise and exits 0, with no `> ` anywhere in the output.

### Scope 3 — the done-when, proven

- **Files:** Modify `crates/arx/tests/claude_main.rs`.
- **Build:** The shown-equals-logged check as a regression test, plus the live interactive check.
- **Steps:**
  1. New integration test `terminal_shows_what_the_transcript_logs`: drive the built binary with the stub sidecar (`crates/arx/tests/fixtures/stub-sidecar`, harness pattern as in the existing tests), pipe two prompts, capture stdout, then read the workspace's `threads/main.jsonl` and assert: every `turn` row with `role: "main"` has its `text` present in the captured stdout, and every `turn` row with `role: "operator"` matches a piped prompt line.
  2. Live checks, run and their output quoted in the verdict:
     - Piped, real body: `printf 'reply with exactly the word turquoise\n' | cargo run -q -p arx -- run` → turquoise, exit 0.
     - Interactive, real body, pty via `script`: `printf 'say ok\n' | script -qec './target/debug/arx run ./arx.toml' /dev/null` (build first with `cargo build -p arx`) → output contains the `> ` prompt marker and the streamed reply; exit 0. This is the card's "a session runs from the terminal with no direct claude invocation", exercised on a real terminal device.
- **Verify:** `cargo test -p arx` — full suite green, the new integration test included. Both live commands above with the quoted expected shapes.

## Out of scope

- The other three ruled surfaces: gate ask prompt (L2-02), checkpoint command (L2-06), reset-armed notice and orientation report (L3-02). They arrive as new `TerminalMsg`/`OperatorMsg` variants on this seam.
- Any TUI framework, alternate screen, colors, line editing, input history.
- Changes to the sidecar protocol, `claude_code.rs`, `transcript.rs`, `config.rs`, or the `gauge`/`check-config` commands.
- Multi-thread rendering, async, or an actual queue behind the seam.

Done when the three scopes' Verifies pass: the suite green with the five new unit tests and the shown-equals-logged integration test, piped output byte-identical (pre-existing claude_main tests untouched), and both live checks — piped turquoise and the pty session showing prompt and streamed reply.

## Review handoff

dispatch spec-runner on docs/specs/2026-09-02-l1-04-terminal-thin.md (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
