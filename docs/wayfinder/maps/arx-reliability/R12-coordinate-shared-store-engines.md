---
id: R12
effort: arx-reliability
labels: wayfinder:task
status: open
stage: ready
assignee: none
blocked-by: none
priority: P2
next: focused-trace
brief: none
verification: none
---

# R12 — Coordinate shared-store engines and caches

[Board](map.md) · [Audit finding R12](findings/project-review-2026-09-12.md#R12)

## Problem and evidence

Two sessions use the shared store, or an external command edits/commits truth while a long-running engine has already loaded that scope. Engine keeps an unversioned in-memory map and has no root-wide mutation lock. External changes remain invisible to the cached reader; separate writers can contend or interleave git operations.

Recall can stay stale for the rest of a session, and overlapping writes lack the serialization promised at the interface.

Evidence strength: New. Own successful writes invalidate the local cache correctly; existing hand-edit tests reopen the engine and therefore miss the long-lived case. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Use a store-root OS lock around mutations and version-based invalidation for loaded scopes. Specify visibility of manual edits.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx-engine/src/engine/mod.rs](../../../../crates/arx-engine/src/engine/mod.rs#L129)
- [crates/arx-engine/src/engine/mod.rs](../../../../crates/arx-engine/src/engine/mod.rs#L259)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: store. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Separate mutation serialization from read-cache visibility. Confirm the promised visibility of committed external changes versus in-progress manual edits, and define the root-wide lock/version strategy. Coordinate the lock with R03's transaction design without treating shared files as a dependency.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [ ] Two engine instances cannot interleave mutations into mixed or unrelated git commits.
- [ ] A long-lived engine observes externally committed claim changes under the agreed visibility rule.
- [ ] Own writes and manual-edit/reopen behavior remain correct.
- [ ] Overlapping operations and interruption do not leave a permanent lock; cache invalidation uses coherent persisted state.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Use two engine instances: commit through one, recall through the other, and interleave mutations without mixed commits.

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

No work has been claimed. No brief, implementation or verification is attached yet. When the stage changes, replace this paragraph with only the settled choices, named remaining gap and artifact links the next worker needs.

