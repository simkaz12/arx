---
id: E08
effort: arx-reliability
labels: wayfinder:task
status: open
stage: needs-decision
assignee: none
blocked-by: none
priority: P3
next: grill-me
brief: none
verification: none
---

# E08 — Settle the meaning of the reset fired event

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L20)

## Existing point

The existing brief and test ordering disagree on whether fired means the reset began or its ritual passed.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: grill-me.

Agree when fired must occur, including floor, unavailable-body recovery and failed rituals; record the answer before changing emission order.

The expectation is not locked. Flow must use the owner's answer rather than infer approval from this card.

## Done when

- [ ] The event's meaning and required ordering are recorded for every reset outcome.
- [ ] A subsequent implementation, if needed, is verified against that agreed event sequence.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: main, events. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

No work is claimed. The original record above is the starting evidence; there is no new brief or verification result yet.

