# Preserve hand work when capture fails

## Problem / Solution

A failed diff or trace write can destroy a hand's only edits while main receives a trace handle. Return a typed capture outcome, retain the hand root and worktree when capture fails, and report the failure with recovery paths. Successful capture still permits teardown. A killed write hand's delivery must name its sealed applicable diff.

## Inherited agreement

### Observable behavior and examples

- If staging or collecting a write hand's diff fails, main receives a mechanical capture failure with the cause and retained root/worktree paths. The edits remain recoverable there.
- If writing the trace fails, the same retention and failure contract applies. Neither failure advertises a sealed trace or applicable diff, even if some capture files exist.
- Normal completion and explicit kill seal the required capture before deleting the worktree/root. Successful killed write-hand delivery names the diff that main can apply.
- Stopping or removing the container remains part of terminal cleanup. Retaining files after capture failure does not require a running container.

### Evidence

- [R02 card](../wayfinder/maps/arx-reliability/R02-preserve-failed-hand-captures.md) and [audit R02](../wayfinder/maps/arx-reliability/findings/project-review-2026-09-12.md#R02) establish the failure and accepted repair [read].
- [L2-07 contract](../wayfinder/maps/arx-v0/L2-07-container-runtime.md) requires capture before teardown and an applicable diff reachable from the trace [read].
- The inspected workspace is dirty on main at `3c92977302a28a8accec18b79c5434a74a82b299`. Existing roster/reviewer/Codex and operator changes are part of the audited implementation. That commit alone is not the implementation described here [ran: git status/log].

- Focused trace found the same discarded diff/sweep failures and unconditional teardown in the CLI path (`container_hand.rs:485-605`) and native path (`:794-893`). Killed delivery loses its diff in `ledger.rs` and `main.rs`; terminal and panes already accept a diff. Panes also need to recognize capture failure as a settled failure outcome [read].
- Scratch fault probes confirmed that moving a worktree's `.git` file makes staging fail, and a regular file at the trace-directory path makes directory creation fail [ran by hand-ex]. These probes establish injection methods; the complete delivery regressions have not run.

### Execution gate

The owner authorized R02 implementation on 2026-09-13 with 'continue', following the implementation handoff. That authorization persists through runner, reviewer, verification and opening its scoped PR. Before dispatch, establish a coherent main base containing the relevant existing changes, preserve unrelated work, record that revision here, and create branch `r02-preserve-failed-hand-captures` from it. Do not include the owner's unrelated changes in the R02 commit or silently substitute the older clean HEAD for the audited tree.

### Baseline validation, 2026-09-13

Local and fetched remote main both remain at `3c92977302a28a8accec18b79c5434a74a82b299`; no branch or open PR contains the existing runtime changes. A reviewable snapshot of the 72 pre-existing changed paths is at `/tmp/arx-r02-baseline-5tdq_nkz/baseline.patch`, with `paths.txt` beside it. It excludes the reliability board and R02 brief. Neither the index nor runtime files were changed during this check.

The full dirty baseline passed `cargo test --workspace --no-fail-fast` (755 passed, 2 ignored), config validation, and sidecar `npm test` (15 passed). The first workspace attempt failed `a_turn_usage_row_carries_elapsed_ms` at 198 ms against a 200 ms minimum; its exact rerun and the complete rerun passed. No timing assertion was changed. The Codex stub in the cached image matched the working script byte-for-byte.

Logs are `/tmp/arx-r02-baseline-workspace-2026-09-13.log`, `/tmp/arx-r02-baseline-usage-rerun-2026-09-13.log`, `/tmp/arx-r02-baseline-workspace-no-fail-fast-2026-09-13.log`, `/tmp/arx-r02-baseline-check-config-2026-09-13.log`, and `/tmp/arx-r02-baseline-sidecar-2026-09-13.log`. These prove the baseline only, not R02. The pre-existing changes still need a separately authorized commit before R02 can branch from a coherent main base.

## Decisions locked

- D1: Capture success and failure are typed outcomes. Missing optional fields cannot stand in for a failed capture.
- D2: Only a successful sealed capture permits removal of the hand root/worktree. On failure, delivery includes the cause and usable recovery paths.
- D3: Incomplete capture exposes no valid trace/diff handle. Partial files may remain for recovery but are not a deliverable.
- D4: A successful killed write hand names its sealed applicable diff in terminal delivery.

## Constraints

Keep the change within terminal hand capture and delivery. Preserve host-body behavior and terminal lifecycle semantics except where the failure contract requires a change. Update `prompts/main.md` with main's new observable behavior.

Use scratch repositories, stores and workspaces for every failure probe. Never use the live store. Do not add inline comments. Included source files must not gain inline test modules: integration tests include source with `#[path]`, so those tests register twice. Use integration test files or the existing test module in `main.rs`.

## Execution plan

### Scope 1: Preserve failed captures and deliver their outcome

**Files:** Modify `crates/arx/src/container_hand.rs`, `crates/arx/src/ledger.rs`, `crates/arx/src/main.rs`, `crates/arx/src/panes/state.rs`, `crates/arx/src/panes/ui.rs`, `crates/arx/src/panes/app.rs`, `crates/arx/tests/hands_in_containers.rs`, `crates/arx/tests/native_in_container.rs`, `CONTEXT.md`, and `prompts/main.md`. Add any focused pane-state assertion to the existing test module in `main.rs`.

**Build:** Both CLI and native terminal paths use the same capture success/failure contract. Main receives a mechanical failure with recovery paths on capture errors. Successful killed write hands expose their applicable diff.

**Steps:**

1. Add integration regressions named `capture_failure_retains_work_on_diff_error` and `capture_failure_retains_work_on_trace_write_error` in `hands_in_containers.rs`. Drive a fake write hand through the existing real process/container/ledger/main test seam. Wait for its spawn record and written probe file before injecting the fault, using bounded polling rather than a fixed delay.
2. For the diff failure, temporarily move the worktree's `.git` file before collection. For trace-write failure, create a regular file where the trace directory must be created. Inject these independently. Before the fix, run both and record their expected failure because the current path removes recovery files and advertises a trace.
3. Each regression asserts a mechanical capture failure reaches main, the terminal ledger outcome records failure, the diagnostic names the retained root/worktree, and the probe edits remain readable. Assert no sealed trace or applicable diff is advertised. Restore fault-injection metadata and remove only test-owned leftovers during cleanup, including failed assertions.
4. Replace discarded diff/sweep errors with a typed capture result shared by CLI and native finalization. A success contains the sealed trace metadata and optional applicable diff. A failure carries its cause and retained recovery paths. Only return success after all required capture writes complete. Handle output-copy/sweep errors through the same failure result. Route native `Outcome::TraceFailed` through final capture as well; its early return must not destroy the worktree/root before capture. An execution-trace error and a capture error are distinct: successful final capture may preserve partial execution evidence and the diff, while failed final capture must retain recovery files.
5. Gate root/worktree deletion on capture success. Preserve the files after any capture failure while completing the hand's terminal lifecycle and container shutdown. Do not leave a failed hand in flight forever or convert an incomplete capture into an ordinary successful verdict.
6. Carry the outcome through ledger persistence, main's injection text and terminal delivery. A kill remains terminal and recognizably killed even when its capture fails. Adapt existing Delivery constructions and matches within the owned files; preserve existing host-hand behavior. Add the successful killed write hand's diff to its durable row and main/terminal delivery.
7. Add `capture_failure_native_trace_write_retains_root` in `native_in_container.rs`. Run `hand-ds` against the existing `FakeScript::Stall`, wait for its spawn row/root, place a regular file at its trace-directory path, kill the hand and close stdin. Assert native finalization reports capture failure with the retained root, advertises no trace/diff, removes the container and settles without hanging. Make panes recognize capture failure as a settled failure: no success marker or running row. Cover that settlement through the existing pane-state interface in the main test module.
8. Strengthen `a_killed_write_hand_still_leaves_an_applicable_diff` to require the delivered and persisted diff path, then apply that named diff in the scratch repository. Keep the existing successful capture/apply and cleanup assertions.
9. Update the standing prompt and the Hand/Capture surface clauses in `CONTEXT.md` to explain the mechanical failure, recovery paths, and the absence of usable trace/diff handles after failure. Describe only implemented behavior.

**Verify:**

- `cargo test -p arx capture_failure -- --test-threads=1` fails for the stated reasons before the fix and passes afterward. The CLI diff/trace probes and native trace probe retain their recovery files and reach main without valid handles; pane settlement also passes.
- `cargo test -p arx --test hands_in_containers a_killed_write_hand_still_leaves_an_applicable_diff -- --exact --test-threads=1` passes and proves that the delivered diff applies after successful kill cleanup.
- `cargo test -p arx --test hands_in_containers --test native_in_container -- --test-threads=1` preserves normal capture, isolation and cleanup behavior in both callers.

## Out of scope

R01 symlink policy, orphan reconciliation, retry/recovery commands, project/thread identity, container hardening, unrelated dirty changes and other reliability cards. R02 preserves recovery files and explains their location; it does not implement their later automated recovery.

## Review handoff

Runner and independent reviewer receive this brief, R02 and the linked capture contract. Work only in the listed files; other workers and owner edits may exist. Do not revert their changes, switch branches or commit. If a new owned file is necessary, report the concrete caller gap to the coordinator before expanding the brief.

After focused checks, run `cargo test -p arx` and `cargo test --workspace`, plus `cargo run -p arx -- check-config ./arx.toml`. Record actual totals and failures without assuming the old locked counts. Docker must be available; if a stub script changes under an approved scope expansion, remove its cached test image before testing. Existing dirty fixture changes also require checking image freshness before relying on their results.

A runner PASS goes to a fresh independent spec-reviewer, then flow's verify skill. Verification must account for each R02 checkbox with executable evidence, including both finalization paths. A failure returns to the named local correction, missing fact or disputed decision.

After those gates pass under implementation authorization, update the card/map, commit only scoped changes, push normally and open a PR to main. Record the PR URL, verified revision and evidence; leave the PR open for owner review. Do not merge or delete its branch. Planning completion is not implementation verification or card completion.
