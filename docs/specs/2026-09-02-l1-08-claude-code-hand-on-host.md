# L1-08 — claude-code hand on the host

Card: `docs/wayfinder/maps/arx-v0/L1-08-claude-code-hand-on-host.md`. Ruled by D16 (`e652ce3a`, `0bd4df2f`), D12 (`e79bed85`), `5f17e29e` — if a step below lands against one of those rulings, stop and report; do not bend the step or the ruling.

## Problem / Solution

L1-07 proved the dispatch seam with a fake body: main dispatches, taps, steers, kills, verdicts land at step boundaries. Nothing real has ever sat behind the ledger. This brief puts the first real adapter there — `claude -p` spawned as a child on the host. The brief goes in with grounding only as text inside it, the verdict comes out through the ledger, and the hand's raw transcript is on disk under the thread before main sees the verdict. No container, no clamp, no credential isolation, no fail-loud shortfall — all of that is L2-08.

## Decisions locked

- `claude -p` is the hand drive surface: a one-shot child, hand-only. Main stays on the SDK sidecar; the drive surface is never configured, it follows from the role (`e652ce3a`, `0bd4df2f`).
- A hand's `body` and `model` come from its named agent definition under `agents/`, so the `dispatch` tool gains a required `agent` argument (`0bd4df2f`).
- Grounding reaches a claude-code hand as text inside the brief and nothing else. The brief text passes to the child verbatim — no store mount, no MCP surface, no path to `store/arx.jsonl` anywhere in the child's args, env additions, or prompt (`e79bed85`). Store-derived grounding composition is L2-05, not here.
- The hand manages its own window; arx reads no usage from it and wires no nudge or reserve (`5f17e29e`).
- The transcript artifact is Claude Code's own session `.jsonl`, pinned by an arx-generated `--session-id`, copied to `threads/<thread>.<hand>.jsonl` before the verdict is delivered. The verdict line main sees carries that path as its trace handle. This is the placeholder L3-03's sweep replaces.
- The fake body stays behind the seam under the reserved agent name `fake` — no definition file, short-circuits agent resolution. Every L1-07 test keeps passing through it.
- Seam shape for now: an internal body-kind enum matched at dispatch, one spawn function per body with a shared signature. A trait waits until L1-09/L1-10 show the third adapter's shape; two call sites don't justify it yet.
- A running claude-code hand cannot receive a steer (one-shot child, stdin already consumed) and never raises a stop (nothing is mounted that could). Steer against a running claude-code hand returns a mechanical refusal as the tool result and writes a ledger row; the recourse is kill and re-dispatch. D05's verbs stand unchanged — this body lacks a receiver, which D16's role-decides-the-mount ruling allows. Steering via `--input-format stream-json` is a later card if wanted.
- Dispatch on an agent whose body is `codex` or `native` returns an error tool result naming L1-09 / L1-10. Unknown agent names return an error listing the known definitions.

## Execution plan

### Scope 1 — dispatch names an agent; ledger routes by body kind

- **Files:**
  - Modify `sidecar/claude-main.mjs` (dispatch tool schema, ~line 110–133)
  - Modify `crates/arx/src/main.rs` (`on_hand_call`, ~192–235; closure wiring ~352)
  - Modify `crates/arx/src/ledger.rs` (dispatch signature, `HandEntry`, steer refusal)
  - Modify `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs` (pseudo-prompt convention)
  - Modify `crates/arx/tests/claude_main.rs`, ledger unit tests in `ledger.rs`
- **Build:** `dispatch` carries `agent`; resolution happens host-side against the loaded agent definitions; the ledger spawns by body kind; the fake keeps working under the reserved name.
- **Steps:**
  1. Add a required `agent` string to the `dispatch` MCP tool's input schema in `sidecar/claude-main.mjs`; it rides through the existing `{ev:"hand", ...args}` protocol untouched.
  2. Define an internal body-kind enum (`Fake`, `ClaudeCode`, plus stub arms for `Codex`/`Native`) and a `HandSpec` (agent name, body kind, model, child cwd, threads dir, thread name) in `ledger.rs`.
  3. In the `on_hand` closure in `main.rs`, resolve `agent`: `"fake"` → `HandSpec` with kind `Fake`; otherwise look up the resolved agent definitions (`hand-ex.md`, `judge.md`, …) and map config `Body::ClaudeCode` → kind `ClaudeCode`; `Body::Codex`/`Body::Native` → error tool result "body not implemented until L1-09/L1-10"; unknown name → error tool result listing known definitions. Child cwd is the config dir (same as main's).
  4. Change `SpawnLedger::dispatch(brief)` to `dispatch(spec, brief)`; match on the body kind — `Fake` → the existing `spawn_fake`, `ClaudeCode` → `spawn_claude_hand` (scope 2; land scope 1 with an `unimplemented` arm or behind the scope-2 module, whichever compiles cleanly first). Record the agent name in the spawn ledger row and keep the body kind on `HandEntry`.
  5. `steer` on a hand whose kind is `ClaudeCode` and state is Running: do not push onto the control channel; write a ledger row in the existing steer row shape extended with a refusal marker, and return the refusal text as the tool result ("h1 is a claude-code hand mid-turn; steer is not deliverable — kill and re-dispatch"). `tap` and `kill` are unchanged.
  6. Update the stub-sidecar pseudo-prompt convention from `hand:dispatch:<brief>` to `hand:dispatch:<agent>:<brief>` and update every existing test to pass `fake` (ledger unit tests call `dispatch` with a `Fake` spec directly).
- **Verify:** `cargo test -p arx` — the four L1-07 e2e tests (`three_dispatches_keep_main_turning_and_verdicts_land_at_boundaries`, `steer_reaches_a_running_fake`, `kill_removes_the_hand_and_injects_the_stub`, `stop_lands_as_a_mechanical_request_and_the_answer_releases_it`) and all ledger unit tests green with the `agent` argument in place.

### Scope 2 — the claude-code body

- **Files:**
  - Create `crates/arx/src/claude_hand.rs`
  - Modify `crates/arx/src/main.rs` (declare `mod claude_hand;` beside the existing module declarations)
  - Modify `crates/arx/src/ledger.rs` (wire the `ClaudeCode` match arm)
- **Build:** `spawn_claude_hand(spec, hand_id, brief, delivery_tx, control_rx)` — same signature shape as `spawn_fake` — spawns `claude -p` as a process-group child, captures the session file, delivers the verdict.
- **Steps:**
  1. Tripwire first: run `claude -p --help` and confirm `--model`, `--output-format` (with a `json` value), and `--session-id` all exist. If any is missing, STOP and report the actual help output — do not substitute flags. (Local binary was 2.1.240 at R01 time; `--help` is known to under-list at least one documented flag, so check the docs reference before declaring a flag absent.)
  2. Build the command: binary `claude` from `PATH`, args `-p --model <spec.model> --output-format json --session-id <freshly generated uuid v4>`; brief text written to the child's stdin verbatim, then stdin closed; cwd `spec.cwd`; `process_group(0)`; stdout/stderr piped; environment inherited (host hand, no isolation at L1). Do NOT pass `--no-session-persistence` — the session file is the transcript artifact.
  3. Spawn thread mirrors `fake_body`'s loop: poll the child (`try_wait`) while draining `control_rx`. `Control::Kill` → SIGKILL the child's process group and reap (reuse the `kill_and_reap` pattern from `claude_code.rs:123-128`), exit without sending a verdict — the ledger already synthesized `Delivery::Killed`, and the existing late-delivery guard in `handle_delivery` covers any race. `Control::Steer` cannot arrive (scope 1 refuses it host-side); if one does, log and drop.
  4. On child exit: parse stdout as one JSON object; verdict text is its `result` field (note `is_error` if set). Non-zero exit or unparseable stdout → verdict text carries the exit status and a bounded stderr tail (last ~2000 bytes) — the verdict still lands; fail-loud-as-shortfall semantics are L2-08.
  5. Locate the session file by glob `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/projects/*/<session-uuid>.jsonl` and copy it to `<threads_dir>/<thread>.<hand_id>.jsonl` (e.g. `threads/main.h1.jsonl`). Copy completes before the verdict is sent. If the file cannot be found, append a one-line capture-failure note to the verdict text; the verdict still lands.
  6. Send `Delivery::Verdict` with the verdict text; extend the verdict injection format (`format_delivery`, `main.rs:237-243`) so a claude-code hand's line carries the trace path: `[hand h1 verdict] <text> (trace: threads/main.h1.jsonl)`. The fake's format is unchanged.
- **Verify:** `cargo build -p arx` clean; a unit test in `claude_hand.rs` asserting the constructed argv (flags, model, uuid present) and that no argument or added env var contains `store/`.

### Scope 3 — hermetic e2e plus one live run

- **Files:**
  - Create `crates/arx/tests/fixtures/bin/claude` (executable stub script)
  - Create `crates/arx/tests/fixtures/run/agents/hand-cc.md` (`body: claude-code`, `model: claude-sonnet-5`, matching the frontmatter shape of the real `agents/hand-ex.md`)
  - Modify `crates/arx/tests/claude_main.rs`
- **Build:** the L1-08 done-when as tests, with a stub `claude` on `PATH` and `CLAUDE_CONFIG_DIR` isolation so no test touches the real `~/.claude`.
- **Steps:**
  1. Stub script: parse `--session-id` from its args, read the brief from stdin, honor a `[sleep:N]` directive in the brief (seconds), write a two-line fake session `.jsonl` to `$CLAUDE_CONFIG_DIR/projects/stub/<session-id>.jsonl` (create dirs), print `{"type":"result","subtype":"success","is_error":false,"result":"stub verdict: <first line of brief>","session_id":"<session-id>"}` to stdout, exit 0.
  2. Tests spawn the arx binary with `PATH` prefixed by the fixture `bin/` dir and `CLAUDE_CONFIG_DIR` set to a per-test temp dir, mirroring the existing `claude_main.rs` fixture pattern.
  3. Test: dispatch `hand-cc` with a plain brief → verdict row in `main.jsonl` contains `stub verdict:` and `(trace: threads/main.h1.jsonl)`; that file exists and matches the stub's session file byte-for-byte; ledger rows show spawn (with agent `hand-cc`) → verdict.
  4. Test: dispatch `hand-cc` with `[sleep:60]` then kill → process exits promptly, `killed` ledger row, `[hand h1 killed]` transcript row, no stray `claude` stub process left (assert via the process group being gone).
  5. Test: dispatch `hand-cc` with `[sleep:60]` then steer → refusal comes back as the tool result, refusal ledger row present, then kill to clean up.
  6. Test: dispatch on an unknown agent and on `judge` (body `codex` in the run fixture, add it if the fixture lacks one) → error tool results, nothing spawned, no ledger spawn row.
  7. Live run (one real `claude -p` call, subscription-billed, ~cents): from the repo root, pipe an operator turn into `cargo run -p arx -- run` instructing main to dispatch `hand-ex` with a read-only brief such as "Read crates/arx/src/gate.rs and state how many public items it declares; that count is your verdict." Then inspect: `threads/main.jsonl` has the verdict row with the trace path; `threads/main.h<N>.jsonl` exists, is non-empty, and is Claude Code's session format; `grep -c 'store/arx.jsonl' threads/main.h<N>.jsonl` returns 0 (the hand never read the store); the ledger shows spawn → verdict. This step depends on main choosing to call the tool — if it doesn't, re-prompt more explicitly rather than faking the artifact.
- **Verify:** `cargo test -p arx` fully green; the live-run artifacts above inspected and reported in the verdict (paths plus the grep result).

### Scope 4 — card and map

- **Files:** Modify `docs/wayfinder/maps/arx-v0/L1-08-claude-code-hand-on-host.md`; `docs/wayfinder/maps/arx-v0/map.md` only if it records per-card landings (mirror however L1-07's closing was recorded).
- **Build:** the card reflects reality once every Verify above passes.
- **Steps:** flip `status: open` → `status: closed` in the card frontmatter. Do not commit anything.
- **Verify:** `git diff --stat` shows only files this brief names.

## Out of scope

- Container, clamp (tool allowlist, permission mode), credential injection off designated paths, fail-loud shortfall verdicts — L2-08.
- Codex hand body (L1-09), native hand (L1-10), judge as dispatch (L1-11).
- Store-derived grounding composition into briefs — L2-05. Here the brief text is whatever main wrote, verbatim.
- Mid-flight steering of a claude-code hand (`--input-format stream-json`), usage/window/nudge/reserve for CC hands, the real capture sweep (L3-03), gate wiring for hands (the bare hand runs under Claude Code's own defaults; read-only briefs pass headless; a brief needing write permissions will be refused inside the hand — acceptable until L2-08).

Done means: a dispatched claude-code hand completes a read-only brief and its verdict lands in main's window at a step boundary; the hand's raw transcript is on disk under the session before main sees the verdict; the hand never reads arx's store file.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-02-l1-08-claude-code-hand-on-host.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
