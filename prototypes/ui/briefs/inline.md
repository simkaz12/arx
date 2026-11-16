# Prototype `inline`: Claude Code's shape

Class: prototype, throwaway and exploratory. `gate_run: no`, `gate_reason: prototype`. No codex round.

## Bounds

- Every file lives under `prototypes/ui/inline/`. Create nothing outside it. Never touch `crates/`, `sidecar/`, `arx.toml`, the workspace `Cargo.toml` or `Cargo.lock`, `docs/`, `prompts/`.
- The package is its own cargo root: its `Cargo.toml` carries an empty `[workspace]` table so the arx workspace does not claim it. Put a `.gitignore` with `target/` inside the package dir.
- No `arx run`, no `cargo test -p arx`, no Docker. Build only this package, from inside its dir.
- No nerd-font glyphs. Plain Unicode box drawing, bullets and arrows are fine. The target terminal is foot inside herdr, `TERM=xterm-256color`, `COLORTERM=truecolor`; degrade sanely to 256 colours.
- Read `prototypes/ui/SCENARIO.md` first and play it exactly. Invent no data beyond it.
- Prose, in the README and anywhere else, follows `~/.claude/skills/unslop/SKILL.md`.

## Engine shape, shared by all three prototypes

A `scenario` module owns the scripted session: an event enum (`Chunk`, `TurnEnd`, `ToolCall`, `ToolResult` with its admission, `GateAsk`, `GateVerdict`, `Spawn`, `Delivered`, `Notice`, `Usage`, `Checkpoint`, `ResetArmed`, `ResetConfirm`, `Orientation`, `ModelList`, `ModelSwitched`, `ProviderError`, `Aborted`) and the session state (model and model list, scope, store root, occupancy, window, reserve, checkpoint number, hands in flight, last usage). It runs on its own thread and sends events over an mpsc channel with the scenario's delays. The UI thread renders and feeds operator lines, keys and ask answers back over a second channel. This mirrors the real loop, one typed channel each way, so the winning prototype ports onto the L1-04 seam without redesign.

Flags: `--auto` plays every scene without typing, `--fast` drops the delays, `--snapshot` renders each scene's final frame as plain text to stdout and exits with no tty. For this prototype a snapshot is the scrollback lines the scene pushed plus the live area, rendered through ratatui's `TestBackend` at 110x36. Snapshots are how the reviewer sees the work; keep them faithful to what a real terminal would show.

## The direction

The owner's stated bar is Claude Code's own terminal (store fact `44afcdc8`). This prototype takes that shape without copying it line for line.

- ratatui with `Viewport::Inline` and `Terminal::insert_before`. Finished content scrolls into the terminal's native scrollback and stays selectable; no alternate screen. The live area at the bottom holds three things: the in-flight turn, the input box, the status line.
- In-flight turn: while waiting for the first token, a spinner with a verb, the elapsed seconds and an `esc to abort` hint. Then the streamed text, re-wrapped on resize. Below it, any tool call or dispatch in progress with a running timer.
- Input box: a rounded border, a `>` marker, grows to five lines, Up/Down walks history when the buffer is empty, Tab completes a slash command, typing `/` opens a small completion list above the box showing the four commands with one-line descriptions.
- Status line, dim, under the input: model · scope · occupancy against window with the reserve mark · checkpoint number · hands in flight · last turn's tokens and elapsed. It updates while idle, for instance when a hand delivers.
- Transcript rendering in scrollback: operator lines with the `>` marker in a distinct colour; main's reply in markdown-lite (bold headings, bullets, code fences in a dim box, inline code coloured; `tui-markdown` if it compiles against your ratatui, else hand-roll the four cases). Tool calls as one-liners in the shape `⏺ Read crates/arx/src/terminal.rs` with a child line `⎿ inline · 6.3 KB`; an externalized result reads `⎿ externalized 14.8 KB → trace/2026-09-08/t07-bash-3.txt` and its preview under that. A dispatch reads `⏺ dispatch hand-do · claude-code · claude-sonnet-5 · h-0007`, its child line ticking `⎿ running 4.2s` in the live area and settling to `⎿ verdict in 5.9s → trace/…` once delivered, with the verdict text under it.
- Deliveries that land while the operator is idle go into scrollback right away, above the input, with a soft mark so they read as arrivals, not replies.
- Gate ask: a panel in the live area with the tool, the subject, the four choices as a selectable list (keys `a A d D`, arrows too), a note field, Enter to confirm. `A`/`D` echo the durable rule that was appended.
- `/model` picker: same panel style, three models, current one marked, arrows or `j`/`k`, digits, Enter, Esc.
- Notices as dim `[arx]` lines; nudge and reset armed in amber; errors in red. The reset confirm shows the state object in a bordered block and waits for `ok`.
- Esc aborts a stream, Ctrl-C quits, resize re-wraps the live area without corrupting scrollback.

## Verdict

Return as text: what was built, the last 10 lines of `cargo build`, the full `--snapshot` output fenced, a table of every keybinding, and three things you would change if this direction wins. Fence fields: `gate_run: no`, `gate_reason: prototype`.
