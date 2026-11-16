---
id: E11
effort: arx-reliability
labels: wayfinder:task
status: open
stage: needs-check
assignee: none
blocked-by: none
priority: P2
next: focused-trace
brief: none
verification: none
---

# E11 — Resolve provider family independently of its config key

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L29)

## Existing point

An OpenAI-compatible provider named gpt matched Codex's family while the same provider named openai did not.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: focused-trace.

Trace current registry family resolution and derive family from the supported provider/model binding rather than an arbitrary alias.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [ ] Renaming a provider config key does not alter family independence for the same binding.
- [ ] Actually different families remain distinct; same-family notices still follow successful dispatch.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: roster, reviewer. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

No work is claimed. The original record above is the starting evidence; there is no new brief or verification result yet.

