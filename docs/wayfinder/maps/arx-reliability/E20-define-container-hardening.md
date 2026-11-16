---
id: E20
effort: arx-reliability
labels: wayfinder:task
status: open
stage: deferred
assignee: none
blocked-by: none
priority: P1
next: grill-me
brief: none
verification: none
---

# E20 — Define container hardening for live hands

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/map.md#L270)

## Existing point

Network, capability, root-filesystem and resource restrictions were explicitly deferred. Closing egress also affects model access.

The prior owner deferral remains in force. Creating this card does not reopen it.

## Deliverable and next flow step

Next activity: grill-me.

When reopened, agree the containment goals and required network access, then split the work into bounded policy/proxy/resource briefs. This is a design task, not one giant implementation dispatch.

Keep this card off automatic selection. An explicit owner resumption can begin the named investigation or agreement step.

## Done when

- [ ] The owner has agreed the threat/operability constraints, required endpoints and resource expectations.
- [ ] The chosen approach has executable acceptance checks and bounded child briefs before implementation starts.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: container, networking. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Split newly specified implementation work before dispatch; this card does not authorize a whole-feature runner session.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

No work is claimed. The original record above is the starting evidence; there is no new brief or verification result yet.

