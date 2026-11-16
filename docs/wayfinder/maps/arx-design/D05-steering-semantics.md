---
id: D05
labels: wayfinder:grill-with-docs
status: closed
assignee: none
resolved: 2026-08-28
blocked-by: none
grounding: findings/T04-findings.md
---

# D05 — Steering semantics

> **Closed 2026-08-28** after one grill, four rounds — see Resolution. The ticket's subject moved in round 1: the charter's "daily friction with CC input handling" was misattributed to main, and the grill spent itself on the main→hand edge instead.

## Question

T04 corrected SURVEY: the field ships a two-way split (steer-at-boundary / queue-to-idle), abort is an orthogonal control action that by stated intent preserves the queue, and the open bugs across the field live exactly where abort meets the pending queue. arx is the single-process case — the harness owns the loop, so nothing is protocol-impossible here the way mid-turn steer is over Claude's stream-json. To settle:

- **The model.** Two delivery priorities plus separate stop verbs (the field's convention, and opencode PR #32425's soft-cancel/hard-abort force levels), or Goose's three states (deliver, cancel-preserve, discard-on-teardown)? Name abort's effect on the pending queue explicitly — the one place three of five systems have open bugs.
- **Drain granularity.** Per tool call (Claude Code observed, pydantic-ai) or per turn (Goose)? The field's prose doesn't distinguish them; arx's ruling should. This is a D07-adjacent choice — the drain point is a step phase — but grillable on its own evidence.
- **Steer meets reset.** A steer message must not become its own compaction unit and must stay attached to the task it modified (#32157's argument). This bullet has now outrun three vocabularies in a row, and each rewrite changed what the question is even asking rather than just its wording — worth stating plainly rather than patching the framing a fourth time. It started under write→cut→append, asking whether a rewind could orphan an in-flight steer; `5efb4c94` dropped write→cut→append and `a8895a1e` made a cut archive-with-breadcrumb, so the 2026-08-25 note reframed it as cut-meets-steer. `e65225be` (superseded 2026-08-27 by `1e27032c`) then renamed cut/reset (cut is thread death at doneness, reset the mid-thread recycle) — the operation a steer message would actually collide with mid-task is a reset, not a cut. And `da0ccf24` removed the thing being asked about a second time over: there is no rewind command and no archive-with-breadcrumb any more, only reset on a token counter, with the outgoing main writing the state object itself. What the question now has to be, argued fresh rather than inherited: does an in-flight steer message survive a reset, and if the main is mid-turn on a steered task when the token counter forces a reset, does the state object it writes carry that the task was steered, or does the steer's provenance drop on the floor along with everything else the window held.
- **Durability class.** Is a steer message evidence (the trace class), or closer to a fact — content the model must act on? And does arx keep provenance markers (steered/cancelled/aborted, by whom) in the transcript?
- **Queue-to-idle and the mono-task window.** Does hold-to-end-of-task get cheaper or more expensive under no-splice than in the multi-task harnesses the convention comes from?

## Resolution

One grill, four rounds, six rulings, sol silent every round. The charter was wrong about where the friction sat, and every bullet above closed under the corrected subject.

- **Main keeps Claude Code's flow** (`ba965dd0`). Owner: "on main i would like to keep what cc is doing, i do like the main flow of it." An operator message typed mid-turn lands at main's next step boundary, Esc aborts the in-flight step and preserves anything still queued; no end-of-turn queue mode, no per-message priority. The model, drain-granularity, queue-to-idle and abort-meets-queue bullets close by adoption, not by a ruling of arx's own — the two-priority split, Goose's three states and HAPI's queue-default are all rejected for main.
- **The friction is main's access to its hands**, and it resolves into three verbs on an arx-native hand. **Steer** (`001823de`, supersedes `426670ee`): main pushes a message into a running hand, the hand's in-flight step is cancelled, the message appends, the hand continues — the interrupt/steer combo the owner already runs GLM-class hands on. The parent-side hold at the next step boundary is retired: main always has the text when it reaches for a hand, so a hold with no message never occurs. Steer is addressed to one hand and does not cascade; kill is unchanged and does. Arx-native only — a foreign body gets kill, and `426670ee`'s hold-and-continue mechanic closes as not built. The child-initiated stop at a permission wall (`5e8da52e`) is untouched. **Tap** (`7213cb42`, owner ruling over the round's recommendation): main reads a running hand's work so far, read-only, through the ordinary read tool over the hand's live transcript log, admitted like any result; nothing streams into main's window; steer or kill follows from the tap on main's own judgment or the operator's say-so.
- **Dispatch is non-blocking** (`23642b44`), which the verbs above require: the spawn call returns at once and the verdict or stub lands at main's next step boundary as an injected message — the same delivery point the operator's messages use. `6aab1d24`'s invariant stands (sweep before the verdict enters the window, verdict names its evidence); only the vehicle changes.
- **Steer meets reset** — the bullet that outran four vocabularies — closes with nothing extra in the state object (`adec3734`, supersedes `2c198c9f` on the "never mid-dispatch" clause only). Hands in flight run on across a checkpoint and across a reset; the state object lists them as in flight; their verdicts land at the respawned main's first step boundary after the orientation turn. A sent steer lives in the hand's window and both transcript logs, so no provenance drops.
- **Durability class** (`04870014`): a steer is content plus events. It arrives in the hand's window under a one-line harness prefix naming the step it cut, then the message, never removed; steer, tap and kill are transcript-log event lines. Not a trace, not a claim, no ⊘-style markers. D09's instrumentation gains steer/tap/kill counts per dispatch.

Vocabulary in `CONTEXT.md`: Turn, Step, Abort, Steer, Tap added; Stop narrowed to the child-initiated hold; Dispatch, Checkpoint, Reset, Capture surface and Transcript log amended.

Ticket impact: **D04**'s supervision ruling is superseded (`426670ee` → `001823de`) and its dispatch is now non-blocking — the D04 close stands, two of its records moved. **D15**'s checkpoint clock loses one clause. **D09**'s instrumentation list grows by steer/tap/kill.
