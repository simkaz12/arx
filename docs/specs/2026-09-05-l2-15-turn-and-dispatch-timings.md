# L2-15 — turn and dispatch timings in the log

Card: `docs/wayfinder/maps/arx-v0/L2-15-turn-and-dispatch-timings.md`. Ruled by the owner's stated priority of 2026-09-05 (`e9429012`: efficacy; the dispatch-to-delivery cost per hand body is to be measured), `6d50785c` (the event writer lives in the transcript log; the ledger stays the operational record), `36a52426` (one writer on the transcript log; every write goes through `TranscriptLog`), `adec3734` (deliveries land at main's step boundaries). Branch `l2-15-turn-and-dispatch-timings`, cut from `main` at `7702fd1` (L2-14 merged; the baseline is under Ground truth).

Written 2026-09-05 with the owner present; the ruling was the grill. Calls made alone are under "Chosen without a grill".

## Problem / Solution

Nothing in arx says how long anything takes. A main turn's cost is known in tokens and not in seconds; a hand's cost is a container start, a brief, a model run and a sweep, and none of it is a number. The owner asked for the dispatch-to-delivery cost per hand body.

This card adds two additive numbers to main's transcript log. The `usage` row that follows a main turn gains `elapsed_ms`, measured around the blocking sidecar call from the prompt going out to the turn ending. The `delivered` event gains `elapsed_ms`, from the `dispatch` call to the moment the hand's body sent its delivery, and `lag_ms`, from that moment to the landing at main's step boundary, so a hand that finished while the operator was away is never counted as slow. `arx events` prints both. A fake hand's `elapsed_ms` is the container floor with no model in it.

## Decisions locked

- **The turn number.** In `run_turn_cycle`, `Instant::now()` immediately before `adapter.run_turn(...)` and `.elapsed()` immediately after; the value goes on the `usage` row as `elapsed_ms: <u64>` (milliseconds, rounded down). `Line` gains `elapsed_ms: Option<u64>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`; `TranscriptLog::append_usage_timed(usage, elapsed_ms)` is added and `append_usage(usage)` delegates to it with `None`, so every existing row and caller is byte-identical. The row: `{"kind":"usage","usage":{…},"elapsed_ms":N}`. The checkpoint ritual's turns take the same measurement through the same code path.
- **The hand numbers.** `Event::Delivered` gains `elapsed_ms: Option<u64>` and `lag_ms: Option<u64>`, both `#[serde(default, skip_serializing_if = "Option::is_none")]`, declared after `bytes`. `elapsed_ms` is the hand body's send instant minus the dispatch instant; `lag_ms` is the landing instant (the pump appending the event) minus the send instant. Both are set on every `delivered` event the pump writes, whatever the outcome.
- **Where the instants come from.** The ledger's `HandEntry` gains `started: Instant`, set in `dispatch` before the body thread starts. The send instant is stamped by the ledger, not by any body: for each hand, `dispatch` creates an inner channel, hands the body its sender as today, and spawns a forwarder thread that blocks on the inner receiver and forwards each delivery to the ledger's channel as `(Instant::now(), Delivery)`. No body file changes and no `Delivery` variant changes. `try_next` and `wait_next` keep their signatures and return the `Delivery` alone; `try_next_timed` and `wait_next_timed` are added, returning `(Delivery, Timing)` with `Timing { elapsed_ms, lag_ms }` computed at the pull from `started`, the stamp and `Instant::now()`. Main's two pump sites use the timed variants.
- **`arx events`** prints `elapsed_ms=` and `lag_ms=` on a `delivered` line when present, after `outcome=`; nothing else in the table or headings changes.
- **The stub sidecar** gains `ARX_STUB_DELAY_MS`: when set, every reply path sleeps that many milliseconds before its first `delta`. Nothing else in the stub changes.
- **What stays byte-identical.** `KINDS`; every row and event without the new fields; `Delivery`; every body file (`container_hand.rs`, `claude_hand.rs`, `codex_hand.rs`, `native_hand.rs`, `fake_hand.sh`); `run_turn`; the `gate` row's bytes.
- **Codex gate:** behaviour-changing, so the gate runs at end of work; the provider limit holds until 2026-09-07 08:38, so one attempt, `gate_run: no` with the reason, and the reviewer gates on brief conformance.

## Chosen without a grill

- **The send instant is stamped by a ledger forwarder thread, not by the bodies.** A stamp inside each body means four files and a `Delivery` change; a forwarder wakes on the send within microseconds and touches only `ledger.rs`. Rejected: a timestamp field on `Delivery`; a newtype sender; stamping at the pump (the pump runs at step boundaries, so the number would include operator idle time).
- **The untimed pair stays and its warning is carried.** `try_next`/`wait_next` keep their signatures for the ledger's own tests and for any caller that does not want a timing; in a non-test build they are unused, and the crate carries the warning as it carries sixteen others rather than start decorating with `#[allow(dead_code)]`. Rejected: removing the pair (a signature change in a card that promised none); `#[cfg(test)]` on them (a third shape for the same function); the allow attribute.
- **Two numbers on the delivery, not one.** `elapsed_ms` alone would be a lie whenever the operator was away; `lag_ms` beside it says so. Rejected: spawn-to-landing as one number.
- **On the `usage` row, not the turn row.** Tokens and time are the same question, what the turn cost. Rejected: a field on main's reply turn row; a new event kind (`KINDS` is locked).
- **Milliseconds as integers.** Rejected: floats; seconds.
- **The container-start split (docker run to first stdout line) is not a field.** A fake hand's `elapsed_ms` is that floor to within a sweep; if a per-body split is ever needed it is a trace file, not a delivery field. Rejected: instrumenting the body threads now.

## Ground truth

Verified 2026-09-05 by a read-only trace at `4d4362a` (✅ read, quoted), re-pinned after L2-14 merged at `7702fd1`. L2-14 restructured `run_turn_cycle` in `main.rs` (the gate and its error slots now live in `RefCell`s, `gate_call` is one function with four callers), so the `main.rs` line numbers below are stale; the symbols are not. Re-anchor by symbol.

- ✅ `crates/arx/src/transcript.rs`: `Line { kind, turn, text, role, usage, tool, verdict, boundary }` :19-36, no `deny_unknown_fields`, no timestamp on any row; `append_usage` :242-254; `append_event` :279-283; `write_line`/`write_raw` :285-300 the single append path; the inline `#[cfg(test)] mod tests` :439-651 (16 tests) is also compiled into `tests/transcript_log.rs` through its `#[path]` include (12 + 16 = 28); `append_turn_omits_role`, `append_turn_as_serializes_role`, `append_gate_writes_tool_and_verdict` compare exact row strings and survive an additive `None` field; every `Line { … }` literal in `transcript.rs` (five, one per `append_*`) gains `elapsed_ms: None`.
- ✅ `crates/arx/src/events.rs`: `Event` :13-90, `#[serde(tag = "kind", rename_all = "snake_case")]`, `Delivered { hand, outcome, trace: Option<String>, bytes: Option<u64> }` with the optionals `#[serde(default, skip_serializing_if = "Option::is_none")]`; `KINDS` :92-110; `every_kind` :139-158 constructs every variant by name (the `Delivered` literal :153-158 gains the two fields); `tests/events.rs::gate_row_bytes_are_unchanged` byte-pins the `Gate` row only; `every_kind_writes_and_arx_events_counts_it` is structural.
- ✅ `crates/arx/src/claude_code.rs`: `run_turn` :242-343 writes the prompt op :253-254 and returns on `turn_end` :268; `TurnOutcome { text, usage }` :47-51, untouched.
- ✅ `crates/arx/src/main.rs`: `run_turn_cycle` calls `adapter.run_turn(...)` at :952 (one wrap point for operator turns, `arx`-role deliveries and the checkpoint ritual's two prompts); `append_usage` at :1246 after `append_turn_as("main", …)` :1236; `on_hand_call`'s `dispatch` arm :467-513 calling `ledger.dispatch` :495 and writing `Event::Spawn` :497-505; the `pump_deliveries!` macro :1688-1762 with `Event::Delivered` :1741-1746 after `ledger.try_next()`, and the trailing drain :1793-1867 with `Event::Delivered` :1848-1853 after `ledger.wait_next()`; `event_fields` :1902-1962 with the `Delivered` arm :1911 printing `hand=`/`outcome=` only; `cmd_events` :1964-2003; `mod tests` at the tail.
- ✅ `crates/arx/src/ledger.rs`: `Delivery { Verdict {…}, Stop {…}, Killed {…} }` :17-37, no timestamp; `HandEntry { state, brief, pending_question, control_tx, terminal_delivered, kind, agent, model, window }` :255-265; the spawn row :337-339 (container) and :380 (codex); `BodyKind::in_container()` :59-61 is `Fake | ClaudeCode | Native`; codex runs on the host through `codex_hand::spawn_codex_hand` :388-390; the body thread sends on `delivery_tx`; `try_next` (non-blocking) and `wait_next` (blocking) are the pump's two reads.
- ✅ Stub sidecar `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`: no sleep anywhere; the only env knob is `ARX_STUB_TOKENS_PER_TURN` :28; every reply path emits `delta`, `turn`, `usage`, `turn_end` in one tick.
- ✅ Baseline at `7702fd1`, run cold by the L2-14 reviewer: `cargo test --workspace` 410 passed, 1 ignored. `cargo test -p arx` 366: unittests 198, check_config 10, checkpoint 6, claude_in_container 4, claude_main 70, container_runtime 9, events 3, gate 4, hands_in_containers 5, layer_close 12, native_in_container 7, state_object 7, store 3, transcript_log 28; arx-engine 44. Count first; a difference is a stop, not an adjustment.
- ✅ Hazards: no `lib.rs`; `#[path]` includes double-register inline tests (`tests/transcript_log.rs` includes `transcript.rs`, `tests/events.rs` includes `events.rs`; new unit tests go in `main.rs`'s `mod tests`); `store/` and `pid.txt` tracked live state; `KINDS` seventeen and locked; a stray container from a concurrent run and a rare transcript-lock flake are known.
- Trap: `Event::Delivered` literals are three (`events.rs` `every_kind`, the two pump sites); each gains the two fields, none loses one.
- Trap: the forwarder must be spawned for every body kind, codex included, since the pump reads one channel.

## Execution plan

### Scope 1 — the two numbers

- **Files:** Modify `crates/arx/src/transcript.rs`, `crates/arx/src/events.rs`, `crates/arx/src/ledger.rs`, `crates/arx/src/main.rs`. Do not modify `claude_code.rs`, `container_hand.rs`, `claude_hand.rs`, `codex_hand.rs`, `native_hand.rs`, `fake_hand.sh`, `gate.rs`, `config.rs`, any test file (Scope 2 owns tests), `arx.toml`.
- **Build:** `elapsed_ms` on the usage row; `elapsed_ms` and `lag_ms` on the delivered event; both printed by `arx events`.
- **Steps:**
  1. `transcript.rs`: `elapsed_ms` on `Line`; `append_usage_timed`; `append_usage` delegating; the five literals.
  2. `events.rs`: the two fields on `Delivered`; the `every_kind` literal.
  3. `ledger.rs`: `started` on `HandEntry`; the inner channel and forwarder thread in `dispatch`; `Timing`; `try_next_timed` and `wait_next_timed`, with `try_next`/`wait_next` delegating and dropping the timing.
  4. `main.rs`: the `Instant` around `run_turn` and `append_usage_timed`; the two pump sites on the timed variants writing the fields; `event_fields`'s `Delivered` arm; unit tests in `mod tests`: `timing_is_elapsed_to_the_send_and_lag_to_the_pull` (pure arithmetic over three instants) and `a_delivered_line_prints_its_timings_after_outcome` (through `event_fields`). Two tests.
- **Verify:** `cargo build -p arx` with no new warnings except the one this design creates and accepts: `try_next` and `wait_next` become reachable only from `ledger.rs`'s own tests once production uses the timed pair, and the crate's convention is to carry such a warning rather than add an `#[allow(dead_code)]` it has never used (sixteen exist today for the same reason); `cargo test -p arx` → unittests 200 (198 + 2), every other binary at baseline; `cargo run -p arx -- check-config ./arx.toml` byte-identical to `main`'s output; `cargo run -p arx -- events` over a log with a `delivered` row carrying the fields prints `elapsed_ms=`/`lag_ms=` on that line.

### Scope 2 — the done-when as data, the live numbers, the card

- **Files:** Modify `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs` (`ARX_STUB_DELAY_MS` only), `crates/arx/tests/claude_main.rs` (two added tests; every existing test unchanged), `docs/wayfinder/maps/arx-v0/L2-15-turn-and-dispatch-timings.md`. Do not modify `map.md`, `tests/events.rs`, `tests/transcript_log.rs`, `tests/layer_close.rs`, `tests/scripts/l1-close.json`, `tests/gate.rs`, `tests/hands_in_containers.rs`; `tests/common/mod.rs` additive only if a helper is needed (say so).
- **Build:** the done-when as tests, the live numbers, the card closed.
- **Steps:**
  1. Stub: `ARX_STUB_DELAY_MS`.
  2. `tests/claude_main.rs`: `a_turn_usage_row_carries_elapsed_ms` (with `ARX_STUB_DELAY_MS=200` in the child's environment, one plain turn; the `usage` row's `elapsed_ms` is at least 200 and below 5000; a session without the variable still writes a `usage` row that parses and carries `elapsed_ms` at least 0); `a_fake_dispatch_delivered_event_carries_elapsed_and_lag` (dispatch the fake hand with `hand:dispatch:fake:…`, then write the next operator line only after sleeping 500 ms in the test; the `delivered` event carries both fields; `lag_ms` is at least 400, since the fake hand finished while the test slept; `elapsed_ms` is the hand's own time; `arx events` on that log prints both).
  3. Live, on the owner's login, in a scratch workspace as L2-14's live step set it up, with the DeepSeek key loaded (`set -a; . ~/.config/arx/env; set +a`; empty → the DeepSeek dispatch is reported blocked, the other two still run) and the Claude credential available to the hand image as L2-08 arranged: one `arx run` with three operator turns, each dispatching one hand with the brief `Reply with the single word ok.` (`fake`, `hand-ex`, `hand-ds`; the operator line for the first says `the agent name fake is a valid built-in body even though it is not in your roster`, since L2-14's live check saw main refuse it otherwise) and a fourth turn `Reply with the word done.` after the deliveries; from `threads/main.jsonl` quote every `usage` row's `elapsed_ms` and every `delivered` event's `hand`, `elapsed_ms` and `lag_ms`, and `arx events` output for the `delivered` kind. Report the numbers per body; do not interpret beyond the fake hand being the container floor. If the login is unavailable, STOP the step and report it blocked. Sweep everything after.
  4. Close the card: `status: closed`, `assignee: spec-runner`, a `## Shipped` section in the voice of the last cards carrying the live numbers.
- **Verify:** `cargo test -p arx` → unittests 200, claude_main 72 (70 + 2), every other binary at baseline; `cargo test --workspace` 414 passed, 1 ignored; `docker ps -a --filter label=arx.hand` empty; `/tmp/arx-hands` empty; `git status --short` clean under `store/` and for `pid.txt`.

## Out of scope

- The container-start split inside a body; per-step timings inside a native hand's loop.
- A timing on `Delivery::Stop`'s wall wait beyond what `elapsed_ms`/`lag_ms` already say.
- Any change to the ledger file's rows, `Delivery`, the bodies, `KINDS`, or the gate.
- Acting on the numbers (a warm container pool, a smaller image): a later card, once the numbers exist.

Done when Scope 2's Verify holds, the reviewer's must_fix is empty, and the map's frontier entry is written by main.

## Guardrails

Never commit, never `git add`, never stash, never switch branches, never rebase; read-only git only (`git status`, `git diff`, `git log`, `git show <ref>:<path>`). Never write under `store/`, to `pid.txt`, or to `~/.claude`; never print or quote a credential. No inline comments unless one already exists at the site. Clean up every container, hand root and scratch workspace you start.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-05-l2-15-turn-and-dispatch-timings.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
