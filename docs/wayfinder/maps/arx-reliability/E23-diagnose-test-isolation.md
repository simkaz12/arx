---
id: E23
effort: arx-reliability
labels: wayfinder:task
status: open
stage: needs-check
assignee: none
blocked-by: none
priority: P2
next: diagnose
brief: none
verification: none
---

# E23 — Diagnose intermittent test isolation failures

[Board](map.md) · [Original record](../../../../docs/verification/roster-wave-2-codex-main.md#L88)

## Existing point

The audit session observed one transcript-lock test failure that passed on reruns. The earlier verification report also records timing/isolation incidents; a shared cause has not been established.

The transcript-lock failure is recorded in the [audit snapshot](findings/project-review-2026-09-12.md); its cause is unknown.

## Deliverable and next flow step

Next activity: diagnose.

Start with the recorded transcript-lock failure, construct a bounded reproduction loop and isolate its cause. Trace other incidents separately unless evidence proves a common cause.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [ ] The failing test, environment and reproduction rate are recorded before any fix claim.
- [ ] A confirmed cause gets a targeted regression and bounded fix; a non-reproduction is recorded as unresolved, not fixed.
- [ ] The chosen solution does not hide the race through arbitrary sleeps or blanket test serialization.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: test infrastructure. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

No work is claimed. The original record above is the starting evidence; there is no new brief or verification result yet.

