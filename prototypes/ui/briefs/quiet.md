# Prototype `quiet`: line mode, kept, and made usable

Class: prototype, throwaway and exploratory. `gate_run: no`, `gate_reason: prototype`. No codex round.

## Bounds

- Every file lives under `prototypes/ui/quiet/`. Create nothing outside it. Never touch `crates/`, `sidecar/`, `arx.toml`, the workspace `Cargo.toml` or `Cargo.lock`, `docs/`, `prompts/`.
- The package is its own cargo root: its `Cargo.toml` carries an empty `[workspace]` table so the arx workspace does not claim it. Put a `.gitignore` with `target/` inside the package dir.
- No `arx run`, no `cargo test -p arx`, no Docker. Build only this package, from inside its dir.
- No nerd-font glyphs. Plain Unicode box drawing, bullets and arrows are fine. The target terminal is foot inside herdr, `TERM=xterm-256color`, `COLORTERM=truecolor`; degrade sanely to 256 colours.
- Read `prototypes/ui/SCENARIO.md` first and play it exactly. Invent no data beyond it.
- Prose, in the README and anywhere else, follows `~/.claude/skills/unslop/SKILL.md`.

## Engine shape, shared by all three prototypes

A `scenario` module owns the scripted session: an event enum (`Chunk`, `TurnEnd`, `ToolCall`, `ToolResult` with its admission, `GateAsk`, `GateVerdict`, `Spawn`, `Delivered`, `Notice`, `Usage`, `Checkpoint`, `ResetArmed`, `ResetConfirm`, `Orientation`, `ModelList`, `ModelSwitched`, `ProviderError`, `Aborted`) and the session state (model and model list, scope, store root, occupancy, window, reserve, checkpoint number, hands in flight, last usage). It runs on its own thread and sends events over an mpsc channel with the scenario's delays. The UI thread renders and feeds operator lines, keys and ask answers back over a second channel. This mirrors the real loop, one typed channel each way, so the winning prototype ports onto the L1-04 seam without redesign.

Flags: `--auto` plays every scene without typing, `--fast` drops the delays, `--snapshot` plays every scene with no tty and prints what the terminal would have shown, ANSI stripped, scene by scene. Snapshots are how the reviewer sees the work; keep them faithful.

## The direction

No TUI library, no alternate screen, no viewport: the terminal stays a stream of lines the way it is today, and every improvement is something a line stream can carry. This is the "how far without a TUI" point of the triangle, and its strength is that piped mode stays byte-identical to today's output while a tty gets the polish.

- crossterm only, for raw-mode input and colour. A small line editor of your own: cursor movement, Home/End, word delete, Up/Down history, Tab completion of the four slash commands, multi-line by Alt-Enter. When an event arrives while the operator is typing (a delivery, a notice), clear the prompt line, print the event, redraw the prompt with the buffer intact. That redraw is the heart of this direction; get it right.
- Prompt: a two-line affair. Above the input, a dim status line: model · scope · occupancy against window with the reserve mark · checkpoint number · hands in flight · last turn's tokens and elapsed. It redraws in place while idle whenever state changes. Then the `›` marker and the buffer.
- Waiting for the first token: one dim line `… thinking 0.7s  (esc to abort)` overwritten in place, replaced by the stream when it starts. During the stream, the elapsed timer is not shown; at turn end a dim trailer line carries tokens and elapsed.
- Output gutters, two spaces wide, so the roles read at a glance without colour: operator lines under `›`, main's text under a blank gutter, tool calls as dim `  · Read crates/arx/src/terminal.rs   inline 6.3 KB`, an externalized result `  · Bash cargo test …   externalized 14.8 KB → trace/2026-09-08/t07-bash-3.txt` with its preview on the next line, dispatch `  ⇢ h-0007 hand-do · claude-code · claude-sonnet-5` and delivery `  ⇠ h-0007 verdict 5.9s → trace/2026-09-08/h-0007.md` with the verdict text indented beneath. While a hand runs and the operator is idle, the status line's hands field ticks its elapsed.
- Markdown-lite for main's reply: bold headings, bullets kept, code fences indented and dim, inline code coloured. Re-wrap at the terminal width.
- Gate ask: the header line as today, `[arx] ask: Bash cargo test …`, then a single-key read for `a A d D` with the legend on one line, then an optional note prompt. `A`/`D` echo the durable rule appended. Esc denies once.
- `/model`: a raw-mode picker drawn in place below the prompt, three lines, current marked, arrows or `j`/`k`, digits, Enter, Esc; the picker lines are erased after the choice and the switch notice printed.
- Notices as `[arx]` lines, dim for routine, amber for nudge and reset armed, red for errors. The reset confirm prints the state object indented, then the confirm line, and waits for `ok`.
- Piped stdin, no tty: no status line, no marker, no colour, no spinner, no picker (`/model` prints the list as today). Prove it with `./target/debug/quiet --auto --fast | cat` and include the first 30 lines in the verdict.
- Esc aborts a stream, Ctrl-C on an empty buffer quits, Ctrl-C with text clears the buffer. Resize re-wraps nothing already printed; only the status line and prompt redraw.

## Verdict

Return as text: what was built, the last 10 lines of `cargo build`, the full `--snapshot` output fenced, the piped-mode sample, a table of every keybinding, and three things you would change if this direction wins. Fence fields: `gate_run: no`, `gate_reason: prototype`.
