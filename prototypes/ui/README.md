# Terminal prototypes

Three runnable mock fronts for `arx run`, built 2026-09-08 to compare directions before the real
terminal is redesigned. They share one scripted session (`SCENARIO.md`) and the same data the
harness holds today, so the only thing that differs is the rendering. None of them touches
`crates/arx`; each is its own cargo root with an empty `[workspace]` table, and the repo's
`Cargo.toml` and `Cargo.lock` are unchanged. The briefs that produced them are under `briefs/`.

## Chosen: `panes`, round two

The owner tried all three on 2026-09-08 and chose `panes` (store decision `6d3744ae`). It was
then rebuilt in place from `briefs/panes-v2.md` with the owner's three rulings: lazygit and nvim
style modal keys, no editor inside the terminal (herdr tabs cover that), and the right column as
the parties so the operator moves between main, judges and hands freely.

```
cd prototypes/ui/panes
cargo run                  # interactive; press i to type, Esc for normal mode, ? for keys
cargo run -- --auto        # plays the session, then stays up for you to navigate
```

Normal mode: `1`-`4` focus a panel (session, main, judges, hands), `h`/`l` or Tab cycle, `j`/`k`
move and the big view follows, Enter hands the cursor to the big view for `gg`/`G`/`Ctrl-d`/
`Ctrl-u` scrolling, Esc goes back, `?` shows every binding, `q` quits. Insert mode: `i`, type,
Enter sends and stays in insert, Esc leaves. Esc while main streams aborts in either mode.
Bindings live in `panes/keys.toml`; the defaults are embedded, a partial file overrides only what
it names, and the `?` overlay renders from whatever loaded. The file is read once at start.

`inline` and `quiet` stay as they were, for reference.

## Chosen: `panes`, round three

The owner tried round two ("looks ok, moves nicely") and asked for two things, both UI only
(store decision `5be77638`): a hand's or judge's page splits so the operator can see the brief
next to what came back, and the big view stops pulling the transcript away from the operator
while main streams.

**The split page.** Selecting a hand or a judge splits the big view top and bottom, with a
horizontal border between them. The title still names the party. The top pane, `brief`, takes
about a third of the height: one header line (id, agent, role, body, image, model, spawn time),
then the brief the party was dispatched with, rendered through the same markdown-lite renderer
the transcript uses. The bottom pane, `turns`, takes the rest: `running · 4.2s` while the party
is still working, the stop question in full if it stopped there, or, once delivered, the swept
trace rendered as turns — assistant text, dim one-line tool calls, dim truncated tool results —
followed by a verdict block (outcome, verdict text, trace handle, bytes, `elapsed_ms`, `lag_ms`,
and the schema line for a judge). Main's page and the session page never split; only a hand or a
judge has a brief and a trace to show side by side.

**Pane keys.** Enter from a panel opens the split page with the bottom pane focused — usually
what the operator wants first, what came back. `Tab` swaps focus between the two panes;
outside a split page `Tab` still cycles panels, as before. `j`/`k`, `gg`/`G`, `Ctrl-d`/`Ctrl-u`
and the mouse wheel act on whichever pane is focused. Esc returns to the panel the operator came
from. The focused pane's border lights up cyan and bold, the other stays dim gray — the same
contrast the input box uses between insert and normal mode. The bottom line on a split page reads
`NORMAL  4 hands · h-0007   Tab pane · j/k · gg/G · Esc back · ? keys`. The swap lives in its own
`[split]` section of `keys.toml`, so a rebinding shows up under its own heading in the `?`
overlay rather than folded into `normal`.

**Scroll memory.** Round two kept one scroll offset for the whole session; round three keeps one
per pane instead — the main transcript, the session page, and each half of every hand's or
judge's split page — and nothing resets it when the operator moves away and back. A pane is
either following (the bottom stays in view as text appends) or anchored at a row. Scrolling up
from the bottom anchors it; the moment that happens, appended rows stop moving what's on screen.
That's the point of the change — watching main's reply build up no longer costs the operator
their place in the transcript above it. While anchored with rows appended below the view, the
pane's bottom edge carries a dim, right-aligned `12 new lines · G`, counting what showed up since
the anchor.
`G` goes back to following and drops the hint; so does scrolling down until the bottom is in
view. `gg` anchors at the top. Wrapped-row counts are cached per pane, invalidated on resize or
append, and the anchor is a row index into that cache — so a resize keeps the same row in view
instead of drifting with it, the bug round two had.

**Scenario.** `SCENARIO.md` carries a new section, `Scene 3, traces`, scripting what `h-0007` and
`h-0008` actually did: a one-line plan, the tool calls and results, then the verdict each already
had. The turns panes render straight from that script.

## Run

Each prototype builds and runs from its own directory:

```
cd prototypes/ui/inline   # or panes, quiet
cargo run                  # interactive: every Enter plays the next scene
cargo run -- --auto        # plays the whole session at a realistic pace, no typing
cargo run -- --auto --fast # same, without the delays
cargo run -- --snapshot    # prints each scene's final frame as text and exits
```

In interactive mode anything you type is echoed as the operator turn, except the commands the
scenario names, which have to be typed as written: `/model`, `/checkpoint`, `/reset`, `ok`,
`/reste`. Esc aborts a stream, Ctrl-C quits. At the end of `--auto` every prototype stays at the
prompt so you can look around; Ctrl-C leaves.

For `quiet` there is one more run worth doing, since piped mode is that direction's argument:

```
cargo run -- --auto --fast | cat
```

## The three directions

| | `inline` | `panes` | `quiet` |
|---|---|---|---|
| Shape | Claude Code's: scrollback above, a live area at the bottom | full-screen dashboard on the alternate screen | line mode as today, with editing, colour and a status line |
| Library | ratatui inline viewport + crossterm | ratatui + crossterm | crossterm only |
| Transcript | native scrollback, selectable, survives quitting | a scrolling pane, gone on quit | native scrollback |
| Tool calls, admission | `⏺ Read …` with a `⎿ inline · 6.3 KB` child line | one dim line per call in the transcript | one dim gutter line per call |
| Dispatches, deliveries | `⏺ dispatch …` with a ticking timer, delivery inserted the moment it lands | a Hands table with live status; delivery is one transcript line plus the row update | `⇢` on dispatch, `⇠` on delivery, timer in the status line |
| Gate, state, events | status line only | Gate, State and Events panels with an occupancy gauge | status line only |
| Asks and `/model` | bordered panels in the live area, keys `a A d D`, arrows, digits | centred modals | single-key ask; a three-line picker drawn under the prompt |
| Piped stdin | not addressed | not addressed | byte-identical to today's `terminal.rs` shape |
| Source | 2 129 lines | 1 917 lines | 1 963 lines |

## What was checked

- Every prototype's `--snapshot` output is in its builder's verdict and reproduces byte for byte
  across runs.
- All three were run under a pseudo-terminal with `--auto --fast`: `panes` and `quiet` play the
  seven scenes and settle at the prompt with no panic. `inline` needs a terminal that answers the
  cursor-position query the inline viewport sends; under a bare `script` pty it panics at start,
  under a pty that answers it plays through cleanly. foot answers, so it should run there; that is
  the one claim not proven on this machine.
- `inline` sends that cursor-position query on roughly every frame, about two hundred a second
  under `--fast`. It works, but it is a round trip to the terminal each time, and would want
  caching if this direction wins.
- Interactive key handling was driven by hand for `panes` (tmux) and `quiet` (script plus a VT
  emulator). For `inline` the keyboard paths are coded but were not driven by a live keystroke;
  its builder had no tty.

## Proposals riding along in all three

The scenario asked every prototype to show a few things the binary does not do today. They are
behaviour changes, not rendering, and each wants its own ruling before any front adopts it:

- a delivery shown the instant it lands, while the operator is idle. Today `cmd_run` blocks on
  stdin between turns and drains deliveries only after the next line; showing them live means
  reading input on its own thread.
- Esc to abort an in-flight step. `CONTEXT.md` names it; the loop has no key handling yet.
- a transient provider error shown as a notice and retried once, where today every error is
  fatal and exits the process.
- a mistyped slash command answered with a hint instead of being sent to main as chat.
- `/model` as a picker, already ruled by the owner's stated preference (store fact `bb531613`).

## Known gaps

- `inline` and `panes` do not address piped mode; the real terminal has to keep it byte-identical
  under the eleven `claude_main` integration tests, as `quiet` does.
- `quiet`'s piped sample shows the pre-tool sentence twice around the ask: that is today's
  reconciliation rule reproduced faithfully (streamed text differs from canonical, so canonical is
  reprinted), not a prototype bug.
- `panes` drops the `esc to abort` hint between the two halves of `/checkpoint`, and its Hands
  table wraps its status column under 110 columns.
- The reset confirm's "any other line rides into the new main's first turn" path is a stub in
  `inline`; the scripted session only ever types `ok`.
