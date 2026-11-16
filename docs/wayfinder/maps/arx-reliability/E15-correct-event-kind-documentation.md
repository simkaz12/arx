---
id: E15
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P3
next: owner-review
brief: docs/specs/2026-09-13-e15-correct-event-kind-documentation.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
pr: https://github.com/simkaz12/arx/pull/27
verified-revision: 74ef6db3a6c7daf3883b914526405f2b70562928
---

# E15 — Correct the live event-kind documentation

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L36)

## Existing point

The events.rs module documentation omits miss from its enumerated live event kinds.

A limited source check during board preparation still found this point. Confirm the chosen base before editing.

## Deliverable and next flow step

Next activity: to-spec.

Make the enumeration match the supported kinds. Keep this a documentation-only change if current source still confirms the mismatch.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [ ] The documented kind names match the current event definitions.
- [ ] No runtime behavior is changed for this documentation correction.
- [ ] The resolution is linked with evidence. If no change is needed, the original point is explicitly refuted or shown already resolved.
- [ ] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: events docs. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

The [brief](../../../specs/2026-09-13-e15-correct-event-kind-documentation.md) ran on `e15-correct-event-kind-documentation` off `main` at `7805ee6`. `Event::kind` (`events.rs:153-173`) has 21 arms and `KINDS` holds 21 entries in declaration order; both checked against the current source. The doc's "twenty-one" was already correct — untouched.

Git history located the defect precisely: `miss` had a live emitter and sat in the module doc's "own cards" list right after L3-01 landed (commit `342a42e`, `checkpoint`, `miss`, `schema`, `family` and `drain`), then L3-02 (commit `2446c34`) rewrote the sentence to add the `reset`/`floor`/`orientation` clause and dropped `miss` from the list in the process. Tracing `Event::Miss`'s live emitter confirmed the same "own cards" placement is still correct rather than the neighboring `admission`/`pull`/`echo` clause: `miss`'s size is computed once, in `native_hand.rs`'s `miss_for`/`finish` from a `ProviderError::ContextWindowExceeded`, carried through `Outcome::Delivered` into the hand's closure in `container_hand.rs` (`Delivery::Verdict { miss, .. }`), and surfaced by `delivery_miss`/`log_event` in `main.rs`'s delivery-pump loop — one card's worth of plumbing, not the three-site admission-module split that earns the `admission`/`pull`/`echo` clause its own callout. `miss` is restored between `checkpoint` and `schema`, matching both the L3-01 wording and `KINDS`' declaration order (`checkpoint`, `reset`, `miss`, with `reset` excluded because it already has its own clause).

Diff is comment lines only in `events.rs` (verified by `git diff`) plus this card and the brief. `cargo test -p arx --test events` (3 passed) and `cargo build -p arx` both ran clean. No regression test was added: a comment cannot be regression-tested. Codex gate not run — a comment-only diff carries no behaviour-change axis. Remaining before Done: independent reviewer pass and flow verify.

