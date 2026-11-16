---
id: R15
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P2
next: owner-review
brief: docs/specs/2026-09-13-r15-keep-file-listing-in-workspace.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
pr: https://github.com/simkaz12/arx/pull/30
verified-revision: 022f502a8a1e766e5072d6b973dfdf6f98a006b0
---

# R15 — Keep recursive listing inside the workspace

[Board](map.md) · [Audit finding R15](findings/project-review-2026-09-12.md#R15)

## Problem and evidence

A workspace directory contains a symlink to a directory outside the allowed root. Only the starting path is canonicalized. Recursive entry.is_dir follows the symlink without repeating the root check.

The list tool exposes external filenames and types. The ordinary read tool still rejects reading those contents.

Evidence strength: New. Metadata disclosure only; do not conflate this with the host-content capture escape in R01. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Do not descend symlinks, or canonicalize and recheck every directory before descent.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/main.rs](../../../../crates/arx/src/main.rs#L1537)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: main. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Use to-spec on this card and its linked evidence. Recheck the named failure seam against the chosen base and put the regression before the fix. The audit establishes the problem; the brief must lock the implementation scope.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [x] Recursive listing does not expose entries under a directory symlink that escapes the workspace.
- [x] Ordinary nested directories remain listable and returned paths stay within the declared root.
- [x] Dangling links and loops terminate without traversal or a hang. *(Loops: satisfied by the pre-existing depth clamp, not by this card — see the Handoff.)*
- [x] The chosen symlink policy is consistent with the existing read/list interface. *(At the children level; see residual (a) in the Handoff for the entry level.)*
- [x] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [x] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

List a workspace containing a symlink to an external directory; its entries must not appear.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Glossary](../../../../CONTEXT.md) defines main's thread, reset, tool and operator interfaces.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

Implemented in `crates/arx/src/main.rs` (`list_files_call`): chose canonicalize-and-recheck, not no-descend. Before pushing a directory onto the walk stack, the entry is canonicalized and checked against `canonical_root`, mirroring the `canonicalize_in_workspace` guard the read side already uses; a directory that resolves outside the root (an escaping symlink, or one that fails to canonicalize at all) is still shown as an entry but is never descended into, so its children never reach `fs::read_dir` and never appear in the listing. Internal symlinked directories keep working exactly as before. Regression test proved the pre-fix leak (`escape/secret.txt` visible in the listing) before the fix landed; four tests now live in `main.rs`'s test module next to the other file-tool tests: escaping-symlink non-disclosure, ordinary nested directories staying under root, a dangling symlink, and a symlink loop bounded with an explicit 5s channel timeout. `prompts/main.md` was left untouched — it makes no claim about list_files' symlink behavior for this fix to contradict, and ordinary workspaces without escaping symlinks see no change. Independent spec-reviewer returned PASS with an empty `must_fix`, and the separate verify gate returned PASS after re-running `cargo test -p arx list_files` (4/4) and re-reading the control flow. Test count in `main.rs` went 111 to 115, a delta of exactly 4. Codex round 1 returned approve with no material findings. Brief: `docs/specs/2026-09-13-r15-keep-file-listing-in-workspace.md`.

Two residuals were named by both gates and deliberately shipped rather than silently passed.

**(a) The escaping entry itself is still listed.** An escaping directory symlink is withheld from descent but still emitted as a `name/` line. Its name was already visible before this card — it is a real object inside the workspace, and that was never the defect — so the only fresh signal is the trailing slash, which confirms the external target currently resolves to a directory. Read strictly, the brief's consistency sentence ("a path the read tool refuses must not be a path the list tool enumerates") is met for children but not for the entry's own path, since `read_file` on that symlink would be refused. Both gates judged this a strict subset of the "metadata disclosure only" class the card already scopes, one level thinner, and shippable.

**(b) Internal symlink loops get no defense from this card.** This one is a tracked gap, not a caveat. The loop fixture's `loop/self` points back at its own parent, so it resolves cleanly with no `ELOOP`: `canonicalize()` succeeds, `stays_in_root` is true, and the new gate prunes nothing. The walk re-descends `loop/self/self/...` and terminates solely because the pre-existing `depth.clamp(1, 8)` runs out — code this card did not touch, and the test exercises exactly that boundary at `depth: 8`. So "loops terminate" is true today as a consequence of an unrelated safety net. Raising or removing that clamp, which is plausible since it currently caps useful listings at eight levels, reintroduces unbounded recursion with no code path having ever addressed it. Identity-based cycle detection is the durable fix. No follow-up card has been created; that is an owner call, not one to take unilaterally on the board.

