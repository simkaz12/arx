---
id: R11
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

# R11 — Preserve claim markdown across reopen

[Board](map.md) · [Audit finding R11](findings/project-review-2026-09-12.md#R11)

## Problem and evidence

An otherwise valid question contains a line such as ## Answer. Rendering inserts raw fields between headings; parsing splits on the first marker. Text belonging to the question is reclassified as the answer on reopen.

Persisted claim content silently differs from what was saved.

Evidence strength: New. A source-derived example is question 'Which section?\n\n## Answer\n\nStill the question', answer 'blue': reopen shortens the question and prefixes the answer with the remaining question text. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

At minimum reject structural marker lines before save. A fuller format fix must escape or delimit fields without ambiguity.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx-engine/src/engine/claim_file.rs](../../../../crates/arx-engine/src/engine/claim_file.rs#L57)
- [crates/arx-engine/src/engine/claim_file.rs](../../../../crates/arx-engine/src/engine/claim_file.rs#L93)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: store-format. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Check the locked human-editable claim format and existing files before choosing encoding or parsing changes. Preserve valid old files and accepted field content. If the only compatible solution reserves previously accepted marker lines, bring that restriction to grill-me rather than silently narrowing the interface.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [ ] Every accepted question/answer/why value round-trips exactly, including structural marker strings and multiline markdown.
- [ ] Existing unambiguous claim files still load with the same meaning.
- [ ] Ambiguous legacy files receive the agreed explicit handling; they are not silently reinterpreted.
- [ ] Any new rejected input is rejected before quarantine or claim mutation.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Round-trip every structural marker in each field, including multiline markdown. Existing tests use plain strings.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Engine contract](../arx-v0/L2-04-engine.md) requires human-editable file truth; any format restriction must be explicit.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

No work has been claimed. No brief, implementation or verification is attached yet. When the stage changes, replace this paragraph with only the settled choices, named remaining gap and artifact links the next worker needs.

