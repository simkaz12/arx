# T07 findings — the loop commitments the memory rulings already made

Ticket: [T07](../T07-loop-commitments.md) · resolved 2026-08-24 · produced by a hand-ex research agent from the nine in-force arx decisions' own stored text (fetched via `mcp__tabularium-mcp__decision`), with POSITION.md treated as commentary and checked against the text per the tripwire. SURVEY §1's step shape is the yardstick for tag (b): drain steering queue → pressure check/assembly → (optional thinking/critique) → issue request → consume stream → branch on termination → dispatch tools → admit results.

## 1. `2d15b921` — harness owns the merge

- A1. A discrete merge step exists, and it is what "window assembly" in SURVEY names. (b) undetermined — the ruling doesn't say which sub-operations belong to the merge, and SURVEY's own seam list flags "who owns request construction" as open.
- A2. The merge runs on every step that mutates the window, not only at session start — forced by "mandatory party to every window mutation." (b) satisfies structurally, but whether assembly fires on *every* step versus only pressure-triggered ones is undetermined.
- A3. Explicit model reads and memory's own retrievals must remain distinguishable all the way to admission, since memory is barred from arbitrating between them — provenance metadata must survive dispatch → admit. (a) shared with `0aa87641`. (b) undetermined — SURVEY carries no stated data between dispatch and admit.
- A4. The sha-diff freshness check must be computable harness-side, synchronously, at or before the point worth is judged — working-tree access (or a cache) live inside the step. (a) mechanism reused by `061d7942`, `80bffad0`, `79da6421`. (b) undetermined — no freshness phase exists anywhere in SURVEY's step.
- A5. Harness-owns-validity requires the harness to read `(path, sha)` fields out of trace/residue records that physically live in the store, as inert data — otherwise "harness decides validity" collapses into "memory decides validity." (b) undetermined.

## 2. `9a764559` — window ops: append + rewind, no splice

- B1. Rewind targets a checkpoint, so the window needs addressable checkpoint boundaries established at admission or task-start time. (b) undetermined — no checkpoint-marking phase in SURVEY.
- B2. Mono-task discipline requires a "unit of work" concept distinct from turn or step, so the stale region is provably always the tail. SURVEY's nesting is session ▸ turn ▸ step with no task level. (b) gap — the level this assumption needs isn't in the nesting at all.
- B3. Eviction fires rarely, at "a seam in the work," which must be a different event from the per-step pressure check that arms it — otherwise eviction fires every high-pressure step, contradicting rare-and-batched. (a) the direct substance of question 6. (b) undetermined — SURVEY's single "pressure check" phase doesn't distinguish arming from firing.
- B4. Tail-only staleness requires admitted content to carry task-membership tags so the harness can identify "everything since the last task boundary" at rewind time. (b) undetermined — no tagging in SURVEY's admit phase.

## 3. `061d7942` — traces are evidence, not knowledge

- C1. A subagent's context must be captured at the moment its nested session ends, before discard — archival is ordered relative to the parent's admission of the report, not merely "eventually." (b) undetermined — SURVEY marks "subagent = nested SESSION" at dispatch but says nothing about that session's end.
- C2. A nested session presumably runs the same step shape and needs its own sha-capture hooks recursively. (b) satisfies loosely by nesting, undetermined in mechanism.
- C3. Handle-pulling reuses the ordinary dispatch→admit cycle rather than a special retrieval path. (a) shared with `0aa87641`, `80bffad0`. (b) satisfies.

## 4. `3e2d9a86` — admission ladder: inline / filter / delegate

- D1. The ladder judgment sits inside "admit results," after dispatch, before the next request. (b) satisfies — exactly SURVEY's ordering.
- D2. "Delegate" requires the harness, mid-admission, to spin up and complete an entire nested session before the parent step proceeds — one phase of one step recursively containing a full session. But the ladder's own discriminator ("must this be interacted with?") reads as a decision made *before* a tool runs, not after a result exists, which puts delegate at dispatch-time routing rather than post-dispatch admission. The ruling's text does not resolve which phase it occupies. (b) undetermined — the ladder as written straddles dispatch and admit.
- D3. "Filter" requires a synchronous store *write* (the full artifact) as a precondition for the handle that gets admitted — admission is not read-only. (b) undetermined — whether the write blocks the step is unaddressed.
- D4. Crossover costing assumes harness-side token counting available synchronously between dispatch and admit, no model call involved.

## 5. `0aa87641` — memory defers, never overrides

- E1. Same provenance requirement as A3 — deferral is scoped to results of the model's own tool calls, so explicit-read status must reach admission.
- E2. The decision is necessarily made *after* dispatch (size unknowable at emission time) and *before* the next request. (b) satisfies.
- E3. With no splice, a later pull of a deferred handle can only *append* the body — the earlier summary can never be retracted. This borrows `9a764559`'s op-set restriction and extends it from eviction to admission-error recovery, a widening neither ruling states. (b) satisfies (append-only consistent), but the joint dependency is unstated in either text.
- E4. Deferral must complete before "issue request" for the next step — the direct textual basis for POSITION §8's hot-path claim, now per tool result rather than per turn.

## 6. `80bffad0` — refusal = deferral + residue, two gates

- F1. A residue lookup must run at or before the point a repeat-read tool call executes — pre-dispatch or dispatch-interposed, structurally different timing from D1/E2's post-dispatch admission. The ruling's text does not say whether the read still executes and refusal only substitutes what gets *admitted* (a special case of `0aa87641`'s deferral, post-dispatch) or whether memory blocks dispatch outright (an override `0aa87641` explicitly forecloses). Neither the ruling nor POSITION resolves this. (b) undetermined — SURVEY has a permission check before dispatch but no memory-consultation hook there. (c) possible tension with `0aa87641`'s rejected-override clause, depending on reading — reported, not adjudicated.
- F2. Applicability needs "the question the earlier read was made under" — the harness must extract and hand memory a representation of current intent distinct from the raw tool call, an artifact SURVEY's step never names.
- F3. Same sha-lookup dependency as A4/C2, now at dispatch-adjacent time — a third distinct point in the step where freshness must be live.

## 7. `79da6421` — residues are shared, key holds a set

- G1. Applicability must be re-evaluated on *every* repeat-read attempt, never memoized — a third clock distinct from admission's per-tool-result rhythm and eviction's per-seam rhythm, arguably higher-frequency than either. (b) undetermined — no phase for this exists in SURVEY at any frequency.
- G2. POSITION's q5 caveat characterizes the applicability match as "semantic matching, which is embeddings, which is a cosine threshold" — but the decision's own text never mentions embeddings or a matching mechanism; it only says applicability "cannot be precomputed or cached." If the caveat is right, this clock requires an embedder round trip inside a pre-dispatch critical path — materially heavier than the ruling specifies. A gap between decision text and POSITION's inference, not a decision-text claim.
- G3. Bulk invalidation ("the file changing kills its whole key at once") depends on the same sha mechanism recurring yet again, at residue-set-validity time.

## 8. `df14a2bc` — dedup adjudication folds at a seam

- H1. A save's own admission must be non-blocking — real adjudication is deferred entirely outside the step loop. A memory-*write*'s admission is structurally different from a memory-*read*'s admission, which is judged synchronously. (b) satisfies loosely for the save call; the seam where adjudication later runs is, like eviction's seam, undefined in SURVEY — the second instance of question 6's different-clocks problem.
- H2. "A handle back to the turns that produced the save" assumes those turns are still reachable when the fold runs — either still in the live window or already captured by `061d7942`'s trace archival. If eviction rewinds past them before the fold fires, the fold's usefulness depends entirely on trace archival having captured them. Neither ruling states this dependency; inferred here.

## 9. `dc433ba5` — Tabularium stays out-of-process, deferred not rejected

- I1. The cost model assumes memory is consulted roughly once per turn or step. Directly contradicted in frequency by `2d15b921` (mandatory party to every mutation) plus `3e2d9a86`/`0aa87641` (admission per tool result — a step dispatching N tools needs N admission judgments). (c) direct conflict, already named in POSITION §8 and pitfall `8a3b81f7`. Reported, not adjudicated.
- I2. Grounding is a session-level event above the turn/step loop. (b) satisfies.
- I3. The trait surface is assumed to be a single call shape (`assemble(window, budget)`). But the other rulings collectively describe several distinct memory-facing judgments per step — worth, ladder tier, defer/admit, refusal/residue, applicability re-check — each potentially a separate call. Whether these fold into one call or fan out is undetermined by any ruling, and is exactly what would settle whether the trait is chatty.

## Cross-ruling table

| Assumption | Rulings making it | Field-shape status | Conflict |
|---|---|---|---|
| Admission runs post-dispatch, pre-next-request | 2d15b921, 3e2d9a86, 0aa87641 | satisfies | — |
| A separate pre-dispatch/dispatch-interposed memory check exists | 80bffad0, 79da6421 | undetermined, no phase named | tension with the above — two different "where memory sits" answers |
| Eviction fires rarely, at an undefined seam outside the per-step loop | 9a764559 | undetermined | shares the gap with the dedup fold |
| Dedup fold fires rarely, at an undefined seam outside the per-step loop | df14a2bc | undetermined | same gap; a third clock — question 6 generalizes to three, not two |
| Harness-side sha-diff freshness, computed synchronously | 2d15b921, 061d7942, 80bffad0, 79da6421 | undetermined — no freshness phase in SURVEY | recurs at four distinct points, never budgeted collectively |
| No-splice / append-only restriction | 9a764559 (eviction), 0aa87641 (admission-error recovery) | satisfies | unstated joint dependency |
| Handle-as-pointer / never-assertable material | 061d7942, 0aa87641, 80bffad0 | satisfies — same tool-call mechanism reused | consistent (question 7's own observation) |
| Memory call frequency ≤ ~1/turn (cheap transport) | dc433ba5 | contradicted once the ladder is per-tool-result | direct conflict with 2d15b921 + 3e2d9a86 + 0aa87641 |
| Dedup fold's queued handle stays reachable | df14a2bc | depends on 061d7942 or the window not having rewound past it | unstated dependency on 9a764559 |

## Consolidated step-shape requirements the ruling set implies

Stated as gaps against SURVEY §1's named phases, not recommendations:

1. A named merge/assembly sub-phase distinct from "window assembly," clarifying what belongs to it versus request construction (2d15b921).
2. Provenance metadata (explicit-read vs retrieval; which tool call produced which result) carried from dispatch through admit (2d15b921, 0aa87641).
3. A checkpoint/task-boundary concept above the step level — a fourth nesting tier or an explicit marker (9a764559).
4. A distinct arm-vs-fire split for eviction: pressure check every step, firing not (9a764559, question 6).
5. The same arm-vs-fire split for the dedup fold — a second, independent instance (df14a2bc).
6. A synchronous, harness-side freshness `(path, sha)` lookup available at multiple distinct points: merge/worth judgment, trace read, refusal gate, residue-set validity (2d15b921, 061d7942, 80bffad0, 79da6421).
7. A hook at nested-session termination for trace capture, before the subagent's context is discarded (061d7942).
8. Clarity on whether "delegate" is a pre-dispatch routing decision or a post-dispatch admission decision (3e2d9a86).
9. A synchronous store-write path reachable from within "admit results," for the filter tier (3e2d9a86).
10. A representation of "current question/intent" distinct from the raw tool call, for the applicability gate (80bffad0, 79da6421).
11. A non-blocking admission path for save-type tool calls, distinct from the blocking path for read-type results (df14a2bc).
12. A defined relationship between the eviction seam, the dedup-fold seam, and archived-trace reachability (df14a2bc, 9a764559, 061d7942).

## Questions 6 and 7, restated as required step structure (not answered)

**Question 6 — different clocks.** The step shape must expose at minimum two independently triggerable hooks sharing the same window object: a per-tool-result hook gated on nothing but "a result exists" (admission's clock), and a per-seam hook gated on pressure *and* an independent "unit of work complete" signal that the session▸turn▸step nesting does not name (eviction's clock). This inventory adds a third: the dedup fold needs the same rare/batched hook shape but is triggered by save events, not pressure — so "different clocks" is, on this evidence, at minimum three clocks, not two.

**Question 7 — cost depends on when eviction runs.** If eviction runs inside the same step whose completion call just returned (between admit and the next issue-request), its cost sits on that turn's hot path, same as admission — undermining the rare-and-batched framing `9a764559` relies on for "rewind is free." If it runs between one turn's end and the next turn's start, off the step loop entirely, the cost story holds — but SURVEY names no such gap; "pressure check" sits at the top of every step, not at a turn boundary. Question 7's cost argument and question 6's clock argument are not separable: both turn on whether SURVEY's single "pressure check" phase is one clock that mostly no-ops, or genuinely two.

## Discrepancies found (reported, not resolved)

- The **write → cut → append rewind ordering**, which POSITION §6 and this ticket's brief both treat as part of what the rewind ruling requires, is **not present in `9a764559`'s stored text**. The decision defines the op-set and the cache-cost rationale only; the ordering constraint exists solely as POSITION.md prose and an unlinked memory fact (`f88c7521`), never written into any of the nine in-force decisions.
- The "armed on pressure, fired at a seam" two-phase model for eviction is likewise not verbatim in `9a764559` — the decision argues cache mechanics and mono-task scoping, not an explicit arm/fire split.
- `79da6421`'s applicability check is characterized by POSITION (q5 caveat) as embeddings-and-a-cosine-threshold; the decision's own text says only that applicability "cannot be precomputed or cached," with no stated mechanism. The cost claim is POSITION's inference.
- `80bffad0`'s refusal mechanism is ambiguous between post-dispatch admission substitution (consistent with defer-not-override) and pre-dispatch veto (in tension with it). The text supports either; POSITION does not flag the ambiguity.
- `3e2d9a86`'s ladder mixes a decision that reads as pre-dispatch routing ("delegate") with decisions that read as post-dispatch content judgments ("inline"/"filter") — an internal seam ambiguity within one ruling.
- `dc433ba5`'s own discriminator is, on this evidence, already contradicted by the combined call frequency the other rulings impose — the one conflict POSITION §8 names outright.

## Confidence

High on the per-ruling assumption extraction (verified against fetched decision text, not paraphrase) and on the `dc433ba5` conflict (corroborated by stored pitfall `8a3b81f7`). Medium on the `80bffad0` pre/post-dispatch ambiguity and the delegate-tier phase ambiguity — genuine textual gaps, but a different close reading could narrow them. Low on anything downstream of "three clocks, not two" — that count treats the dedup fold as structurally identical to eviction's clock, argued from shared rationale rather than an explicit joint ruling.
