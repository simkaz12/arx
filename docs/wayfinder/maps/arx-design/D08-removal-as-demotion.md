---
id: D08
labels: wayfinder:grill-with-docs
status: closed
assignee: none
blocked-by: none
grounding: findings/T06-findings.md, findings/T01-findings.md, findings/T02-findings.md
---

# D08 — Removal as demotion: closing question 7

## Question

Question 7 asked whether eviction, like the other three removal seams, must leave a handle. The research moved it from symmetry argument to evidence. Blocked on D07 because the cost side depends on when eviction runs relative to the completion call. To settle:

- **The evidence now on the table.** Goose never deletes — `agent_invisible` flags split the durable store from the request-time projection, with restoration available (T01): an independently arrived-at instance of demotion-not-removal, in production, at both whole-history and single-tool-pair granularity. T06 shows the mixed tail that would make eviction-without-a-handle dangerous appeared in 1 of 16 sampled high-pressure windows, plausibly *because* delegation already demotes failed material to verdicts. Does that close the question in favour of handles-everywhere, or does it show the pressure case is rare enough that a bare pointer suffices?
- **The shape at eviction.** SURVEY §8's anti-lobotomy framing: a drop with no handle is a silent, permanent, unfalsifiable mistake. The counter was cost — a description occupies window space at the seam least able to afford it. Does Goose's two-layer split (store keeps everything; projection decides visibility) dissolve the cost objection, since the handle can live in the store rather than the window, with only a pointer surviving in-window?
- **One rule or four.** If eviction leaves a handle, admission's deferral, refusal's handle, rewind's archive and eviction collapse into "nothing is removed, only demoted." Is that collapse worth ruling as the single removal law, and does the projection step then become part of the assembler's interface (T01's last question) or a separate stage?

**Ruled since (2026-08-24, decisions `a8895a1e`/`ee932ea8`):** rewind and refusal are now both demotion-to-archive — cut-to-archive with a breadcrumb, and refusal-as-handle. Three of the four seams are settled on demotion; this ticket narrows to the last one, eviction proper, and to whether the collapse gets ruled as the single removal law.

**Inspect before grilling (2026-08-24, after D07 grill 1).** This ticket may have no content left. Decision `5efb4c94` removed eviction entirely — nothing evicts, folds, or compacts mid-session under harness policy — so the fourth seam this ticket exists to settle no longer has a referent, and rewind is operator-invoked with cut-to-archive already leaving a handle. What may still be live is the third bullet alone: whether "nothing is removed, only demoted" gets ruled as the single removal law across the remaining seams, and whether the projection step joins the assembler's interface or stands as its own stage. Read `5efb4c94` first and decide whether that residue justifies a grill or folds into D02.

## Closed without a grill — 2026-08-24

The inspection above ran. Verdict: no grill left. Bullets 1 and 2 lost their referent, bullet 3 split and neither half is a removal question.

- **Bullet 1, does eviction leave a handle.** No eviction to leave one. `5efb4c94` cites the same T06 measurement this bullet was going to weigh — zero eviction events across 270 sessions — as its reason for deleting the automatic path.
- **Bullet 2, the cost of a handle at the seam under pressure.** Gone with the seam. Nothing runs at a capacity moment, so a description can no longer be charged against a window that cannot afford it.
- **Bullet 3, one rule or four.** Every seam that survives already demotes: admission externalizes to a handle (`e65b2a35`), refusal is a bare handle (`ee932ea8`), rewind is cut-to-archive plus a breadcrumb (`a8895a1e`), a trace is reachable by handle and never assertable (`061d7942`). Ruling "nothing is removed, only demoted" would restate three in-force decisions. Its only unspent work is binding seams that do not exist yet, which is D04's. The projection half belongs to D02.

Residue folded out, not dropped: the removal law as a constraint on undesigned seams → **D04**; the projection step's placement → **D02**.

**Not folded anywhere, raised at the close.** SURVEY §8 states arx's one structural claim as "mandatory party to every window mutation, sole owner of eviction content — no instance found in the field." `5efb4c94` deleted the eviction half. What remains of memory's in-loop authority is a size threshold at admission, which is arithmetic rather than judgement. Whether the bet survives as "the harness owns admission" is D03's to answer and D09's to test.

**Stale givens found while verifying, annotated in place 2026-08-24.** Both files still described eviction as live. Neither was rewritten: every passage keeps its original prose, with a dated block beside it in the convention those files already use for superseded reasoning.

- POSITION §5 — the no-LLM-on-the-write-path guardrail's open question about extending to window-level eviction. Moot; there is nothing to extend it to.
- POSITION §6 question 1 — memory as sole owner of what survives eviction, stated in the paragraph, the arx row of the table and the reading of `c8f20ac3`. Also the `Keep`/`Fold`/`Drop`/`Admit` instruction set, which is down to `Admit` alone, and the `Keep`-contiguity invariant that existed to make a plan compile into a rewind.
- POSITION §6 question 2 — "when to fire", the ranking sub-decision that chose what survives among the droppable, and the three-tier admission ladder (`3e2d9a86`, superseded by `e65b2a35`). The mixed-tail case this question left hanging is answered by T06's 1-in-16.
- POSITION §6 question 3 — the corpus-versus-window eviction table, whose right-hand column has emptied.
- POSITION §6 question 6 — dissolved. One clock cannot span two seams.
- POSITION §6 question 7 — closed, header included.
- SURVEY §7 — the Blueprint 2026 tier-ladder corroboration and the OpenDev compaction tension.
- SURVEY §8 — the structural bet and the anti-lobotomy asymmetry.
