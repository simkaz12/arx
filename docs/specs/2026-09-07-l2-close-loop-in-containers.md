# L2-close — the loop closes in containers

Card: `docs/wayfinder/maps/arx-v0/L2-close-loop-in-containers.md`. Ruled by the map's Notes (layer gate: each layer ends in one scripted end-to-end session across every seam at that depth, the scripts accumulate, L2-close re-runs L1-close's script unchanged before its own; `4c1ee2ce` — thin done-checks are on the seam contract and the crossing is a log line), `068d4cbf` (a hand's container gets an allowlist, empty by default: no host env, no home; its body's own auth the one mandatory entry, off every designated path), `84c34f6a` (the codex clamp is the container mount; the judge returns JSON under arx's schema), `58b34830` (the use claim is tested only at L3-04; this card is a rehearsal, not that test). Branch `l2-close-loop-in-containers`, cut from `main` after L2-12 merged.

Written 2026-09-07 without a grill, on the owner's clearance to finish layer 2 today. Every call main made alone is listed under "Chosen without a grill". Written before L2-12 landed, against L2-12's brief; the runner's first step is to confirm the L2-12 shapes named under Ground truth against the landed code and STOP if any differ.

## Problem / Solution

Every layer-2 card has landed and each carries its own tests, but nothing yet runs one session across all of them: the rule gate answering from the terminal, the engine's grounding and recall, a save landing in quarantine and a drain committing it, a checkpoint taken while a hand is still running, three bodies in containers with their credentials off the designated paths, a native hand's stop answered by main, a judge held to the schema, and `arx events` reading the whole thing back. L1-close proved the loop on the host with a script whose predicates name only rows arx writes itself; that script has been running under the layer-2 adapters since L2-08 moved the first body into a container, and still passes.

This card adds the second script beside the first. `l2-close.json` drives one session through every layer-2 seam over the stub sidecar and the stub bodies; a Rust layer check beside it reads what the predicate grammar cannot (the quarantine file, the checkpoint's hands-in-flight block, the store's commit log, the traces' credential lines, the containers gone at exit, `arx events`' output). The drain is a second `arx run` over the same workspace and thread, because the quarantine id it names is generated at save time. The L1-close script runs first in the same test binary, unchanged, and must pass before the L2 script starts. The live rehearsal on the owner's login runs the same shape once through the real sidecar and real bodies; it is a runner procedure, not a checked-in test.

## Decisions locked

- **Two scripts, one binary.** `crates/arx/tests/scripts/l1-close.json` is byte-identical to `main`. `crates/arx/tests/scripts/l2-close.json` is new. `crates/arx/tests/layer2_close.rs` is new, with exactly two tests in this order: `l1_close_script_passes_unchanged_under_layer_two` (a fresh workspace, the L1 script through `common::load_script` and `check_script`, nothing else) and `l2_close_loop_in_containers` (the L2 script, the drain run, the layer check). `layer_close.rs` and its twelve tests are unedited.
- **The fixture.** The L2 workspace is the run fixture (`crates/arx/tests/fixtures/run/`) as the L1-close test builds it, with: the store seeded as L1-close seeds it (a hot claim and a plain claim, the hot one's text in the system row); `[main] checkpoint = <delta>` set high enough that only `/checkpoint` fires the ritual, with `checkpoint-author.md` and `checkpoint-truth.md` planted as `checkpoint.rs` plants them, the authoring file naming the in-flight hand; a `[gate]` table `deny = ["mcp__arx__decisions_query"]`, `ask = ["mcp__arx__ping", "read_file(secret*)"]`; the fake provider server `native_in_container.rs` uses, scripted so `hand-ds`'s first tool call is `read_file` on `secret.txt`; `ARX_TEST_FAKE_KEY` set; the stub images and stub credentials for claude-code and codex planted; `ARX_CANARY_L2CLOSE=host-leak` set in arx's own environment for the run.
- **The operator lines of `l2-close.json`, in order.** Each answer to an ask sits on the very next line, because an ask's answer and the next operator turn are read from the same stdin lock:
  1. `plain start` (a turn; the system row before it carries the grounding)
  2. `store:recall:{"query":"<the hot claim's subject>"}` (recall over the engine; the main-role reply carries the hot claim's id)
  3. `tool:mcp__arx__ping` (an ask)
  4. `a fine by me` (the answer, from the terminal)
  5. `tool:mcp__arx__decisions_query` (denied by rule)
  6. `store:save:{"scope":"testscope","kind":"fact","question":"what closes layer two","answer":"the containers","why":"L2-close rehearsal"}` (a save intent; the reply is the landscape; the candidate lands in quarantine)
  7. `hand:dispatch:hand-cc:Summarise seeded.diff for the close`
  8. `hand:dispatch:hand-cx:Review seeded.diff for the close`
  9. `hand:dispatch:hand-ds:read secret.txt and say what it holds` (the native hand's `read_file` on `secret.txt` holds at the wall; the stop lands in main's window; the stub main answers it with its `proceed` steer, which the wall reads as a denial; the hand continues and delivers)
  10. `hand:dispatch:fake:[delay:2500] [verdict:slow done]`
  11. `/checkpoint` (fires while the fake is in flight; the state object lists it)
  12. `hand:dispatch:judge:Judge seeded.diff and return a verdict`
  13. `plain check`
- **The predicates of `l2-close.json`.** Rows arx writes itself only, never main's own turn text: the `system` row; a `turn` for each operator line; `gate` rows for `recall` (allow), `ping` (allow, `by` operator), `decisions_query` (deny, `by` rule), `save` (allow), each `dispatch` (allow), `steer` (allow); `spawn` rows `h1`..`h5` for `hand-cc`, `hand-cx`, `hand-ds`, `fake`, `judge`; a `wall` row for `h3`; a `steer` row for `h3`; `delivered` rows for `h1`..`h5`; a `checkpoint` row; a `schema` row for `h5`; `arx`-role turns containing `[hand h1 verdict]` .. `[hand h5 verdict]`, `[hand h3 stop]`; ordering with `after` where the seam demands it (the wall before its steer, the steer before `h3`'s delivery, the checkpoint row before `h4`'s delivery, the `schema` row before `h5`'s verdict turn). If a predicate needs a row field the grammar cannot bind, the assertion moves to the Rust layer check; the grammar is not extended in this card.
- **The drain run.** After the session, the test reads the quarantine id by question (`store.rs`'s `quarantine_id_by_question` shape), then runs a second session over the same workspace and thread with the single line `store:drain:{"suggestions":[{"id":"<id>","action":"promote","reason":"L2-close"}]}`. Checks: exactly one new commit in the store's git log after the drain and none after the main session, subject `drain main@<turn>: promoted 1, dropped 0`; the quarantine directory empty; the promoted claim present under the store.
- **The layer check, in Rust.** The quarantine file after the main session holds the saved question and answer and a `handle` with `thread` and integer `turn >= 1`; `threads/main.checkpoints/1.md` exists and its `## Hands in flight` block contains `- h4 fake`; the ledger has five `spawn` rows in order with roles `hand, hand, hand, hand, judge`; `threads/main.h1/session.jsonl` line 2 is `{"type":"cred","present":true}` and `threads/main.h2/session.jsonl` line 2 is `{"stub":"auth","present":true,"mode":"600"}`; both stubs' env line (below) reports `canary` false; `grep -r` over `threads/` for `stub-token`, `accessToken`, `claudeAiOauth`, `refresh_token`, `access_token`, `id_token`, `host-leak` and the value of `ARX_TEST_FAKE_KEY` is empty; `h5`'s window text carries a JSON object that `judge::validate_verdict` accepts and the `schema` row says `ok`; `h3`'s verdict text contains `denied`; `docker ps -a --filter label=arx.hand` is empty and `/tmp/arx-hands` is empty or absent after both runs; `arx events main` on the run dir exits 0 and its output contains `## gate (`, `## spawn (`, `## steer (`, `## wall (`, `## delivered (`, `## checkpoint (`, `## schema (` in `KINDS` order, with the gate count equal to the number of gate rows.
- **The env canary.** `tests/fixtures/claude-stub/claude` and `tests/fixtures/codex-stub/codex` each gain one more line in the file they already write into the trace: `{"type":"env","canary":<true|false>}` for claude (after its `cred` line), `{"stub":"env","canary":<true|false>}` for codex (after its last existing line), true when `ARX_CANARY_L2CLOSE` is set in the stub's environment. Appended, never inserted; no existing line moves. One locked test counts the claude stub's lines: `crates/arx/tests/claude_main.rs::claude_code_hand_dispatch_delivers_stub_verdict_with_trace` asserts `session_lines.len() == 3` at line 1055 and then each line's content; it takes the fourth line (`assert_eq!(session_lines.len(), 4, …)` plus one added content check that line 4 is `{"type":"env","canary":false}`), every other assertion in it and every other test in the file unedited (amended 2026-09-07 after the runner stopped on it; the brief had said the added line was safe if tests index rather than count, and one counts).
- **Deadline.** `common::run_session`'s `SESSION_DEADLINE` is 5 s. The extended session carries five container dispatches, a 2.5 s fake and a two-round checkpoint; if it does not fit, `run_session` gains a sibling taking an explicit deadline and the L2 test uses it with 30 s. `SESSION_DEADLINE` itself and every existing caller stay as they are.
- **Nothing else moves.** No source file under `crates/arx/src/` changes. If the layer check finds a seam that does not behave as its landing entry says, that is a finding for the frontier note, not a fix in this card: STOP and report it.
- **Amendments, 2026-09-07, sanctioned by main after the run.** (1) The rehearsal's `[gate]` table asks on `mcp__arx__recall`, not `mcp__arx__ping`: the real sidecar names the probe `arx_ping` and it crosses no gate (L2-14's landed finding), and `Rule::matches` is an exact string match; the hermetic fixture keeps `mcp__arx__ping`, where the stub's `tool:` line names the tool as the rule does. (2) The codex gate's six fixes all sit inside `crates/arx/tests/layer2_close.rs` and are accepted: the leak scan runs first and prints filenames and a fixed label only, never a needle or file content; it reads grep's exit status explicitly; the L1 script runs behind a process-wide `Once` so cargo's parallel runner cannot start the L2 test first. Its two overruled findings (content-addressed stub images; a wider needle list) are overruled on this brief's own text.

## Chosen without a grill

- The drain as a second run over the same thread rather than a placeholder the harness substitutes into the script: the quarantine id is generated at save time and the stdin blob is written whole before the session starts.
- The native hand's stop answered by the stub main's existing `proceed` steer, which the wall reads as a denial. The card asks for a stop raised and answered; the live rehearsal answers it with `allow`.
- The fake hand as the in-flight hand for the checkpoint, not one of the three real bodies: it is the only body whose duration the script controls.
- `decisions_query` as the tool denied by rule: it is a tool the script has no other use for, so the denial changes nothing downstream.
- The env canary as one additive line per stub rather than a new test binary for host-environment isolation, which `container_runtime.rs` already covers at the `docker run` argument level.
- Two tests in the new binary rather than folding the L2 script into `layer_close.rs`: the L1 file and its twelve counts stay locked.

## Ground truth

Verified by read-only traces on 2026-09-07 against the L2-09 landed tree; L2-12 shapes taken from its brief and to be confirmed first.

- Harness: `crates/arx/tests/common/mod.rs` `run_session` (513-595) spawns `arx run <config>` in the workspace, writes the operator lines as one blob to stdin, waits `SESSION_DEADLINE = 5s` (477), parses `threads/main.jsonl`; `run_session_with_stdout` (597-599) also returns stdout. `Script`/`Expect` (345-372, `deny_unknown_fields`): `name`, `kind`, optional `tool` (exact or `__<tool>` suffix), `verdict`, `role`, `contains` (on `text` only), `after`, `hand`, `by`; `check_script` (379-475) binds each predicate to the lowest-index unused row, validates `after` up front, collects every failure. Kinds in `l1-close.json`: `system`, `gate`, `spawn`, `turn`, `delivered`, `steer`; 27 predicates.
- Stub sidecar dialect (`tests/fixtures/stub-sidecar/claude-main.mjs`): `hand:dispatch:<agent>:<brief>`, `hand:steer:<hand>:<text>`, `tool:<name>`, `store:<verb>:<json>` (146-274); a `[hand <id> stop]` prompt is answered automatically with a steer whose message is `proceed` (177-190); `[arx] checkpoint … author the state object` reads `checkpoint-author.md` then `checkpoint-author-2.md`, the truth check reads `checkpoint-truth.md` (192-219); default replies echo. `ARX_STUB_DELAY_MS` and `ARX_STUB_TOKENS_PER_TURN` are the timing knobs.
- L1-close: `tests/scripts/l1-close.json` operator lines are a save (`store:save:{…"answer":"kestrel"…}`), dispatches of `hand-cc`, `hand-cx`, `hand-ds` (`say the word`), `judge`, `fake` (`[delay:50] [stop:which db?] [verdict:fake done]`), then `plain check`. `tests/layer_close.rs::l1_close_loop_on_host` builds the stub images, plants both stub credentials, asserts the 27 predicates, then in Rust: one quarantine file with `question`/`answer`/`handle.thread`/`handle.turn >= 1`; the save gate's next `main` turn contains `stub-store:` and `skylark`; five `spawn` rows `h1`..`h5` for `hand-cc/hand-cx/hand-ds/judge/fake`, roles `hand/hand/hand/judge/hand`; a `stop` row and an un-refused `steer` row for `h5`; a `verdict` row per hand; delivery texts `stub verdict:` / `native says skylark` / `fake done` with directory-shaped handles; `threads/main.h1/session.jsonl` argv contains `-p`, `threads/main.h2/session.jsonl` argv contains `--sandbox`, `danger-full-access`, `project_doc_max_bytes=0`.
- Gate: `tests/gate.rs` and `tests/scripts/l2-02-gate.json`: `[gate]` with `deny`/`ask` arrays of tool names, subject globs allowed (`read_file(secret*)`); the ask renders `[arx] ask: <tool> <subject>` then the key line; answers `a`/`A`/`d`/`D` with an optional note; the gate row is `{"kind":"gate","tool":…,"verdict":…}` plus `by` (`rule`, `operator`, `hook`) and `reason` when present; closed stdin denies with `[gate denied: no operator answer, stdin closed]`. An ask's answer is read through the same `read_operator` on the same stdin lock as the next operator turn (`main.rs:968-970`, `1878`).
- Store: `tests/store.rs`: grounding is hot and ember only (67-79); `store:save:` returns the landscape and commits nothing (82-120), the quarantine file under `store/quarantine/main/` carries `id`, `question`, `answer`, `handle`; `quarantine_id_by_question` (28-37) reads the id back; `store:drain:{"suggestions":[{"id","action":"promote"|"drop","supersedes"?,"reason"}]}` is one commit with subject `drain <thread>@<turn>: promoted N, dropped M` (`assert_drain_subject`, 40-52), empties the quarantine, and its `main` turn carries `"promoted":N`; the existing drain test runs the drain as a second session over the same run dir (178).
- Checkpoint: `[main] checkpoint = N` drives `Checkpoint::due()` (`main.rs:634-640`); `/checkpoint` fires the ritual unconditionally and is not a logged turn (`main.rs:1888-1890`); the file is `threads/<thread>.checkpoints/<ordinal>.md` (`state_object.rs:23-25`); the `## Hands in flight` block lists `- <hand> <agent>` from `ledger.in_flight_hands()` (`main.rs:643-649`, `ledger.rs:643`); `tests/checkpoint.rs::a_hand_in_flight_is_listed_and_its_verdict_lands_after` (206-242) dispatches `fake:[delay:1200] slow`, then `/checkpoint`, and asserts `- h1 fake` in the authoring prompt and in `1.md`, and the checkpoint's `pass` event before the verdict turn.
- Native hand: `tests/native_in_container.rs`: an `ask` rule on the hand's tool call writes a `wall` row `{hand, tool}` and an `arx`-role turn `[hand h1 stop] tool read_file on secret.txt — allow or deny? answer with the steer tool` (296-313); `steer_until_accepted` (56-75); `allow` releases the read (`top secret`), any other answer denies it (`denied: <answer>`, 321-324); `hand-ds` is `body: native`, `model: fake-model`, `provider: fake` over `[providers.fake]` (`openai-compatible`, base_url patched by `native_run_fixture`) with `ARX_TEST_FAKE_KEY` required (`main.rs:353-359`); the key never appears in `docker inspect` or under `threads/` (420-472).
- Credentials: claude `session.jsonl` line 2 `{"type":"cred","present":true}` and the trace grep for `stub-token|accessToken|claudeAiOauth` empty (`claude_in_container.rs:40-82`); codex `session.jsonl` line 2 `{"stub":"auth","present":true,"mode":"600"}` and the grep for `refresh_token|access_token|id_token` empty (`codex_in_container.rs:60-94`); `forwarded_env` skips `HOME`/`CODEX_HOME` (`container_hand.rs:263-274`); `docker run` carries no host env beyond `-e HOME=/hand/home` and the definition's `env:` names.
- `arx events`: `main.rs:2088-2129` prints a `kind / count` table then `## <kind> (<n>)` per non-empty kind in `KINDS` order with one `- <fields>` line per row; malformed event rows exit non-zero (`tests/events.rs:82-109`).
- L2-12 (from its brief, confirm first): `schema` rows `{"kind":"schema","hand":"hN","outcome":"ok"}`; the judge's window text `[hand hN verdict] {"verdict":"pass","summary":"stub verdict: …","findings":[]} (trace: threads/main.hN)`; `judge::validate_verdict(&str) -> Result<(), String>`; the codex stub answering `--output-schema` with JSON; `tests/judge.rs` with six tests; the counts after L2-12 as its landing entry states them.
- Counts on the L2-12 tree as its runner reported them: arx 403 over sixteen binaries (unittests 223, check_config 10, checkpoint 6, claude_in_container 4, claude_main 72, codex_in_container 4, container_runtime 9, events 3, gate 4, hands_in_containers 5, judge 6, layer_close 12, native_in_container 7, state_object 7, store 3, transcript_log 28); arx-engine 44 passed, 1 ignored. Container tests share one daemon; run the suite alone. L2-12 also added `common::rewrite_judge_definition`, gave the run fixture's `prompts/main.md` the judge sentence, forced a judge's `write` to `false` at `resolve_hand_spec`, and its live check showed a claude-code judge fences its JSON in a markdown block, which the strict validator refuses as `not JSON`; the hermetic judge in this card is codex-bodied and unaffected.
- Real `arx.toml`: `[providers.deepseek]` (`openai-compatible`, `DEEPSEEK_API_KEY`); the L2-15 live run dispatched `hand-ds` on it.

## Execution plan

### Scope 1 — confirm the ground, write the script

- **Files:** Create `crates/arx/tests/scripts/l2-close.json`; Modify `crates/arx/tests/fixtures/claude-stub/claude`, `crates/arx/tests/fixtures/codex-stub/codex` (the env line each).
- **Build:** the extended script and the two stub lines.
- **Steps:**
  1. Confirm every L2-12 item under Ground truth against the landed code (`git log -1` shows the L2-12 merge; `grep -n '"schema"' crates/arx/src/events.rs`; the judge test's window-line assertion; the codex stub's `--output-schema` branch). Any difference: STOP, report.
  2. Write `l2-close.json` per Decisions locked, `card: L2-close`.
  3. The two stub lines.
- **Verify:**
  - `python3 -c 'import json; json.load(open("crates/arx/tests/scripts/l2-close.json"))'` → no error.
  - `cargo test -p arx --test claude_in_container --test codex_in_container --test layer_close --test judge` → all green with the stub lines in place (the images rebuild from the fixture directories; if the `Once` cache serves a stale image, `docker rmi` the stub tags once and say so).

### Scope 2 — the binary and the layer check

- **Files:** Create `crates/arx/tests/layer2_close.rs`; Modify `crates/arx/tests/common/mod.rs` only if the deadline sibling or a shared helper is needed (say which); Modify `crates/arx/tests/claude_main.rs` for the one line-count assertion named under "The env canary" (amended 2026-09-07).
- **Build:** the two tests per Decisions locked.
- **Steps:**
  1. `l1_close_script_passes_unchanged_under_layer_two`: `sha256sum crates/arx/tests/scripts/l1-close.json` equal to `git show main:crates/arx/tests/scripts/l1-close.json | sha256sum` asserted inside the test via `git show`; then the L1 session and `check_script`.
  2. `l2_close_loop_in_containers`: the fixture per Decisions locked, the session, `check_script`, the drain run, the layer check.
- **Verify:**
  - `cargo test -p arx --test layer2_close` alone → 2 passed; on a failure, `docker ps -a --filter name=arx` first, say what you found, then re-run once.
  - `cargo test -p arx` alone → every binary green; counts stated with deltas from the L2-12 landing entry: `layer_close` 12 unchanged, `layer2_close` 2, `claude_main` 72 with one assertion re-pointed, every other binary unchanged.
  - `git diff --stat main -- crates/arx/src/` → empty.
  - Do not modify `tests/layer_close.rs`, `tests/scripts/l1-close.json`, `tests/scripts/l2-02-gate.json`, or any test file other than the ones named.

### Scope 3 — the rehearsal on the owner's login, the card

- **Files:** Modify `docs/wayfinder/maps/arx-v0/L2-close-loop-in-containers.md` (`## Shipped`, status `closed`).
- **Build:** one live session of the same shape through the real sidecar and real bodies.
- **Steps:**
  1. A scratch workspace (never the repo's own `store/`): the real `arx.toml` copied with `[store]` pointed at a scratch store seeded with one hot claim, `[gate]` as in the fixture, the real definitions (`hand-ex`, `hand-cx`, `hand-ds` on deepseek, `judge`), a scratch repo with `seeded.diff` and a `secret.txt`, `DEEPSEEK_API_KEY` present in the environment.
  2. Drive the real Claude Code main through piped stdin with operator lines of the same shape, in English for a real main: ask it to ping (answer the ask `a fine` on the next line), to query decisions (denied by rule), to save the claim, to dispatch the four hands and the judge with briefs of the same intent, `/checkpoint` while the slowest hand is in flight, and to answer the native hand's stop with `allow` through steer when it lands. Then the drain run with the quarantine id.
  3. Quote verbatim: the ask prompt and its answer; the deny; the save's landscape reply; each `[hand hN …]` window line; the stop and the steer; the checkpoint file's hands block; the judge's JSON; the drain commit subject; `arx events` for the thread; `docker ps -a` and `/tmp/arx-hands` after.
  4. Card: `## Shipped`, every deviation, status `closed`.
- **Verify:**
  - `cargo test --workspace` alone → every count restated cold.
  - `cargo run -p arx -- check-config ./arx.toml` → byte-identical to `main`'s.
  - `git status --short` → only the files this brief names plus the brief and the map; nothing under `store/`, no `pid.txt`.

## Out of scope

- Any change under `crates/arx/src/`; a seam that misbehaves is a frontier finding, not a fix here.
- Extending the `Expect` grammar (numeric predicates, JSON-body predicates).
- Admission, reset and the CLI-main floor, the capture sweep's diff-survival check, the daily-driver cutover: layer 3.
- The judge over a real diff review beyond the schema check; the drain's judge.

Done means: `layer2_close.rs` runs the L1 script unchanged and then the L2 script green, alone on the daemon; the quarantine holds the save until the drain run makes the store's only new commit with the locked subject; the checkpoint lists the in-flight hand; every body's credential line is present in its trace and no token or canary anywhere under `threads/`; the native hand's wall, stop and steer are rows; the judge's verdict validates; `arx events` groups the run; no container and no hand root remain; the rehearsal on the owner's login ran once with everything quoted.

## Guardrails

- Never commit, never rebase, never stash, never switch branches, never `git add`. Read-only git toolkit on this repository: `git status`, `git diff`, `git log`, `git show <ref>:<path>`; `git worktree add` only for the scratch build of `main` (remove it after). Git writes inside temp directories and scratch stores the tests and the rehearsal create are fine.
- `store/` and `pid.txt` at the repo root are tracked live state: never write there, never point a session at that store, never stage them.
- Never print, log, or quote the contents of any credential file or the value of any API key.
- No inline comments. Unslop applies to the card's Shipped paragraph and the verdict.
- If a locked count, a "keeps its assertions" clause, or a seam's documented behaviour cannot hold as written, STOP on that scope and report the exact conflict with evidence rather than bending it.
- Run the test suite alone on the Docker daemon; before re-running a failed container test, `docker ps -a --filter name=arx` and say what you found.
- Leave no container running, no `/tmp/arx-hands` entry, no scratch directory or worktree when a scope ends. Built images stay.
- The codex gate: run it; do not end your turn waiting on a round; poll the round log for its closing "Next steps" block in a bounded loop, then evaluate.

## Owner items

- The hermetic native stop is answered with the stub main's `proceed`, which the wall reads as a denial; the rehearsal answers `allow`. Both are "a stop answered by main".
- The drain is a second run over the same thread; the card's "then a drain run" is read literally.
- The rehearsal costs real Claude, Codex and DeepSeek calls; the numbers are the owner's to hold.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-07-l2-close-loop-in-containers.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
