---
id: L2-13
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-11
ruled-by: `5f17e29e`, D15 (`e2b0ceb2`, `8b9f41ae` — the rewind-to-fit ladder), D14 (`3a5aefb6`)
---

# L2-13 — native hand exhaustion and respawn

## Deliverable

The continuation `5f17e29e` rules and no other card tests: an arx-native hand that exhausts its own window resets the same way main does — rewind-to-fit by the reported overshoot, respawn, the same brief continuing to a verdict. Split from L2-11 by the 2026-08-31 debate (S-SOL-3) so the forced-failure path gets its own session; L2-10's check stops at surfacing the exhaustion, this card survives it.

## Done when

A native hand on a deliberately small window is forced over it mid-brief; the exhaustion, rewind and respawn are lines in the transcript log; the same brief finishes after the respawn and its verdict lands at main's step boundary; L2-11's done-check still passes unchanged.

## Ruled by

`5f17e29e`, D15 (`e2b0ceb2`, `8b9f41ae`), D14 (`3a5aefb6`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

`provider.rs` gained `RESERVE_TOKENS = 2048` and `estimate_tokens(messages) -> u64` beside `Window` — UTF-8 bytes of every text block, tool-call argument JSON, and tool-result content, divided by four and rounded up. `rewind_to_fit(window, target) -> Vec<Message>` walks the window from the tail, message by message, summing `estimate_tokens` on each cut candidate until the running total meets `target` or only the brief is left, then calls the existing `Window::rewind` once with the surviving count — `rewind`'s first caller, no longer dead code.

`native_hand.rs`: the step loop is a `while step <= spec.steps` around an inner `'attempt` loop, so a miss-driven retry re-sends the same step without touching the budget. `handle_step_miss` is the new arm both error sites in the loop call — a stream-item error and a rejected `complete()` both feed it the same `ProviderError`. It writes the trace row (`miss` for `ContextWindowExceeded`, `error` for anything else, keeping the existing failure text and `miss_for` unchanged for every other case), then decides: nothing to cut (`window.len() <= 1`, the first-step case the pinned test locks) or the respawn bound already spent (`respawns >= MAX_RESPAWNS`, 2) ends the brief exactly as L2-10b did, `miss` set on the outcome; otherwise it rewinds by `rewind_target` (`requested - maximum + RESERVE_TOKENS` when the vendor's numbers are present, else `max(estimate/4, RESERVE_TOKENS)`), archives the cut to `<archive-dir>/rewind-<n>.jsonl` (one JSON row per cut message, appended so a second cut under the same ordinal doesn't clobber the first), and calls `author_state_object` for the state-object turn. That function loops on its own `provider.complete` with the rewound window plus the fixed `RESPAWN_PROMPT` appended (not persisted to the window); any termination is accepted and the accumulated text kept, a `ToolUse` call included; a `ContextWindowExceeded` here runs `ladder_step` — the same miss/rewind bookkeeping, no counter, ending the brief once the window is down to the brief alone and still rejected. On success `handle_step_miss` writes `state_object`, rebuilds the window as `Window::new(brief)` plus one user message wrapping the object, writes `respawn`, and the step loop resumes. `Trace` gained a `path: PathBuf` field (the one direct-construction test updated to match) and `archive_dir()`, so the rewind archive always lands beside the trace file the caller opened — `spec.threads_dir` for the flat native-hand trace, `root.root` for the containerized one — with no new parameter threaded through the existing call sites.

`container_hand.rs`: `run_native_in_container`'s sweep still starts from `[(transcript_path, "transcript.jsonl")]`, then globs `root.root` for `rewind-*.jsonl` and appends each as its own `(path, filename)` pair before calling `container::sweep` — nothing to glob on a brief that never missed, so the happy path's `extra` vec is unchanged.

Two unit tests landed in `provider.rs` (`estimate_counts_text_tool_calls_and_results`, `rewind_to_fit_never_cuts_the_brief`) and five in `native_hand.rs`: `a_miss_mid_brief_rewinds_authors_and_respawns_then_finishes` (four scripted calls — tool round, miss, authoring, final — asserting the third call's messages, the fourth call's messages, the four trace-row kinds in order, and the archive file), `a_miss_inside_authoring_rewinds_again_down_to_the_floor` (two tool rounds sized so the main rewind and two ladder rewinds each cut a different slice, ending in three `miss`/`rewind` pairs before `state_object`/`respawn`), `a_third_miss_ends_the_brief_loud` (two survived misses then a third past `MAX_RESPAWNS`, verdict text and `miss: Some`), `a_miss_on_the_first_step_ends_the_brief_as_before` (the L2-10b path, now tagged `miss` instead of `error` in the hand's own trace — the pinned `claude_main.rs` test reads main's log, not this file, and stays untouched), `a_kill_between_stages_ends_the_brief_with_no_delivery` (a provider wrapper that flips the cancel flag right after the authoring call returns, caught inside `author_state_object`'s stream loop).

`native_in_container.rs` gained `a_native_hand_survives_a_context_window_miss_and_finishes_the_brief`: a four-connection `FakeScript::Sequence` (tool call, a 400 naming requested 9000 and maximum 8192, the object turn, the final `ARX-L2-13-OK` turn) run through a real `arx run` session. It asserts the injected verdict text, the transcript's `miss`/`rewind`/`state_object`/`respawn` order, `rewind-1.jsonl` present under the trace directory and listed in `trace.md`, the fourth request's body carrying only the brief and the object, and no `miss` row in main's own log. The six existing tests in the file are untouched — L2-11's done-check still passes on them.

`cargo test -p arx` after Scope 1: unittests 189 (182 + 7), every other binary at its Scope-1 baseline. After Scope 2: `native_in_container` 7 (6 + 1), everything else the same. `cargo test --workspace`: 386 passed, 1 ignored (arx 342: unittests 189, check_config 8, checkpoint 6, claude_in_container 4, claude_main 57, container_runtime 9, events 3, gate 4, hands_in_containers 5, layer_close 12, native_in_container 7, state_object 7, store 3, transcript_log 28; arx-engine 44) — 8 new tests over the 378-passed baseline, matching the 7 unit plus 1 integration test actually written. One flake surfaced mid-run and nowhere else: `tests::ember_refuses_an_unregistered_scope` failed once under full-suite parallelism with "transcript log is locked by another writer," passed alone and on every rerun — a pre-existing lock-contention flake unrelated to this diff, not touched by it.

Live: with `DEEPSEEK_API_KEY` loaded from `~/.config/arx/env`, a scratch workspace (root `arx.toml` with `sidecar` pointed at the real `sidecar/` dir, `agents/`, `store/arx/`, and a fresh git init since dispatch requires a repository) ran a real Claude Code main. The operator line `hand:dispatch:hand-ds:Reply with the single word ARX-L2-13-OK.` dispatched `h1` against the real DeepSeek endpoint and returned `ARX-L2-13-OK` on the first request — no miss, no respawn, no regression on the happy path. A forced live overflow was not attempted, per the brief, since it would spend the key on a request `hand-ds`'s 1M window would only reject by design. Container and hand-root sweep confirmed empty before and after every run in this card.

Codex gate: `gate_run: no` — the provider's usage limit is in force until 2026-09-07 08:38, confirmed by one attempt.
