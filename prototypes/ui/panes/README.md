# panes, round two

Full-screen ratatui front for the arx scenario. Round one put the harness machinery in one
column beside the transcript. Round two, built from the owner's 2026-09-08 rulings, keeps that
split but rebuilds two things: modal keys in the lazygit/nvim manner, and the right column as
parties — main, judges, hands — rather than a fixed set of gauges.

## Run it

```
cargo build             # from inside this directory; the empty [workspace] table keeps
                         # the arx workspace from claiming the package
cargo run                # interactive, types the scenario yourself
cargo run -- --auto      # plays the whole scenario with realistic pacing
cargo run -- --fast      # drops the delays (combine with --auto for a fast run-through)
cargo run -- --snapshot  # renders each scene's final frame as plain text and exits, no tty
```

## Modes

Two modes, normal and insert, plus modals that take keys regardless of either.

- **Normal** (default): digits and `h j k l` navigate; `i` or `Enter` (from the big view) opens
  insert; `?` shows the key table; `q` quits.
- **Insert**: every key types into the input box until `Esc`. Submitting a line keeps insert
  open, since the usual next move is another line.
- **Modal** (a gate ask, the `/model` picker): intercepts keys regardless of mode and hands
  control back to whichever mode was interrupted once it resolves.

`Esc` aborts the in-flight turn whenever main is streaming, in either mode, before anything else
runs. `Ctrl-C` quits from anywhere.

The right column is four numbered panels: `1` session, `2` main, `3` judges, `4` hands. Moving
the cursor in a panel drives the big view onto that row at once, the way lazygit's main view
follows the panel cursor. `Enter` on a panel hands cursor control to the big view itself, for
scrolling; `Esc` hands it back. Inside the big view, `g` needs a second `g` within the same beat
to jump to the top (`gg`, vim-style); a bare `g` inside a panel jumps straight there, since a
panel has no notion of "you might mean something else."

## Keybindings

Every binding lives in `keys.toml`, next to `Cargo.toml`, read once at startup relative to the
process's working directory (so `cargo run` from this directory finds it). The file has three
tables — `[normal]`, `[insert]`, `[modal]` — each mapping an action name to a list of keys:

```toml
[normal]
insert        = ["i", "Enter"]
down          = ["j", "Down"]
...
```

A missing file falls back to the defaults compiled into the binary (the same file, embedded via
`include_str!`, so the shipped defaults and this doc never drift apart). A file that only
mentions some actions overrides just those; anything it doesn't name keeps its default. The `?`
overlay renders straight from the loaded table, grouped by context, so a rebinding shows up there
with no other change needed. The action names themselves (`insert`, `panel_1`, `down`, `top`, …)
are fixed by the app; only which keys fire them is configurable.

## Parties

A party is anything that ran a model this session: each main incarnation, each judge, each hand.
Selecting one in a panel shows its page in the big view:

- **main**: that incarnation's transcript, streaming live if it's the current one, frozen with an
  `ended: <reason>` header if a `/reset` retired it.
- **hand / judge**: a header (id, agent, role, body, container image, model, spawn time), a
  running elapsed timer while in flight, and on delivery the outcome, verdict text, trace handle,
  bytes, `elapsed_ms`, `lag_ms` — plus a schema line for judges. While running, the page says so
  and shows nothing else, because the harness has nothing else to show it.
- **session**: model, the occupancy gauge with its reserve ceiling and nudge tick, checkpoint
  number and delta, turn count, hands in flight, gate allow/deny/ask counts with the recent
  decisions and durable rules, a pending ask if one is open, the last turn's usage, and — on the
  full page — the model list with the current one marked, scope, store root, and grounding.

## Shape

- `src/scenario.rs` — the scripted session: an `Event` enum matching what `terminal.rs` and its
  call sites emit today, plus `ExchangeEnd` (round two's own bookkeeping: an exchange the operator
  is waiting on can carry more than one streamed part, e.g. `/checkpoint`'s state object and
  truth-check, so this fires once at the true end rather than after every `TurnEnd`). Runs on its
  own thread, one scene per submitted line.
- `src/state.rs` — `SessionState`: main incarnations (each with its own transcript), hands and
  judges as separate lists, gate tallies, and the session numbers. Holds no UI interaction state.
- `src/keys.rs` — loads and parses `keys.toml` (a small hand-rolled reader, no toml/serde
  dependency), and turns a crossterm key event into the same key-name vocabulary the file uses.
- `src/app.rs` — `App`: mode, which panel or the big view has cursor focus, per-panel selection,
  scroll position (with a small cache so a long transcript's wrapped-row count isn't recomputed
  every frame — only on resize or once the page grows), the open modal, and the channel handles.
  `AutoDriver` feeds the same scripted lines and key answers on a timer for `--auto`.
- `src/ui.rs` — the ratatui layout: the big view and its input box, the four right-column panels,
  the hint bar, and the ask / model-picker / help overlays.
- `src/main.rs` — CLI flags, the interactive event loop (key and mouse handling delegated to
  `App`), and the `--snapshot` driver.

## What changed behaviour, per SCENARIO.md

Two scenes ask for behaviour the real loop doesn't have today, both flagged there as proposals,
carried over from round one:

- **Scene 6, the provider error.** Today an `overloaded_error` is fatal; this prototype shows it
  as a recoverable notice, retries once after a couple of seconds, and folds the turn back in with
  `Back. Where were we?`.
- **Scene 7, an unrecognised slash command.** Today `/reste` rides into main as chat. This
  prototype intercepts it locally, shows a hint listing the four commands, and never sends the
  line.

## Three things to change if this direction wins

1. **Per-party scroll position isn't remembered.** Leaving a page and coming back resets it to
   top (ended party) or bottom (live party) rather than wherever you left it. Fine at this
   session's length; a long-lived session would want the scroll position kept per party.
2. **Mouse hit-testing trusts the previous frame's layout.** It reads the panel and big-view
   rectangles captured during the last draw, so a scroll wheel event that lands in the same frame
   as a resize can be off by one render. Cheap to fix by clamping the click to the new frame
   instead of trusting the cached one, just not done yet.
3. **`keys.toml` is read once at startup.** Editing it mid-session has no effect until the next
   run. A real build would want a reload binding or a file-watch, especially once more actions
   accrete and rebinding becomes a normal thing to do without restarting.
