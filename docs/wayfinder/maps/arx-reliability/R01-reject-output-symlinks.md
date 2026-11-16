---
id: R01
effort: arx-reliability
labels: wayfinder:task
status: open
stage: blocked
assignee: none
blocked-by: R02
priority: P1
next: to-spec
brief: none
verification: none
---

# R01 — Reject output symlinks during capture

[Board](map.md) · [Audit finding R01](findings/project-review-2026-09-12.md#R01)

## Problem and evidence

A hand places an absolute symlink under /hand/out, pointing to a file readable by the host arx process. copy_dir_all treats a symlink as a file and calls host-side fs::read on it. The target is resolved on the host during sweep, outside the container's mounts, and copied into the persistent trace.

The capture module defeats the filesystem isolation of the container. Relative links can also copy credentials from the sibling hand home. Directory symlinks error rather than recurse; regular-file targets are the confirmed source path.

Evidence strength: New. High-confidence source trace; no exploit was executed. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Reject symlinks throughout output capture using symlink_metadata. Treat rejection as a capture failure and preserve the hand root for inspection.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/container.rs](../../../../crates/arx/src/container.rs#L274)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: capture. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Use to-spec on this card and its linked evidence. Recheck the named failure seam against the chosen base and put the regression before the fix. The audit establishes the problem; the brief must lock the implementation scope.

Execution prerequisites: [Preserve hand work when capture fails](R02-preserve-failed-hand-captures.md). These are completion dependencies, not a reason to repeat their investigation.

## Done when

- [ ] Absolute, relative, nested and dangling output symlinks never cause their targets to be read, copied or hashed.
- [ ] Unsafe output produces the explicit failed-capture result from R02; the root stays available for inspection.
- [ ] Regular output and successful write-diff capture still work.
- [ ] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [ ] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Exercise container::sweep with a symlink to an external sentinel file. Assert that its contents are neither copied nor hashed and that capture fails explicitly.

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

No work has been claimed. No brief, implementation or verification is attached yet. When the stage changes, replace this paragraph with only the settled choices, named remaining gap and artifact links the next worker needs.

