# L1-09 — codex hand on the host

Card: `docs/wayfinder/maps/arx-v0/L1-09-codex-hand-on-host.md`. Ruled by D16 (`e652ce3a`, `0bd4df2f`, `68c366dc`, `ceb0390f`), `docs/wayfinder/maps/arx-design/findings/codex-cli-drive-surfaces.md`, R02 (`docs/wayfinder/maps/arx-v0/findings/R02-codex-drive-gaps.md`) — if a step below lands against one of those, stop and report; do not bend the step or the ruling.

## Problem / Solution

L1-08 put the first real adapter behind the dispatch seam; the codex slots it left are dead ends — `Body::Codex` resolves to "not implemented until L1-09" (`main.rs:225`) and `BodyKind::Codex` hits `unimplemented!` (`ledger.rs:346`). This brief fills them: `codex exec --json` spawned as a one-shot process-group child on the host, base instructions replaced by arx's, AGENTS.md discovery suppressed, verdict read from the JSONL event stream, usage from `turn.completed` recorded on the verdict ledger row, rollout file copied under `threads/` as the trace. No container, no clamp, no credential isolation — L2-09. This same adapter is what the judge dispatches through at L1-11.

## Decisions locked

- `codex exec --json` is the codex hand drive surface: one-shot child, role-agnostic adapter, nothing to intercept because exec never asks (`e652ce3a`; findings §2). Drive surface follows from the role, never configured (`0bd4df2f`).
- Argv is fixed: `exec --json --model <spec.model> --skip-git-repo-check -c instructions="<ARX_HAND_INSTRUCTIONS>" -c project_doc_max_bytes=0`, brief piped to stdin verbatim. The git-repo guard is codex's own operator rail, redundant under arx's dispatch. No `--ephemeral` — the rollout file is the trace artifact. No path to `store/arx.jsonl` in any arg, env addition, or prompt.
- Base instructions replaced wholesale via `-c instructions=` (`68c366dc`) with a built-in single-line constant `ARX_HAND_INSTRUCTIONS`: `You are a hand dispatched by the arx harness. Execute the brief you receive as your prompt; your final message is your verdict — state it plainly.` The `instructions`/`repo_instructions` fields already on `RoleConfig` stay unwired for both bodies — wiring them is a later card, and wiring them for codex only would fork the bodies.
- AGENTS.md discovery off via `-c project_doc_max_bytes=0` — R02 §1 verified this is a true off-switch (no filesystem read attempted), per `ceb0390f`.
- Usage is read exactly once, from `turn.completed` (R02 §2: exec emits it once per process, at the end), and recorded verbatim on the verdict ledger row. Main's window line does not carry it. This is recording, not window management — no nudge, no reserve, `5f17e29e` stands.
- The trace artifact is codex's rollout `.jsonl`, located via the thread id from `thread.started` under `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-*<thread-id>.jsonl` (`CODEX_HOME` defaulting to `~/.codex`), copied to `threads/<thread>.<hand>.jsonl` before the verdict is delivered — same trace handle shape as L1-08, same L3-03 placeholder status.
- A running codex hand refuses steer exactly like a claude-code hand (one-shot child, stdin consumed): mechanical refusal as the tool result plus a `refused: true` ledger row; kill and re-dispatch is the recourse. Today `ledger.rs:391-397` matches `BodyKind::ClaudeCode` only and codex would silently fall through to accept-steer semantics — that branch must be extended, not left to the default.
- Dispatch on the reserved name `judge` returns an error naming L1-11. The adapter itself stays role-agnostic; the reservation lives in name resolution, so L1-11 can give the judge its schema without unwinding plain-hand semantics.
- Seam shape unchanged from L1-08's lock: body-kind enum matched at dispatch, one spawn function per body with the shared signature. The trait question waits for L1-10.
- Verdict comes from the JSON stream (last `agent_message` item), never from `--output-last-message`. A clean exit with no agent message is a failure verdict, not a silent empty one — same rule the codex review forced onto `build_verdict_text` in L1-08.
- The stdout reader parses the JSONL stream line-by-line into small state (thread id, last agent message, usage, last error) plus a bounded raw tail — it never retains the full stream, so the L1-08 OOM finding is designed out rather than patched in.

## Ground truth and tripwires

- Event and field names in the findings (`thread.started` / `item.completed` / `turn.completed`, usage's five token fields) are source-cited, not runtime-probed. Scope 1 opens with a live probe to pin the real shapes; its quota-blocked fallback is a shallow clone of `openai/codex` to read `codex-rs/exec/src/exec_events.rs`.
- Codex quota was exhausted earlier today (usage-limit error, reset ~19:25). If any live step hits it, STOP that step and report it blocked in the verdict — never substitute stub output for a live artifact.
- `-c key=value` TOML quoting semantics are unverified. Keep `ARX_HAND_INSTRUCTIONS` single-line; if the live trace lacks the instruction text, try TOML-quoted vs bare value and report which held.
- On this host's procps-ng 4.0.7, `ps -g <n>` selects by *session*, not process group. Assert process-group membership with `ps -o pid= -<pgid>` (bare negative PGID), as the L1-08 kill test does.
- `codex exec --help` must confirm: `--json`, `--model`, `-c/--config`, `--skip-git-repo-check`, and how the stdin prompt is taken (omitted prompt or `-` positional). Any missing → STOP and report the help output; do not substitute flags. Local binary was codex-cli 0.147.0 at R02 time.
- Model id for codex-bodied definitions is `gpt-5.6-sol` — owner-authored in `agents/judge.md`, the blessed value on this host. (Amended by the owner 2026-09-02: `gpt-5.4-codex` is superseded after a live 400 rejection — "The 'gpt-5.4-codex' model is not supported when using Codex with a ChatGPT account." The owner set `gpt-5.6-sol` on 2026-09-02.)

## Execution plan

### Scope 1 — the codex body

- **Files:**
  - Create `crates/arx/src/codex_hand.rs`
  - Modify `crates/arx/src/main.rs` (`mod codex_hand;`; `resolve_hand_spec` ~193–237)
  - Modify `crates/arx/src/ledger.rs` (dispatch match arm ~346, steer refusal ~391, `Delivery::Verdict`, verdict row ~462)
  - Modify `crates/arx/src/claude_hand.rs` (visibility of shared helpers only)
- **Build:** `spawn_codex_hand(spec, hand_id, brief, delivery_tx, control_rx)` — same signature as `spawn_claude_hand` — spawns `codex exec` as a process-group child, parses the event stream, copies the rollout, delivers the verdict with usage.
- **Steps:**
  1. Tripwire: `codex exec --help` per Ground truth above.
  2. Probe: in a fresh temp dir containing an `AGENTS.md` with the line `ARX-L1-09-AGENTSMD-CANARY`, run `codex exec --json --model gpt-5.4-codex --skip-git-repo-check "Reply with exactly: OK"` (no suppression flags), saving stdout. Pin the real event/field names from it and conform both the parser and the scope-2 stub to them; note any divergence from the findings in the verdict. Grep this run's rollout under `$CODEX_HOME/sessions/` for the canary — it must appear (this is the positive control that makes scope 3's absence check meaningful; if it does not appear, report that and treat the scope-3 canary grep as inconclusive rather than passing). Keep the temp dir until scope 3 cleanup. Quota-blocked → fall back to reading `exec_events.rs` from a shallow clone for shapes, and mark the positive control blocked.
  3. `codex_hand.rs`: `build_argv(spec)` per the locked argv (plus the stdin positional if help requires it); `Command::new("codex")`, cwd `spec.cwd`, `process_group(0)`, stdin/stdout/stderr piped, environment inherited; write the brief to stdin verbatim, close it.
  4. Stdout reader thread: line-by-line `serde_json` parse into `{thread_id, last_agent_text, usage, last_error}` plus a ~2000-byte raw tail for diagnostics; unparseable lines fold into the tail only. Stderr reuses `read_tail`. Make `claude_hand`'s `kill_and_reap`, `wait_for_exit`, `read_tail`, and `ExitOutcome` `pub(crate)` and reuse them rather than copying; no new shared module until L1-10 forces the shape.
  5. Exit loop mirrors the claude hand: poll `try_wait` against `control_rx`; `Control::Kill` or channel disconnect → SIGKILL the process group, reap, exit without a verdict (ledger already synthesized `Delivery::Killed`). Steer cannot arrive (refused host-side); log and drop if one does.
  6. Verdict: `turn.completed` seen and last agent message non-empty → that text. Otherwise failure text `codex exec failed — <exit status or turn.failed/error message>; stderr tail: <tail>` — the verdict still lands.
  7. Trace: glob `$CODEX_HOME/sessions/*/*/*/rollout-*<thread_id>.jsonl`, copy to `<threads_dir>/<thread>.<hand_id>.jsonl` before delivery; not found → append ` (capture failed: ...)` to the verdict text, `trace: None`.
  8. Extend `Delivery::Verdict` with `usage: Option<serde_json::Value>`; fake and claude-code paths pass `None`; the verdict ledger row gains a `usage` field (null when absent). `format_delivery` is unchanged — usage stays out of main's window.
  9. `main.rs`: `Body::Codex => BodyKind::Codex` (delete the L1-09 error string); before role lookup, `agent == "judge"` → error `'judge' is the reserved judge definition; judge dispatch lands at L1-11`. The native error is unchanged.
  10. `ledger.rs`: split the combined arm — `Codex => crate::codex_hand::spawn_codex_hand(...)`, `Native` keeps `unimplemented!("body not implemented until L1-10")`; extend the steer-refusal branch to `ClaudeCode | Codex` with body-appropriate text (`h1 is a codex hand mid-turn; steer is not deliverable — kill and re-dispatch`).
- **Verify:** `cargo build -p arx` clean; unit tests in `codex_hand.rs`: argv (both `-c` overrides, model, `--skip-git-repo-check`, no arg contains `store/`), verdict from a happy stream, clean exit with no agent message → failure path, `turn.failed` → failure path, usage captured from `turn.completed`, raw tail bounded. All existing ledger unit tests green.

### Scope 2 — hermetic e2e

- **Files:**
  - Create `crates/arx/tests/fixtures/bin/codex` (executable stub script)
  - Create `crates/arx/tests/fixtures/run/agents/hand-cx.md` (`body: codex`, `model: gpt-5.4-codex`, frontmatter shape of `hand-cc.md`)
  - Modify `crates/arx/tests/claude_main.rs`
- **Build:** the card's done-when as tests, with a stub `codex` on `PATH` and `CODEX_HOME` pointed at a per-test temp dir so no test touches the real `~/.codex`.
- **Steps:**
  1. Stub script, event shapes conformed to the scope-1 probe: write own pid (`$$` = pgid under `process_group(0)`) to `$CODEX_HOME/codex-stub.pid` and its full argv to `$CODEX_HOME/codex-stub-argv`; read the brief from stdin; honor a `[sleep:N]` directive; pick a thread id (`stub-$$-<epoch>`); write a two-line fake rollout to `$CODEX_HOME/sessions/2026/09/02/rollout-2026-09-02T00-00-00-<thread-id>.jsonl` (create dirs); emit the JSONL events — `thread.started` with the thread id, `item.completed` with an agent message `stub verdict: <first line of brief>`, `turn.completed` with a fixed usage object; exit 0.
  2. Tests follow the existing pattern: `PATH` prefixed with the fixture `bin/` dir, `CODEX_HOME` set per-test, `fresh_run_fixture()` copies.
  3. Test: dispatch `hand-cx` with a plain brief → verdict row in `main.jsonl` contains `stub verdict:` and `(trace: threads/main.h1.jsonl)`; that file byte-matches the stub's rollout; ledger shows spawn (agent `hand-cx`) → verdict whose `usage` matches the stub's numbers; the argv dump contains `--json`, `--model gpt-5.4-codex`, `-c project_doc_max_bytes=0`, the instructions override, and no entry containing `store/`.
  4. Test: dispatch `hand-cx` with `[sleep:60]` then kill → prompt exit, `killed` ledger row, `[hand h1 killed]` transcript row, process group empty before-and-after asserted via `ps -o pid= -<pgid>`.
  5. Test: dispatch `hand-cx` with `[sleep:60]` then steer → mechanical refusal as the tool result, `refused: true` ledger row, then kill to clean up.
  6. Update `dispatch_on_unknown_agent_and_on_a_codex_bodied_definition_are_refused_before_spawn` (`claude_main.rs:1242`): dispatching `judge` now asserts the L1-11 reservation error; the unknown-agent half is unchanged; rename the test to match what it now proves.
- **Verify:** `cargo test -p arx` fully green — every pre-existing test plus the new ones.

### Scope 3 — one live run

- **Files:** Create `agents/hand-cx.md` (`body: codex`, `model: gpt-5.4-codex`, one prose line, shape of `agents/hand-ex.md`).
- **Build:** the done-when exercised against the real codex CLI on the operator's login (subscription-billed, ~cents).
- **Steps:**
  1. If the scope-1 probe was quota-blocked, retry it now; still blocked → STOP, report this scope blocked.
  2. Create a repo-root `AGENTS.md` containing the line `ARX-L1-09-AGENTSMD-CANARY` (temporary — removed in step 5).
  3. From the repo root, pipe an operator turn into `cargo run -p arx -- run` instructing main to dispatch `hand-cx` with a read-only brief such as: `Read crates/arx/src/terminal.rs and state how many public items it declares; that count is your verdict.` If main doesn't call the tool, re-prompt more explicitly rather than faking the artifact.
  4. Inspect: `threads/main.jsonl` has the verdict row with `(trace: threads/main.h<N>.jsonl)`; that file exists, is non-empty, and is codex's rollout format; `grep -c ARX-L1-09-AGENTSMD-CANARY threads/main.h<N>.jsonl` → 0 (valid because the scope-1 positive control showed the canary does land when discovery is on); `grep -c 'store/arx.jsonl' threads/main.h<N>.jsonl` → 0; grep the trace for `hand dispatched by the arx harness` → ≥1 (instructions replacement landed; 0 → investigate `-c` quoting per Ground truth and report); the ledger verdict row carries `usage` with nonzero `input_tokens` and `output_tokens`.
  5. Delete the repo-root `AGENTS.md` and the scope-1 temp probe dir.
- **Verify:** the artifacts and grep results above inspected and reported in the verdict (paths plus counts).

### Scope 4 — card

- **Files:** Modify `docs/wayfinder/maps/arx-v0/L1-09-codex-hand-on-host.md`.
- **Build:** the card reflects reality once every Verify above passes.
- **Steps:** flip `status: open` → `status: closed` in the frontmatter. `map.md` stays untouched — L1-06/07/08 closings recorded nothing there. Do not commit anything.
- **Verify:** `git diff --stat` shows no tracked file beyond the pre-existing dirty baseline; every path this brief touches is untracked or named above.

## Out of scope

- Container, clamp (sandbox mode, approval policy, tool allowlists), credential injection — L2-09. The bare hand runs under codex's own defaults; read-only briefs pass.
- Native hand (L1-10); judge as dispatch, `--output-schema`, and the parse-with-explicit-failure wrapper R02 §3 demands for the judge's driver (L1-11).
- Store-derived grounding composition into briefs — L2-05. Brief text is whatever main wrote, verbatim.
- Wiring `RoleConfig.instructions` / `repo_instructions` for any body; the app-server drive surface (a codex-bodied main); mid-flight steer via `codex exec resume`; usage-driven window management, nudge, or reserve; the real capture sweep (L3-03).

Done means: a dispatched codex hand completes a read-only brief and its verdict lands in main's window at a step boundary; an AGENTS.md in the working directory is verifiably not read (with a positive control proving the check can fail); the `turn.completed` usage object is on the verdict ledger row; the rollout is on disk under `threads/` before main sees the verdict.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-02-l1-09-codex-hand-on-host.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
