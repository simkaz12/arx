# L1-05 — the gate seam, allow-and-log

Card: `docs/wayfinder/maps/arx-v0/L1-05-gate-seam-allow-and-log.md`. Ruled by D06 (`ce46b9ea`, `86875715`, `799b0967`), D16 (`70260cad`, `96d1ad3a`, `705e4df2`). A scope that lands against any of them stops and reopens the ruling in the store first.

## Problem / Solution

Main's tool surface is already stripped — the sidecar has run `disallowedTools: ['*']` with `settingSources: []` since L1-03 — but nothing crosses arx, because nothing is mounted and no callback exists. The moment a tool becomes callable, it would execute on the body's say-so alone. The design ruled one pre-dispatch gate returning allow / deny / ask, with every call crossing arx before it executes.

This card fixes the seam and ships its first adapter: a `Gate` trait deciding a `GateCall` into a `GateVerdict`, an `AllowAll` adapter, every crossing written to the transcript by the seam's caller, and the crossing wired end to end — the SDK's `canUseTool` in the sidecar raises a gate event over the protocol, `run_turn` answers with the verdict mid-turn. One probe tool (`arx_ping`) is mounted as the first real member of the arx-owned surface so a live session drives a real crossing. Deny and ask are constructible variants serializing to the PreToolUse hook shape; L2-02's rules adapter lands behind the same trait.

## Decisions locked

- Main gets schema-level stripping plus one pre-dispatch gate returning allow / deny / ask (`ce46b9ea`). This card's posture is allow-everything; rules and the deny/ask adapter are L2-02.
- The gate speaks Claude Code's PreToolUse hook contract (`799b0967`): `GateVerdict` serializes to the permission-decision fields — `{"permissionDecision":"allow"}`; deny adds `permissionDecisionReason`. The rule table ships empty; no rules config in this card.
- The tool surface is harness-owned configuration, a set of names built at spawn with no method that adds to it (`86875715`). `arx_ping` is mounted by the sidecar as arx's tool, always on; it stands in until real verbs (dispatch / tap / steer / kill, save, drain) arrive with later cards.
- Adapters narrow the body's richer answers and never widen them (`70260cad`). The wire carries allow|deny only; `Ask` never crosses — it is resolved arx-side before a reply (`96d1ad3a`, ask suspends for the party above, which is L2-02's machinery). In this card `run_turn` forwards `Allow` as allow and maps anything else to deny with the reason — fail-safe until the suspend exists.
- Grill rulings this session: the seam is a `Gate` trait (`decide(&GateCall) -> GateVerdict`) with `run_turn` growing an `on_gate` callback beside `on_delta`, so `claude_code.rs` carries the protocol exchange and no gate knowledge. The gate line is written by the seam's caller in `cmd_run`, never the adapter, so "every crossing is logged" is a seam invariant every future adapter inherits. The gate row is `{"kind":"gate","tool":...,"verdict":...}` — tool and verdict only, no input (inputs can be huge; richer capture is L2-01), no turn linkage. Wire shape: `{ev:'gate', id, tool, input}` / `{op:'gate_verdict', id, verdict, reason?}`, id echoed for correlation.
- The transcript is arx-written and widened in place, never moved (L1-02/L1-03 precedent): `append_gate` is a new dedicated method with the same single-buffer `write_all` discipline; existing row shapes and tests untouched.
- Piped-mode stdout stays byte-identical for a session that makes no tool call. The pre-existing `claude_main.rs` tests run with assertions unchanged; the only permitted edit to them is threading the new `on_gate` argument through direct `run_turn` callers.
- The sidecar protocol keeps its existing shape: `init`/`prompt`/`shutdown` and `ready`/`delta`/`turn`/`usage`/`turn_end`/`fatal` untouched; `gate`/`gate_verdict` is the protocol's first mid-turn exchange. Both sides are ready for it: the sidecar's readline listener stays live during a turn, and `run_turn`'s stdin lock is free after the prompt write.
- A gate line that cannot be written fails the session — the call is never allowed silently. Mechanism is the runner's choice (the L1-04 `chunk_err` pattern fits); the behavior is pinned.
- `terminal.rs`, `config.rs`, `check-config`, `gauge` untouched. Gate lines never render at the terminal.

## Execution plan

### Scope 1 — the gate seam's types

- **Files:** Create `crates/arx/src/gate.rs`. Modify `crates/arx/src/main.rs` (add `mod gate;` only, in this scope). Modify `crates/arx/src/transcript.rs` (`append_gate`).
- **Build:** The seam's types, the `AllowAll` adapter, the transcript row — nothing wired yet.
- **Steps:**
  1. Define the call and verdict:
     ```rust
     pub struct GateCall { pub tool: String, pub input: serde_json::Value }
     pub enum GateVerdict { Allow, Deny { reason: String }, Ask }
     ```
     Serde serialization to the PreToolUse permission-decision shape: `Allow` → `{"permissionDecision":"allow"}`; `Deny{reason}` → `{"permissionDecision":"deny","permissionDecisionReason":<reason>}`; `Ask` → `{"permissionDecision":"ask"}`.
  2. Define the seam: `pub trait Gate { fn decide(&mut self, call: &GateCall) -> GateVerdict }` and `pub struct AllowAll` returning `Allow` for every call.
  3. `transcript.rs`: add optional `tool` and `verdict` fields to `Line` (skipped when absent, existing rows byte-identical) and `pub fn append_gate(&mut self, tool: &str, verdict: &str)` writing `{"kind":"gate","tool":...,"verdict":...}`.
  4. Unit tests in-module:
     - gate: `verdict_serializes_to_pre_tool_use_json` — all three variants' exact JSON asserted.
     - gate: `a_caller_matches_all_three_verdicts` — constructs `Allow`, `Deny{..}`, `Ask` and matches over each; the done-when's compile-against-all-three proof.
     - gate: `allow_all_allows_any_call`.
     - transcript: `append_gate_writes_tool_and_verdict` — exact row bytes; a turn row appended after it still numbers correctly.
- **Verify:** `cargo test -p arx` — the four new unit tests green; all pre-existing green and unchanged. Count first: baseline is 75 (31 unit, 6 check_config, 13 claude_main, 25 transcript_log). If it is not 75, STOP and report rather than adjusting expectations.

### Scope 2 — the crossing, wired end to end

- **Files:** Modify `sidecar/claude-main.mjs`, `crates/arx/src/claude_code.rs`, `crates/arx/src/main.rs`, `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`.
- **Build:** `canUseTool` → gate event → `on_gate` → verdict reply; `arx_ping` mounted; `cmd_run` logging every crossing.
- **Steps:**
  1. Sidecar: mount `arx_ping` — `createSdkMcpServer` (in-process, `@anthropic-ai/claude-agent-sdk`) with one tool `arx_ping` (no input, returns `"pong"`), added to the query options' `mcpServers`. Add `canUseTool`: assign an incrementing id, emit `{ev:'gate', id, tool, input}`, await the matching `{op:'gate_verdict', id, ...}` via a pending map resolved from the existing readline listener; map allow → `{behavior:'allow', updatedInput: input}`, deny → `{behavior:'deny', message: reason}`.
  2. TRIPWIRE: if `disallowedTools: ['*']` swallows the mounted tool (Scope 3's live probe can't reach `arx_ping`), switch the stripping to a form that keeps every built-in out while admitting the mounted server's tools (named disallows, or `allowedTools` naming the mcp tool) and report the change in the verdict. The stripped-built-in live check still gates either way.
  3. `claude_code.rs`: `run_turn` takes `on_gate: impl FnMut(&GateCall) -> GateVerdict` beside `on_delta`. New match arm for `"gate"`: parse id/tool/input, call `on_gate`, write `{op:'gate_verdict', id, verdict, reason?}` to the child's stdin (re-take the stdin lock). `Allow` forwards as allow; any other verdict goes as deny with its reason (`Ask` → deny, reason `"ask not yet supported"`).
  4. `main.rs` `cmd_run`: construct `AllowAll` before the loop; the `on_gate` closure decides, then `append_gate`s the crossing before returning the verdict — the crossing is logged before the call executes. `RefCell` or split borrows for transcript access shared with the post-turn appends is fine. A failed gate append fails the session before any turn append, per the pinned behavior.
  5. Stub sidecar: a prompt whose text starts with `tool:` names comma-separated tools; for each, emit `{ev:'gate', id, tool: <name>, input: {}}` and await its `{op:'gate_verdict', id}`; after all verdicts, emit the normal delta/turn/usage/turn_end sequence with the usual `stub:` reply. The `hang`/`die`/plain triggers are unchanged.
  6. Thread the new `on_gate` argument through existing direct `run_turn` callers in tests with a pass-through allow closure; assertions unchanged.
- **Verify:** `cargo test -p arx --test claude_main` — all pre-existing tests green with no edits beyond the argument threading. `printf 'reply with exactly the word turquoise\n' | cargo run -q -p arx -- run` against the root `arx.toml` prints turquoise and exits 0.

### Scope 3 — the done-when, proven

- **Files:** Modify `crates/arx/tests/claude_main.rs`.
- **Build:** The N-crossings check as a regression test, plus the two live checks.
- **Steps:**
  1. New integration test `n_tool_calls_produce_n_gate_lines`: stub-sidecar harness, one prompt `tool:alpha,tool:beta,tool:gamma`, run to completion, then read the workspace's `threads/main.jsonl` and assert exactly three `{"kind":"gate",...}` rows for the run — tools `alpha`, `beta`, `gamma` in order, every verdict `"allow"` — with the turn row landing after them.
  2. Live checks, run and their output quoted in the verdict:
     - Probe, real body: `printf 'call the arx_ping tool and tell me exactly what it returned\n' | cargo run -q -p arx -- run` → reply mentions pong, exit 0, and `threads/main.jsonl` gains a `{"kind":"gate","tool":<name containing arx_ping>,"verdict":"allow"}` row (the wire name carries the mcp server prefix; assert on the substring).
     - Stripped built-in, real body: `rm -f /tmp/arx-l1-05-proof` then `printf 'use the Bash tool to create the file /tmp/arx-l1-05-proof, then reply done\n' | cargo run -q -p arx -- run` → exit 0, `/tmp/arx-l1-05-proof` does not exist afterward, and no gate row naming Bash appears for that turn — the built-in is schema-stripped, so the call never reaches the gate, let alone executes.
- **Verify:** `cargo test -p arx` — full suite green, the new integration test included. Both live commands above with the quoted expected shapes.

## Out of scope

- Rules, the deny/ask adapter, the config tiers, ask's suspend/resume — L2-02 behind the same trait (`96d1ad3a`).
- The real mounted verbs (dispatch / tap / steer / kill with L1-07; save and drain later); `arx_ping` stands in and may be dropped when they land.
- Timeout or hang handling for an unanswered gate event; hand-side gating and containers; instrumentation events beyond the gate row (L2-01).
- `terminal.rs`, `config.rs`, `check-config`, `gauge`; the sidecar protocol's existing ops and events.

Done when the three scopes' Verifies pass: the suite green with the four new unit tests and the N-crossings integration test, pre-existing claude_main assertions untouched, and both live checks — the probe call gated allow and logged, the stripped built-in never executing.

## Review handoff

dispatch spec-runner on docs/specs/2026-09-02-l1-05-gate-seam-allow-and-log.md (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
