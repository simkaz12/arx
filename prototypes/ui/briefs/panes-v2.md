# Prototype `panes`, round two: lazygit navigation, parties as panels

Class: prototype, throwaway and exploratory. `gate_run: no`, `gate_reason: prototype`. No codex round.

The owner tried all three prototypes on 2026-09-08 and chose `panes`. Round two rebuilds it in place with the owner's three rulings from that session:

1. Modal keys in the lazygit and nvim manner. `i` enters insert mode to type to main, Esc returns to normal mode, where digits and `h j k l` navigate panels and `?` shows the keys. Every binding lives in a config file so more can be added later.
2. No editor inside the terminal. The owner runs arx inside herdr and opens nvim or lazygit in another tab.
3. The right column is the parties: main, judges, hands, so the operator moves between them freely, with context numbers, model and permissions in the top-right panel.

## Bounds

- Every file lives under `prototypes/ui/panes/`. Rebuild in place; keep what round one got right. Create nothing outside it. Never touch `crates/`, `sidecar/`, `arx.toml`, the workspace `Cargo.toml` or `Cargo.lock`, `docs/`, `prompts/`, or the sibling prototypes.
- Never run `cargo init` or `cargo new` inside the repo: it registers the package in the root workspace and lockfile. The package already exists with an empty `[workspace]` table. After your first cargo command run `git diff -- Cargo.toml Cargo.lock` at the repo root and confirm it is empty; if it is not, `git checkout -- Cargo.toml Cargo.lock` and report it.
- No `arx run`, no `cargo test -p arx`, no Docker. Build only this package, from inside its dir.
- No nerd-font glyphs. Plain Unicode box drawing, bullets and arrows are fine. Target terminal: foot inside herdr, `TERM=xterm-256color`, `COLORTERM=truecolor`.
- `prototypes/ui/SCENARIO.md` stays the script. Invent no data beyond it, with the one extension below (parties).
- Prose, in the README and anywhere else, follows `~/.claude/skills/unslop/SKILL.md`.

## Layout

```
┌ 2 main ────────────────── big view ─────────────────┐┌ 1 session ────────────┐
│ the selected party's page                           ││ model  claude-opus-5   │
│ (main: its transcript, streaming live)              ││ occupancy gauge        │
│                                                     ││ 63 900 / 200 000       │
│                                                     ││ reserve 40 000  cp 2   │
│                                                     ││ gate allow 2 ask 1     │
│                                                     ││ rules +1  · pending 0  │
│                                                     │└────────────────────────┘
│                                                     │┌ 2 main ───────────────┐
│                                                     ││ main #2  sonnet  live  │
│                                                     ││ main #1  opus   ended  │
│                                                     │└────────────────────────┘
│                                                     │┌ 3 judges ─────────────┐
│                                                     ││ h-0008 codex-judge  ✓  │
│                                                     │└────────────────────────┘
│                                                     │┌ 4 hands ──────────────┐
│                                                     ││ h-0007 hand-do  5.9s   │
│┌ input ─────────────────────────────────────────────┐│                        │
││ >                                                  ││                        │
│└────────────────────────────────────────────────────┘└────────────────────────┘
 NORMAL  2 main   i insert · 1-4 panels · j/k move · Enter open · ? keys · q quit
```

The big view stays left, as in round one. The right column is four numbered panels. The bottom line is the mode, the focused panel, and the first few bindings for that context, the way lazygit's bottom line works.

## Parties

A party is anything that ran a model this session: each main incarnation, each judge, each hand. The scenario yields two mains (main #1 on claude-opus-5 until the reset, main #2 on claude-sonnet-5 from checkpoint 3), one judge (`h-0008`), one hand (`h-0007`). Each has a page the big view shows when it is selected:

- **main page**: that main's transcript, exactly round one's transcript rendering (operator `>` lines, markdown-lite reply, dim tool-call lines with admission, dispatch lines, notices, the state object block on `/reset`), streaming live for the live main, frozen for an ended one with a header line saying when and why it ended (`ended: reset from checkpoint 3`).
- **hand page** and **judge page**: a header with id, agent, role, body, container image, model, spawn time and status; the elapsed timer while running; on delivery the outcome (verdict / stop / killed), the verdict text in full, the trace handle, bytes, `elapsed_ms`, `lag_ms`; for a judge the schema line (ok / malformed). While running, the page says tap is the read path and shows nothing else, because the harness has nothing else.

The panels list parties one per line: main incarnations newest first with model and `live`/`ended`; judges and hands with id, agent, and a status cell that ticks `4.2s` while running and settles to `✓ 5.9s` on a verdict, `stop` or `killed` otherwise. Moving the cursor in a panel selects that party into the big view at once, like lazygit's main view following the cursor. The big view's title names the selected party.

The **session** panel (1) carries: model, the occupancy gauge against the window with the reserve ceiling and nudge tick as in round one, the numbers under it, checkpoint number with delta, turn count, and permissions: gate allow / deny / ask counts, durable rules appended this session, and a pending ask if one is open. Selecting panel 1 shows a session page in the big view: the same numbers in full plus the model list with the current one marked, scope, store root, grounding, and the gate rules added.

## Modes and keys

Two modes. **Normal** is the default; keys navigate. **Insert** puts the cursor in the input box; every key types until Esc. A gate ask or a picker is a modal that takes keys regardless of mode and returns to the mode it interrupted.

Bindings live in `keys.toml` beside `Cargo.toml`, loaded at startup, with the defaults embedded in the binary so a missing file still runs and a partial file overrides only what it names. The `?` overlay renders from the loaded table, grouped by context, so a rebinding shows up there without any other change. The README says where the file is and shows its format. Default table:

```toml
[normal]
insert        = ["i", "Enter"]        # Enter with the input focused
panel_1       = ["1"]
panel_2       = ["2"]
panel_3       = ["3"]
panel_4       = ["4"]
next_panel    = ["l", "Tab"]
prev_panel    = ["h", "BackTab"]
down          = ["j", "Down"]
up            = ["k", "Up"]
top           = ["g"]                 # gg in the big view; single g in a panel
bottom        = ["G"]
page_down     = ["Ctrl-d", "PageDown"]
page_up       = ["Ctrl-u", "PageUp"]
open          = ["Enter"]             # in a panel: focus the big view on the party
back          = ["Esc"]               # big view -> the panel it came from
help          = ["?"]
quit          = ["q", "Ctrl-c"]
abort         = ["Esc"]               # while main streams, in any mode

[insert]
submit        = ["Enter"]
newline       = ["Alt-Enter"]
normal        = ["Esc"]
history_up    = ["Up"]
history_down  = ["Down"]
complete      = ["Tab"]

[modal]
confirm       = ["Enter"]
cancel        = ["Esc"]
down          = ["j", "Down"]
up            = ["k", "Up"]
```

Esc while main is streaming aborts the step, in either mode, and the bottom line says so while it applies. `q` in normal mode quits without a confirm; Ctrl-C quits from anywhere. The mouse wheel scrolls whatever is under the pointer.

The input box is at the bottom of the big view. In normal mode it shows dim with the buffer kept; in insert mode its border highlights and the cursor sits in it. Slash commands, Tab completion, history and the `/` popup are as in round one. Submitting returns to normal mode? No: stay in insert after submit, since the operator usually types the next line; Esc leaves.

## Carry over from round one

Keep the scenario thread, the state reducer, the markdown-lite renderer, the gauge, the gate-ask modal (`a A d D`, arrows, note field), the `/model` picker, the reset confirm, `--auto`, `--fast`, `--snapshot` at 110x36 through `TestBackend`. Fix the three things round one named: the spinner and `esc to abort` hint dropping between the two halves of `/checkpoint` (model a turn as possibly several streamed parts), the hands table wrapping under 110 columns (short status vocabulary, drop a column when narrow), and the per-frame scroll math (cache wrapped-row counts per party page, invalidate on resize or append). Drop the events panel and Ctrl-E.

`--snapshot` prints the seven scene frames as before, plus these frames: the `?` overlay, panel 1 selected with its session page, the hand page for `h-0007` while running (scene 3, before delivery) and after, the judge page for `h-0008` after delivery, main #1's frozen page after the reset. Label each frame.

## Verdict

Return as text: what was built, the last 10 lines of `cargo build`, the root `git diff -- Cargo.toml Cargo.lock` result, the full `--snapshot` output fenced, the final `keys.toml`, and three things you would change next. Fence fields: `gate_run: no`, `gate_reason: prototype`.
