# L2-06b — the checkpoint ritual in the loop

Card: `docs/wayfinder/maps/arx-v0/L2-06b-checkpoint-ritual.md`, cut from L2-06 on 2026-09-04. Ruled by D15 (`adec3734` — checkpoints fire on a token-delta clock between turns or by operator command, cadence off means reset-only, every checkpoint runs the full ritual, each authored from the window and never from the previous object, hands in flight run on across it and the object lists them; `18fec68a` — authoring turn, harness-prompted truth-check turn, mechanical check on every write blocking on failure; `18b4928b` — the prior object is on disk, never the template; `eff1f01a` — the harness mints the handle on every checkpoint; `25664aa5` — the delta is config with no design-time default), D07 (`7dae481c` — no removal on a pressure clock), D16 (`952d3b69` — under a CLI-bodied main the ritual turns stay in the window; rewind is native-only), D05 (`23642b44` — dispatch is non-blocking; hands belong to the ledger, not the window), D09 (`340b3f2e` — instrumentation lands as harness rows), plus L2-06a's landed module (`state_object.rs`: `Header`, `StateObject`, `entries`, `check`, `write_checked`, `latest`, `checkpoints_dir`, `hash_file`; the `[main] checkpoint` key). Branch `l2-06b-checkpoint-ritual`, cut from `main` at `2f6ca32` (L2-10a merged).

Written 2026-09-04 without a grill, on the owner's overnight rule. Every call main made alone is listed under "Chosen without a grill" so the morning read starts there.

## Problem / Solution

L2-06a gives arx a state object it can read, write and check, and a `[main] checkpoint` key nothing reads. The loop still runs turn after turn with no checkpoint: the nudge is the only thing that watches occupancy, and it only speaks.

This card puts the checkpoint into `arx run`. Between turns, when occupancy has grown by the configured delta since the last checkpoint, or when the operator types `/checkpoint`, arx runs the ritual as three harness-prompted steps over the same `cycle!("arx", …)` path hand deliveries already use: an authoring turn whose prompt names the ordinal and the ledger's hands in flight and asks for the object body in L2-06a's grammar; a truth-check turn whose prompt carries the authored body and asks for `ok` or a corrected body; then L2-06a's check, and `write_checked` with a header arx mints from the reply's handle. A failing check gets one more authoring round with every failure named, then the checkpoint is abandoned loud. Every attempt lands as `Event::Checkpoint { check, delta }` in the transcript log and one notice on the terminal. Hand deliveries are not pumped between the ritual's turns, so a verdict that arrives mid-ritual lands at the turn boundary after it. Under the Claude-Code-bodied main the ritual turns stay in the window; nothing rewinds. The stub sidecar learns to answer the two prompts from files the test writes, and to grow its reported usage per turn on request, so the clock and the ritual are proven end to end without a model.

## Decisions locked

- The clock: `base` starts at 0 for every `arx run` (a fresh sidecar child is a fresh window, so nothing is recovered from earlier sessions); after every turn's `append_usage`, `occupancy = transcript::occupancy(usage)`; when `resolved.main.checkpoint` is `Some(delta)` and `occupancy.saturating_sub(base) >= delta`, the ritual runs before the next operator line is read. After a passing checkpoint `base = occupancy` as measured at the ritual's last turn. Absent key: the clock never fires; `/checkpoint` still works.
- The operator command: a stdin line that trims to exactly `/checkpoint` is taken by arx and never becomes a turn; it runs the ritual at once. Any other line is a turn, as today. No other command exists. A `/checkpoint` while the ritual would find a checkpoint already in progress cannot happen (the loop is single-threaded); a `/checkpoint` at EOF-adjacent timing behaves like any other line.
- The ordinal: `state_object::latest(&checkpoints_dir(threads_dir, "main"))` plus one, so ordinals continue across sessions.
- The authoring prompt, exactly (with `{n}` the ordinal and `{hands}` the ledger's non-terminal hands as `- <id> <agent>` lines, or `- none`):
  ```
  [arx] checkpoint {n}: author the state object from this window, not from any earlier object. Reply with the object body only, in this shape and nothing else:
  ## Position
  <what is in flight, what is decided, what is open, the next concrete move, which files are hot>
  ## Target
  <the target>
  ## Drift
  <whether the target drifted, and how>
  ## Hands in flight
  {hands}
  Cite a file as `- file: <path> @ <hash>` where <hash> is the output of `git hash-object <path>` run now, and a decision as `- decision: <id>`. Do not cite anything you have not verified in this window.
  ```
  The ledger's hands are read through `SpawnLedger` (add `pub fn in_flight_hands(&self) -> Vec<(HandId, String)>` returning id and agent for every entry not terminally delivered).
- The truth-check prompt, exactly (with `{body}` the authoring reply verbatim):
  ```
  [arx] checkpoint {n}: truth-check. Below is the object you just authored. Check every line against this window and against disk. Reply `ok` if every line holds; otherwise reply with the corrected object body in full, same shape.
  {body}
  ```
  A reply that trims to `ok` (case-insensitive) keeps the authored body; any other reply is the body.
- The check and the write: `state_object::check(body, config_dir, store, scope)` where `scope` is `resolved.store_scope` (when the store has no scope, decision entries fail with `DecisionResolves` and detail `no store scope`); on `Ok`, `write_checked` with `Header { thread: "main", checkpoint: n, at: <now, RFC 3339 UTC seconds>, turn: <the handle of the reply turn whose text became the body> }`; the handle is what `log.append_turn_as("main", …)` returned for that reply (`run_turn_cycle` must surface the reply handle to its caller — widen its `Ok(())` to `Ok(Handle)`, every call site adjusting, or expose `log.last_handle()` right after the cycle; the runner picks the smaller diff and says which). Success: `Event::Checkpoint { check: "pass", delta: occupancy.saturating_sub(base) }`, terminal notice `[arx] checkpoint {n} written: threads/main.checkpoints/{n}.md`, `base = occupancy`.
- On failure: `Event::Checkpoint { check: "fail", delta }`, terminal notice `[arx] checkpoint {n}: check failed — <probe> <entry> (<detail>); …`, then one more round — a second authoring prompt equal to the first with this line appended before the shape: `The mechanical check failed on your last attempt: <probe> <entry> (<detail>); … Fix every one.` — followed by the truth-check prompt and the check again. A second failure: `Event::Checkpoint { check: "fail", delta }` again, notice `[arx] checkpoint {n} abandoned after two rounds`, no file, `base` unchanged.
- `Event::Checkpoint { check: String, delta: u64 }` keeps its shape; `KINDS` stays 17. `check` is `pass` or `fail`.
- Deliveries: `pump_deliveries!` does not run between the ritual's turns; it runs after the ritual as it runs after any operator turn. The nudge's pending line, if any, rides the authoring prompt like any inbound turn.
- The stub sidecar (`crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`) gains two additive behaviours, both inert unless triggered: a prompt starting with `[arx] checkpoint ` and containing `author the state object` is answered with the contents of `checkpoint-author.md` in the sidecar's cwd on the first such prompt and `checkpoint-author-2.md` on the second (falling back to the first when the second file is absent); a prompt starting with `[arx] checkpoint ` and containing `truth-check` is answered with the contents of `checkpoint-truth.md` (falling back to `ok` when absent). Usage: when the env var `ARX_STUB_TOKENS_PER_TURN` is set to an integer `N`, the stub reports `input_tokens: N × (number of turns answered so far in this process)` and the two cache fields as 0, for every turn it answers; unset, today's constants. No existing test sets it, so no existing count or assertion moves. Which prompt prefix the stub matched is echoed nowhere; the transcript already carries the prompt as an `arx` turn row.
- Tests keep every locked count; the new binary `crates/arx/tests/checkpoint.rs` carries this card's done-when.
- Codex gate: behaviour-changing diff, the gate runs. The provider limit is in force until 2026-09-07 08:38: one attempt, `gate_run: no` with the reason, and the reviewer gates on brief conformance.

## Chosen without a grill

- **`base` restarts at 0 per `arx run`.** The clock measures growth of the live window; a new run has a fresh window and no growth. Recovering a base from a previous session's events would compare a new window against an old one. Rejected: an `occupancy` field on the event for recovery.
- **The model supplies the hash.** The check's value at write time is catching a claim the author did not verify; if arx filled hashes in, `FileHash` could never fail at write. A Claude-Code-bodied main has Bash and `git hash-object` is one command. Rejected: arx completing `- file:` entries.
- **Two rounds, then abandon.** `18fec68a` blocks the write on failure and says nothing about retries; one repair round is the smallest thing that makes a single typo survivable, and a second failure means the window is not in a state to author, which the operator should hear. Rejected: unbounded rounds; no repair round.
- **`/checkpoint` as the one operator command.** The terminal has no command grammar today; one exact line is the smallest one, and it cannot collide with prose. Rejected: a general `/` prefix with a parser; a control character.
- **No delivery pump inside the ritual.** `adec3734` lands verdicts after the orientation turn at a reset; the checkpoint analogue is after the ritual. A verdict injected between authoring and truth-check would put new material in the window between the object and its check. Rejected: pumping between ritual turns.
- **The truth-check reply replaces wholesale.** A partial correction would need a merge grammar; the ruling only asks the author to check and correct. Rejected: line-level patches.
- **Stub replies from files, usage from an env var.** The stub already answers by prefix; files let each test choose its bodies without a new operator grammar, and the env var keeps every existing usage assertion intact. Rejected: `checkpoint:` operator lines; a usage directive in the prompt text.

## Ground truth

Verified by a read-only trace on 2026-09-04 (✅ read by the trace, quoted here) plus main's own reads. Line numbers move; re-anchor by symbol. L2-06a's module is quoted from its brief's Decisions locked; the runner confirms the landed names before Scope 1.

- ✅ `run_turn_cycle(role, text, adapter, …, log, terminal, stdin, gate, …, store, ledger, last_handle, resolved, config_dir, nudge) -> Result<(), CycleFailure>` `main.rs:564-586`; a turn begins at `log.append_turn_as(role, text)` :593 and ends at `log.append_turn_as("main", &outcome.text)` :888 plus `terminal.render(TerminalMsg::TurnEnd)` :931; usage appended at :897 then the main-seat nudge :903-928 (`nudge.pending` prefixed onto the next inbound turn at :587-591).
- ✅ `cmd_run` `main.rs:958`: operator loop reads `read_operator(&mut stdin_lock)` :1197, `cycle!("operator", &line)` :1207, `pump_deliveries!()` :1208; shutdown drain :1212-1231; `pump_deliveries!` macro :1122-1166 (`ledger.try_next()`, `Event::Delivered`, `format_delivery`, `cycle!("arx", &text)`); `cycle!` :1178 and :1253 are the two `arx`-role sites.
- ✅ `Nudge { threshold, said, pending }` :524-528; `hand_nudge` :530-544; `COMPACT_WINDOW = 1_000_000` :39; `cmd_gauge` :1237-1264.
- ✅ Sidecar protocol: events `ready`, `fatal`, `delta`, `turn`, `usage`, `turn_end`, `gate`, `store`, `hand`; ops `init`, `prompt`, `gate_verdict`, `store_result`, `hand_result`, `shutdown`; no system-message injection after `init`, no session id, no resume; a harness-prompted turn is an ordinary `prompt` op. `ClaudeCodeMain::run_turn(&mut self, prompt, on_delta, on_gate, on_store, on_hand) -> Result<TurnOutcome, _>`; `TurnOutcome { text, usage: Option<Value> }`.
- ✅ Stub sidecar `claude-main.mjs`: prefixes `hand:` :88, `[hand … stop]` :127, `tool:` :150, `store:` :180; the plain-turn fallthrough :204-217 echoes `stub:<text>` and reports `usage { input_tokens: 7, cache_creation_input_tokens: 0, cache_read_input_tokens: 40, output_tokens: 5 }` (occupancy 47, the number the nudge test pins: `nudge: main at 47 tokens, past 40`).
- ✅ `transcript.rs`: `TranscriptLog::append_turn_as(role, text) -> Handle`, `last_handle`, `occupancy(usage)` :282-287 (sums `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`), `resolve(root, &Handle)` :335-365, `events(root, thread)` :367-393; `Handle { thread, turn }` :13-17.
- ✅ `terminal.rs`: `TerminalMsg { Chunk, TurnEnd, Notice, Ask }` :3-8; `OperatorMsg::Line(String)` :22-25; `read_operator` :76-89 reads one line, strips the newline, no command grammar.
- ✅ `events.rs`: `Checkpoint { check: String, delta: u64 }` among the reserved kinds :69-89; `KINDS` 17.
- ✅ `SpawnLedger::in_flight(&self) -> usize` counts entries not terminally delivered; entries hold the agent name (the spawn row carries `agent`).
- ✅ Config after L2-06a: `RoleConfig.checkpoint: Option<u64>`, `RoleRow.checkpoint`, `main.checkpoint must be positive`.
- ✅ L2-06a's module (`crates/arx/src/state_object.rs`): `Header { thread, checkpoint, at, turn }`, `StateObject { header, body }`, `render`, `parse`, `entries`, `hash_file`, `Probe`, `CheckFailure { probe, entry, detail }`, `check(body, root, store, scope)`, `write_checked(dir, obj, root, store, scope)`, `latest(dir)`, `checkpoints_dir(threads_dir, thread)`.
- ✅ Baseline at `2f6ca32` (L2-10a merged): 325 passed, 1 ignored (arx 281: unittests 151, check_config 8, claude_in_container 4, claude_main 50, container_runtime 8, events 3, gate 3, hands_in_containers 5, layer_close 12, state_object 7, store 3, transcript_log 27; arx-engine 44). Two known flakes under the default parallel run, neither this card's: one `transcript.rs` file-lock unit test under load, and a cross-binary Docker name race (`claude_main`'s kill test filtering `arx-h1-` can see another binary's container); single-threaded is stable. `RoleConfig` now also carries `steps` (L2-10a).
- ✅ Hazards: `crates/arx` has no `lib.rs`; `claude_main.rs` includes `gate.rs`, `claude_code.rs`; new unit tests for `main.rs` go in its own test module. `store/` and `pid.txt` are tracked live state. Docker is a test-time requirement of other binaries; leave no container behind.
- Trap: `checkpoint` under `[main]` is the delta; the fixture `tests/fixtures/run/arx.toml` has no such key, so tests that want the clock append it the way `with_main_nudge` appends `nudge` (`claude_main.rs:2091-2098`).
- Trap: the `arx` role turn rows carry the prompt text, so a test can assert on the authoring prompt (hands listed) by reading the transcript.

## Execution plan

### Scope 1 — the ritual in the loop

- **Files:** Modify `crates/arx/src/main.rs`, `crates/arx/src/ledger.rs` (`in_flight_hands` only), `crates/arx/src/terminal.rs` (only if `/checkpoint` is best recognised there — otherwise untouched). Do not modify `events.rs`, `transcript.rs`, `state_object.rs`, `config.rs`, any body module, the sidecar under `sidecar/`.
- **Build:** the clock, the command, the ritual, the events and notices, the no-pump rule.
- **Steps:**
  1. `ledger.rs`: `pub fn in_flight_hands(&self) -> Vec<(HandId, String)>`; unit test `in_flight_hands_lists_running_hands_with_their_agent` beside the existing ledger tests (dispatch two fakes with `[delay:2000]`, kill one, drain, assert the list).
  2. `main.rs`: a `Checkpoint` state struct beside `Nudge` (`delta: Option<u64>`, `base: u64`, `last_occupancy: u64`); `run_turn_cycle` records the turn's occupancy into it after `append_usage` (add the struct to its parameters as `nudge` is); the reply handle surfaced per Decisions locked; `fn run_checkpoint_ritual(...)` implementing the prompts, the check, the write, the events and notices, using `cycle!`-equivalent calls without `pump_deliveries!`; the operator loop: after `read_operator`, `line.trim() == "/checkpoint"` → ritual, `continue`; after every `cycle!("operator", …)` and before `pump_deliveries!()`, the clock test → ritual. Unit tests in `main.rs`'s test module for the pure parts: `authoring_prompt_lists_hands_or_none`, `truth_check_reply_ok_keeps_the_authored_body` (a small pure function `resolve_truth_check(authored: &str, reply: &str) -> String`), `clock_fires_on_the_delta_and_rebases` (a pure `Checkpoint::due(&self) -> bool` over `base`/`last_occupancy`/`delta`).
- **Verify:** `cargo test -p arx` → unittests baseline + 3 (main.rs) + 1 (ledger); every other binary unchanged; `cargo run -p arx -- check-config ./arx.toml` byte-identical to `main` (scratch worktree, removed after).

### Scope 2 — the stub, the done-when as data, the card

- **Files:** Modify `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs` (the two additive behaviours), `crates/arx/tests/common/mod.rs` (helpers: `with_main_checkpoint(config_path, delta)`, `plant_checkpoint_replies(workspace, author: &str, author2: Option<&str>, truth: Option<&str>)`, `hash_of(path) -> String` calling `git hash-object`). Create `crates/arx/tests/checkpoint.rs`. Modify `docs/wayfinder/maps/arx-v0/L2-06b-checkpoint-ritual.md`. Do not modify `map.md`, `tests/claude_main.rs`, `tests/layer_close.rs`, `tests/scripts/l1-close.json`, or any other test file.
- **Build:** the stub behaviours; six tests; the card closed.
- **Steps:**
  1. The stub per Decisions locked. Confirm with a one-off run that, with `ARX_STUB_TOKENS_PER_TURN` unset, the existing `claude_main` suite is unchanged (49 green, the nudge test's `47` intact).
  2. `tests/checkpoint.rs`, each test on `fresh_run_fixture()` (the fixture's `run/` is a git repository with one commit since L2-07; `arx.toml` sits in it so `- file: arx.toml @ <hash_of(run/arx.toml)>` is a valid entry), over `common::run_session` with the env carrying `ARX_STUB_TOKENS_PER_TURN` where the clock is under test; the store scope is the fixture's `testscope` with `plant_engine_scope` so a `- decision: <id>` entry can resolve:
     - `two_crossings_produce_two_objects_that_pass`: `checkpoint = 1000`, `ARX_STUB_TOKENS_PER_TURN=600`; authored body cites `arx.toml` with its true hash and the planted claim's id; truth `ok`; five plain operator turns; `run/threads/main.checkpoints/1.md` and `2.md` exist; both parse with `checkpoint: 1` and `2`; two `checkpoint` events with `check: pass`; each header's `turn` resolves via `transcript::resolve` to the authoring reply's text (the body).
     - `a_false_position_is_replaced_by_the_truth_check_reply`: authored body cites `missing.md @ <any 40 hex>`; truth reply is a corrected body citing `arx.toml`; `/checkpoint`; the written `1.md` carries the corrected body; one `pass` event; no `fail` event.
     - `a_failing_check_gets_one_more_round_then_passes`: authored body cites `arx.toml` with a wrong hash; truth `ok`; second authoring file cites the true hash; `/checkpoint`; the transcript has two authoring prompts, the second containing `FileHash arx.toml`; events `fail` then `pass`; `1.md` carries the second body.
     - `two_failed_rounds_abandon_the_checkpoint`: both authoring files wrong; `/checkpoint`; two `fail` events; no file under `main.checkpoints/`; stdout carries `abandoned after two rounds`.
     - `the_operator_command_fires_without_a_crossing`: no `checkpoint` key; `/checkpoint`; `1.md` exists; the transcript has no `operator` turn row whose text is `/checkpoint`.
     - `a_hand_in_flight_is_listed_and_its_verdict_lands_after`: `hand:dispatch:fake:[delay:6000] slow`; then `/checkpoint`; the authoring prompt row contains `- h1 fake`; `1.md`'s Hands in flight section lists `- h1 fake` (the stub's authored file is written by the test to include it); the `pass` event precedes the `[hand h1 verdict]` row in the transcript.
  3. Close the card: `status: closed`, `assignee: spec-runner`, a `## Shipped` section in the voice of the last cards: the clock and its base, the command, the three prompts verbatim, the repair round and the abandon rule, the events and notices, the no-pump rule, the stub's two additive behaviours, and any deviation.
- **Verify:**
  - `cargo test -p arx` → checkpoint 6; claude_main 49; every other binary as Scope 1; state the unittest total.
  - `cargo test --workspace` total stated.
  - Live check on the owner's login, in a scratch git repository with one commit and a `README.md`, the root `arx.toml` copied with `[providers.*]` dropped, `[store] scope = "live"`, `store/live/` created, `[main] checkpoint` absent: one operator turn ("Read README.md and tell me its first line."), then `/checkpoint`; quote the three prompts and replies from `threads/main.jsonl`, the written `threads/main.checkpoints/1.md` in full, and the `checkpoint` event row; then a second `/checkpoint` and confirm `2.md` was authored fresh (its `at` and `turn` differ). Scratch removed after; `docker ps -a --filter label=arx.hand` empty.
  - `git status --short` shows no change under `store/` or to `pid.txt`.

## Out of scope

- Reset, respawn, orientation, rewind, the reserve and the floor (L3-02).
- A default token delta (`25664aa5`).
- Rewinding ritual turns out of the window (native-only, `952d3b69`).
- Any operator command other than `/checkpoint`; a command grammar.
- Checking hands against the ledger; arx completing `- file:` entries.
- The state object as orientation input at spawn (L3-02's probe).

Done when Scope 2's Verify holds, the reviewer's must_fix is empty, and the map's frontier entry is written by main.

## Guardrails

- Never commit, never rebase, never stash, never switch branches, never `git add`. Read-only git toolkit on this repository: `git status`, `git diff`, `git log`, `git show <ref>:<path>`; `git worktree add` only for the scratch build of `main` (remove it after). Git writes inside temp directories the tests and the live check create are fine.
- `store/` and `pid.txt` at the repo root are tracked live state: never write there, never stage them.
- No inline comments. Unslop applies to the card's Shipped paragraph and the verdict.
- If a locked count cannot hold as written, STOP on that scope and report the exact conflict with evidence rather than bending the count or the test.
- Leave no container running and no `/tmp/arx-hands` entry when a scope ends.

## Owner items (morning read)

- The three calls most worth a veto: the model supplies file hashes itself; two rounds then abandon; `base` restarting at 0 per `arx run`.
- The prompts' exact wording is in the brief and the card; they are the first text a main reads at a checkpoint and the place to tune when the first real objects come in (`25664aa5`).

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-04-l2-06b-checkpoint-ritual.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
