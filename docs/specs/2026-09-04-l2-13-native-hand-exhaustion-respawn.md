# L2-13 — native hand exhaustion and respawn

Card: `docs/wayfinder/maps/arx-v0/L2-13-native-hand-exhaustion-respawn.md`. Ruled by `5f17e29e` (an arx-native hand's ContextWindowExceeded takes the same continuation as main's, on the hand's own window — rewind-to-fit, the hand authors its own state object, respawns, carries on under the same brief; the miss event goes in the hand's trace; a hand that resets at all is one reading for the "shortening hand scopes" fog item), D15 (`e2b0ceb2` — rewind-to-fit before the ritual is mechanical cut-to-archive, the discarded tail survives on disk under a handle; `8b9f41ae` — if the vendor still rejects after rewind-to-fit, rewind by the parsed overshoot plus a reserve and resend, no retry counter because every iteration strictly shrinks, a fixed step when the vendor gives no numbers, the floor is the system prompt plus the state-object prompt), D14 (`3a5aefb6` — a 400 on the wire is a reserve miss, one instrumentation event carries the size), D07 (`7dae481c` — append and rewind, no splice), plus L2-10a's `Window::rewind`, L2-10b's `ContextWindowExceeded { overshoot }` and `Delivery::Verdict.miss`, and L2-11's `run_brief` on the container path. Branch `l2-13-native-hand-exhaustion-respawn`, cut from `main` at `877f085` (L2-10c merged; the baseline is under Ground truth).

Written 2026-09-04 without a grill, on the owner's overnight rule. Every call main made alone is listed under "Chosen without a grill" so the morning read starts there.

## Problem / Solution

After L2-10b a native hand that overflows its window ends: the miss is classified, the overshoot parsed, the verdict names it, the event lands, and the brief is lost. `Window::rewind` exists with no caller. The ruling says the hand continues the way main will: rewind to fit, author its own state object, respawn on a fresh window, carry on under the same brief.

This card is that continuation, inside `run_brief`. On `ContextWindowExceeded` the loop cuts the window's tail by the overshoot plus a reserve, measured in the same bytes-per-token estimate the gauge has to live with until L2-10b's calibration event says otherwise, and archives the cut messages under the hand's trace. It then asks the model, on the rewound window, for a compact state object — position, target, drift, next move, in prose — and takes the reply as the object. It respawns: a fresh window of the brief plus the object, and the step loop resumes with the same step budget left. A second miss inside the authoring call cuts by the newly parsed overshoot plus the reserve and asks again, down to the floor of the brief and the state-object prompt alone; a miss on the very first step, with nothing to cut, ends the brief as L2-10b already does. Every stage is a row in the hand's transcript — `miss`, `rewind`, `state_object`, `respawn` — and the miss still reaches main's log through the delivery as today. Respawns are bounded per brief so a hand that cannot fit its own object stops loud rather than loops.

## Decisions locked

- The estimate: `provider::estimate_tokens(messages: &[Message]) -> u64` = total UTF-8 bytes of every text, tool-call argument JSON and tool-result content divided by 4, rounded up. `RESERVE_TOKENS = 2048`. Both live in `provider.rs` beside `Window`; the reserve is a constant, not config, until L2-10b's calibration event has numbers (`25664aa5`'s reasoning for the delta applies).
- Rewind-to-fit: given `overshoot: Option<Overshoot>`, the target cut is `requested - maximum + RESERVE_TOKENS` tokens when numbers are present, else `max(estimate / 4, RESERVE_TOKENS)`; `Window::rewind` is applied from the tail one message at a time (never cutting the first message, the brief) until the cumulative estimate of the cut messages reaches the target; the cut messages are written to `<trace>/rewind-<n>.jsonl` (one JSON row per message, `n` the respawn ordinal) and the transcript gets `{"kind":"rewind","tokens_cut":<estimate>,"messages_cut":<count>,"archive":"rewind-<n>.jsonl"}`.
- The state object for a hand: after the rewind, one `complete` call with the rewound window plus the user message `[arx] your window overflowed and was rewound. Write a compact state object for yourself: what you have done, what you learned, what is decided, what is open, and your next concrete move under the brief. Prose, no tool calls.` The reply's text is the object; a `ToolUse` termination on this call is ignored (its calls are not executed) and the text kept. Transcript row `{"kind":"state_object","text":…}`. This is the hand's own object, in the hand's trace; it is not L2-06a's checked file and carries no header.
- Respawn: `window = Window::new(brief)` then `window.append(Message { role: User, content: [Text("[arx] state object from before the reset:\n<object>")] })`; transcript row `{"kind":"respawn","ordinal":<n>}`; the step loop continues with the remaining `steps` budget (steps already taken count). `MAX_RESPAWNS = 2` per brief; a third miss ends the brief with the verdict `native hand failed — context window exceeded after 2 respawns` and `miss` set as L2-10b does.
- The ladder inside authoring (`8b9f41ae`): if the state-object call itself returns `ContextWindowExceeded`, rewind again by that overshoot plus the reserve and retry the authoring call; when the window is down to the brief alone and the vendor still rejects, end the brief with the L2-10b verdict and miss (the floor). No retry counter: each iteration strictly shrinks the window.
- A miss on the first step of a fresh window (nothing to cut but the brief) ends the brief as L2-10b does — no rewind, no respawn.
- Every miss, first or later, still surfaces to main: `Delivery::Verdict.miss` is set only when the brief ends on a miss; a miss that was survived by a respawn is recorded in the hand's transcript only, and the verdict's `text` gains no marker. The `miss` event in main's log therefore means "the hand died of it"; the survived ones are the hand's business (`5f17e29e`: the miss event goes in the hand's trace).
- The kill flag is honoured between every stage (after the rewind, after the authoring call, after the respawn).
- `native_in_container.rs` and the L2-11 done-check keep every test and assertion unchanged (a miss without respawn is not exercised there).
- Codex gate: behaviour-changing diff, the gate runs. The provider limit is in force until 2026-09-07 08:38: one attempt, `gate_run: no` with the reason, and the reviewer gates on brief conformance.

## Chosen without a grill

- **A bytes-over-four estimate and a fixed reserve.** No tokenizer is in the crate and `5fadfcc0` says the gauge is settled by measuring; a coarse estimate plus a reserve that absorbs its error is exactly the ruling's shape, and the miss event is what calibrates it later. Rejected: a tokenizer crate; per-model reserves.
- **The cut is archived under the trace, not dropped.** `e2b0ceb2` says the discarded tail survives on disk under a handle; the hand's trace directory is its handle. Rejected: dropping the tail.
- **The hand's state object is prose in one call, unchecked.** `5f17e29e` says the hand authors its own object; L2-06a's grammar and check exist for main's continuity across a respawned process, while a hand's object lives one respawn and dies with the brief. Rejected: L2-06a's checked file for hands; a truth-check turn for hands (two calls on a window that just overflowed).
- **Two respawns, then stop.** A hand that overflows three times under one brief is a brief cut too big, which the ruling names as the reading; stopping loud gives main the signal. Rejected: unbounded respawns.
- **Survived misses stay in the hand's trace.** The ruling puts the miss event there; main's log records the miss only when the hand died of it, so the count main sees stays meaningful. Rejected: a `miss` event per survived overflow in main's log.
- **The step budget is not reset at respawn.** `steps` caps the brief, not the window. Rejected: a fresh budget per respawn.

## Ground truth

Verified by a read-only trace on 2026-09-04 against the L2-10b working tree (✅ read by the trace, quoted here). L2-10c merged after the trace and touched only `anthropic.rs`, `openai_compat.rs`, `provider.rs`, `tests/claude_main.rs` and `tests/common/mod.rs`; nothing this card quotes from `native_hand.rs` or `container_hand.rs` moved, but line numbers may have; re-anchor by symbol.

Baseline at `877f085`, run cold by the L2-10c reviewer: `cargo test --workspace` 378 passed, 1 ignored. `cargo test -p arx` 334: unittests 182, check_config 8, checkpoint 6, claude_in_container 4, claude_main 57, container_runtime 9, events 3, gate 4, hands_in_containers 5, layer_close 12, native_in_container 6, state_object 7, store 3, transcript_log 28; arx-engine 44. Count these first; if any binary differs from this line before you edit, stop and report rather than adjusting the brief's arithmetic.

- ✅ `provider.rs`: `ProviderError { Transient { kind, retry_after }, Fatal { kind }, ContextWindowExceeded { overshoot: Option<Overshoot> } }` :109-115; `Overshoot { requested, maximum }` :103-107 (`Copy`); `Window { messages }` :46-75 with `new(brief)` :51-55, `append` :57-59, `rewind(keep) -> Vec<Message>` :61-66 (`split_off(keep)`, warned dead today), `as_slice` :68-70, `len` :72-74; `Message::user(text)` :38-43; `Retrying` :151-201 retries `Transient` only, so a `ContextWindowExceeded` reaches the loop at once; `Cancel` :77. `estimate_tokens`, `RESERVE_TOKENS`, `rewind_to_fit`: absent.
- ✅ `native_hand.rs`: `run_brief_inner(provider, spec, hand_id, trace, cancel, window, host, gate, steer_rx, delivery_tx, recall_engine) -> Outcome` :215-227 and `run_brief(...)` :401-412; the step loop `for step in 1..=spec.steps` :245 with `provider.complete` :254; the two error arms :256-260 (`complete` failed) and :274-278 (stream item) both `trace_write!(json!({"kind": "error", "text": err.to_string()}))`, `let miss = miss_for(&err)`, `return finish(format!("native hand failed — {err}"), None, miss, cancel)`; `miss_for` :149-157 (`requested - maximum`, `Some(0)` when unknown, `None` otherwise); `Outcome { Delivered { text, usage, miss }, Killed, TraceFailed(String) }` :41-49; `Trace { file }` :21-23 with `open(path)` :26-32 and `write(&Value)` :34-38 (one JSON line per call); `last_usage` :305; the cap arm :313-315; cancel checks :281, :376, :205; `window.append` :299 (assistant) and :367 (tool result); `ToolHost` :51-53, `ContainerHost` :74-102; `LocalHost`, `execute_turn`, `run_native_hand`, `spawn_native_hand_threads`, `spawn_native_hand` are reachable only from unit tests (dead-code warnings in a normal build).
- ✅ `container_hand.rs`: `run_native_in_container(spec, hand_id, brief, delivery_tx, control_rx, prepared)` :528; `transcript_path = root.root.join("transcript.jsonl")` :595; `Trace::open(&transcript_path)` :607 and `native_hand::run_brief(...)` :614-625 called directly; the sweep's extras `let extra = vec![(transcript_path, "transcript.jsonl".to_string())]` :714 and `container::sweep(&trace_dir, &index, &root.out, diff.as_deref(), spec.kind.label(), &extra)` :715 — a `rewind-<n>.jsonl` under `<root>` is one more tuple; the delivery :722-742 (`Delivery::Verdict { hand, text, trace: Some(trace_handle), bytes, usage, walls: Vec::new(), miss }` / `Delivery::Killed { hand, trace, bytes }`).
- ✅ `ledger.rs`: `Delivery::Verdict { hand, text, trace, bytes, usage, walls, miss }` :17-37; `HandSpec` :87-107 with `steps: u32` and `window: Option<u64>`; the verdict row carries `occupancy`/`window` when both exist :526-546 and never `miss`.
- ✅ `main.rs`: `delivery_miss` :561-564; `Event::Miss { size }` and the notice at :1562-1578 and :1669-1685 — a survived miss must not reach `Delivery::Verdict.miss`, or main will log it.
- ✅ `tests/common/mod.rs`: `FakeScript { Sse, Status, StatusWithRetryAfter, Stall, Sequence }` :122-128; `FakeState.requests` as `(path, authorization, body)` :130-135; `Sequence` answers the n-th connection with the n-th script :183-195.
- ✅ `tests/claude_main.rs`: `native_hand_reports_a_context_window_miss_as_verdict_event_and_notice` :1978-2018 (the miss shape this card must keep for the unsurvivable case: a 400 on the first step); `tests/native_in_container.rs` six tests (`read_file_executes_inside_the_container_and_the_store_is_masked`, `a_deny_rule_reaches_the_model_as_the_tool_result`, `an_ask_rule_holds_at_the_wall_and_main_s_steer_releases_it`, `a_kill_while_holding_at_the_wall_removes_the_container`, `recall_is_served_on_the_host_when_listed`, `the_key_is_nowhere_under_threads_or_in_docker_inspect`).
- ✅ Hazards: `crates/arx` has no `lib.rs`; Docker name races across concurrent test processes (check `docker ps -a --filter label=arx.hand` before blaming a test); `store/` and `pid.txt` are tracked live state; additive fields touch every literal (`Outcome::Delivered` sites if it grows).
- Trap: the miss test above pins a `miss` row in main's log for a first-step 400; this card's respawn path must leave that test untouched and add its own test where the 400 comes on a later step.

## Execution plan

### Scope 1 — the continuation

- **Files:** Modify `crates/arx/src/provider.rs` (`estimate_tokens`, `RESERVE_TOKENS`), `crates/arx/src/native_hand.rs`, `crates/arx/src/container_hand.rs` (designate `rewind-<n>.jsonl` files into the sweep). Do not modify `ledger.rs`, `main.rs`, `events.rs`, `transcript.rs`, `openai_compat.rs`, any test file other than the ones named in Scope 2.
- **Steps:**
  1. `provider.rs`: `estimate_tokens`, `RESERVE_TOKENS`; unit tests `estimate_counts_text_tool_calls_and_results` and `rewind_to_fit_never_cuts_the_brief` (a helper `rewind_to_fit(window, target) -> Vec<Message>` beside `Window`).
  2. `native_hand.rs`: the miss arm in the step loop per Decisions locked (rewind, archive, authoring call, respawn, the ladder, the bound); the four transcript rows; unit tests with a scripted provider: `a_miss_mid_brief_rewinds_authors_and_respawns_then_finishes` (scripts: ToolUse with a large tool result, ContextWindowExceeded with numbers, a text reply for the object, EndTurn; asserts four calls, the third call's messages are the rewound window plus the authoring prompt, the fourth call's messages are exactly the brief and the object, transcript rows `miss`, `rewind`, `state_object`, `respawn` in order, verdict from the fourth call, `miss: None` on the outcome, the archive file present with the cut messages), `a_miss_inside_authoring_rewinds_again_down_to_the_floor` (ContextWindowExceeded twice during authoring with shrinking numbers, then success), `a_third_miss_ends_the_brief_loud` (`MAX_RESPAWNS` exceeded → the verdict text and `miss: Some`), `a_miss_on_the_first_step_ends_the_brief_as_before`, `a_kill_between_stages_ends_the_brief_with_no_delivery`.
  3. `container_hand.rs`: designate every `<root>/rewind-*.jsonl` into the sweep beside `transcript.jsonl`.
- **Verify:** `cargo test -p arx` → unittests baseline + 7 (state), every other binary unchanged.

### Scope 2 — the done-when as data, the card

- **Files:** Modify `crates/arx/tests/native_in_container.rs` (one added test; every existing test unchanged), `docs/wayfinder/maps/arx-v0/L2-13-native-hand-exhaustion-respawn.md`. Do not modify `map.md`.
- **Steps:**
  1. `native_in_container.rs`: `a_native_hand_survives_a_context_window_miss_and_finishes_the_brief` — `hand-ds` against a `FakeScript::Sequence` of an SSE tool-call turn (the tool reads `arx.toml`), a `Status(400, <context-length body with requested 9000 and maximum 8192>)`, an SSE text turn (the object), an SSE EndTurn turn (`ARX-L2-13-OK`); asserts the verdict injection contains `ARX-L2-13-OK`, `threads/main.h1/transcript.jsonl` carries `miss`, `rewind`, `state_object`, `respawn` rows in that order, `threads/main.h1/rewind-1.jsonl` exists in the trace directory and `trace.md` lists it, the fourth request body's messages are the brief and the object only, and main's transcript log has no `miss` event (the hand survived).
  2. Live, with the key loaded (empty → STOP the step, report blocked): one `hand-ds` dispatch with "Reply with the single word ARX-L2-13-OK." to show no regression on the happy path; a forced live overflow is not attempted (it would spend the key on a rejected request against `hand-ds`'s 1M window).
  3. Close the card: `status: closed`, `assignee: spec-runner`, a `## Shipped` section in the voice of the last cards; note that L2-11's done-check passed unchanged.
- **Verify:** `cargo test -p arx` → `native_in_container` 7, every other binary as Scope 1; `cargo test --workspace` total stated; `docker ps -a --filter label=arx.hand` empty; `/tmp/arx-hands` empty; `git status --short` clean under `store/` and for `pid.txt`.

## Out of scope

- Main's reset, rewind and orientation (L3-02); the reserve as config; a tokenizer.
- A checked state object for hands; a truth-check turn for hands.
- Reporting survived misses in main's log; the "shortening hand scopes" counter (fog).
- Codex and claude-code hands' exhaustion (they manage their own windows, `5f17e29e`).

Done when Scope 2's Verify holds, the reviewer's must_fix is empty, and the map's frontier entry is written by main.

## Guardrails

- Never commit, never rebase, never stash, never switch branches, never `git add`. Read-only git toolkit on this repository; `git worktree add` only for a scratch build of `main` if a comparison needs it (remove it after).
- `store/` and `pid.txt` at the repo root are tracked live state: never write there, never stage them.
- Never print or log the DeepSeek key.
- No inline comments. Unslop applies to the card's Shipped paragraph and the verdict.
- If a locked count or a "keeps its assertions" clause cannot hold as written, STOP on that scope and report the exact conflict with evidence rather than bending the count or the test.
- Leave no container running and no `/tmp/arx-hands` entry when a scope ends.

## Owner items (morning read)

- The three calls most worth a veto: the bytes-over-four estimate with a fixed 2048-token reserve; two respawns then stop; survived misses recorded only in the hand's trace.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-04-l2-13-native-hand-exhaustion-respawn.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
