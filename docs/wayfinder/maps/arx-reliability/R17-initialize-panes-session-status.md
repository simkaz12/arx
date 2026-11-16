---
id: R17
effort: arx-reliability
labels: wayfinder:task
status: open
stage: ready
assignee: none
blocked-by: none
priority: P3
next: to-spec
brief: none
verification: none
---

# R17 — Initialize panes with known session status

[Board](map.md) · [Audit finding R17](findings/project-review-2026-09-12.md#R17)

## Problem and evidence

Open panes before the first completed main turn, especially with an existing transcript/checkpoint. App state starts with default status and a blank main entry. Startup sends no initial snapshot.

Model, window, reserve, checkpoint, and turn can appear blank or zero until later activity.

Evidence strength: New. Blank status is confirmed; replaying full historical conversation is a separate product choice. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Send known status before the first prompt. Decide transcript replay separately.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/panes/state.rs](../../../../crates/arx/src/panes/state.rs#L223)
- [crates/arx/src/panes/mod.rs](../../../../crates/arx/src/panes/mod.rs#L58)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: panes. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Use to-spec on this card and its linked evidence. Recheck the named failure seam against the chosen base and put the regression before the fix. The audit establishes the problem; the brief must lock the implementation scope.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [ ] Before the first model reply, panes shows the effective model, window, reserve, checkpoint and committed turn.
- [ ] Starting with a persisted pin or existing transcript does not show default zero/blank values instead of known state.
- [ ] The first status update remains consistent with the startup snapshot.
- [ ] This card does not add historical transcript replay.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Initialize against a nonempty transcript and assert correct status before any model response.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Glossary](../../../../CONTEXT.md) defines the front and session gauge; historical transcript replay is not assumed.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

No work has been claimed. No brief, implementation or verification is attached yet. When the stage changes, replace this paragraph with only the settled choices, named remaining gap and artifact links the next worker needs.

