---
id: R02
effort: arx-reliability
labels: wayfinder:task
status: open
stage: blocked
assignee: none
blocked-by: coherent-main-base
priority: P1
next: establish-coherent-main-base
brief: docs/specs/2026-09-12-r02-preserve-failed-hand-captures.md
verification: none
---

# R02 — Preserve hand work when capture fails

[Board](map.md) · [Audit finding R02](findings/project-review-2026-09-12.md#R02)

## Problem and evidence

git add/git diff fails during collection, or writing the captured trace fails. Worktree::diff().ok() discards the diff error. Sweep failure becomes absent optional fields. The worktree and root are removed anyway, and delivery still advertises a trace handle.

The only copy of a hand's edits can disappear, or main receives a handle to an incomplete trace without a mechanical failure.

Evidence strength: New. High-confidence source trace. Existing tests cover successful capture. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Return a typed capture outcome. Only delete the worktree/root after a sealed capture succeeds. On failure, preserve them and deliver an explicit error with their paths.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/container_hand.rs](../../../../crates/arx/src/container_hand.rs#L485)
- [crates/arx/src/container_hand.rs](../../../../crates/arx/src/container_hand.rs#L566)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: capture. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

[Execution brief](../../../specs/2026-09-12-r02-preserve-failed-hand-captures.md) prepared on 2026-09-12. The owner authorized implementation on 2026-09-13 with 'continue'. The remaining execution gate is a coherent main base containing the audited existing changes; `coherent-main-base` is not a predecessor card ID. Once satisfied, record the revision in the brief, create its task branch, then dispatch runner, reviewer and verify under the board's PR handoff rules.

No logical prerequisite card is known. Shared-file overlap remains a scheduling constraint.

## Done when

- [ ] Independent diff failure and trace-write failure reach main as mechanical capture failures.
- [ ] The only worktree/root is retained on failure, with a usable diagnostic path.
- [ ] A failed or incomplete capture advertises no sealed trace or applicable diff.
- [ ] Successful completion and explicit kill still seal capture before deleting temporary work.
- [ ] A killed write hand's terminal delivery names its sealed applicable diff, so retained work is findable without guessing a trace path.

The killed-diff discoverability point is also carried from the [existing map](../arx-v0/map.md#L260); it belongs to this same capture/delivery result.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Inject diff failure and trace-write failure independently; assert retained work, an error delivered to main, and no claimed valid trace/diff handle.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Container capture contract](../arx-v0/L2-07-container-runtime.md) requires isolation and capture before teardown.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

The [brief](../../../specs/2026-09-12-r02-preserve-failed-hand-captures.md) locks typed capture failure, retained recovery paths, no valid handles after failure, and killed-diff delivery. The focused trace confirmed discarded capture errors in both CLI and native finalization. It named regressions in `hands_in_containers.rs` and `native_in_container.rs`, plus pane settlement checks. Scratch probes validated fault injection; the complete regressions have not run.

The current audited tree is dirty on main at `3c92977302a28a8accec18b79c5434a74a82b299`; the runner/reviewer migration overlaps R02's files and cannot be replaced with that clean HEAD. Implementation is authorized as of 2026-09-13. The remaining gate is a coherent committed main base. No implementation, branch, commit or verification verdict was produced. The baseline validation recorded in the brief passed 755 workspace tests (2 ignored), config validation and 15 sidecar tests; the initial elapsed-time assertion failed once and passed on rerun. No existing branch or PR supplies the missing committed base. Claim released pending separate authorization to commit the pre-existing changes.
