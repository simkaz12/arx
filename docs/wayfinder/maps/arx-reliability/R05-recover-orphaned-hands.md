---
id: R05
effort: arx-reliability
labels: wayfinder:task
status: open
stage: blocked
assignee: none
blocked-by: R02
priority: P1
next: focused-trace
brief: none
verification: none
---

# R05 — Reconcile orphaned hands after restart

[Board](map.md) · [Audit finding R05](findings/project-review-2026-09-12.md#R05)

## Problem and evidence

arx crashes or is killed while a container hand and its worktree are active. SpawnLedger::open recovers the next identifier but initializes an empty hands map. There is no startup reconciler that finds, kills, and sweeps nonterminal hands.

Containers, detached worktrees, and evidence can survive outside the restarted process. Main never receives the promised recovered terminal delivery.

Evidence strength: New. The glossary explicitly promises this recovery; implementation is absent. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Persist cleanup metadata before launch. Reconcile nonterminal ledger rows with labeled containers and hand roots on startup, then record and seal the recovered result before accepting dispatches.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/ledger.rs](../../../../crates/arx/src/ledger.rs#L311)
- [CONTEXT.md](../../../../CONTEXT.md#L121)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: capture, ledger. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Confirm which cleanup identifiers already exist in durable spawn rows, and how to distinguish an orphan from a hand owned by another live arx process. Reuse R02's capture result and the existing kill/sweep contract.

Execution prerequisites: [Preserve hand work when capture fails](R02-preserve-failed-hand-captures.md). These are completion dependencies, not a reason to repeat their investigation.

## Done when

- [ ] After arx is killed during a delayed write hand, restart finds the nonterminal hand and records a recovered terminal result.
- [ ] Orphaned containers are stopped and captured; cleanup succeeds or preserves a failed capture under R02.
- [ ] A recovered hand has a truthful trace/failure handle before startup reports its terminal state.
- [ ] Reconciliation is idempotent and does not kill containers belonging to another live session.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Kill arx during a delayed write hand, restart, and assert no orphan container/worktree remains and a terminal ledger row plus captured trace exists.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Container capture contract](../arx-v0/L2-07-container-runtime.md) requires isolation and capture before teardown.
- [Glossary](../../../../CONTEXT.md) promises recovery of orphaned containers/worktrees from the durable ledger.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

No work has been claimed. No brief, implementation or verification is attached yet. When the stage changes, replace this paragraph with only the settled choices, named remaining gap and artifact links the next worker needs.

