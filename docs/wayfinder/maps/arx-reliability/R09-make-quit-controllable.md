---
id: R09
effort: arx-reliability
labels: wayfinder:task
status: open
stage: needs-decision
assignee: none
blocked-by: none
priority: P2
next: grill-me
brief: none
verification: none
---

# R09 — Keep quit controllable while work is active

[Board](map.md) · [Audit finding R09](findings/project-review-2026-09-12.md#R09)

## Problem and evidence

Quit panes while a hand is hung or stopped waiting for steering. Quitting during a long main turn also does not itself send its Stop control. The UI exits, but shutdown waits for in-flight hands before shutting down the adapter. The operator has lost the controls needed to resolve a stopped or hung hand.

arx can stay alive indefinitely with no usable front.

Evidence strength: Production gap inherited from an explicit fake-hand layer-1 deferral; the real-hand behavior needs a policy. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Keep a draining front with kill controls or define a bounded quit policy that stops, kills, and sweeps remaining work. Send main an explicit stop when quit requires it.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/main.rs](../../../../crates/arx/src/main.rs#L3743)
- [crates/arx/src/panes/app.rs](../../../../crates/arx/src/panes/app.rs#L1281)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: main, panes, ledger. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Run grill-me on this question:

Should quit retain a visible draining front with kill controls, or cancel remaining work within a defined deadline? Set how stopped hands, an active main turn, EOF and a second quit request behave. The old fake-hand infinite wait is not an agreed policy for production hands.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [ ] Quit during a delayed main turn follows the agreed stop/finish behavior.
- [ ] Quit with a delayed or stopped hand leaves an effective way to terminate it; the process does not become invisibly unmanageable.
- [ ] All terminated work is captured or explicitly preserved on capture failure before exit.
- [ ] EOF and interactive quit follow their recorded policies, including any deadline or escalation.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Quit during a delayed main, delayed hand, and stopped hand. Verify termination controls remain effective and traces are sealed.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- The question above remains open. Listing a suggested fix does not decide it.

## Contract references

- [Glossary](../../../../CONTEXT.md) defines main's thread, reset, tool and operator interfaces.
- [Glossary](../../../../CONTEXT.md) defines the front and session gauge; historical transcript replay is not assumed.
- [Glossary](../../../../CONTEXT.md) promises recovery of orphaned containers/worktrees from the durable ledger.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

No work has been claimed. No brief, implementation or verification is attached yet. When the stage changes, replace this paragraph with only the settled choices, named remaining gap and artifact links the next worker needs.

