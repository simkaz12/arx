---
id: E18
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

# E18 — Agree and apply empty-tools semantics across bodies

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L47)

## Existing point

An empty tools list negates tools for Codex but invokes defaults for Claude. Prior work permitted the mismatch while disclosed.

Also recheck the [old native default-tool disclosure observation](../arx-v0/findings/carried-findings.md#L12). Its named hand-ds definition has disappeared, so do not assume that observation remains current.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: grill-me.

Agree what omitted and explicitly empty tool lists mean across bodies, then brief the adapter changes and roster presentation together.

The expectation is not locked. Flow must use the owner's answer rather than infer approval from this card.

## Done when

- [ ] The agreed omitted/empty/explicit-list matrix is covered for supported bodies.
- [ ] Main's roster description matches each body's effective tool possession.
- [ ] The old native default-tool disclosure point is confirmed or retired using the current rendered roster.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: config, body adapters. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

No work is claimed. The original record above is the starting evidence; there is no new brief or verification result yet.
