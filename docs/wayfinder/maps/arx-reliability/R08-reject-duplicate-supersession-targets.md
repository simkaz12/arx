---
id: R08
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P1
next: owner-review
brief: docs/specs/2026-09-13-r08-reject-duplicate-supersession-targets.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
pr: https://github.com/simkaz12/arx/pull/28
verified-revision: 6d6048bb88027bede0d717f771ce5c1a36306c74
---

# R08 — Reject duplicate supersession targets

[Board](map.md) · [Audit finding R08](findings/project-review-2026-09-12.md#R08)

## Problem and evidence

Two promoted candidates in one drain both supersede the same in-force claim. Validation checks each plan against the pre-write cache independently. Apply writes both successors, then overwrites the predecessor's singular superseded_by field twice.

Both successors claim the predecessor, but it points only to the second. A successful commit contains contradictory lineage.

Evidence strength: New. High-confidence source trace; existing supersession tests use only one successor per target. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Reject duplicate scope/target supersession pairs during whole-plan validation, before any write.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx-engine/src/engine/mod.rs](../../../../crates/arx-engine/src/engine/mod.rs#L649)
- [crates/arx-engine/src/engine/mod.rs](../../../../crates/arx-engine/src/engine/mod.rs#L728)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: store. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Use to-spec on this card and its linked evidence. Recheck the named failure seam against the chosen base and put the regression before the fix. The audit establishes the problem; the brief must lock the implementation scope.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [x] A drain containing two promotions for one scope/target is rejected before any mutation.
- [x] Rejected batches leave claim files, quarantine and git state unchanged.
- [x] Valid batches with different predecessors still commit and preserve bidirectional lineage.
- [x] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [x] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Submit two candidates targeting one claim. Assert rejection with byte-identical store and quarantine.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Engine contract](../arx-v0/L2-04-engine.md) and [glossary](../../../../CONTEXT.md) define file truth, drain consistency and internal write coordination.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

Delivered and awaiting owner review. `validate_drain_plan` now scans the whole plan into a `BTreeSet<(scope, target_id)>` before its per-candidate loop and rejects a duplicate supersession target outright, so no `fs::write` or `git::commit` can run on a contradictory batch. Two regressions cover it: one asserts the rejection leaves the scope directory, quarantine directory, `git status --porcelain` and head count byte-identical; the other proves two supersessions against *different* predecessors still commit as one and keep lineage bidirectional. `cargo test -p arx-engine` is 47 passed, 1 ignored, 0 failed.

Named limitation, deliberately not fixed here. The rejection returns `StoreError::UnsupportedSuggestion`, whose Display reads "unsupported drain suggestion: …". The check also fires on the auto-drain path (`drain()`, `suggestions: None`), where no caller supplied a suggestion at all — two plain `save()` calls naming the same `supersedes` target trip it. That string reaches a real agent turn via `main.rs:1856,1880`, so the wording can misdirect someone debugging a rejected background drain toward a suggestion they never sent. The `detail` half is accurate, so it is a rough edge rather than a dead end. The clean fix is a distinct `DuplicateSupersessionTarget { scope, target_id }` variant plus one Display arm — about 12 lines in `lib.rs`, which this brief placed out of scope. Nothing matches `StoreError` exhaustively (`main.rs:1849-1881` special-cases only `Io` and falls through to a catch-all), so a new variant cascades nowhere. Worth folding in the next time `lib.rs` is open.

Remaining gap: none for this card. R03, R11, R12 and R13 all edit `crates/arx-engine/src/engine/mod.rs` and must branch from `main` only after this PR merges.

