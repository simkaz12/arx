---
id: D07
labels: wayfinder:grill-with-docs
status: closed
assignee: none
blocked-by: none
grounding: findings/T07-findings.md, findings/T01-findings.md, findings/T06-findings.md
---

# D07 — The step shape and the clocks

## Question

Rule the turn cycle the memory rulings already assume, instead of letting them keep assuming it. Against T07's twelve consolidated step-shape requirements: which does arx's step adopt, which get redesigned, which expose a ruling that should change instead? Specifically to settle:

- **The clocks.** T07 counts three, not two: admission per tool result, eviction armed-per-step and fired-at-a-seam, the dedup fold triggered by saves. Is that the ruling — three named hooks on one window object — and what is the "unit of work complete" signal eviction's seam needs, given the session▸turn▸step nesting has no task tier (T07 B2/B3)? T01 gives the donor's answer for comparison: Goose checks compaction once per turn plus reactively on error, and T06 shows the incumbent never evicts at all.
- **The discrepancies between store and POSITION.** The write→cut→append rewind order lives only in POSITION prose — ratify it into the ruling set or amend it. Same for the arm/fire split. `80bffad0`'s refusal is ambiguous between post-dispatch substitution and pre-dispatch veto, the latter in tension with `0aa87641` — pick a reading and write it down. `3e2d9a86`'s delegate tier straddles dispatch and admit — assign it a phase.
- **Freshness.** The sha lookup is needed at four distinct points (T07 requirement 6) and budgeted at none. One mechanism, where does it live in the step, and what does it cost per firing?

The output is the step written down as arx's own, with each memory ruling's assumption either satisfied by a named phase or explicitly re-ruled.

**Ruled since (2026-08-24, decisions `a8895a1e`/`ee932ea8`):** the rewind mini-protocol is gone — rewind is cut-to-archive plus a harness-authored breadcrumb, no model call at the cut, and the `80bffad0` ambiguity dissolved with the residue registry. What this ticket still owns from that bullet: where the archive hook and the breadcrumb sit in the step, and the delegate-tier phase assignment.

## Resolved so far (grill 1, 2026-08-24)

**The clocks are settled.** Admission per tool result is the only clock inside the loop, against T07's proposed three. Eviction is not an in-loop clock at all — nothing evicts, folds, or compacts mid-session under harness policy, so the arm/fire split and the "unit of work complete" seam both dissolve. Decision `5efb4c94` (supersedes `9a764559`). Rewind survives as an operator-invoked command for the poison case, never as policy. *(Superseded 2026-08-30 by `7dae481c`: no removal still fires on a pressure clock, but rewind also fires as a mechanical step of the checkpoint and reset rituals, not by operator command alone.)*

**Store/POSITION discrepancies cleared.** write→cut→append is dropped outright (it lived only in POSITION §6 prose and the unlinked fact `f88c7521`). The arm/fire split is moot. `80bffad0`'s ambiguity was already dissolved by `ee932ea8`.

**New territory the grill opened.** Making the cut cheap is arx's job: a three-seat apparatus — worker, observer, probe — around a small live state object, with verification split by what each seat can see. Decisions `ab0077b4` (the apparatus) and `b27d9b6c` (the artifact's contents, lifecycle, and the operator-supplied doneness signal; superseded 2026-08-27 by `4833a3a3` — contents clause stands, lifecycle now authored by the outgoing main at reset). Vocabulary captured in `CONTEXT.md` at the repo root.

**Blocking edges released.** D03 and D08 are unblocked — the step-shape question that gated them is answered. D08 may have collapsed entirely: it asks whether eviction leaves a handle, and eviction is gone while cut-to-archive already leaves one. Check before grilling it.

## Resolved (grill 2, 2026-08-24) — ticket closed

**The delegate tier — ruled.** The admission ladder is two tiers, not three: inline below the threshold, externalize-plus-handle above it. Decision `e65b2a35` supersedes `3e2d9a86`.

Two moves got it there. First, the filter tier's "admit the extract that matters" was priced against an artifact size that does not occur — T06 puts the largest tool result across 270 sessions at ~18k tokens, and the incumbent already externalizes anything bigger into a preview plus a path. Owner ruling: the handle is good enough while there is nothing worth filtering. Second, with no extraction left, delegate's two examples came apart. "Hand a bulky artifact to a reader" was filter with a richer return and died with it; "run the suite, diagnose, fix" was never an admission decision at all, because no result exists yet — it is the model dispatching a subagent, which T01/T02 found is structurally a tool call. So T07's D2 ("the ladder straddles dispatch and admit") was right about the symptom: it straddled because it held two mechanisms under one name.

**The other three items collapsed** — argued in grill 2 against the post-grill-1 ruling set, uncontested, no separate decision recorded:

- **Freshness `(path, sha)`.** T07 gave it four call sites. Two died with the residue subsystem (`ee932ea8`, `a8895a1e`). The trace-record read only mattered while something judged applicability, and nothing does. What survives is `ab0077b4`'s mechanical check, which fires on observer writes — beside the loop. No in-loop caller, so no phase to assign and no hot-path budget to set.
- **Archive hook and breadcrumb.** The breadcrumb fires at rewind, which is operator-invoked and therefore sits between turns. Trace capture fires at nested-session end, which is D04's seam. Neither is a phase in the step. *(Corrected 2026-08-30 by `7dae481c`: the archive hook is a step phase on the provider-error branch, when a 400 fires rewind-to-fit mid-step, and sits between turns otherwise.)*
- **Provenance and non-blocking saves.** Provenance is a field on the result record, not a phase. A save's own result is small, so it admits inline, and `df14a2bc` already moved adjudication to an offline fold. The mechanism question — what field, on what record — stays open where it already lives, as D03's provenance bullet.

**Not settled, noticed in passing.** Four things in arx now share one shape: spawn a bounded side-context, hand it material and a question, get a small answer back — observer, probe, model-initiated subagent, and (had it survived) the reducer. Whether that is one deep module with several adapters or several things that merely rhyme was raised and not ruled. It belongs to D04 or a redraw, not here.
