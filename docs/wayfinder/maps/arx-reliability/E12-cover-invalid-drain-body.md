---
id: E12
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P3
next: owner-review
brief: docs/specs/2026-09-13-e12-cover-invalid-drain-body.md
verification: runner PARTIAL (defect discovery, nothing failed); independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
pr: https://github.com/simkaz12/arx/pull/33
verified-revision: d49d59cbac5e96f84ec51bdf0d7a38f5dbbd2236
---

# E12 — Persist coverage for invalid drain definitions

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L30)

## Existing point

Refusal of agents/drain.md with a bad body was checked manually but lacked a persisted regression.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: focused-trace.

Confirm the current definition validation and add a meaningful configuration/dispatch refusal check where the gap remains.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [x] An invalid drain body is refused at the intended interface with a useful diagnostic. *(Useful for `steps`/`window`; see the Handoff for `nudge`/`checkpoint`, where the diagnostic names the wrong file.)*
- [x] A valid drain definition still loads and dispatches through the intended role — the dispatch-refusal test uses the unmodified `tests/fixtures/run/` workspace, which already wires drain up and loads cleanly.
- [x] The resolution is linked with evidence. The original point was revalidated as still true: no persisted regression existed for any of the five cases.
- [x] Any implementation has a linked brief, runner PARTIAL, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: config, drain. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

Delivered and awaiting owner review. Runner PARTIAL, independent reviewer PASS with an empty `must_fix`, verify PASS. PARTIAL is a narrative label, not a shortfall: a mid-run discovery corrected the brief, the brief was amended, and against the amended scope nothing was withheld. Both later gates ruled it ready to land.

`crates/arx/tests/check_config.rs` gained eight regressions pinning `agents/drain.md` rejection (16 tests to 24): a missing frontmatter block, each of the four retired keys, a generic deserialize failure, and zero-valued `steps`/`window` proving the `NonPositiveSteps`/`NonPositiveWindow` role tag reads `"drain"` rather than `"main"`. `crates/arx/tests/claude_main.rs` gained one session-level test pinning the `dispatch: drain runs through the drain tool` refusal, appended with zero deletions. No production code changed, so the brief's test-only codex-gate waiver holds.

The tests were checked for falsifiability twice over. The runner temporarily flipped every new assertion to a wrong value, watched it fail, and restored. Both later gates then traced each assertion's code path independently and found no tautology — notably, the retired-key tests are not redundant with `#[serde(deny_unknown_fields)]`, because without the explicit `RETIRED_FRONTMATTER_KEYS` scan an unknown `body` key would still fail but fall through to the generic `FrontmatterParse` branch with a different message.

**Defect found, deliberately not fixed here, and wider than first described.** `check_numeric_fields` (`config.rs:458-478`) correctly rejects `nudge: 0` / `checkpoint: 0`, but `ConfigError::NonPositiveNudge` and `NonPositiveCheckpoint` are unit variants carrying no role, so their `Display` (`config.rs:297-298`) prints the literal `"main.nudge must be positive"` / `"main.checkpoint must be positive"` whatever file actually offended.

This is not drain-specific. `check_role` is invoked at `config.rs:865` (runner), `:883` (reviewer), `:902` (every hand, by name in a loop) and `:938` (drain), all routing through the same `check_numeric_fields` — so the message misattributes for **every non-main role**. An operator debugging a rejected role file will go and edit `main` first, because the message says to, at exactly the moment they have least ability to tell the message is lying.

The blast radius is bounded, and that was confirmed rather than assumed. Every other `ConfigError` variant either carries no role concept or already threads the right identifier. `NonPositiveReserve`, `ReserveNotBelowWindow` and `MainWindowTooHigh` also hardcode `"main"`, but legitimately and by construction: they are produced only by `check_main`, which is called exactly once, at `config.rs:777`, against `startup.main` — a table with no role-file analogue. This pair is the sole instance.

The fix is small — give both variants a `role: String` like their two siblings — but it is a `config.rs` production edit that would void this card's test-only waiver and mix a behaviour change into a coverage card. Recorded for its own card, with enough detail that it need not be re-derived.

Incidental finding, pre-existing on `main` and unrelated to this card: `ConfigError::MainRoleBodyMismatch` (`config.rs:200-204`, `Display` at `:288-291`) is defined but never constructed anywhere under `src/`. Dead code of the same shape as E19's unused `gate::append_rule`, and probably belongs with whatever ruling E19 receives.

Remaining gap: none for this card.

