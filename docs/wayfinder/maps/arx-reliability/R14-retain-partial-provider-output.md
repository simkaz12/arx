---
id: R14
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P2
next: owner-review
brief: docs/specs/2026-09-13-r14-retain-partial-provider-output.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
pr: https://github.com/simkaz12/arx/pull/31
verified-revision: 290d6de5677c695a83c75adc614b8584a49a4f7a
---

# R14 — Retain incomplete provider output in traces

[Board](map.md) · [Audit finding R14](findings/project-review-2026-09-12.md#R14)

## Problem and evidence

A provider emits text, then disconnects, times out, or sends malformed SSE. Response blocks remain local until a terminal chunk. The error path writes an error row without preserving those blocks.

The trace loses useful evidence of what the hand produced before failing.

Evidence strength: New. Adapter tests prove content-then-error can occur; current loop failure tests start with an error. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Record received blocks as explicitly incomplete output before error handling. Do not promote them to a successful verdict.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/native_hand.rs](../../../../crates/arx/src/native_hand.rs#L562)
- [crates/arx/src/native_hand.rs](../../../../crates/arx/src/native_hand.rs#L298)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: native. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Use to-spec on this card and its linked evidence. Recheck the named failure seam against the chosen base and put the regression before the fix. The audit establishes the problem; the brief must lock the implementation scope.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [x] Content followed by timeout, disconnect or malformed stream is preserved as explicitly incomplete evidence. *(Named tests cover 5 of the 6 mode/path cells; see the Handoff for why the sixth is not a code fork.)*
- [x] The hand still fails; partial content is not promoted to a successful verdict or accepted state object.
- [x] Successful terminal streams and errors before any content retain their existing outcomes.
- [x] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [x] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Feed content followed by an error through the native loop and assert an incomplete trace row plus a failed terminal result.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Native loop contract](../arx-v0/L2-10a-native-loop-multi-turn.md) and [glossary](../../../../CONTEXT.md) define failure and cancellation behavior.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

Delivered and awaiting owner review. Received blocks are now written as an explicit `{"kind": "incomplete", "content": [...]}` trace row before error handling runs, via a shared `write_incomplete` helper that no-ops on empty blocks. The marker reuses the file's existing `kind` discriminator, so an incomplete row is identified directly rather than inferred from its neighbours. Inline tests went 27 to 34, a delta of exactly 7. `cargo test -p arx --bin arx native_hand` is 34 passed, 0 failed.

**This card fixed more than its title claims, and that is worth stating plainly.** Codex round 1 found that `author_state_object`'s disconnect case had no terminal-chunk check at all: it fell through to `return Ok(joined_text(&content_blocks))`, so truncated authoring output was promoted into an *accepted state object*, appended into the reset conversation window, and triggered an unwanted respawn. That is a correctness defect, not the evidence-quality improvement this card's "Done when" language describes. It is fixed with a `saw_terminal` flag; the reviewer and the verify gate each read that branch against the main loop's own disconnect branch and confirmed they are equivalent — same trigger, same trace-row sequence, same literal error text, same `finish()` arguments. The regression asserts no `state_object` row and no `respawn` row. Fixing it here was licensed by the brief, which instructed the runner to check the second accumulation path for the same defect and fix it if found.

Coverage gap, judged non-functional. Named tests exist for all three failure modes on the main loop, but on the state-object path only disconnect has a dedicated test; timeout and malformed share one generic test using `ProviderError::Fatal`. `ladder_step` branches solely on `ContextWindowExceeded` versus everything else and never inspects the error text, so timeout and malformed take an identical path — the missing cells are a naming gap, not an untested branch.

Adjacent observation, deliberately not fixed here. Both loops poll the cancel flag mid-accumulation and return `Killed` *without* calling `write_incomplete`, so a killed hand's partial content is dropped with no incomplete row. That is deliberate-cancellation semantics and falls outside this card's three named failure modes, but it is a genuine drop of the same kind. Worth its own card if trace completeness under kill matters. The verifier also confirmed by grep that `author_state_object` and `run_brief_inner` are the *only* two content-accumulation paths in the file, rather than assuming two was the complete set.

Remaining gap: none for this card.

