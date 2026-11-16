---
id: E19
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

# E19 — Resolve the unused gate rule writer

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L48)

## Existing point

gate::append_rule has no production caller after Ask removal; only its own tests keep it alive.

A limited source check during board preparation still found this point. Confirm the chosen base before editing.

## Deliverable and next flow step

Next activity: to-spec.

Apply the deletion test against current callers. Remove the unused interface and its implementation-specific tests unless a still-required production use is found.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [ ] No supported runtime behavior relies on the removed interface.
- [ ] Remaining gate configuration and deterministic rule checks pass; no speculative replacement interface is added.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: gate. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

No work is claimed. The original record above is the starting evidence; there is no new brief or verification result yet.

