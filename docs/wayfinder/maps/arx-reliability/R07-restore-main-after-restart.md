---
id: R07
effort: arx-reliability
labels: wayfinder:task
status: open
stage: needs-decision
assignee: none
blocked-by: R05, R06
priority: P1
next: grill-me
brief: none
verification: none
---

# R07 — Restore main from arx state after restart

[Board](map.md) · [Audit finding R07](findings/project-review-2026-09-12.md#R07)

## Problem and evidence

Restart arx after a checkpoint and later work have been recorded. Startup opens the old log and checkpoint metadata, creates a fresh vendor session, resets last_handle to turn zero, and accepts input without orienting main from the checkpoint or transcript.

The operator's next line reaches a main with no restored work state while the durable transcript continues its old numbering.

Evidence strength: Missing capability, not a model-switch regression. The glossary promises crash recovery, but L3-02 only specified in-process reset. Absence is confirmed; ordinary reopen semantics need an owner decision. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Define whether every reopen resumes, or distinguish crash recovery with a clean-close marker. For recovery, seed the handle from the log and run orientation before accepting operator input.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/main.rs](../../../../crates/arx/src/main.rs#L2990)
- [crates/arx/src/main.rs](../../../../crates/arx/src/main.rs#L3135)
- [CONTEXT.md](../../../../CONTEXT.md#L132)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: main, ledger. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Run grill-me on this question:

Does every ordinary reopen resume the existing thread, or should only an unclean exit trigger recovery? If a new conversation remains possible, how is it selected? Agree this together with R04's thread lifetime; do not infer it from the old in-process reset implementation.

Execution prerequisites: [Reconcile orphaned hands after restart](R05-recover-orphaned-hands.md), [Reopen transcripts with a torn final write](R06-reopen-torn-transcripts.md). These are completion dependencies, not a reason to repeat their investigation.

## Done when

- [ ] A second process orients main from the agreed checkpoint and transcript gap before accepting its first operator turn.
- [ ] last_handle starts from the recovered committed log, and recovered hand state reflects R05's reconciliation.
- [ ] Clean exit, crash and an explicit fresh start behave exactly as agreed.
- [ ] The fresh vendor session consumes arx's recovery state without resuming an old vendor session.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Use two arx processes: the first records a checkpoint and subsequent turns; the second must orient on that state and its transcript gap before its first operator turn.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- The question above remains open. Listing a suggested fix does not decide it.

## Contract references

- [Glossary](../../../../CONTEXT.md) defines main's thread, reset, tool and operator interfaces.
- [Glossary](../../../../CONTEXT.md) promises recovery of orphaned containers/worktrees from the durable ledger.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

No work has been claimed. No brief, implementation or verification is attached yet. When the stage changes, replace this paragraph with only the settled choices, named remaining gap and artifact links the next worker needs.

