# R03 — Make interrupted drains recoverable

Card: `docs/wayfinder/maps/arx-reliability/R03-recover-interrupted-drains.md`
Board: `docs/wayfinder/maps/arx-reliability/map.md`
Base: `main` @ `7805ee6`
Branch: `r03-recover-interrupted-drains`

## Problem — already traced, do not re-derive

`apply_drain_plan` (`crates/arx-engine/src/engine/mod.rs:711-787`) writes each successor claim, then repairs the predecessor, then commits the whole batch once, then deletes the quarantine rows. A crash between the successor write and the commit leaves the successor `.md` on disk but untracked in git.

On retry two separate faults combine:

- The idempotency guard at `mod.rs:724-727` is `if claim_path.exists() { continue; }`. It fires because the successor file already exists, and `continue` skips the rest of the loop body — so the **predecessor repair at `mod.rs:751-760` never runs**, and that candidate contributes nothing to `touched`.
- `to_delete.push(plan.path.clone())` at `mod.rs:719` runs at the top of the loop, *before* that guard. The deletion loop at `mod.rs:768-773` is gated only on the batch-level commit call, not on any per-candidate outcome — so the quarantine row for the candidate whose repair was just skipped is deleted anyway.

Net effect: the predecessor is never marked superseded, and the quarantine evidence that would allow another retry is gone.

`git::commit` (`crates/arx-engine/src/engine/git.rs:49-82`) is two separate subprocess calls (`git add` then `git commit`), which is why the window exists at all.

## Deliverable

Gate quarantine-row deletion on a **verified per-candidate outcome** — successor written, predecessor repaired if the plan has one, and the batch committed — rather than on bare successor-file existence plus "the batch commit ran".

Make the idempotency check verify the predecessor repair too, not just the successor file's existence.

## Locked scope decision — read this before designing

Do **not** invent a durable journal, write-ahead log, or in-progress marker protocol. That is a larger design change and is explicitly out of scope for this card. The fix is to make the existing retry path correct: per-candidate outcome tracking, and deletion gated on it.

If you conclude the minimal fix genuinely cannot work without a new on-disk protocol, STOP and report that finding rather than building one.

## Scope — owned paths

Only these files may change:

- `crates/arx-engine/src/engine/mod.rs`
- `crates/arx-engine/src/engine/git.rs` — only if resumability of the add/commit pair demands it; justify in your verdict if you touch it
- `crates/arx-engine/tests/engine.rs`
- `docs/wayfinder/maps/arx-reliability/R03-recover-interrupted-drains.md` (frontmatter and Handoff only)
- this brief

Anything else is out of scope. STOP and report rather than widening.

## Concurrent work — do not collide

PR #28 (`r08-reject-duplicate-supersession-targets`) is open and unmerged. It added one hunk to this same file at `@@ -647,+18` inside `validate_drain_plan`. **Do not touch that function or that region.** Your work is in `apply_drain_plan` and below. Keeping clear of line ~647 is what lets both PRs merge cleanly.

## Order of work

1. Write the failing regression first. Simulate the partial state directly: pre-create the successor `.md` in the scope directory with its quarantine row still present, then call `drain()`. Against current code the predecessor must come back un-superseded *and* the quarantine row must be gone. Record that output.
2. Then implement the outcome gating.
3. Re-run and show the retry completing the repair, or preserving the quarantine row so a further retry can.

## Required cases

- A retry after a crash between successor write and commit completes the predecessor repair.
- If a retry cannot complete, the quarantine row survives so the work is not lost. Losing evidence is the defect; do not trade one loss for another.
- A genuine `git::commit` failure still returns `StoreError::Git` and leaves candidate files in place — this is an existing ruling, do not regress it.
- The happy path is unchanged: one commit, quarantine cleared, predecessor marked `Superseded`.

## Tests that must keep passing, not be gutted

- `drain_promotes_as_one_commit_and_clears_quarantine` — `mod.rs:937-984`
- `supersession_lineage_through_save_and_drain` — `tests/engine.rs:158-226`
- `drain_is_all_or_nothing` — `mod.rs:987-1030` (pre-write validation only; unaffected, but confirm)

## Verify

```
cargo test -p arx-engine
```

No Docker needed. Do NOT use `cargo test --workspace` as evidence — it is flaky from unrelated container/timing tests in `crates/arx`, a crate you are not touching.

## Constraints

- Scratch stores only. NEVER `~/.local/share/arx/store`.
- Do NOT commit, switch branches, or run any git write beyond reading. Work only inside `~/Projects/arx-wt/r03-recover-interrupted-drains`.
- No `git stash` or `git checkout` — two agents breached that tonight.

## Non-goals

R12's shared-engine locking and cache coordination. R08's duplicate-target rejection. R11 and R13. Do not drift into them even though they share this file.
