---
id: R03
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P1
next: owner-review
brief: docs/specs/2026-09-13-r03-recover-interrupted-drains.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS on the delivered scope, with two Done-when items explicitly unmet — see Handoff
pr: https://github.com/simkaz12/arx/pull/35
verified-revision: 12868c537af1b348fa1dd271deac0af024f084e6
---

# R03 — Make interrupted drains recoverable

[Board](map.md) · [Audit finding R03](findings/project-review-2026-09-12.md#R03)

## Problem and evidence

A write or git commit fails after at least one successor claim has been written. Successors and predecessors are changed before commit. Retry sees an existing successor file, skips the entire plan including predecessor repair and commit accounting, then deletes the durable quarantine row.

Claims can remain untracked, staged for an unrelated later commit, or linked to a predecessor that is still in force. The retry can remove the candidate needed to finish recovery.

Evidence strength: New. High-confidence source trace; exact filesystem failure interleavings are not reproduced. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Make the drain a recoverable transaction, with staged state and a durable completion record. File existence alone must not mean that predecessor update and commit succeeded.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx-engine/src/engine/mod.rs](../../../../crates/arx-engine/src/engine/mod.rs#L711)
- [crates/arx-engine/src/engine/git.rs](../../../../crates/arx-engine/src/engine/git.rs#L49)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: store. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Trace one failed successor write and one failed git commit through restart/retry. Establish a recoverable transaction strategy against the existing all-or-nothing contract, including all-drop batches. If this exceeds one build session, split the card before briefing; do not invent the transaction protocol in a runner window.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [~] Injected failures during successor/predecessor writes, git add/commit and quarantine deletion leave a recoverable state. **Partial.** Recoverable for a crash before predecessor repair. A crash *after* the repair write but before commit is recoverable only by manual intervention — see residual 1 below.
- [~] Restart and retry complete the intended batch without broken lineage, untracked promoted truth or claims folded into an unrelated commit. **Partial**, same window and same cause as above.
- [x] An existing successor file is not treated as proof that all durable steps finished. Fully met: `git::is_uncommitted` replaces bare file-existence with a git-state check on both the successor and the predecessor.
- [~] All-drop batches have a recoverable completion record; a crash cannot silently erase half their evidence. **Half met.** The erasure half is fixed structurally: the unconditional `to_delete.push` that sat above the Drop/Promote branch is gone, and each arm now queues its quarantine row for deletion only after its own work succeeds. The *completion record* half is not built — no journal or durable marker exists, and inventing one was locked out of this card's scope.
- [x] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [x] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

**This card is not Done.** Two behavioural items above are partial and one is half met. It sits at `pr-open` because the delivered diff passed all three gates on its own scope, not because the card's deliverable is complete.

## Verification starting point

Fault-inject successor write, predecessor write, git add, git commit, and quarantine deletion. Restart and retry; assert complete lineage and one recorded result. The existing all-or-nothing test only rejects invalid plans before writing.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- The known all-drop interruption defect is included because it uses the same drain completion path.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Engine contract](../arx-v0/L2-04-engine.md) and [glossary](../../../../CONTEXT.md) define file truth, drain consistency and internal write coordination.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

Delivered and awaiting owner review, but **the card's deliverable is only partly met** — see the qualified Done-when items above before treating this as closed.

What landed. `apply_drain_plan` no longer gates on bare file existence. The unconditional `to_delete.push` that sat above the Drop/Promote branch is removed; each arm now queues its quarantine row for deletion only after its own work succeeds. `git::is_uncommitted` (new, in `git.rs`) distinguishes "written but never committed" from "already landed", so a retry re-stages a crash-surviving file into the batch commit instead of leaving it permanently untracked, and does not force a commit with nothing staged. Two regressions cover a pre-created successor with an unrepaired predecessor, and a `.git/index.lock`-forced commit failure that leaves the quarantine row intact and then recovers on retry. `cargo test -p arx-engine` is 47 passed, 1 ignored.

Relative to `main` this is a strict narrowing of bad outcomes: from permanent data loss on a crash before commit, to no data loss on any crash before commit, with one window now requiring manual intervention rather than self-healing.

**Residual 1 — poison-pill stall on the drain queue, fixed on this PR's branch.** If a crash landed after the predecessor was fs-repaired to `Superseded` but before the commit, `validate_drain_plan` (`mod.rs:657-717`) used to reject that candidate on every subsequent retry with `MalformedClaimFile{"...is already superseded"}`, because it loaded current filesystem state and saw `lifecycle != InForce` with no allowance for a predecessor already pointing at the promoted successor.

That blast radius was wider than one row: `validate_drain_plan` runs over the *entire* candidate list before `apply_drain_plan` touches anything and returns on the first invalid candidate, so one poisoned row blocked every other pending claim on that thread. No bytes were lost — the quarantine row, the successor and the repaired predecessor all survived — but promotion for that thread stopped indefinitely.

A P1 review comment on this PR asked for the fix directly in `validate_drain_plan`: teach it that a predecessor already pointing at *this exact successor id* is idempotent-complete rather than a conflict. `validate_drain_plan` and `apply_drain_plan` now share one predicate, `accepts_supersession_from` (built on `already_repaired_by` for the exact-match check), so a retry that finds the predecessor already repaired by the current successor is accepted instead of rejected. A predecessor pointing at a *different* successor is still rejected as a genuine conflict, with no automatic way out — clearing it still takes an operator or hand recognising the mismatch and issuing a `Drop` via `drain_with` (decision `68714658`, "the drain is two pens"), past the `UncoveredCandidate` guard at `mod.rs:635`. Three regressions cover the fixed and the still-rejected cases: restart after the predecessor rewrite landed but before the commit, restart after a successful commit but before the quarantine row was deleted, and `drain_rejects_retry_when_predecessor_was_repaired_by_a_different_successor`, which pins that a predecessor repaired by a different successor is still refused on retry.

This fix landed in a follow-up PR after #35 merged without it; `cargo test --workspace` passed on the follow-up branch before merge.

**Residual 2 — no durable completion record.** The all-drop erasure defect is fixed, but neither drops nor promotions write a journal or marker proving a batch finished. The card asks for one; this card did not build it, because its brief explicitly forbade inventing a transaction protocol in a runner window, consistent with this card's own "do not invent the transaction protocol in a runner window" instruction.

**Brief defect, recorded so the next worker does not inherit it.** The brief written for this card never mentioned all-drop batches, despite the card naming them in both Done-when and Constraints. They were fixed incidentally, because the unconditional `to_delete.push` was the shared defect — not because the brief asked. The brief also substituted "make the existing retry path correct" for the card's "establish a recoverable transaction strategy", and skipped the card's named `focused-trace` step by reusing an earlier trace. The delivered work is sound within the scope it set itself; that scope was narrower than the card's.

**Design note.** Idempotency shifted from file-existence to git-state, which means `git status --porcelain` is now shelled out per touched path, per drain call. Correct, and the brief permitted touching `git.rs` with justification, but it is a real change in mechanism and cost that the brief did not anticipate.

Remaining gap: residual 2, and a decision on whether it reopens this card or becomes its own. Residual 1 is closed by the follow-up PR — see above.

