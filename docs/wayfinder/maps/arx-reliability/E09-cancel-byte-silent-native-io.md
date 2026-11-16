---
id: E09
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

# E09 — Make native cancellation interrupt silent I/O

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L22)

## Existing point

Native provider calls block until bytes arrive or the 60-second request timeout expires. The existing brief explicitly accepts that limit.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: grill-me.

Decide whether to reopen the accepted cancellation bound. If reopened, investigate an interruptible provider seam and brief stalled-header and byte-silent-body behavior.

The expectation is not locked. Flow must use the owner's answer rather than infer approval from this card.

## Done when

- [ ] The cancellation bound is explicit for both stalled headers and a byte-silent response body.
- [ ] If changed, cancellation tests exercise genuinely silent I/O rather than keep-alive lines; teardown remains recorded.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: native, provider. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

No work is claimed. The original record above is the starting evidence; there is no new brief or verification result yet.

