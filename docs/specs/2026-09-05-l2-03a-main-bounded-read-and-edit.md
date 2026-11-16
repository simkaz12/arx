# L2-03a — main's bounded read and edit

Card: `docs/wayfinder/maps/arx-v0/L2-03a-main-bounded-read-and-edit.md`, first of the L2-03 split. Ruled by the owner's rulings of 2026-09-05 on the L2-03 draft: `de2622ef` (escalation between hand tiers is main's judgement, never the definition's; main may jump a tier), `f102a51e` (main may make minimal edits itself, on the same footing as bounded reads, because a small fix main can see whole costs less than re-initialising a hand), `eabaa612` (bounded is the read tool's property: it caps its return and reports truncation, and a truncated read is the signal to dispatch); by `f5b5caf4` (delegate discipline is taste in the prompt wherever main has edit tools; the structural line that survives is that children hold no store-write surface), `70260cad` (main's tool surface under a Claude Code body is arx-owned), `3a873e9a` (no store-write surface for children). Branch `l2-03a-main-bounded-read-and-edit`, cut from `main` at `8387b13`.

Written 2026-09-05 with the owner present; the rulings above were the grill. The calls main made alone are under "Chosen without a grill".

## Problem / Solution

Main has ten tools and cannot read a file or a trace. A hand's verdict is therefore a claim main cannot check, escalation has nothing to act on, and a one-line fix costs a hand's spawn, brief and verdict. The owner ruled that main may do a bounded read or a minimal edit itself and escalate on its own judgement.

This card gives main the two tools that make those rulings true: `read_file` and `edit_file`, registered in the sidecar beside the ten, served by arx on the host over the same request-and-reply wire the store and hand tools use, crossing the same gate before they run, and leaving one `file` row in main's transcript log per call. The read is bounded to the workspace and cut at a fixed cap with a trailer that says so. The edit is one exact-once replacement in one existing file and refuses arx's own state. The prompt lines that tell main when to use them ship with L2-03b, once the tools exist.

## Decisions locked

- Two MCP tools on main's surface, registered in `sidecar/claude-main.mjs` beside the ten. Schemas: `read_file { path: string }`; `edit_file { path: string, old: string, new: string }`; every field required. Descriptions verbatim:
  - `read_file`: `Read one UTF-8 text file under the workspace. path is relative to the workspace, or absolute inside it. Returns the content, cut at 16 KiB with a final line saying how much was cut; a cut read is the signal to dispatch a hand for the rest.`
  - `edit_file`: `Replace one exact occurrence of old with new in one existing file under the workspace. Fails when old is absent or matches more than once, and refuses arx's own state (store/, threads/, pid.txt). For a fix you can see whole; anything wider is a hand's brief.`
- The wire mirrors `store` and `hand`: the sidecar emits `{ev: "file", id, tool, args}` with `tool` one of `read_file` or `edit_file`, and awaits `{op: "file_result", id, result}`. In `claude_code.rs`, `FileCall { tool: String, args: serde_json::Value }` and `run_turn` gains `mut on_file: impl FnMut(&FileCall) -> serde_json::Value` beside `on_store` and `on_hand`; the `"file"` arm mirrors the `"store"` arm. The sidecar carries no path logic; a `fileToolResult` renders the result the way `storeToolResult` does.
- The result shape: `{"text": <string>}` on success, `{"error": <string>}` otherwise. The sidecar returns the string as the tool's text result in both cases.
- Bounding, in Rust: the root is `config_dir` canonicalised; the candidate is `config_dir.join(path)` canonicalised (an absolute path joins to itself). A candidate that does not start with the root: `read_file error: path escapes the workspace` (the same text with `edit_file` for the edit). A candidate that fails to canonicalise: `<tool> error: <path>: <io error>`.
- `edit_file` refuses, on the canonical path and before any read, anything under the root's `store/` or `threads/` and the root's `pid.txt`: `edit_file error: <rel path> is arx's own state`. `read_file` refuses nothing under the root.
- The cap: `const READ_CAP_BYTES: usize = 16 * 1024;` in `main.rs` beside the handler, not config. Content over the cap is cut at the last char boundary at or before the cap and followed by `\n[arx] truncated: <returned> of <size> bytes; the rest is a hand's job`. Content at or under the cap is returned whole, no trailer. A file that is not UTF-8: `read_file error: <rel path> is not UTF-8`.
- `edit_file`: read the file; `old` must occur exactly once (`content.matches(old).count() == 1`), else `edit_file error: old text matches <n> times`; write `content.replacen(old, new, 1)` with `fs::write`; result `{"text": "edited <rel path>"}`. No file creation, no whole-file write, no regex.
- The gate: both calls cross `canUseTool` like the ten, under whatever name form the ten reach the gate with (`mcp__arx__read_file`, `mcp__arx__edit_file`); find how `mcp__arx__recall` meets a `recall` rule today and admit the two tools the same way. `subject_field` in `config.rs` gains `edit_file → path`; `read_file → path` exists. No rule is added to the repo's `arx.toml`.
- The record: `TranscriptLog::append_file(text)` writes `{"kind": "file", "text": …}` (a `Line` with `kind` and `text` only, the shape of `append_system`), one row per call, written by the `on_file` closure after the handler returns. Texts: a read, `<rel path>: read <returned> bytes`, or `<rel path>: read <returned> of <size> bytes, truncated`; an edit, `<rel path>: edit\n--- old\n<old>\n+++ new\n<new>`; an error, `<rel path>: <the error text>` (`<path as given>` when there is no canonical path). `<rel path>` is the canonical path relative to the root. The existing `Event::Gate` row is untouched.
- Nothing else on main's surface changes: the ten descriptions, the standing prompt, the layout and the roster are L2-03b's.
- Codex gate: behaviour-changing, so the gate runs at end of work; the provider limit holds until 2026-09-07 08:38, so one attempt, `gate_run: no` with the reason, and the reviewer gates on brief conformance.

## Chosen without a grill

- **Rust executes, the sidecar relays.** The workspace rule has one implementation, in the process that owns the workspace. Rejected: bounding in JS (a second implementation of the rule in a file that is a wire).
- **16 KiB as the cap.** About four thousand tokens, "a few screens"; the cost of a wrong guess is one hand dispatch, cheap and visible in the log. Rejected: a line count (bytes are what the window pays for); a config knob before a first use sizes one.
- **Exact-once replace, no create.** A fix main can state in a sentence is one anchored replacement; creating files and multi-site changes are a brief. Rejected: a whole-file write; a regex.
- **A `file` row rather than an event kind.** `KINDS` is seventeen and locked by `tests/events.rs`; a transcript row is additive and lands beside the `system` and `turn` rows a reader already walks. Rejected: `Event::File`.
- **Refusing arx's own state in code, not only through the gate.** The store-write prohibition is structural for hands; an edit tool on main that could write `store/` would open the same door for main. Rejected: relying on a `[gate]` rule the operator may not write.

## Ground truth

Verified 2026-09-05 by a read-only trace at `8387b13` (✅ read by the trace, quoted here). Line numbers move; re-anchor by symbol.

- ✅ `sidecar/claude-main.mjs`: `import { query, createSdkMcpServer, tool } from '@anthropic-ai/claude-agent-sdk'` :2; `canUseTool` :13-25; `pendingStores` and `callStore(verb, args)` emitting `{ev: 'store', id, verb, args}` :28-34; `pendingHands` and `callHand` :43-49; the ten registrations :57-135 with handlers of the shape `async (args) => storeToolResult(await callStore('save', args))` :73 and `handToolResult(await callHand('dispatch', args))` :114; the op loop :227-253 handling `init`, `prompt`, `gate_verdict`, `store_result`, `hand_result`.
- ✅ `crates/arx/src/claude_code.rs`: `SpawnOpts { model, system_prompt: String, cwd }` :31-35; `run_turn(…, mut on_store: impl FnMut(&StoreCall) -> Value, mut on_hand: impl FnMut(&HandCall) -> Value, …)` :234-235; the `"gate"` arm :255-274; the `"store"` arm :276-288 (`on_store(&call)`, then `json!({"op": "store_result", "id": id, "result": result})`); the `"hand"` arm :289-300. No `#[cfg(test)]` module exists in this file, and `tests/claude_main.rs` `#[path]`-includes it (:16-19), so any test for this card lives in `main.rs`'s `mod tests` (:1926) or in `tests/claude_main.rs`, never inline here.
- ✅ `crates/arx/src/main.rs`: `run_turn_cycle` :815-974 with the inline `on_gate` closure (`decide()` :729-739; an `Ask` is resolved on the terminal by `read_operator` :852-953 before a verdict is written back, so the sidecar never sees `ask`); the `on_store` and `on_hand` closures passed to `run_turn` inside it (find by `on_store`); `Event::Gate` appended :836-841 and :961-966; `append_turn_as("main", …)` :1069; `mod tests` :1926. No row records a tool call's input or result today; the gate row carries tool and verdict only.
- ✅ `crates/arx/src/transcript.rs`: `Line { kind, turn, text, role, usage, tool, verdict }` :19-34; `append_system` :223-234 (the shape to copy); `append_usage` :236-247.
- ✅ `crates/arx/src/config.rs`: `subject_field` :67-78 (`read_file → path`, `recall → query`, no `edit_file`); `parse_checked_rule` :188-197 rejects a patterned rule whose tool has no subject with `<tool> has no subject — use the bare tool name`; `Resolved` :231-244.
- ✅ `crates/arx/src/gate.rs`: `Gate::decide(&mut self, call: &GateCall) -> GateVerdict` :46-48; `GateVerdict::{Allow, Deny { reason }, Ask}` :13-17; `Table::decide_with` :517-539, repo tier in full then user tier, default `Allow`. Main and hands share one `Table`; `set_context("main", &dir)` at main.rs :1347 changes hook payload only.
- ✅ `crates/arx/src/native_hand.rs`: `LocalHost::read_file` :65-76 canonicalises both sides and checks `starts_with`, error text `read_file error: path escapes the working directory`; the pattern to mirror. Hands' `read_file` is untouched by this card.
- ✅ Stub sidecar `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`: a prompt beginning `store:<verb>:<json>` crosses the gate as `mcp__arx__<verb>`, emits `{ev: 'store', id, verb, args}` and replies `stub-store:<json result>` :213-228; `hand:<verb>:<payload>` likewise :~120-147; `crossGate` :89-96 throws on a non-allow verdict; `store_result` and `hand_result` op handlers :62-73; `init` ignores `op.systemPrompt` :52-53. A `file:<tool>:<json>` trigger replying `stub-file:<json result>` is the mirror.
- ✅ `crates/arx/tests/claude_main.rs`: 57 tests; `run_cli_streams_stub_reply_and_writes_transcript_rows` :149-174 locks `rows.len() == 4` for a plain turn (a turn with no tool call must still write exactly four rows; a `file` row appears only on a file call); the run fixture and its per-test copy come from `tests/common/mod.rs`; `tests/gate.rs` shows how a test plants `[gate]` rules.
- ✅ Baseline at `8387b13`, run cold by the trace: `cargo test --workspace` 386 passed, 1 ignored. `cargo test -p arx` 342: unittests 189, check_config 8, checkpoint 6, claude_in_container 4, claude_main 57, container_runtime 9, events 3, gate 4, hands_in_containers 5, layer_close 12, native_in_container 7, state_object 7, store 3, transcript_log 28; arx-engine 44. Count these first; a difference is a stop, not an adjustment.
- ✅ Hazards: `crates/arx` has no `lib.rs`; `#[path]` includes double-register inline tests; `store/` and `pid.txt` are tracked live state; `KINDS` is seventeen and locked; a stray container from a concurrent run is a known flake (`docker ps -a --filter label=arx.hand`).
- Trap: `run_turn` gains a parameter. Every call site — in `main.rs`, and any in `tests/claude_main.rs` or `tests/layer_close.rs` that calls it directly — takes the new closure; count the call sites before editing and list them in the verdict (additive parameters touch every literal).
- Trap: the `file` row must not appear on a turn with no file call, or the four-row lock breaks.

## Execution plan

### Scope 1 — the wire, the handlers, the row

- **Files:** Modify `sidecar/claude-main.mjs`, `crates/arx/src/claude_code.rs`, `crates/arx/src/main.rs`, `crates/arx/src/transcript.rs`, `crates/arx/src/config.rs`. Do not modify `events.rs`, `native_hand.rs`, `container_hand.rs`, `ledger.rs`, `arx.toml`, any test file (Scope 2 owns tests). `gate.rs` only if the `mcp__arx__` name normalisation lives there and `edit_file` needs one line in it; say so in the verdict.
- **Build:** the two tools end to end, with unit tests for the handler.
- **Steps:**
  1. `sidecar/claude-main.mjs`: `pendingFiles` and `callFile(tool, args)` mirroring `callStore`; the `file_result` op handler mirroring `store_result`; two `tool(...)` registrations with the locked names, schemas and descriptions, handlers `async (args) => fileToolResult(await callFile('read_file', args))` and the same for `edit_file`; `fileToolResult` rendering `text` or `error` as the text result.
  2. `claude_code.rs`: `FileCall`; `run_turn`'s `on_file` parameter; the `"file"` arm writing `{"op": "file_result", "id", "result"}`.
  3. `main.rs`: `READ_CAP_BYTES`; `fn file_call(root: &Path, call: &FileCall) -> serde_json::Value` implementing Decisions locked (bounding, the refusal list, the cap and trailer, the exact-once edit, every error text); the `on_file` closure in `run_turn_cycle` calling it and then `log.append_file(...)` with the locked row texts; unit tests in `mod tests` on a `tempfile` workspace: `read_file_is_bounded_to_the_workspace`, `read_file_returns_a_small_file_whole`, `read_file_cuts_at_the_cap_and_says_so`, `edit_file_replaces_exactly_one_match`, `edit_file_refuses_zero_or_many_matches`, `edit_file_refuses_arx_state`. Six tests.
  4. `transcript.rs`: `append_file`.
  5. `config.rs`: `edit_file` in `subject_field`.
- **Verify:** `cargo build -p arx` with no new warnings; `cargo test -p arx` → unittests 195 (189 + 6), every other binary at baseline; `cargo run -p arx -- check-config ./arx.toml` byte-identical to `main`'s output.

### Scope 2 — the done-when as data, the stub, the live check, the card

- **Files:** Modify `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs` (the `file:` trigger only), `crates/arx/tests/claude_main.rs` (six added tests; every existing test unchanged), `crates/arx/tests/check_config.rs` (one added test), `docs/wayfinder/maps/arx-v0/L2-03a-main-bounded-read-and-edit.md`. Do not modify `map.md`, `tests/layer_close.rs`, `tests/scripts/l1-close.json`, `tests/gate.rs`, `tests/common/mod.rs` unless a helper must be added (additive only; say so).
- **Build:** the done-when as tests, the live check, the card closed.
- **Steps:**
  1. Stub: a prompt beginning `file:<tool>:<json>` crosses the gate as `mcp__arx__<tool>`, emits `{ev: 'file', id, tool, args}`, awaits `file_result`, and replies `stub-file:<json result>` with the usual `delta`, `turn`, `usage`, `turn_end` rows, exactly as the `store:` branch does.
  2. `tests/claude_main.rs`, each on its own copy of the run fixture: `main_reads_a_workspace_file_and_the_log_records_it` (prompt `file:read_file:{"path":"arx.toml"}`; the reply carries the file's first line; a `file` row `arx.toml: read <n> bytes` with `<n>` the file's size); `main_read_is_cut_at_the_cap` (plant a 40 KiB UTF-8 file; the reply ends with `[arx] truncated: <returned> of 40960 bytes; the rest is a hand's job`; the row says `truncated`); `main_read_outside_the_workspace_is_refused` (`/etc/hostname` and `../` both answer `read_file error: path escapes the workspace`; the row carries it); `main_edits_a_workspace_file_exactly_once` (plant `NOTE.md` with `alpha beta alpha`; edit `beta` → `gamma`; file reads `alpha gamma alpha`; the row carries `--- old\nbeta\n+++ new\ngamma`; a second edit of `alpha` answers `edit_file error: old text matches 2 times` and the file is unchanged); `main_edit_of_arx_state_is_refused` (`store/x.md` answers `edit_file error: store/x.md is arx's own state`; nothing is written); `a_gate_rule_denies_edit_file_by_path` (the fixture copy's `arx.toml` gains `[gate] deny = ["edit_file(docs/*)"]` the way `tests/gate.rs` plants rules; the edit of `docs/x.md` is denied at the gate, the `gate` event row says so, no `file` row is written, the file is unchanged).
  3. `tests/check_config.rs`: `edit_file_rule_is_admitted` — a config with `[gate] deny = ["edit_file(store/*)"]` loads, and `edit_file` without a pattern still loads.
  4. Live, on the owner's login, real sidecar, in a scratch workspace set up as L2-13's live step did (the L2-13 card's `## Shipped` section and the map's landed entry record the recipe: `git init` plus an empty commit, `sidecar` as an absolute path in the copied `arx.toml`): plant `NOTE.md` containing `model: sonnet`; one `arx run` with the operator turn `Use read_file on arx.toml and quote its first line. Then use edit_file on NOTE.md to change sonnet to SONNET. Reply with the first line and the word done.`; assert the reply, the two `file` rows in the scratch `threads/main.jsonl`, and `NOTE.md` changed. If the login is unavailable, STOP the step and report it blocked.
  5. Close the card: `status: closed`, `assignee: spec-runner`, a `## Shipped` section in the voice of the last cards.
- **Verify:** `cargo test -p arx` → unittests 195, claude_main 63 (57 + 6), check_config 9 (8 + 1), every other binary at baseline; `cargo test --workspace` 399 passed, 1 ignored; `docker ps -a --filter label=arx.hand` empty; `/tmp/arx-hands` empty; `git status --short` clean under `store/` and for `pid.txt`.

## Out of scope

- The size rule and the escalation rule in the prompt, the standing prompt file, the four-band layout, the ten descriptions, the roster, the system row's two blocks: L2-03b.
- A `write_file` or create tool; a diff crate; a config knob for the cap; a regex edit.
- Hands' `read_file` and the container path; the gate's own semantics.
- Reset and respawn (L3-02).

Done when Scope 2's Verify holds, the reviewer's must_fix is empty, and the map's frontier entry is written by main.

## Guardrails

Never commit, never `git add`, never stash, never switch branches, never rebase; read-only git only (`git status`, `git diff`, `git log`, `git show <ref>:<path>`). Never write under `store/`, to `pid.txt`, or to `~/.claude`; never print or quote a credential. No inline comments unless one already exists at the site. Clean up every container and hand root you start.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-05-l2-03a-main-bounded-read-and-edit.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
