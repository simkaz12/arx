# Shared scenario for the terminal prototypes

Every prototype under `prototypes/ui/` plays this session, in this order, with this content. The
point of the prototypes is to compare three fronts on the same material, so a builder changes the
rendering, never the scenario. The vocabulary is `CONTEXT.md`'s: turn, step, dispatch, hand, judge,
gate ask, admission, checkpoint, reset, nudge, orientation report, ember, scope, store.

## What arx actually emits today (mock this, invent nothing beyond it)

From `crates/arx/src/terminal.rs` and the call sites in `main.rs`, the harness sends the operator:

- `Chunk{text}` streamed partials of main's reply, then `TurnEnd{text}` with the canonical text.
  A turn can carry two text blocks around a tool call (the "pre-tool sentence" case).
- `Ask{tool, subject}` from the gate; the operator answers with one key, `a` allow once, `A` allow
  always, `d` deny once, `D` deny always, and an optional note after the key. `A`/`D` append a
  durable rule to the repo gate file.
- `Notice{text}` for: reset armed (`window at {occ} of {window}, reserve {reserve}`), nudge
  (`main at {occ} tokens, past {threshold}`), miss (a model rejected the window), checkpoint
  written / check failed / abandoned, reset confirm (the state object body, then "ok to tear down
  and respawn; any other line rides into the new main's first turn as an operator note"),
  store landscape degraded, model list / switched / unlisted / already, model mismatch.
- Every fatal error is `error: …` on stderr and the process exits.

Operator input is one line at a time. Slash commands: `/reset`, `/model`, `/model <id>`,
`/checkpoint`. Anything else, including a mistyped slash command, goes to main as a chat turn.

Data the loop holds and never shows, which a prototype may surface (all real fields):

- gate verdicts allow / deny, with the rule that decided (`Event::Gate`)
- admission per tool result: inline, or externalized with byte count and a trace handle
- dispatch: hand id, agent name, role (hand / judge), body, model (`Event::Spawn` + config)
- delivery: outcome (verdict / stop / killed), trace handle, bytes, `elapsed_ms`, `lag_ms`
- judge schema ok / malformed
- checkpoint number, base and last occupancy; reserve window, ceiling, armed flag
- usage per turn: input, output, cache read tokens, model, `model_source` (sdk / assumed),
  turn elapsed ms. There is no cost field anywhere; do not invent one.
- current model and the config's model list; scope; store root
- hands in flight (`ledger.in_flight_hands()`)

A dispatch never blocks a turn. The hand's delivery lands at main's next step, and today the
operator sees nothing until they submit their next line. Prototypes are free to show the delivery
the moment it arrives; that is one of the things being compared.

## Fixed session facts

```
scope        arx
store root   ~/.local/share/arx/store
models       claude-opus-5 (current), claude-sonnet-5, claude-haiku-4-5
window       200000   reserve 40000   nudge threshold 120000
grounding    hot 4 claims · 2 embers
checkpoint   2 written earlier at occupancy 61 412
```

## Scenes

The mock engine plays one scene per submitted operator line. The text typed is echoed as the
operator turn regardless of content, except the slash commands named below, which must be typed
as written. A `--auto` flag plays every scene with realistic delays and no typing. A `--fast`
flag removes the delays. Timings below are for the default pace.

### Scene 1: a plain reply

Main streams this in chunks of a few words over roughly two seconds, after a 700 ms wait for the
first token (that wait is real and today shows nothing):

```
The store root moved out of the repo in L3-05, so a fresh clone has no `store/` at all. Two things follow:

- `arx run` refuses an unregistered scope, and registration is still a hand-typed `mkdir` under `~/.local/share/arx/store`.
- The tracked `pid.txt` is a leftover of the L1-close session. Nothing writes it now.

I'd delete `pid.txt` in the polishing pass and leave the store registration as it is.
```

Usage after the turn: input 18 240, output 142, cache read 16 900, model claude-opus-5 (sdk),
elapsed 3 180 ms. Occupancy after: 63 900.

### Scene 2: tool calls, an ask, admission

1. Main calls `Read` with subject `crates/arx/src/terminal.rs`. Gate: allow by rule
   `Read *`. Admission: inline, 6 312 bytes.
2. Main streams a pre-tool sentence: `terminal.rs is still the L1-04 adapter; let me see what the tests pin down.`
3. Main calls `Bash` with subject `cargo test -p arx --test claude_main 2>&1 | tail -40`. Gate: ask.
   The prototype presents the ask. Whatever the operator answers, continue as allowed. If they
   answered `A`, show that a rule `Bash cargo test -p arx*` was appended to `.arx/gate.toml`.
4. Tool result: externalized, 14 803 bytes, handle `trace/2026-09-08/t07-bash-3.txt`, preview
   `test result: ok. 11 passed; 0 failed`.
5. Main streams the post-tool text:

```
Eleven integration tests pin the byte shape of piped output, so a richer terminal has to keep piped mode byte-identical and add the new look only when stdin is a tty.
```

`TurnEnd` carries both sentences joined by a blank line. Usage: input 33 110, output 96, cache
read 18 240, elapsed 9 640 ms. Occupancy after: 79 300.

### Scene 3: a dispatch and a judge, delivery while idle

1. Main calls `dispatch` for agent `hand-do`, role hand, body claude-code in container
   `arx-hand-claude:2.1.258`, model claude-sonnet-5. Hand id `h-0007`. Gate: allow by rule
   `dispatch *`.
2. Main calls `dispatch` for agent `codex-judge`, role judge, body codex in container
   `arx-hand-codex:0.152.1`, model gpt-5.4. Hand id `h-0008`.
3. Main ends the turn with:

```
Dispatched hand-do to delete `pid.txt` and drop its mention from CLAUDE.md, and the codex judge to review that diff once it lands. I'll fold both when they report.
```

4. The operator is idle at the prompt. After 6 s `h-0007` delivers: outcome verdict, text
   `PASS: pid.txt removed, CLAUDE.md hazard line rewritten, 23 tests green`, trace
   `trace/2026-09-08/h-0007.md`, 2 118 bytes, elapsed 5 940 ms, lag 12 ms.
   After 9 s `h-0008` delivers: outcome verdict, schema ok, text
   `no findings; the hazard paragraph now says nothing writes pid.txt, which matches the tree`,
   trace `trace/2026-09-08/h-0008.md`, 611 bytes, elapsed 8 870 ms, lag 8 ms.
   Show each the moment it lands, without waiting for a keystroke.
5. On the operator's next line, main streams:

```
Both back. The hand's diff is in the working tree, uncommitted, and the judge found nothing. Your call whether to commit it with the rest of the polishing work.
```

### Scene 4: `/model`

The operator types `/model`. Present a picker over the three models with the current one marked.
Arrow keys or `j`/`k` move, Enter selects, Esc cancels, the digit keys select directly. On
selecting claude-sonnet-5 show the switch as arx reports it: `model: claude-opus-5 -> claude-sonnet-5`.
`/model claude-sonnet-5` typed in full switches without the picker. `/model gpt-5` shows the
unlisted notice with the list.

### Scene 5: nudge, checkpoint, reset

1. Occupancy jumps to 121 800 on the next turn's usage. Show the nudge:
   `main at 121800 tokens, past 120000`.
2. Then reset armed: `window at 161200 of 200000, reserve 40000; /reset to reset now`.
3. The operator types `/checkpoint`. arx's own authoring prompt is echoed as an arx-role turn
   (today it prints verbatim; the prototype may fold it):
   `Write the state object: position, target, drift. Cite decision ids you rely on.`
   Main streams a short state object:

```
position   polishing pass, day 1; pid.txt removed, /model landed, doubled reply fixed
target     arx usable as the daily driver for arx work (L3-04)
drift      none; terminal look still unruled (44afcdc8)
relies on  580e402e, 57aa480a
```

   Then the truth-check prompt, `Check every claim above against the tree and the store.`,
   main's `all four hold`, and the notice `checkpoint 3 written: threads/main.checkpoints/3.md`,
   delta +58 400 since checkpoint 2.
4. The operator types `/reset`. Show the confirm: the state object body, then
   `ok to tear down and respawn; any other line rides into the new main's first turn as an
   operator note`. The operator types `ok`. Show teardown, respawn, the orientation report
   streamed by the new main:

```
Resuming from checkpoint 3. Position and target are clear, the drift line is empty, both decision ids resolve. Ready.
```

   and the orientation verdict `ready`. Occupancy resets to 9 120.

### Scene 6: abort and a non-fatal error

1. Main starts streaming a long reply (any filler, two paragraphs). The operator presses Esc
   mid-stream. Show the abort and the partial text kept in the transcript.
2. On the next line the provider returns a transient error, `overloaded_error: retry after
   2 s`. Today this is fatal. The prototype shows it as a recoverable notice, retries once
   after 2 s, and the turn completes with `Back. Where were we?`. Label this clearly as a
   proposal in the README, since it changes behaviour.

### Scene 7: an unknown command

The operator types `/reste`. Today this goes to main as chat. The prototype shows a hint
listing the four commands and does not send the line. Also a proposal.

## Comparison points every prototype must make visible

- the wait before the first token, and the stream itself
- main's own tool calls as they happen, and the admission outcome of each result
- gate asks as a live keypress with an optional note, and the durable-rule side effect of `A`/`D`
- dispatches as identities with a running elapsed timer, and deliveries the instant they land
- model, scope, occupancy against window and reserve, checkpoint number, hands in flight, last
  turn's tokens and elapsed, all readable at a glance
- pickers for bounded choices (the owner's standing preference: never a printed list plus retyping)
- Esc to abort, Ctrl-C to quit, Up/Down for history, Tab to complete a slash command
- a resize mid-session that does not corrupt the screen

## Scene 3, traces

Scene 3 gives each delivery a verdict text and a trace handle, not the trace itself. A prototype
that shows a hand's or judge's steps needs something to render, so this section scripts both
traces: a list of turns, each an assistant text, a tool call with its arguments, or a tool result,
the same shape a real swept trace holds.

### h-0007 (`hand-do`)

Brief: `delete pid.txt and drop its mention from CLAUDE.md`

1. Plan: read the hazard paragraph, delete pid.txt, reword the line to match, then run the suite.
2. `Read CLAUDE.md`
3. Result: `pid.txt at the root is a leftover of the L1-close session, still tracked; nothing removes it yet.`
4. `Bash rm pid.txt`
5. `Edit CLAUDE.md` — reword the hazard line to say nothing writes pid.txt now
6. `Bash cargo test -p arx`
7. Result: `test result: ok. 23 passed; 0 failed`
8. Verdict: `PASS: pid.txt removed, CLAUDE.md hazard line rewritten, 23 tests green`

### h-0008 (`codex-judge`)

Brief: `review the diff h-0007 landed against its brief; findings or none`

1. `Bash git diff`
2. Result: `-pid.txt`, `CLAUDE.md | 2 +-`, `2 files changed, 1 insertion(+), 2 deletions(-)`
3. `Read CLAUDE.md`
4. Result: `pid.txt at the root is a tracked leftover of the L1-close session: nothing writes it now, and no change to it is staged.`
5. The diff removes pid.txt and the hazard paragraph now says nothing writes it, matching the tree. Nothing else in the diff touches state the brief didn't name.
6. Verdict: `no findings; the hazard paragraph now says nothing writes pid.txt, which matches the tree` — schema ok
