---
id: R18
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P3
next: owner-review
brief: docs/specs/2026-09-13-r18-record-family-after-reviewer-spawn.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
pr: https://github.com/simkaz12/arx/pull/34
verified-revision: 359ac37151511435f18632f969e5428e12dc9f51
---

# R18 — Record reviewer family only after dispatch succeeds

[Board](map.md) · [Audit finding R18](findings/project-review-2026-09-12.md#R18)

## Problem and evidence

Select a same-family reviewer whose dispatch preparation then fails, such as for a missing credential or image. The family warning/event is written before ledger.dispatch succeeds.

The transcript and UI describe a reviewer invocation that never happened.

Evidence strength: New. Instrumentation correctness; lower priority than work preservation. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Emit the calculated family event only after successful dispatch, before the spawn event.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/main.rs](../../../../crates/arx/src/main.rs#L787)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: main. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Use to-spec on this card and its linked evidence. Recheck the named failure seam against the chosen base and put the regression before the fix. The audit establishes the problem; the brief must lock the implementation scope.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [x] A same-family reviewer that fails preparation emits neither a family collision event nor a spawn event.
- [x] Successful same-family dispatch records exactly one family event in the agreed order before the spawn event.
- [x] Successful different-family dispatch emits no collision warning.
- [x] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [x] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Fail same-family reviewer preparation and assert no family or spawn event.

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

Delivered and awaiting owner review. The family Notice and `Event::Family` write moved out of the pre-dispatch position and into the `Ok(hand)` arm of `match ledger.dispatch(spec, brief)`, immediately before the `Event::Spawn` write. Because both now sit inside the same match arm, a preparation failure makes them structurally unreachable rather than merely skipped.

The notice text is unchanged: `spawn_role` — a variable that already existed for the spawn message — replaces the positional `spec.role.label()`, which is required because `spec` is moved into `ledger.dispatch` before this code now runs. Both gates confirmed the rendered string is byte-identical, one of them by substituting into both format strings programmatically rather than by eye.

Three regressions in `main.rs`'s own test module (440 to 443, delta exactly 3), placed beside the existing dispatch and ledger tests and well clear of the file-tool tests at ~5091. They read events back from disk through `transcript::events` rather than a `RecordingFront`, whose `event()` is a no-op — a test against that sink would have proved nothing. The preparation failure is driven by a real missing image tripping `image_present()` in `container_hand::prepare`, not a mock. Ordering is asserted by index comparison, not by presence.

**Known residual, deliberately shipped, and it is wider than the card's own framing — now closed for this branch.** When the family-event *write* fails (`main.rs:802-806` at this card's verified revision), the just-dispatched reviewer was not cancelled, and its container could outlive arx. Codex raised this; it was overruled, and both later gates upheld the overrule after independent checks. A later review round reopened the question: `cancel_dispatched_hand` now retries `ledger.kill()` until it sends, then drains until the hand is no longer in flight, reissuing `kill()` on any drain error, before this branch returns `Err`. A regression, `failed_family_event_write_cancels_and_sweeps_the_already_dispatched_hand`, asserts `ledger.in_flight() == 0` afterward.

The honest part of that decision: this diff **does** newly expose the window. `ledger::dispatch` creates the worktree, writes the durable spawn-ledger row and starts the container all before returning `Ok`, so pre-move a family-write failure had no hand to orphan. The no-cancellation pattern itself is pre-existing — the `Event::Spawn` write-failure branch at `main.rs:811-823` has always had it, and `adapter_for_kill` is main's own body adapter, so killing it kills main rather than the hand.

Why it is still safe to ship, established structurally rather than by cross-reference: `CONTEXT.md`'s Spawn ledger entry defines that file as "written before the child runs... a restarted arx reads it to find orphaned containers and worktrees, kill them, and sweep." That row is written unconditionally inside `ledger::dispatch` and lives in a different file from the transcript log where `Event::Family` fails, so the artifact a reconciler needs already exists the moment dispatch succeeds. The failure is also a genuine process exit — `CycleFailure::ViaClone` propagates through `fail_on!` to `ExitCode::FAILURE` — so the next invocation is a real restart hitting `SpawnLedger::open`, which is exactly [R05](R05-recover-orphaned-hands.md)'s chartered seam.

**Instruction for R05's future brief, recorded here because it will otherwise be lost.** R05 is still card-stage with no locked algorithm. A brief that scopes reconciliation to, say, nonterminal rows older than some threshold would technically miss a hand orphaned microseconds after registration. R05's verification recipe must explicitly cover *kill immediately after dispatch returns Ok, before any instrumentation write*. The family-write branch this card widened now closes itself, via `cancel_dispatched_hand`, without needing R05; the `Event::Spawn` write-failure branch described above is the still-open instance, and R05's problem statement does not currently name it.

Remaining gap: none for this card.

