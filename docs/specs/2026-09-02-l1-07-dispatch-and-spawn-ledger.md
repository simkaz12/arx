# L1-07 — dispatch and the spawn ledger, with a fake hand

Card: `docs/wayfinder/maps/arx-v0/L1-07-dispatch-and-spawn-ledger.md`. Ruled by D04 (`5e8da52e`, `001823de`, `6aab1d24`), D05 (`23642b44`, `ba965dd0`), D15 (`eff1f01a`), D16 (`e652ce3a`), `3a873e9a`. A scope that lands against any of them stops and reopens the ruling in the store first.

## Problem / Solution

Main can talk and be gated but cannot delegate: no verb spawns a hand, nothing tracks one in flight, and nothing carries a verdict back. The design ruled dispatch as a non-blocking tool call, a spawn ledger holding every hand in flight, verdicts landing at main's next step boundary as injected messages, tap/steer/kill as main-initiated verbs, and stop as the child-initiated hold.

This card fixes the seam with a fake body behind it: a `SpawnLedger` over an append-only ledger file, a `FakeHand` that returns a verdict after a delay or raises a stop, the four verbs mounted as sidecar tools crossing a new `hand`/`hand_result` protocol exchange, and delivery wired into `cmd_run`'s loop — so the whole dispatch surface is proven before any real body (L1-08 to L1-10) sits behind it.

## Decisions locked

- Dispatch is non-blocking (`23642b44`): the tool returns the minted hand id at once; the verdict — or its mechanical stub — is injected at a later boundary, never returned from the call.
- The v0 step boundary is the turn boundary. `ba965dd0` keeps Claude Code's shipped intra-turn flow, and the SDK owns those steps; arx's coarsest boundary is between `run_turn` calls, so delivery happens after a turn's appends and render, before the next operator line is consumed. Mid-turn injection waits for a body whose step clock arx owns (L2-10).
- The ledger is one of the three durable artifacts (`eff1f01a`): append-only, under the thread dir — `threads/main.ledger.jsonl` beside the transcript — with the spawn row written before the child runs. A spawn row that cannot be written aborts the dispatch: the tool returns an error and no hand spawns. The ledger never opens `threads/main.jsonl`; the transcript's writer lock stays single-owner.
- The verbs on main's mounted surface are dispatch / tap / steer / kill (`e652ce3a`). They are tools, so every call crosses the gate (`canUseTool`) and gets its gate row for free.
- Steer pushes a message into one running hand and does not cascade (`001823de`). The fake appends steered text into its eventual verdict, so a landed steer is observable. Kill is teardown: the hand leaves the in-flight set, a `killed` row lands in the ledger, and a mechanical stub is injected — cancelled children never end in silence (`6aab1d24`). Cascade-down-the-spawn-tree is vacuous for a childless fake; noted, not built.
- Stop is the child-initiated hold (`5e8da52e`): the fake holds, a small mechanical request is injected into main's window, and main's answer releases it. The answer is a steer addressed to the stopped hand — the same push, opposite direction of need — and main's loop never blocks on the waiting child.
- Injected messages are mechanical text with pinned prefixes, parseable by tests and by main: `[hand h1 verdict] <text>`, `[hand h1 stop] <question> — answer with the steer tool`, `[hand h1 killed] exit: killed`. Each injection is a full turn cycle: the text is appended as a turn row with role `"arx"` via the existing `append_turn_as`, rendered as a `Notice`, sent to main through `run_turn`, and main's reply lands as a normal turn. No new transcript row kinds; `transcript.rs` untouched.
- The stub for a killed or crashed fake carries hand id and exit status only. Trace handles arrive with real bodies (`6aab1d24`'s sweep is L1-08+/L3-03); the fake has no trace and no store access — durable claims stay main's alone (`3a873e9a`), which the fake satisfies by construction.
- At operator EOF, in-flight hands are waited for and their deliveries injected before shutdown — the EOF drain — so a scripted session always sees its verdicts. A stop raised during the drain still injects and can be released by a steer in main's reply turn. A hand whose stop is never answered hangs the drain; timeout machinery is out of scope and the risk is accepted for the fake.
- Wire shape follows the gate precedent: `{ev:'hand', id, verb, args}` / `{op:'hand_result', id, result}`, correlated by id from the existing readline listener; `run_turn` grows `on_hand: impl FnMut(&HandCall) -> serde_json::Value` beside its other callbacks, so `claude_code.rs` carries the exchange and no ledger knowledge.
- Fake behavior is directed by the brief text: `[delay:<ms>]` (default 100), `[stop:<question>]` (hold before the verdict), `[verdict:<text>]` (default `done: <brief>`). Hand ids are `h1`, `h2`, … in dispatch order.
- A ledger write failure inside the `on_hand` closure fails the session by the L1-05 `gate_err` pattern, except the spawn-row case above, which fails only the dispatch.
- Sibling card L1-06 also grows `run_turn` a callback and the sidecar a tool set. The steps here are additive: if L1-06 landed first, add the `hand` arm and `on_hand` parameter beside its `store` counterparts rather than assuming their absence.
- Piped-mode stdout stays byte-identical for a session that dispatches nothing. Pre-existing test assertions change only by threading the new `on_hand` argument through direct `run_turn` callers.
- `terminal.rs` (beyond calling its existing `Notice`), `transcript.rs`, `gate.rs`, `config.rs`, `check-config`, `gauge` untouched.
- No commits, no git writes. Prior state is inspected read-only: `git status` / `diff` / `log`, and `git show <ref>:<path>` to read a file at a ref.

## Execution plan

### Scope 1 — the ledger and the fake

- **Files:** Create `crates/arx/src/ledger.rs`. Modify `crates/arx/src/main.rs` (add `mod ledger;` only, in this scope).
- **Build:** `SpawnLedger`, `FakeHand`, the ledger file — nothing wired.
- **Steps:**
  1. Types: `HandId` (the `h<n>` string), `Delivery { Verdict { hand, text }, Stop { hand, question }, Killed { hand } }`, `HandState { Running, Stopped, Done, Killed }`, `LedgerError` in `TranscriptError`'s mold.
  2. `SpawnLedger::open(threads_dir, thread)` — appends to `threads/<thread>.ledger.jsonl` with the transcript's single-buffer `write_all`-then-flush discipline. Rows, exact shapes: `{"kind":"spawn","hand":"h1","brief":...}`, `{"kind":"stop","hand":"h1","question":...}`, `{"kind":"steer","hand":"h1","message":...}`, `{"kind":"verdict","hand":"h1","text":...}`, `{"kind":"killed","hand":"h1"}`.
  3. `dispatch(&mut self, brief: &str) -> Result<HandId, LedgerError>`: write the spawn row first (`eff1f01a`), then spawn the fake — a `std::thread` parsing the brief's directives, sleeping the delay, sending `Delivery`s over an `mpsc` channel the ledger holds the receiver of, and listening on its own control channel for `Steer(text)` (append to the verdict; if stopped, release) and `Kill`.
  4. `steer(&mut self, hand, message)`, `kill(&mut self, hand)`, `tap(&self, hand) -> Option<serde_json::Value>` (state, brief, pending-stop question if any). steer/kill write their rows; kill sends the control message, marks `Killed`, and queues the `Killed` delivery itself so a wedged fake cannot suppress its own stub.
  5. Delivery accounting: `try_next(&mut self) -> Result<Option<Delivery>, LedgerError>` (non-blocking; writes the verdict/stop row as the event is handed out) and `wait_next(&mut self) -> ...` (blocking, for the EOF drain); `in_flight(&self) -> usize` counting `Running` + `Stopped`.
  6. Unit tests in-module: `spawn_row_precedes_the_fake_running` (dispatch with a long delay; the row is on disk before any delivery), `verdict_arrives_after_delay_and_is_rowed`, `steer_text_lands_in_the_verdict`, `stop_holds_until_steer_releases`, `kill_yields_a_killed_delivery_and_row`, `ids_increment_in_dispatch_order`.
- **Verify:** Count first: `cargo test -p arx` baseline is 84 (35 unit, 6 check_config, 17 claude_main, 26 transcript_log) plus whatever L1-06 landed — record the actual number; if pre-existing tests are red, STOP and report. Then the six new unit tests green.

### Scope 2 — the crossing, wired end to end

- **Files:** Modify `sidecar/claude-main.mjs`, `crates/arx/src/claude_code.rs`, `crates/arx/src/main.rs`, `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`.
- **Build:** Four tools mounted; the `hand`/`hand_result` exchange; boundary delivery and the EOF drain in `cmd_run`.
- **Steps:**
  1. Sidecar: add `dispatch` (brief), `tap` (hand), `steer` (hand, message), `kill` (hand) to the existing `arx` server's tool list. Parameter schemas as L1-06 pinned (zod from the SDK's tree; same tripwire). Each handler emits `{ev:'hand', id, verb, args}` with `id = nextHandCallId++`, awaits `{op:'hand_result', id, result}` via a `pendingHands` map on the existing readline listener, returns the result JSON as text content.
  2. `claude_code.rs`: define-or-import `HandCall { verb: String, args: serde_json::Value }` beside `GateCall`; `run_turn` takes `on_hand: impl FnMut(&HandCall) -> serde_json::Value`; new `"hand"` match arm mirroring the gate arm (parse, call, write `{op:'hand_result', id, result}` under a fresh stdin lock).
  3. `main.rs`: factor the per-turn cycle out of the loop body into a helper — append the prompt row under its role, `run_turn` with freshly built closures, append reply and usage, render `TurnEnd` — used by the operator path (role `"operator"`) and the injection path (role `"arx"`). The existing `chunk_err`/`gate_err` handling moves with it unchanged.
  4. `cmd_run`: open `SpawnLedger` before the loop. The `on_hand` closure serves the four verbs from the ledger, returning `{"hand":"h1"}` / tap's JSON / `{"ok":true}` / `{"error":...}`; ledger row failures follow the pinned failure split.
  5. Delivery: after each turn cycle, loop `try_next` — each `Delivery` formats its pinned message and runs an injection turn cycle, then re-checks (a reply may steer or kill). At operator EOF, while `in_flight > 0` or deliveries remain, `wait_next` and inject; then shutdown as today.
  6. Stub sidecar: a prompt starting `hand:` is `hand:<verb>:<rest>` — `hand:dispatch:<brief>`, `hand:tap:<id>`, `hand:steer:<id>:<message>`, `hand:kill:<id>` — emit one `{ev:'hand', ...}` exchange, then the normal delta/turn/usage/turn_end with turn text `stub-hand:<result-json>`. A prompt starting `[hand ` and containing ` stop]`: parse the id, emit a `steer` exchange with `{"hand":<id>,"message":"proceed"}`, then a normal stub turn. Other `[hand ` prompts take the plain stub path. Existing triggers unchanged.
  7. Thread the new `on_hand` argument through existing direct `run_turn` callers in tests with a closure returning `{"error":"no hands"}`; assertions unchanged.
- **Verify:** `cargo test -p arx` — pre-existing tests green with no edits beyond argument threading. `printf 'reply with exactly the word turquoise\n' | cargo run -q -p arx -- run` prints turquoise, exits 0, stdout byte-identical.

### Scope 3 — the done-when, proven

- **Files:** Modify `crates/arx/tests/claude_main.rs`.
- **Build:** The done-when as integration tests plus live checks.
- **Steps:**
  1. `three_dispatches_keep_main_turning_and_verdicts_land_at_boundaries` — stub workspace; prompts: three `hand:dispatch:[delay:50] job <n>` lines, one plain line, EOF. Assert: three spawn rows precede any verdict row in the ledger; the transcript holds three role-`"arx"` turn rows matching `[hand h<n> verdict]`, each followed by a main turn row; the plain prompt's turn shows main still taking operator turns while hands ran.
  2. `steer_reaches_a_running_fake` — dispatch `[delay:800] steerable`, then `hand:tap:h1` (assert the echoed result says running), then `hand:steer:h1:go faster`, EOF. Assert the ledger's steer row precedes the verdict row and the verdict text contains `go faster`.
  3. `kill_removes_the_hand_and_injects_the_stub` — dispatch `[delay:60000] doomed`, then `hand:kill:h1`, EOF. Assert a killed row, a role-`"arx"` turn matching `[hand h1 killed]`, and the run exits without waiting the sixty seconds.
  4. `stop_lands_as_a_mechanical_request_and_the_answer_releases_it` — dispatch `[delay:50] [stop:which db?] needs an answer`, EOF. Assert order in the transcript: the role-`"arx"` `[hand h1 stop] which db?` turn, then the stub's steer-driven reply turn, then the role-`"arx"` `[hand h1 verdict]` turn; and in the ledger: stop row, steer row, verdict row.
  5. Live checks, real body, root config, output quoted in the verdict:
     - `printf 'use the dispatch tool three times with briefs alpha, beta, gamma, then reply dispatched\nwhat verdicts have you received so far?\n' | cargo run -q -p arx -- run` → exits 0; `threads/main.ledger.jsonl` has three spawn rows and three verdict rows; the transcript shows `[hand h1 verdict]` … `[hand h3 verdict]` role-`"arx"` rows and main's replies acknowledge them.
     - `printf 'dispatch a hand with brief [delay:5000] slow, then kill it with the kill tool, then reply ok\n' | cargo run -q -p arx -- run` → exits 0 well under five seconds after the turn, ledger has spawn and killed rows, transcript has the `[hand h1 killed]` stub turn.
- **Verify:** `cargo test -p arx` — full suite green, the four new integration tests included. Both live commands with the quoted expected shapes.

## Out of scope

- Real bodies behind the ledger — Claude Code (`L1-08`), Codex (`L1-09`), native (`L1-10`); the judge (`L1-11`).
- The capture sweep, trace handles on verdicts, trace archives (`6aab1d24`'s sweep half; L2-01/L3-03).
- Hands crossing a checkpoint or reset (L2-06); containers and clamps (L2-07+).
- Stop/steer for foreign bodies (a CC-bodied hand has kill only, `001823de`) — moot until L1-08.
- Timeouts for unanswered stops or hung hands; recursion accounting (`20e9c406` rules it uncounted anyway).
- Mid-turn delivery on an arx-owned step clock — L2-10.

Done when the three scopes' Verifies pass: three fakes dispatched while main keeps taking turns, every verdict at a boundary and in the log, a steer reaching a running fake, a kill removing its hand and stubbing it, and a stop crossing as a mechanical request that main's answer releases.

## Review handoff

dispatch spec-runner on docs/specs/2026-09-02-l1-07-dispatch-and-spawn-ledger.md (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
