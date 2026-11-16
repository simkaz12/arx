---
id: X03
effort: arx-reliability
labels: wayfinder:task
status: open
stage: external
assignee: none
blocked-by: none
priority: P2
next: focused-trace
target-project: skillbook
brief: none
verification: none
---

# X03 — Clean up runner-owned wait processes

[Board](map.md) · [Original observation](../arx-v0/findings/carried-findings.md#L43)

## Existing point

Prior runner sessions left polling/wait shells alive for hours after returning a verdict. This is an old observation that needs revalidation.

## Deliverable

Trace the canonical runner workflow's process ownership and termination paths. Verify that success, failure and interruption leave no runner-owned polling process behind.

## Routing and constraints

This follow-up belongs in `~/Projects/skillbook`. Start a focused trace there using that repository's instructions. Do not edit generated files under `~/.codex` or `~/.claude`, and do not apply arx's card-landing commit exception to another repository.

Keep this card as the single pointer until a canonical skillbook task is created. Then replace its handoff with that task's link and let the receiving tracker own progress; do not maintain two active copies.

## Done when

- [ ] The current canonical workflow is checked against the original observation.
- [ ] If behavior still fails, a bounded skillbook task/brief owns the correction.
- [ ] Resolution evidence or the receiving task is linked; a transfer is labeled transferred, not an implemented arx fix.

## Handoff

No receiving task exists yet. No work is claimed.

