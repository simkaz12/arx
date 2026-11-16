---
id: E16
effort: arx-reliability
labels: wayfinder:task
status: open
stage: needs-check
assignee: none
blocked-by: none
priority: P3
next: diagnose
brief: none
verification: none
---

# E16 — Clean temporary roots owned by engine tests

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L44)

## Existing point

An overnight run left hundreds of arx-engine-* directories. The old observation needs checking against current test helpers.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: diagnose.

Identify which current helpers fail to clean their own roots, then fix cleanup on success and failure paths.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [ ] A bounded test run cleans the temporary roots it creates.
- [ ] Cleanup never removes another process's roots and preserves requested debugging evidence under an explicit policy.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: engine tests. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

No work is claimed. The original record above is the starting evidence; there is no new brief or verification result yet.

