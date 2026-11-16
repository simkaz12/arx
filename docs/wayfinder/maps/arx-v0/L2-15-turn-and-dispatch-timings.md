---
id: L2-15
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-14
ruled-by: owner preference 2026-09-05 (`e9429012` — efficacy is a priority; the dispatch-to-delivery cost per hand body is to be measured), `6d50785c` (the event writer lives in the transcript log; the ledger stays the operational record), `36a52426` (one writer on the transcript log)
---

# L2-15 — turn and dispatch timings in the log

## Deliverable

Two numbers in main's transcript log, both additive: how long a main turn took from prompt sent to turn end, on the `usage` row; and how long a hand took from `dispatch` to the moment its body sent the delivery, on the `delivered` event, with the pump's lag to the landing beside it so operator idle time is never mistaken for hand time. `arx events` prints them. A fake hand's duration is the container floor with no model in it, so the container cost per body falls out of the same number.

## Done when

On the stub sidecar: a turn's `usage` row carries `elapsed_ms`; a fake dispatch's `delivered` event carries `elapsed_ms` and `lag_ms`, and `lag_ms` grows when the delivery waits for the next operator turn. On the owner's login: one `arx run` dispatching a fake hand, a claude-code hand and a DeepSeek native hand reports each body's `elapsed_ms` and main's per-turn `elapsed_ms`, quoted in the card.

## Ruled by

`e9429012`, `6d50785c`, `36a52426` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

`transcript.rs`: `Line` gained a trailing `elapsed_ms: Option<u64>`; `append_usage_timed(usage, elapsed_ms)` holds the write, `append_usage` delegates with `None`. The crate carries four `Line { … }` construction sites, not the five the brief's Ground truth counted — `append_turn_inner`, `append_system`, `append_usage_timed`, `append_file` — and each now sets the field (`None` on the first three, the parameter on the fourth).

`events.rs`: `Event::Delivered` gained `elapsed_ms`/`lag_ms` after `bytes`, both skip-if-`None`; `every_kind`'s literal sets both to `None`.

`ledger.rs`: `HandEntry` gained `started: Instant`, stamped in `dispatch` before either branch spawns a body thread. Every body kind now gets its sender through a fresh inner `mpsc::channel`, not the ledger's own — `dispatch` spawns one forwarder thread per hand that blocks on the inner receiver and re-sends `(Instant::now(), Delivery)` onto the ledger's channel, so `delivery_tx`/`delivery_rx`/`pending` all carry the stamped pair now. `Timing { elapsed_ms, lag_ms }` and `Timing::from_instants(started, stamp, now)` do the arithmetic; `try_next_timed`/`wait_next_timed` pull a stamped delivery and compute `Timing` against `Instant::now()`, `try_next`/`wait_next` call them and drop the timing. `kill`'s synthetic `Delivery::Killed` and the two inline tests that reached the channel or `handle_delivery` directly (`a_crash_reported_delivery_still_writes_the_killed_row`, `a_native_verdict_row_carries_occupancy_and_window`) took the matching tuple edit; both `HandEntry` literals inside `mod tests` gained `started: Instant::now()`. No test added, removed or renamed.

`main.rs`: `run_turn_cycle` takes an `Instant::now()` immediately before `adapter.run_turn(...)` and reads `.elapsed()` immediately after, landing on `append_usage_timed`. Both pump sites — the `pump_deliveries!` macro and the trailing drain — read `try_next_timed`/`wait_next_timed` and set `elapsed_ms`/`lag_ms` on their `Event::Delivered` literals. `event_fields`'s `Delivered` arm prints `elapsed_ms=`/`lag_ms=` after `outcome=` when present. Two new tests in `mod tests`: `timing_is_elapsed_to_the_send_and_lag_to_the_pull` (three `Instant`s built by adding `Duration`s, no sleep) and `a_delivered_line_prints_its_timings_after_outcome` (through `event_fields`, with and without the fields).

Scope 1's own build carries one warning beyond baseline: `try_next`/`wait_next` (and, folded into the existing `append_turn`/`append_gate` line, `append_usage`) are now reachable only from tests, since production calls exclusively the timed variants per the locked design — confirmed against a `7702fd1` worktree (16 warnings there, 17 here). `check-config` output is byte-identical to `main`'s. `cargo test -p arx`: unittests 200 (198 + 2), every other binary at baseline.

Stub sidecar (`tests/fixtures/stub-sidecar/claude-main.mjs`): `stubDelayMs`/`sleep`/`delayBeforeReply` read `ARX_STUB_DELAY_MS` and, when set, await before the first `delta` of every one of the nine reply paths (`hang`, `hand:`, the auto-steer `[hand … stop]` trigger, both checkpoint prompts, `tool:`, `store:`, `file:`, the plain fallback); `die` writes no delta and is untouched.

`tests/claude_main.rs` gained two tests. `a_turn_usage_row_carries_elapsed_ms`: `ARX_STUB_DELAY_MS=200` on one plain turn puts `elapsed_ms` at 200 or above and under 5000 on the `usage` row; a session without the variable still writes a `usage` row whose `elapsed_ms` parses. `a_fake_dispatch_delivered_event_carries_elapsed_and_lag`: dispatches `fake`, then writes the next operator line from the test only after a real sleep, and checks the `delivered` row's `lag_ms` grew because the delivery sat waiting on the operator. The brief's own numbers (500 ms sleep, `lag_ms >= 400`) assumed a container floor under 500 ms; measured live under `cargo test`'s full parallelism the fake hand's own `elapsed_ms` ran 780–1100 ms, so the sleep is 4000 ms and the floor is `lag_ms >= 1500` — confirmed stable across repeated full-suite runs. `tests/common/mod.rs` took no edit; `run_session`, `fresh_run_fixture` and `cargo_bin` already covered both tests.

Scope 2's binding run: `cargo test -p arx` — unittests 200, `claude_main` 72 (70 + 2), every other binary at baseline. `cargo test --workspace` — 414 passed, 1 ignored. One flake surfaced during a `--no-fail-fast`-style sweep of repeated full-workspace runs, on both this tree and an unmodified `7702fd1` worktree checked for comparison: a transcript or ember unit test failing only under full parallel load, never alone — the documented "transcript log is locked by another writer" class, pre-existing and unrelated to this diff. Alone, every binary passed at the counts above.

Live, on the owner's login: a scratch workspace (`git init` plus an empty commit, `sidecar` pointed at the repo's own `sidecar/` by absolute path, `prompts/main.md` and `agents/` copied, `store/arx` created) ran one `arx run` with the DeepSeek key loaded and the host's Claude Code credential picked up by `container_hand.rs` as L2-08 arranged. Four operator turns — dispatch `fake` (with the note that `fake` is a valid built-in body though absent from the roster), dispatch `hand-ex`, dispatch `hand-ds`, then `Reply with the word done.` — plus main's own follow-up turns inspecting `h1`'s trace before trusting it. `threads/main.jsonl`'s `usage` rows carried `elapsed_ms` of 7392, 13751, 3571, 3839, 2107, 3960, 1543 across the session's seven main turns. The three `delivered` events: `h1` (fake) `elapsed_ms=879 lag_ms=2725`; `h2` (hand-ex) `elapsed_ms=3182 lag_ms=2259`; `h3` (hand-ds) `elapsed_ms=3043 lag_ms=1026`. `arx events` printed all three under `## delivered (3)` with `elapsed_ms=`/`lag_ms=` after `outcome=verdict`. The fake hand's 879 ms is the container floor with no model in it; hand-ex's and hand-ds's larger numbers carry a real model turn on top of their own container or process start. Scratch workspace, containers and `/tmp/arx-hands` swept clean after.

Codex gate: `gate_run: no`. The provider's usage limit is in force until 2026-09-07 08:38; one attempt was made and hit it.
