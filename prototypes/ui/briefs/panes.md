# Prototype `panes`: the machinery in its own column

Class: prototype, throwaway and exploratory. `gate_run: no`, `gate_reason: prototype`. No codex round.

## Bounds

- Every file lives under `prototypes/ui/panes/`. Create nothing outside it. Never touch `crates/`, `sidecar/`, `arx.toml`, the workspace `Cargo.toml` or `Cargo.lock`, `docs/`, `prompts/`.
- The package is its own cargo root: its `Cargo.toml` carries an empty `[workspace]` table so the arx workspace does not claim it. Put a `.gitignore` with `target/` inside the package dir.
- No `arx run`, no `cargo test -p arx`, no Docker. Build only this package, from inside its dir.
- No nerd-font glyphs. Plain Unicode box drawing, bullets and arrows are fine. The target terminal is foot inside herdr, `TERM=xterm-256color`, `COLORTERM=truecolor`; degrade sanely to 256 colours.
- Read `prototypes/ui/SCENARIO.md` first and play it exactly. Invent no data beyond it.
- Prose, in the README and anywhere else, follows `~/.claude/skills/unslop/SKILL.md`.

## Engine shape, shared by all three prototypes

A `scenario` module owns the scripted session: an event enum (`Chunk`, `TurnEnd`, `ToolCall`, `ToolResult` with its admission, `GateAsk`, `GateVerdict`, `Spawn`, `Delivered`, `Notice`, `Usage`, `Checkpoint`, `ResetArmed`, `ResetConfirm`, `Orientation`, `ModelList`, `ModelSwitched`, `ProviderError`, `Aborted`) and the session state (model and model list, scope, store root, occupancy, window, reserve, checkpoint number, hands in flight, last usage). It runs on its own thread and sends events over an mpsc channel with the scenario's delays. The UI thread renders and feeds operator lines, keys and ask answers back over a second channel. This mirrors the real loop, one typed channel each way, so the winning prototype ports onto the L1-04 seam without redesign.

Flags: `--auto` plays every scene without typing, `--fast` drops the delays, `--snapshot` renders each scene's final frame as plain text to stdout and exits with no tty, through ratatui's `TestBackend` at 110x36. Snapshots are how the reviewer sees the work; keep them faithful.

## The direction

arx is a harness, and the operator runs hands, a judge, a gate and a store as much as they chat with main. This prototype gives that machinery a home of its own so the conversation stays clean. It is the full-screen answer; `inline` is the scrollback answer, `quiet` the no-TUI answer.

- ratatui on the alternate screen. Left column, about two thirds: the transcript, scrollable with PageUp/PageDown and the mouse wheel, auto-following while at the bottom, and the input box under it. Right column: stacked panels. Bottom row: a one-line key hint bar.
- Right column panels, top to bottom:
  - **State**: model, scope, store root, an occupancy gauge against the window with the reserve ceiling drawn as a mark and the nudge threshold as a tick, checkpoint number with its delta, turn count.
  - **Hands**: a table of every dispatch this session: id, agent, role, body, model, status. Status ticks `running 4.2s` while in flight and settles to `verdict 5.9s` on delivery; a delivered row is selectable with arrows when the panel has focus, and Enter opens its verdict text in a modal.
  - **Gate**: allow / deny / ask counts for the session, the last few verdicts one per line with the rule that decided, and any durable rule appended by `A`/`D` this session.
  - **Events**: a tail of the last dozen events in the one-line shape `arx events` prints (`kind field=value …`), newest at the bottom. Ctrl-E hides or shows it.
- Transcript rendering: operator lines marked `>`; main's reply in markdown-lite (bold headings, bullets, code fences in a dim box, inline code coloured). Tool calls stay in the transcript as a single dim line each, `· Read crates/arx/src/terminal.rs → inline 6.3 KB`, because they are part of the turn; dispatch lines say only `· dispatched h-0007 hand-do` and point at the Hands panel for everything else. A delivery while idle appears as one line in the transcript and updates its Hands row at the same instant.
- In-flight turn: spinner with a verb and elapsed seconds at the transcript's tail, then the stream. `esc to abort` in the hint bar while streaming.
- Input: a bordered box, one to five lines, Up/Down history, Tab completes a slash command, `/` shows the four commands in a popup above the box.
- Gate ask and `/model` picker: centred modals. Ask shows tool, subject, the four choices selectable with `a A d D` or arrows, a note field, Enter. Picker shows the three models, current marked, arrows or `j`/`k`, digits, Enter, Esc.
- Focus: input by default; Tab with an empty buffer cycles focus through Hands and back; the focused panel gets a highlighted border.
- Notices: dim `[arx]` lines in the transcript; nudge and reset armed in amber and mirrored as a colour change on the gauge; errors red. Reset confirm shows the state object in a bordered block in the transcript and waits for `ok`.
- Ctrl-C quits and restores the terminal. Resize relays out without corruption; below 80 columns the right column collapses and the State panel becomes a single line above the input.

## Verdict

Return as text: what was built, the last 10 lines of `cargo build`, the full `--snapshot` output fenced, a table of every keybinding, and three things you would change if this direction wins. Fence fields: `gate_run: no`, `gate_reason: prototype`.
