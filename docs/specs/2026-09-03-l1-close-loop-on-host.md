# L1-close — the loop closes on the host

Card: `docs/wayfinder/maps/arx-v0/L1-close-loop-on-host.md`. Ruled by the map's layer-gate note and `4c1ee2ce` (each layer ends in one scripted end-to-end session; thin done-checks are on the seam contract, the crossing is a log line), with `58b34830` (the use claim: arx hosts the day's work, falsified when the owner runs bare `claude` instead) as the thing this card rehearses and does not test. If a step below lands against one of those, or against `23642b44`, `5e8da52e`, `001823de`, `da0b0d8b` or `e652ce3a`, stop and report; do not bend the step or the ruling.

## Problem / Solution

Eleven cards closed, each with its own hermetic test and one live probe, and nothing has run the whole loop in one sitting: grounding, a save, a hand of each body, the judge, a stop answered from main's side, all in a single `arx run`. Every seam was proven alone; the layer gate wants them proven together, from the terminal, as one session whose record is the transcript log.

This brief ships that session as data plus a runner. The script file lists the operator's lines and the transcript rows the session must produce; the runner drives `arx run` with the lines, reads `threads/main.jsonl`, and binds every expected row. The expectations name only rows arx writes itself (the system row, gate rows, the `arx`-role turns that carry deliveries), never what main's model chose to say, so L2-close can run the same file over the rule gate, the engine store and containerised hands without editing it. Beside the script sits this layer's own flat-file check: the quarantine file with its handle, the ledger's five spawn rows, the stub bodies' verdict texts, and the record that the only `claude` launched was arx's hand. One live run of the same session on the owner's login, over the real sidecar and real bodies, is the rehearsal of the daily shape and the only place "no bare `claude`" means anything; it is a runner procedure, not a checked-in test.

## Decisions locked

- One scripted session, driven through `arx run`'s stdin, crossing every layer-1 seam once (`4c1ee2ce`). The script is a file under `crates/arx/tests/scripts/` and the runner is an integration test beside the arx binary's tests. Nothing in this card adds a seam or moves one.
- Expectations are transcript-log rows only. A predicate names `kind` and, per kind, `tool`/`verdict`, `role`, a `contains` substring and an `after` ordering against another predicate. No predicate reads main's own turn text, the ledger, the store or a trace file; those assertions live in the layer check beside the script and are allowed to be layer-1 specific.
- Every tool call is a gate line. In the real sidecar `canUseTool` fires for every mounted tool, the SDK hands it `mcp__arx__<name>`, and the gate row is written before the call runs (`sidecar/claude-main.mjs:13-25,238`; `main.rs:427-445`). The stub sidecar today emits no gate event for its `hand:` and `store:` forms; this card makes it emit one with the same `mcp__arx__<verb>` name before each such call, so the hermetic run carries the rows the live run carries. A predicate's `tool` matches the bare name or the `mcp__<server>__<name>` form, so the script never names a server.
- Dispatch is non-blocking and a verdict lands as an injected `arx`-role turn between main's turns, never inside one (`23642b44`; `pump_deliveries!` at `main.rs:670-687`, the EOF drain at `main.rs:709-725`). The script asserts each delivery as its own `arx`-role turn row placed after the gate row of the dispatch that produced it. It does not assert the order in which hands finish.
- The stop is child-initiated and answered on main's next step (`5e8da52e`). At layer 1 only L1-07's fake raises one (card; `75ca8e75` for claude-code, the same posture in L1-09 and L1-10). Steer binds arx-native hands (`001823de`); the script steers only the fake. The stub sidecar already answers a `[hand hN stop]` delivery with `steer … "proceed"`; after this card that steer crosses the gate first.
- The save returns a landscape and lands in a per-thread quarantine file carrying the thread handle; the store refuses nothing and drain is not called (`da0b0d8b`; L1-06's shape). Grounding reaches main as text in the system prompt and so as the transcript's `system` row (`compose_store_grounding`, `main.rs:158-186`, `593`).
- The judge is a dispatch of `judge` through the codex adapter with `role: "judge"` on its spawn row (`1e6ad121`, `e652ce3a`, L1-11). The three bodies and the judge are dispatched by the agent names the run fixture already defines; no new agent definition.
- Single writer on the transcript stays: the runner reads `threads/main.jsonl` only after `arx run` exits, and nothing in this card opens the log from a second process while the run is alive.
- The gate row shape stays `{"kind":"gate","tool":…,"verdict":…}`. The L1-11 finding that the row does not carry the dispatch's `agent` is a follow-up card, not this one.
- The 47 tests in `claude_main.rs` keep their bodies. Helpers they share with the new runner move to `crates/arx/tests/common/mod.rs`; they are not duplicated.
- Product code under `crates/arx/src`, `crates/arx-engine/src` and `sidecar/` is untouched. Everything this card writes is under `crates/arx/tests/` and the card file.

## Chosen without a grill

The owner asked for this brief after L1-11 landed, with no grill; these calls are mine and are the first things to revisit if a scope reads wrong.

- The stub sidecar gates its own tool calls. Without it the hermetic script cannot state "every tool call a gate line" as a transcript assertion, and the stub would be lying about the surface it stands in for. Tests that break only because a gate row now precedes a hand or store call are updated to expect it; any other breakage stops the card.
- The script's operator lines are the stub sidecar's dialect (`hand:dispatch:…`, `store:save:…`). Main's body is not something the layer checks swap; the gate, the store and the hand bodies are, and they sit behind arx. Adapter-neutrality is carried by the expectations, which name only rows arx writes.
- Shared helpers move to `tests/common/mod.rs` rather than being copied into the new test crate. Cargo compiles the module into each test crate that declares it; `#![allow(dead_code)]` at its top keeps the per-crate unused warnings out.
- The live rehearsal is a runner procedure with prose operator lines and count-based checks, not a checked-in test. A real main decides which tools to call and how many times, so exact hand ids and exact row counts would make the test flaky about the model instead of the harness.
- The `claude` stub gains an append-only argv log, mirroring the codex stub's dump. It is the hermetic observable for "the only `claude` launched was arx's hand".
- Drain and recall are not in the session. The card names grounding and a save for the memory seam and nothing more; both verbs have their own L1-06 tests.

## Ground truth and tripwires

- Baseline: `cargo test --workspace` is 196 green (104 `arx` unit, 8 `check_config`, 47 `claude_main`, 26 `transcript_log`, 11 `arx-engine`); `cargo build --workspace` shows 6 dead-code warnings. Record the actual numbers before touching anything; anything red at the start is pre-existing, report it, do not fix it.
- The real sidecar registers ten tools: `arx_ping`, `save`, `recall`, `decisions_query`, `decision`, `drain`, `dispatch`, `tap`, `steer`, `kill` (`sidecar/claude-main.mjs:60-129`). Run `grep -c "^    tool(" sidecar/claude-main.mjs` first (the registrations are wrapped across lines, so the pattern matches the opening call, not the quoted name); ten is the expected answer, anything else means STOP and report.
- Live gate rows use the prefixed name. The root `threads/main.jsonl` (the harness's own cumulative log from prior live probes, gitignored) holds rows like `{"kind":"gate","tool":"mcp__arx__dispatch","verdict":"allow"}`; counted 2026-09-03: 30 dispatch, 157 tap, 5 kill, 4 recall, 2 arx_ping, 1 each of save, steer, drain, decisions_query. Do not edit or delete that file.
- Stub sidecar today (`crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`): `hand:<verb>:<payload>` at lines 79-115 emits `{ev:'hand'}` straight away; `store:<verb>:<json>` at 146-168 emits `{ev:'store'}` straight away; `tool:<names>` at 138-145 is the only form that emits `{ev:'gate', id, tool, input:{}}` and awaits `gate_verdict`; a `[hand <id> stop]` prompt at 117-136 issues `steer` with message `proceed`. The `dispatch` payload splits once on `:` into agent and brief, and an `@` in the agent becomes `args.body` (85-92).
- Transcript rows (`crates/arx/src/transcript.rs`): `system {kind,text}`, `turn {kind,turn,text,role?}`, `usage {kind,usage}`, `gate {kind,tool,verdict}`. Ledger rows (`ledger.rs`): `spawn {hand,agent,role,brief}`, `dispatch` (refusal only), `steer`, `killed`, `verdict {hand,text,trace,usage}`, `stop {hand,question}`. Spawn, steer and stop are ledger-only; their transcript trace is the gate row of the call and, for terminal states, the injected `arx`-role turn.
- Delivery text (`format_delivery`, `main.rs:360-367`): `[hand h1 verdict] <text> (trace: threads/main.h1.jsonl)`, `[hand h1 stop] <question> — answer with the steer tool`, `[hand h1 killed] exit: killed`. The injected text is run as a full turn cycle with role `arx`. Confirm in `run_turn_cycle` (`main.rs`, around 427-480) that the prompt is appended as a turn row carrying the cycle's role before main's reply is appended; if the `arx`-role row carries the reply instead of the injected text, the `contains` predicates on `arx` rows are wrong: STOP and report.
- Hand ids are minted at spawn in dispatch order, and operator lines run sequentially, so the five dispatches below are h1 to h5. If the run mints them differently, STOP and report; do not renumber the script to fit.
- The fake hand: agent name `fake`, `BodyKind::Fake`, `spawn_fake` at `ledger.rs:299`, driven by brief markers `[delay:<ms>]`, `[stop:<question>]`, `[verdict:<text>]`. The L1-07 stop test's line is `hand:dispatch:fake:[delay:50] [stop:which db?] needs an answer` (`claude_main.rs:1150`). Read `spawn_fake` before writing the script's fake line: if the post-stop verdict text comes from a `[verdict:…]` marker, use `[verdict:fake done]`; if it is fixed, expect that text in the layer check instead.
- Stubs under `crates/arx/tests/fixtures/bin/`: `claude` writes `$CLAUDE_CONFIG_DIR/claude-stub.pid` and a session file and prints `stub verdict: <first brief line>`; it writes no argv. `codex` writes `$CODEX_HOME/codex-stub-argv` (overwrite, not append), a rollout file and NDJSON events with usage. Two codex launches in one `CODEX_HOME` leave one argv file; do not assert on codex argv in this card.
- The run fixture (`crates/arx/tests/fixtures/run/arx.toml`) has `sidecar = "../stub-sidecar"`, `[main]`, `[providers.fake]` (openai-compatible, `base_url = "http://127.0.0.1:0"`, `api_key_env = "ARX_TEST_FAKE_KEY"`) and `[providers.stubbed]`; it has no `[store]` table. `store_test_workspace()` (`claude_main.rs:325-348`) writes its own toml with `[store]\nscope = "testscope"` and seeds `store/testscope.jsonl` from `planted_claims()` (`303-323`): a hot claim `what is the launch codename` / `skylark` and a plain one. `seeded.diff` with the line `CANARY: ARX-L1-11-JUDGE-CANARY` is in the run fixture.
- Helpers in `claude_main.rs`: `copy_dir_recursive` (140-151), `fresh_run_fixture` (153-159), `native_run_fixture(base_url)` (161-168, patches `http://127.0.0.1:0`), `stub_dir`, `stub_claude_bin_dir`, `claude_hand_path` (42-48), `fake_openai_server(FakeScript) -> (String, Arc<FakeState>)` (284-301) with `handle_fake_connection` (224-282), `planted_claims`, `store_test_workspace`. Line numbers are from 2026-09-03; confirm before moving.
- Exemplars: `judge_dispatch_over_a_seeded_diff_delivers_a_verdict_through_the_codex_body` (1419), `stop_lands_as_a_mechanical_request_and_the_answer_releases_it` (1143), `native_hand_text_turn_delivers_verdict_trace_and_usage` (1827, the `FakeScript::Sse` shape and the `say the word` line), `save_quarantines_and_leaves_the_store_file_untouched` (804, the exact `store:save:` line), `grounding_lands_in_the_system_row` (758), `three_dispatches_keep_main_turning_and_verdicts_land_at_boundaries` (981).
- After stdin EOF, `cmd_run` blocks in the drain loop until no hand is in flight. A stop that main never answers hangs the run. Hermetically the stub always answers; the live scope runs under `timeout`.
- Root config for the live scope: `arx.toml` has `[main] model = "claude-sonnet-5"`, `[store] scope = "arx"`, `[providers.deepseek]` with `api_key_env = "DEEPSEEK_API_KEY"`; `agents/` holds `hand-ex` (claude-code, claude-sonnet-5), `hand-cx` (codex, gpt-5.6-sol), `hand-ds` (native, deepseek-v4-flash), `judge` (codex, gpt-5.6-sol). The key is sourced from `~/.config/arx/env`. Live probes of each body passed in L1-08 to L1-11.
- Prose rule: everything you write passes `~/.claude/skills/unslop/SKILL.md`; read it by path before writing the card paragraph or any doc comment.

## Execution plan

### Scope 1 — the stub sidecar gates its own tools; the claude stub logs its launches

- **Files:** Modify `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`; Modify `crates/arx/tests/fixtures/bin/claude`; Modify tests in `crates/arx/tests/claude_main.rs` only where step 4 says.
- **Build:** the stub emits a gate event named `mcp__arx__<verb>` and waits for the verdict before every `hand:` and `store:` call, mirroring `canUseTool`. The `claude` stub appends its argv to `$CLAUDE_CONFIG_DIR/claude-stub-argv`, one line per launch.
- **Steps:**
  1. In the stub, add `async function crossGate(tool, input)` that allocates an id, writes `{ev:'gate', id, tool, input}` and returns a promise resolved by `gate_verdict` exactly as the existing `tool:` form does (reuse its pending map). On any verdict other than `allow`, throw; `AllowAll` is the only layer-1 gate and a deny here is a harness bug worth a loud failure.
  2. In the `hand:` branch, call `await crossGate('mcp__arx__' + verb, args)` before emitting `{ev:'hand'}`. In the `store:` branch, `await crossGate('mcp__arx__' + verb, args)` before `{ev:'store'}`. In the `[hand … stop]` branch, the steer it issues goes through the same `crossGate('mcp__arx__steer', …)`.
  3. In `fixtures/bin/claude`, after the pid write, add `printf '%s\n' "${args[*]}" >> "$config_dir/claude-stub-argv"`.
  4. Run `cargo test -p arx --test claude_main`. Tests that fail only because a `gate` row now precedes a hand or store call (a row count, an exact row sequence, a "no gate rows" assertion) are updated to expect the row, and each such edit is listed in the report. A failure for any other reason means STOP and report.
- **Verify:**
  - `printf '{"op":"init"}\n{"op":"prompt","text":"hand:dispatch:fake:x"}\n' | timeout 3 node crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs | head -3` prints `{"ev":"ready"}` then a line containing `"ev":"gate"` and `"tool":"mcp__arx__dispatch"` and no `"ev":"hand"` line (the stub is waiting for the verdict when the timeout kills it).
  - The same pipe with `store:save:{}` shows `"tool":"mcp__arx__save"`.
  - `cargo test -p arx --test claude_main` green at 47.

### Scope 2 — shared helpers and the script runner in `tests/common`

- **Files:** Create `crates/arx/tests/common/mod.rs`; Modify `crates/arx/tests/claude_main.rs`.
- **Build:** the helpers the new test crate needs live once, and a script runner exists: `Script`, `Expect`, `load_script`, `check_script`, `run_session`.
- **Steps:**
  1. Create `tests/common/mod.rs` with `#![allow(dead_code)]`. Move, verbatim, `copy_dir_recursive`, `fresh_run_fixture`, `native_run_fixture`, `stub_dir`, `stub_claude_bin_dir`, `claude_hand_path`, `FakeScript`, `FakeState`, `handle_fake_connection`, `fake_openai_server`, `planted_claims`, plus the `use` lines they need. Make each `pub`. In `claude_main.rs` add `mod common; use common::*;` and delete the moved definitions. Test bodies do not change.
  2. Add to the module:
     - `pub struct Script { pub card: String, pub operator: Vec<String>, pub expect: Vec<Expect> }` and `pub struct Expect { pub name: String, pub kind: String, pub tool: Option<String>, pub verdict: Option<String>, pub role: Option<String>, pub contains: Option<String>, pub after: Option<String> }`, both `serde::Deserialize`.
     - `pub fn load_script(path: &Path) -> Script`.
     - `pub fn check_script(rows: &[serde_json::Value], script: &Script) -> Result<(), Vec<String>>`. Binding rule: predicates are processed in file order; each binds to the lowest-index row not yet bound that has the same `kind` and satisfies every given field: `tool` equal to the row's `tool` or the row's `tool` ending in `"__" + tool`; `verdict` and `role` equal; `contains` a substring of the row's `text`. `after` names an earlier predicate and requires this binding's index to exceed that one's. Every predicate that fails to bind is reported as `"<name>: no unbound row matches"` or `"<name>: bound at <i>, not after <other> at <j>"`; the function returns all failures, not the first.
     - `pub fn run_session(workspace: &Path, config: &str, lines: &[String], env: &[(&str, String)]) -> Vec<serde_json::Value>`: spawns the built `arx` binary (`assert_cmd::cargo::cargo_bin("arx")`) with `run <config>` in `workspace`, `PATH` from `claude_hand_path()` unless overridden, pipes the lines joined with `\n` plus a trailing newline as stdin, waits for exit, asserts exit success, and returns the parsed rows of `<workspace>/threads/main.jsonl`.
  3. Where an existing `claude_main.rs` test already does what `run_session` does, leave it alone; this card does not refactor tests.
- **Verify:**
  - `cargo test -p arx --test claude_main` green at 47; `git diff --stat` is not available for these untracked files, so report `grep -c '^fn \|^pub fn ' crates/arx/tests/common/mod.rs` and confirm the moved names no longer appear as definitions in `claude_main.rs` (`grep -n '^fn fresh_run_fixture' crates/arx/tests/claude_main.rs` is empty).
  - `cargo build --workspace` still at the baseline warning count.

### Scope 3 — the L1-close script and this layer's check

- **Files:** Create `crates/arx/tests/scripts/l1-close.json`; Create `crates/arx/tests/layer_close.rs`.
- **Build:** the checked-in session and the test that runs it hermetically, then verifies the flat files.
- **Steps:**
  1. Write `l1-close.json`:
     - `card`: `L1-close`.
     - `operator`, in this order:
       1. the `store:save:` line from the L1-06 exemplar, with `scope` `testscope`, `kind` `fact`, `question` `what is the launch codename`, `answer` `kestrel`, `why` `L1-close rehearsal`
       2. `hand:dispatch:hand-cc:Summarise seeded.diff for the close`
       3. `hand:dispatch:hand-cx:Review seeded.diff for the close`
       4. `hand:dispatch:hand-ds:say the word`
       5. `hand:dispatch:judge:Judge seeded.diff and return a verdict`
       6. `hand:dispatch:fake:[delay:50] [stop:which db?] [verdict:fake done]` (marker per the tripwire)
       7. `plain check`
     - `expect`, in this order: `grounding` (system, contains `what is the launch codename`); `save-gate` (gate, tool `save`, verdict `allow`); `cc-gate`, `cx-gate`, `ds-gate`, `judge-gate`, `fake-gate` (gate, tool `dispatch`, verdict `allow`); `cc-verdict` (turn, role `arx`, contains `[hand h1 verdict]`, after `cc-gate`); `cx-verdict` (`[hand h2 verdict]`, after `cx-gate`); `ds-verdict` (`[hand h3 verdict]`, after `ds-gate`); `judge-verdict` (`[hand h4 verdict]`, after `judge-gate`); `fake-stop` (turn, role `arx`, contains `[hand h5 stop] which db?`, after `fake-gate`); `steer-gate` (gate, tool `steer`, verdict `allow`, after `fake-stop`); `fake-verdict` (turn, role `arx`, contains `[hand h5 verdict]`, after `steer-gate`); `check-turn` (turn, role `operator`, contains `plain check`).
  2. In `layer_close.rs`, `mod common; use common::*;` and the test `l1_close_loop_on_host`:
     - Start `fake_openai_server(FakeScript::Sse(...))` with the chunk shape from the native exemplar and the text `native says skylark`.
     - Build the workspace: `native_run_fixture(&base_url)`, then append `\n[store]\nscope = "testscope"\n` to `run/arx.toml`, write `run/store/testscope.jsonl` from `planted_claims()`, create `<tmp>/codex-home` and `<tmp>/claude-config`.
     - Env: `PATH` = `claude_hand_path()`, `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `ARX_TEST_FAKE_KEY=test-key`.
     - `run_session` with the script's operator lines; `check_script` on the rows; on `Err`, panic with every failure joined by newlines.
     - Layer check, on the flat files under `run/`:
       - `store/quarantine/main/` holds exactly one file; its JSON has `question` `what is the launch codename`, `answer` `kestrel`, `handle.thread` `main` and an integer `handle.turn` ≥ 1.
       - the `main`-role turn row after `save-gate` contains `stub-store:` and `skylark` (the landscape carried the seeded claim; the save itself never said skylark).
       - `threads/main.ledger.jsonl`: five `spawn` rows, `h1`..`h5` with agents `hand-cc`, `hand-cx`, `hand-ds`, `judge`, `fake`; `role` is `judge` on h4 and `hand` on the rest; a `stop` row for h5 and a `steer` row for h5 without `refused`; a `verdict` row for each of h1..h5.
       - the `arx`-role rows bound by `cc-verdict`, `cx-verdict`, `judge-verdict` contain `stub verdict: Summarise seeded.diff for the close`, `stub verdict: Review seeded.diff for the close`, `stub verdict: Judge seeded.diff and return a verdict` respectively, each with `(trace: threads/main.hN.jsonl)`; `ds-verdict`'s row contains `native says skylark`; `fake-verdict`'s row contains the fake's post-stop text.
       - `<claude-config>/claude-stub-argv` has exactly one line, and that line carries the print-mode flag `claude_hand.rs` passes (read the `Command` at `claude_hand.rs:214` for the exact flag). This is the hermetic "no bare `claude`": the run fixture's sidecar is a plain script with no SDK, and the only `claude` launched was the hand.
  3. Unit tests in `layer_close.rs` for `check_script`, each a few rows built with `serde_json::json!`: a predicate binds to the first unbound row; two `dispatch` predicates bind to two distinct rows; `tool` `dispatch` matches `mcp__arx__dispatch` and does not match `dispatcher`; an `after` violation is reported by both names; a missing row is reported by name and the other failures are still listed.
- **Verify:**
  - `cargo test -p arx --test layer_close` green: the session test plus the unit tests, count reported.
  - Force a failure once and revert: change `contains` of `fake-stop` to `which table?` and confirm the panic lists `fake-stop: no unbound row matches` and nothing else; restore the file.
  - `cargo test --workspace` green at the baseline plus this card's additions, numbers reported.

### Scope 4 — one live rehearsal on the owner's login

- **Files:** none checked in. A temp workspace, deleted at the end. The report carries the excerpts.
- **Build:** the same session over the real sidecar and real bodies: `hand-ex` on `claude -p`, `hand-cx` on `codex exec`, `hand-ds` on DeepSeek, the judge on codex, the fake for the stop.
- **Steps:**
  1. Preflight: `source ~/.config/arx/env` and confirm `DEEPSEEK_API_KEY` is set; run the codex quota check L1-11 used (`codex exec --model gpt-5.6-sol` over a one-line prompt in a temp dir); confirm `claude --version` prints. If any preflight fails for reasons outside the code (auth, quota, network), mark this scope blocked with the exact error text and continue to scope 5.
  2. Build the workspace in a temp dir: copy root `arx.toml` and set `sidecar = "<absolute path to repo>/sidecar"` and `[store] scope = "closescope"`; copy `agents/`; write `store/closescope.jsonl` with the hot claim from `planted_claims()`; copy `crates/arx/tests/fixtures/run/seeded.diff` to the workspace root.
  3. Snapshot `pgrep -x claude` and `pgrep -f 'claude-agent-sdk|/claude '` before the run; these pids (this Claude Code session among them) are excluded from the check below.
  4. Start `timeout 900 arx run arx.toml` in the workspace with stdin from a file holding these lines, one per turn:
     1. `Save this to the store as a fact in scope closescope: question "what is the launch codename", answer "kestrel", why "L1-close rehearsal". Then tell me what the landscape returned.`
     2. `Dispatch hand-ex with this brief: read seeded.diff in the working directory and reply with its CANARY line, nothing else.`
     3. `Dispatch hand-cx with the same brief.`
     4. `Dispatch hand-ds with this brief: reply with the single word skylark.`
     5. `Dispatch judge with this brief: review seeded.diff and say whether the change is correct, quoting the CANARY line.`
     6. `Dispatch the agent named fake with exactly this brief, verbatim: [delay:50] [stop:which db?] [verdict:fake done]`
     7. `plain check`
     While it runs, sample every half second: for each `claude` process not in the snapshot, walk `ps -o ppid= -p` upward and record whether the `arx run` pid is an ancestor. Keep the samples.
  5. After exit, check with `grep`/`jq` on `threads/main.jsonl` and `threads/main.ledger.jsonl`, reporting each count:
     - the `system` row contains `what is the launch codename`;
     - gate rows: at least 1 `mcp__arx__save`, at least 5 `mcp__arx__dispatch`, at least 1 `mcp__arx__steer`;
     - `arx`-role turn rows containing `[hand h` and ` verdict]` for five distinct hand ids; one containing ` stop] which db?`;
     - the ledger has five `spawn` rows, one with `role` `judge`, one with agent `fake`; a `stop` and an unrefused `steer` for the fake's id;
     - `store/quarantine/main/` holds one file with `handle.thread` `main`;
     - the verdict texts of the `hand-ex`, `hand-cx` and judge hands each contain `ARX-L1-11-JUDGE-CANARY`; the `hand-ds` verdict contains `skylark`;
     - the sampler saw at least one `claude` process and every one it saw had the `arx run` pid as an ancestor.
  6. If the real main does not call the tool a line asks for, rephrase that line once and rerun the whole session; a second miss is a finding, reported with the turn rows, not fixed by editing `agents/main.md` or the sidecar. If the run hits the timeout, kill it, report the last twenty transcript rows, and mark the scope failed.
  7. Delete the temp workspace. Do not touch the repo's own `threads/` or `store/`.
- **Verify:** the counts in step 5 as a table in the report, the sampler summary, and the `arx`-role verdict rows quoted verbatim.

### Scope 5 — the card

- **Files:** Modify `docs/wayfinder/maps/arx-v0/L1-close-loop-on-host.md`.
- **Build:** the card records what shipped and closes.
- **Steps:**
  1. Set `status: closed` in the frontmatter.
  2. Add a short `## Shipped` paragraph naming the script path, the runner's file, the stub sidecar change, and the live rehearsal's outcome (or its blocked reason). Prose passes unslop.
  3. `map.md` stays untouched; main writes the frontier entry.
- **Verify:** `grep -n '^status:' docs/wayfinder/maps/arx-v0/L1-close-loop-on-host.md` prints `status: closed`; `git status --short docs/wayfinder/maps/arx-v0/map.md` shows no change from this card.

## Out of scope

- Any layer-2 adapter or event: the rule gate, deny and ask, the engine, containers, checkpoint, `arx events`, the verdict schema, a real stop producer. L2-close extends this script; it does not need this card to anticipate it.
- Drain, recall, tap and kill in the session. Each has its own L1 test; the card names what the session crosses.
- Adding `agent` or any field to the gate row (the L1-11 follow-up card). Any change under `crates/arx/src`, `crates/arx-engine/src` or `sidecar/`.
- The native hand's kill liveness under a stalled read (`c2109345`, carried to L2-10).
- Tuning `agents/main.md` or the sidecar's system prompt to make the live main more obedient; a miss is reported.
- Refactoring the 47 `claude_main` tests onto `run_session`.
- An LD_PRELOAD open-trace for the hermetic scope; the argv log and the SDK-free stub sidecar are the observables here.

Done when: `cargo test -p arx --test layer_close` runs the checked-in script green with every seam crossing bound to a transcript row, the layer check beside it passes on the quarantine file, the ledger and the argv log, the whole workspace suite is green, the live rehearsal's counts are reported (or its block named), and the card is closed with `map.md` untouched.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-03-l1-close-loop-on-host.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
