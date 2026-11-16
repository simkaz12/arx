---
id: R16
effort: arx-reliability
labels: wayfinder:task
status: open
stage: blocked
assignee: none
blocked-by: R04
priority: P2
next: to-spec
brief: none
verification: none
---

# R16 — Isolate fallback traces by project and thread

[Board](map.md) · [Audit finding R16](findings/project-review-2026-09-12.md#R16)

## Problem and evidence

The normal trace directory is unwritable and two sessions use matching main/hand and turn counters. Fallback roots are /tmp/arx-traces/main and main.h1, with reset sequence numbers. create_new correctly refuses a pre-existing filename.

A fallback that should preserve an oversized tool result instead fails on another session's file.

Evidence strength: New, degraded path only. create_new prevents overwriting existing evidence. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Namespace fallback roots by persistent project/session identity, with an explicit retention policy.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/main.rs](../../../../crates/arx/src/main.rs#L1773)
- [crates/arx/src/native_hand.rs](../../../../crates/arx/src/native_hand.rs#L729)
- [crates/arx/src/admission.rs](../../../../crates/arx/src/admission.rs#L254)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: main, native, admission. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Use to-spec on this card and its linked evidence. Recheck the named failure seam against the chosen base and put the regression before the fix. The audit establishes the problem; the brief must lock the implementation scope.

Execution prerequisites: [Isolate quarantine by project and thread](R04-isolate-project-thread-quarantine.md). These are completion dependencies, not a reason to repeat their investigation.

## Done when

- [ ] Two projects with matching turn and hand counters preserve distinct fallback traces when their normal trace roots fail.
- [ ] Restart follows R04's identity contract without colliding with prior retained fallback files.
- [ ] Existing fallback evidence is not overwritten; handles still resolve to the correct trace.
- [ ] Fallback retention and cleanup do not delete another session's evidence.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Force the normal trace path to fail for two projects with identical counters; both must preserve distinct results.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Glossary](../../../../CONTEXT.md) defines main's thread, reset, tool and operator interfaces.
- [Native loop contract](../arx-v0/L2-10a-native-loop-multi-turn.md) and [glossary](../../../../CONTEXT.md) define failure and cancellation behavior.
- [Admission contract](../arx-v0/L3-01-admission.md) requires preserved externalized evidence with resolvable handles.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

No work has been claimed. No brief, implementation or verification is attached yet. When the stage changes, replace this paragraph with only the settled choices, named remaining gap and artifact links the next worker needs.

