---
id: E03
effort: arx-reliability
labels: wayfinder:task
status: open
stage: needs-check
assignee: none
blocked-by: none
priority: P3
next: focused-trace
brief: none
verification: none
---

# E03 — Make arx_ping gating configuration truthful

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L11)

## Existing point

The sidecar answers arx_ping locally, leaving a gate rule on it inert. Roster C may have changed the relevant expectation.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: focused-trace.

Revalidate against current tool-possession and deterministic gate rules. If a behavior choice remains, agree whether to reject the inert configuration or route the call through the gate.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [ ] Supported gate configuration has observable effect or is explicitly refused.
- [ ] The chosen behavior matches the current main/tool-possession contract.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: main, sidecar. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

No work is claimed. The original record above is the starting evidence; there is no new brief or verification result yet.

