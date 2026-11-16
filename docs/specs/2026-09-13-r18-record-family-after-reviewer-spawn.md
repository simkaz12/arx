# R18 — Record reviewer family only after dispatch succeeds

Card: `docs/wayfinder/maps/arx-reliability/R18-record-family-after-reviewer-spawn.md`
Board: `docs/wayfinder/maps/arx-reliability/map.md`
Base: `main` @ `7805ee6`
Branch: `r18-record-family-after-reviewer-spawn`

## Problem

When a same-family reviewer is selected and its dispatch preparation then fails — a missing credential, a missing image — the family warning/event is written **before** `ledger.dispatch` succeeds. The transcript and the UI end up describing a reviewer invocation that never happened.

This is instrumentation correctness. It is lower priority than work preservation, and the card says so; do not inflate it.

## Deliverable

Emit the calculated family event only after a successful dispatch, ordered before the spawn event.

## Scope — owned paths

Only these files may change:

- `crates/arx/src/main.rs`
- `docs/wayfinder/maps/arx-reliability/R18-record-family-after-reviewer-spawn.md` (frontmatter and Handoff only)
- this brief

Anything else is out of scope. STOP and report rather than widening.

## Concurrent work — do not collide

PR #30 (`r15-keep-file-listing-in-workspace`) is open and unmerged and touches this same file, at `@@ -1555` (`list_files_call`) and `@@ -5091` (its tests). **Your seam is around `main.rs:787`** — far from both. Stay there, and add your tests near the dispatch/ledger tests rather than near the file-tool tests at ~5091, so the two PRs merge cleanly.

## Orientation

- `crates/arx/src/main.rs:787` — the card's named seam, where the family warning/event is written
- find `ledger.dispatch` and establish the exact current ordering: family event, then dispatch, then spawn event. Quote it in your verdict before changing it.

## Order of work

1. Write the failing regression first: make same-family reviewer preparation fail, and show the family event is emitted anyway against current code. Record that output.
2. Then move the emission after successful dispatch.
3. Re-run and show neither event emitted on the failure path.

## Required cases

- A same-family reviewer whose preparation fails emits **neither** a family collision event **nor** a spawn event.
- A successful same-family dispatch records exactly **one** family event, ordered **before** the spawn event.
- A successful different-family dispatch emits no collision warning at all.

Assert ordering explicitly, not just presence — "before the spawn event" is part of the contract.

## Tripwire — test placement

`crates/arx` has no `lib.rs`. Per the repo's CLAUDE.md, tests for `main.rs` belong in `main.rs`'s own test module. Note that the dispatch note for a sibling card cited a stale line number for that module; **find the module yourself** and place your tests beside the existing dispatch/ledger tests.

Count the test total before and after. If the delta is not exactly what you added, STOP and report — do not bend a locked count to fit.

## Verify

```
cargo test -p arx --bin arx family
cargo test -p arx --bin arx
```

Do NOT use `cargo test --workspace` or the full `cargo test -p arx` integration suite as your evidence. It is flaky from Docker-backed container/timing tests unrelated to this diff. Do NOT `docker rmi` any stub image.

## Constraints

- Do NOT commit, switch branches, or run any git write. Work only inside `~/Projects/arx-wt/r18-record-family-after-reviewer-spawn`.
- No `git stash` or `git checkout` — two agents breached that tonight.
- Update `prompts/main.md` only if main's observable experience changes. This alters what main *sees in its own transcript*, so think about it properly and justify your answer either way rather than defaulting to "no".

## Non-goals

R10's bounded reads and R15's listing fix also live in `main.rs` and are separate cards. No other audit findings.
