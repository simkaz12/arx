---
id: R10
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

# R10 — Bound file reads before admission

[Board](map.md) · [Audit finding R10](findings/project-review-2026-09-12.md#R10)

## Problem and evidence

read_file requests twenty lines from a multi-GB file, or reads a generated FIFO in the workspace. Main and local native reads load the entire file before slicing. Container native reads run blocking cat through docker exec without a timeout. Admission runs after the expensive read.

A small requested range can exhaust memory. A special file can block the tool indefinitely, and native cancellation is not polled until it returns.

Evidence strength: New. Huge regular files are an ordinary trigger; the FIFO case requires a generated or malicious workspace artifact. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Require regular files; stream bounded ranges. Bound or stream whole-file admission and make container execution timed or cancellable.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/main.rs](../../../../crates/arx/src/main.rs#L1393)
- [crates/arx/src/native_hand.rs](../../../../crates/arx/src/native_hand.rs#L65)
- [crates/arx/src/native_hand.rs](../../../../crates/arx/src/native_hand.rs#L381)
- [crates/arx/src/container.rs](../../../../crates/arx/src/container.rs#L207)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: main, native, capture. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Map ranged and whole-file reads across host and container adapters. Establish how existing admission can preserve whole-read semantics without materializing the whole file. Do not invent a new truncation limit or tool timeout contract without agreement; split by seam if necessary.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [ ] A small requested range from a large file uses bounded memory and returns the requested content.
- [ ] FIFOs and other unsupported special files fail promptly with a clear error.
- [ ] Container reads can be interrupted or end within the agreed bound.
- [ ] Whole reads preserve the agreed admission/externalization behavior without an unbounded allocation.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Read a small range from a large sparse file with bounded memory; reject a FIFO promptly; cancel a stalled container read.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Glossary](../../../../CONTEXT.md) defines main's thread, reset, tool and operator interfaces.
- [Native loop contract](../arx-v0/L2-10a-native-loop-multi-turn.md) and [glossary](../../../../CONTEXT.md) define failure and cancellation behavior.
- [Container capture contract](../arx-v0/L2-07-container-runtime.md) requires isolation and capture before teardown.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

No work has been claimed. No brief, implementation or verification is attached yet. When the stage changes, replace this paragraph with only the settled choices, named remaining gap and artifact links the next worker needs.

