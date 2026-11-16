---
id: R04
effort: arx-reliability
labels: wayfinder:task
status: open
stage: needs-decision
assignee: none
blocked-by: none
priority: P1
next: grill-me
brief: none
verification: none
---

# R04 — Isolate quarantine by project and thread

[Board](map.md) · [Audit finding R04](findings/project-review-2026-09-12.md#R04)

## Problem and evidence

Project A saves a candidate, then project B runs with the same per-user store root. All runs name the engine thread main. Quarantine is root/quarantine/main, with no project or scope segment. B lists and drains A's candidates while its drain hand sees B's repository and B's threads/main.jsonl.

A claim can be judged against the wrong source transcript. Different scopes collide too because scope is only a field in the candidate row. Partner repositories sharing a scope are also affected.

Evidence strength: New, latent. The inspected live store was clean, with one scope and empty quarantine; no existing corruption was observed. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Persist a globally unique project/thread identity and carry it through save, quarantine listing, drain inputs, and transcript resolution.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx-engine/src/engine/mod.rs](../../../../crates/arx-engine/src/engine/mod.rs#L138)
- [crates/arx/src/main.rs](../../../../crates/arx/src/main.rs#L221)
- [crates/arx/src/main.rs](../../../../crates/arx/src/main.rs#L416)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: store, main. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Run grill-me on this question:

How should existing candidates under the ambiguous shared quarantine/main be recovered: leave them in an explicit legacy holding area, or require an operator-supplied project assignment? Also settle whether a thread identity survives ordinary reopen; coordinate that expectation with R07. No candidate may be silently assigned or discarded.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [ ] Two workspaces sharing one store cannot list or drain each other's candidates, for both equal and different scopes.
- [ ] Each candidate resolves its source transcript in the originating workspace/thread.
- [ ] Identity survives or changes across reopen exactly as agreed; it is reusable by R16.
- [ ] Existing legacy candidates follow the agreed migration policy without silent reassignment or loss.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Run two workspaces against one store root, both with identical and different scopes. Each must list and drain only its own candidates and resolve their source handles correctly.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- The question above remains open. Listing a suggested fix does not decide it.

## Contract references

- [Engine contract](../arx-v0/L2-04-engine.md) and [glossary](../../../../CONTEXT.md) define file truth, drain consistency and internal write coordination.
- [Glossary](../../../../CONTEXT.md) defines main's thread, reset, tool and operator interfaces.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

No work has been claimed. No brief, implementation or verification is attached yet. When the stage changes, replace this paragraph with only the settled choices, named remaining gap and artifact links the next worker needs.

