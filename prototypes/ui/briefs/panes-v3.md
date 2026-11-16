# Prototype `panes`, round three: split party page, scroll memory

Class: prototype, throwaway and exploratory. `gate_run: no`, `gate_reason: prototype`. No codex round.

The owner tried round two ("looks ok, moves nicely") and asked for two things. Both were grilled on 2026-09-08 (store decision `5be77638`) and both are rendering only: the prototype shows nothing the harness does not already hold. No new behaviour rides along. Read `prototypes/ui/README.md` and `briefs/panes-v2.md` first; round two's layout, modes, keys and pages stay as they are unless this brief names a change.

1. A hand's or judge's page splits top and bottom: top the brief it was dispatched with, bottom what the harness has of it.
2. The big view remembers where the operator was scrolled, per party, and does not pull the text away while main streams.

## Bounds

- Every file lives under `prototypes/ui/panes/`, with one exception: append a section to `prototypes/ui/SCENARIO.md` (see "Scenario"). Rebuild in place; keep what round two got right. Never touch `crates/`, `sidecar/`, `arx.toml`, the workspace `Cargo.toml` or `Cargo.lock`, `docs/`, `prompts/`, or the sibling prototypes.
- Never run `cargo init` or `cargo new` inside the repo. After your first cargo command run `git diff -- Cargo.toml Cargo.lock` at the repo root and confirm it is empty; if it is not, `git checkout -- Cargo.toml Cargo.lock` and report it.
- No `arx run`, no `cargo test -p arx`, no Docker. Build only this package, from inside its dir.
- No nerd-font glyphs. Target terminal: foot inside herdr, `TERM=xterm-256color`, `COLORTERM=truecolor`.
- Prose, in the README and anywhere else, follows `~/.claude/skills/unslop/SKILL.md`.

## Scenario

Scene 3 dispatches `h-0007` (`hand-do`, claude-code, `arx-hand-claude:2.1.258`, claude-sonnet-5) with the brief "delete `pid.txt` and drop its mention from CLAUDE.md", and `h-0008` (`codex-judge`, codex, `arx-hand-codex:0.152.1`, gpt-5.4) to review the diff. Both deliver while the operator is idle. The scenario gives each a verdict text and a trace handle but no trace content, so append one section to `SCENARIO.md`, titled `Scene 3, traces`, that scripts the turns each trace holds. Change nothing else in that file.

Shape the turns after what a real swept trace holds: a hand's session is a list of events, each an assistant text, a tool call with its arguments, or a tool result. Write for `h-0007` something like: a one-line plan; `Read CLAUDE.md`; `Bash rm pid.txt`; `Edit CLAUDE.md` with the hazard line rewritten; `Bash cargo test -p arx` with `23 passed` as the result; then the verdict text the scenario already has. For `h-0008`: `Bash git diff`; `Read CLAUDE.md`; two sentences of reasoning; the verdict text and the schema line. Keep each trace under fifteen turns and the bytes roughly matching the scenario's figures. `h-0008` needs a one-line brief too; write it as main would ("review the diff h-0007 landed against its brief; findings or none").

## The split page

When the selected party is a hand or a judge, the big view splits into two panes with a horizontal border. The big view's title still names the party (`4 hands · h-0007 hand-do`).

**Top, titled `brief`**, about a third of the height. First a one-line header: id, agent, role, body, image, model, spawn time. Under it the brief, rendered with the markdown-lite renderer the transcript uses. Scrolls on its own when the brief is longer than the pane.

**Bottom, titled `turns`**, the rest. Its content by state:

- running: one line, `running · 4.2s`, ticking, nothing else. Do not print "tap is the read path"; that sentence was a placeholder.
- stopped with a question: `stopped · 4.2s` and the question in full under it.
- delivered: the trace turns rendered like the main transcript (assistant text through markdown-lite, tool calls as the same dim one-liners main's page uses, tool results dim and truncated to a few lines), then a verdict block: outcome, the verdict text in full, the trace handle, bytes, `elapsed_ms`, `lag_ms`, and for a judge the schema line. `killed` and `stop` outcomes show what round two showed, in the same block.

Main's page and the session page do not split.

**Focus.** Enter in a panel opens the big view with the bottom pane focused. `Tab` swaps focus between the two panes while the big view is focused on a split page; in a panel `Tab` still cycles panels, as before. `j`/`k`, `gg`/`G`, `Ctrl-d`/`Ctrl-u` and the mouse wheel act on the focused pane. Esc returns to the panel the operator came from. The focused pane carries the highlighted border, the other the dim one, the same contrast the input box uses between modes. The bottom line on a split page reads `NORMAL  4 hands · h-0007   Tab pane · j/k · gg/G · Esc back · ? keys`.

Add the swap to `keys.toml` and to the `?` overlay under its own context, so a rebinding shows up there. Choose the section name; say it in the README.

## Scroll memory

Replace the single `scroll_from_bottom` with one scroll state per party, kept for the session. Selecting a party restores its state; nothing resets it. A party seen for the first time opens where round two opened it: a live main at the bottom, following; an ended main at the top; a delivered hand or judge at the top of its turns. On the split page each pane has its own state.

A pane's state is either **following** or **anchored at a row**. Following means the bottom stays in view as text appends. The moment the operator scrolls up from the bottom, the pane becomes anchored: appended rows do not move what is on screen. While anchored with rows appended below the view, the pane's bottom edge shows a dim right-aligned hint, `12 new lines · G`, counting rows appended since the anchor. `G` returns to following and clears the hint; so does scrolling down until the bottom is in view. `gg` anchors at the top.

Cache wrapped-row counts per pane as round two did, keyed by pane, invalidated on resize or append; an anchor is a row index into that cache, so a resize re-anchors on the same logical row.

## Carry over from round two

Everything else stays: the scenario thread, the state reducer, the markdown-lite renderer, the gauge, the gate-ask modal, the `/model` picker, the reset confirm, the parties panels, the modal keys with `keys.toml` and the `?` overlay, `--auto`, `--fast`, `--snapshot` at 110x36 through `TestBackend`, the mouse wheel.

## Snapshot frames

`--snapshot` prints round two's frames plus these, each labelled:

- `h-0007` split page while running (scene 3, before delivery), bottom focused.
- `h-0007` split page after delivery, bottom focused, at the top of its turns.
- the same after `G`, showing the verdict block.
- the same after `Tab`, top pane focused.
- `h-0008` split page after delivery, bottom focused, after `G`.
- main's page scrolled up with `k` while scene 1 streams, showing the `n new lines · G` hint.
- main's page after leaving for panel 4 and coming back with `2`, showing the same anchored position.

## Live check

Drive the binary under `tmux new -d -s panes3 -x 110 -y 36` with `send-keys` and `capture-pane`, in `--auto` mode, and paste the captures for: Enter on `h-0007` from panel 4 during scene 3 and after delivery, `Tab` then `j` in the top pane, `Esc` back, scrolling main up during a stream and reading the hint, leaving for panel 4 and returning with `2`. Kill the tmux session when done.

## README

Add a round-three section to `prototypes/ui/README.md` under the chosen-direction section: the split page, the pane keys, the scroll rule, the `keys.toml` addition, and the scenario section appended. Keep the rest as it is.

## Verdict

Return as text: what was built, the last 10 lines of `cargo build`, the root `git diff -- Cargo.toml Cargo.lock` result, the full `--snapshot` output fenced, the tmux captures fenced, the final `keys.toml`, and three things you would change next. Fence fields: `gate_run: no`, `gate_reason: prototype`.
