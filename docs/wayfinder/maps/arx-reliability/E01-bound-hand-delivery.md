---
id: E01
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P1
next: owner-review
brief: docs/specs/2026-09-13-e01-bound-hand-delivery.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
pr: https://github.com/simkaz12/arx/pull/38
verified-revision: 2879eb14aab48bf3c9ccf25d261c2268f39bb392
---

# E01 — Bound hand verdicts before delivery

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L9)

## Existing point

A hand's full verdict is injected into main without admission, so one large delivery can fill the window.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: focused-trace.

Confirm the current delivery path, then admit oversized verdicts as a bounded preview and a handle to complete sealed evidence.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [x] A large hand verdict is preserved outside the window and delivered through a bounded preview/handle. The raw verdict is written whole to a trace file; the preview is capped at `PREVIEW_HEAD_BYTES`.
- [x] Main can retrieve the full verdict, and short verdicts retain their ordinary delivery. Retrieval reuses the existing `admit()`/`is_trace_rooted` echo path rather than a new mechanism; the under-threshold branch is a pure early return, so byte-identical delivery is guaranteed by control flow, not merely observed.
- [x] The resolution is linked with evidence. The original point was revalidated as still true: `admission::admit` had zero call sites anywhere on the delivery path.
- [x] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: main, admission. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

Delivered and awaiting owner review. The carried point was revalidated as still live before any repair: `claude_hand::build_verdict_text` returned the SDK result whole, `format_delivery` wrapped it losslessly, and `run_turn_cycle` appended it to the window with **no** call to `admission::admit` anywhere on that path. Admission was wired for tool results only; a verdict is an injected message, and that was the gap.

New `admit_verdict`/`VerdictRequest` in `admission.rs` and `bound_verdict_delivery` in `main.rs`, wired at both delivery pump sites. Gating applies to the **whole formatted delivery line**, not the raw hand text, so reviewer-schema and drain-proposal validation still see the original unbounded text while everything entering main's window is uniformly bounded — which also closes the well-formed reviewer-JSON branch the problem statement flagged. `claude_hand.rs`, `container_hand.rs` and `native_hand.rs` are untouched: bounding belongs at the delivery seam, not in each body.

Short verdicts are byte-identical by construction. The under-threshold branch returns `req.body.to_string()` before any code that could alter bytes, so no path between `format_delivery` and `append_turn_as` can touch it. All six tripwire assertions named in the brief are unedited, and their fixtures are tens to ~200 bytes against a 4000-byte threshold with no `hand_verdict` override anywhere.

Tests went 440 to 449, delta exactly 9. The verify gate ran the three Docker-backed tripwire files the runner had only reasoned about: `claude_main` 90/90 and `layer_close` 12/12 both clean, and `layer2_close` failed only at `assert_docker_and_hand_roots_clean()` on line 353 — after the verdict assertions at 324-335 had already run and passed — on stray `/tmp/arx-hands` entries whose timestamps and PIDs predate the run. That assertion is unscoped to the current run and belongs to [E23](E23-diagnose-test-isolation.md); nothing here touches `container_hand.rs`.

**Codex found a real defect and it was fixed, not argued.** Round 1: verdict bounding discarded the `Event::Admission` that D09's calibration record depends on. `bound_verdict_delivery` now returns `(String, Vec<Event>)` and both pump sites log each event before injection. The emitted event is structurally identical to the tool-result path's, with one deliberate nuance — `bytes` measures the wrapped delivery line rather than the raw hand text, which is what is actually being bounded. There is correctly no `Pull` or cached-echo analogue: those exist for self-reads and same-sha cache hits, neither of which a verdict has, and retrieval of an externalized verdict reuses the pre-existing path where they already live. Round 2 approved.

**Named residual — the structured-head heuristic.** The preview keeps the verdict's fenced head via `rfind("```yaml")`, not a parser. Taking the *last* fence is deliberate and correct under this project's convention of one fenced block with nothing after it, since it prefers the real verdict over an earlier quoted example. But a verdict whose prose quotes a second **complete** ```yaml block *after* its real head will misdirect the preview, and the leading-slice fallback does **not** catch that — it only fires when no fence exists at all.

The failure is bounded rather than dangerous: the raw verdict is written whole regardless of which fence the heuristic picks, so full retrieval never breaks and only the preview's usefulness degrades. This sits within decision `e65b2a35`'s accepted trade-off that truncation sometimes takes the wrong slice. It is undocumented in code; a comment on `structured_verdict_head` naming the case would be worth adding whenever that function is next touched.

`prompts/main.md` gained a line describing threshold crossing, the structured-head preview, the distinct handle, and explicitly that a failed hand is bounded the same way.

Remaining gap: none for this card.

