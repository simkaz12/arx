---
id: E13
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

# E13 — Make hand verification capabilities explicit

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L34)

## Existing point

Container hands cannot run the Docker-backed suite without host access; the images' Rust toolchain was also unverified.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: grill-me.

Check actual image/toolchain capability, then agree the verification route: host-run checks, supported execution tooling, or explicit hand limitations. Do not silently mount the Docker socket.

The expectation is not locked. Flow must use the owner's answer rather than infer approval from this card.

## Done when

- [ ] Hand briefs/prompt claims match what the image can execute.
- [ ] Required host checks have an explicit owner and recorded result before a card is verified.
- [ ] Any added host capability is scoped and agreed, with the container-isolation implications recorded.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: container, verification. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

No work is claimed. The original record above is the starting evidence; there is no new brief or verification result yet.

