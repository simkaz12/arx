# L2-03b — prompt layout and cache discipline, the build

Card: `docs/wayfinder/maps/arx-v0/L2-03b-prompt-layout-and-cache.md`, second of the L2-03 split. The layout, the standing prompt text and the owner's reactions are in `docs/specs/2026-09-04-l2-03-prompt-layout-draft.md` ("the draft" below); this brief builds that document and adds nothing to its design. Ruled by `bb81d2c6` (layout decided at build against the real prompt and tool surface), D16 `68c366dc` (arx's own system prompt replaces the body's preset) and `ceb0390f` (arx includes the repo instruction file itself so the log records what the model saw), `952d3b69` (reset is kill-and-respawn under a CLI body; the per-thread section changes only at a spawn), `a82c08c5` (recall appends at the tail), D02 `975f65cd`, and the owner's rulings of 2026-09-05 (`de2622ef` escalation is main's judgement; `f102a51e` minimal edits on the same footing as reads; `eabaa612` the standing prompt says only what the binary does, the roster is rendered from the definitions, and bounded is the tool's property). Branch `l2-03b-prompt-layout-and-cache`, cut from `main` at `cf649cc`.

Written 2026-09-05 with the owner present; the draft's "Decisions taken" and "Owner's reaction" sections were the grill. Calls made alone here are under "Chosen without a grill".

## Problem / Solution

The prompt today is one string: a one-sentence default, the repo instructions, the grounding and the quarantine listing, passed to the sidecar whole. Nothing in it is cacheable across sessions, the tool descriptions are one-liners, main has no idea which hands exist, and the two new tools from L2-03a have no line telling main when to use them.

This card lays the prompt out in the draft's four bands. The standing prompt lives in `prompts/main.md` and goes to the sidecar as the static block; the per-thread section (workspace line, repo instructions, grounding, quarantine listing, hand roster) is rendered at spawn as the dynamic block; the sidecar passes `[static, SYSTEM_PROMPT_DYNAMIC_BOUNDARY, dynamic]` so the static block and the tool schemas are the cross-session prefix. The ten descriptions become the one source of per-tool semantics. The system row records both blocks. The measurement runs on the owner's login from the log's existing usage rows. One gap carried from L2-03a closes: main's `read_file` gains a path subject at the gate.

## Decisions locked

- **`prompts/main.md`** holds the standing prompt: the fenced block under "Band two, the standing prompt" in the draft, verbatim, without the fence, ending in one newline. No front matter. `arx.toml` gains `instructions = "prompts/main.md"` under `[main]`. `DEFAULT_MAIN_INSTRUCTIONS` stays as the fallback when `instructions` is unset, its sentence unchanged. The file is not under `agents/` because `config::load` refuses any `agents/main.md` by name (`0bd4df2f`); that guard and its test are untouched.
- **Two blocks.** `compose_system_prompt` returns `SystemPrompt { static_block: String, dynamic: String }`. `static_block` is the instructions file's content exactly as read (or the default sentence). `dynamic` is the per-thread section rendered as below. The repo instructions move from the static string to the dynamic block.
- **The dynamic block**, parts joined by one blank line, empty parts omitted, in this order:
  1. `Workspace <config_dir>. Store scope <scope>.` with `<config_dir>` the absolute config directory as `cmd_run` resolves it. Always present.
  2. `Repo instructions (<path>):\n<file text>` when `[main] repo_instructions` is set; `<path>` as today.
  3. `Store grounding (scope <scope>), as of this window:\n` then one `- [<label>] <question> — <answer>` line per claim, as today's renderer writes them.
  4. `Quarantined this thread, as of this window:\n` then one `- [<kind>] <question>` line per candidate, as today.
  5. `Hands available:\n` then one line per hand in `resolved.hands`, in config order: `- <name>: <model>, <body>, tools <t1, t2>. <role line>` with `tools none` when the definition lists no tools, and the trailing `. <role line>` omitted when the definition has no body line. The judge is not listed.
  The trailing newline of each list part is trimmed before joining.
- **The role line** is the first non-empty line after the definition's closing `---` fence, trimmed, captured into a new `role_line: Option<String>` on `RoleRow` by a second read of the file past the fence. `RoleConfig` and its `deny_unknown_fields` are untouched; `check-config`'s table does not print it.
- **The wire.** `SpawnOpts.system_prompt: SystemPrompt`; the `init` op carries `"systemPrompt": {"static": <static_block>, "dynamic": <dynamic>}`; the sidecar imports `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` from `@anthropic-ai/claude-agent-sdk` and passes `systemPrompt: [op.systemPrompt.static, SYSTEM_PROMPT_DYNAMIC_BOUNDARY, op.systemPrompt.dynamic]`. The marker's value is the SDK's, never spelled in Rust.
- **The system row.** `append_system` takes the two blocks; `text` is `static_block + "\n\n" + dynamic` (the prompt as the model reads it, without the marker) and the row gains `boundary: <byte offset of dynamic within text>` as an additive `Option<usize>` field on `Line`, `skip_serializing_if = "Option::is_none"`, so every other row is byte-identical to today's.
- **The ten descriptions**, verbatim, replacing the one-liners (schemas unchanged; the two L2-03a descriptions unchanged):
  - `arx_ping`: `Probe that answers pong. Confirms the harness is listening; no other use.`
  - `save`: `Put a claim candidate into this thread's quarantine and get back the claims already in force beside it. kind is fact or decision; why says what was rejected or what the code does not show. Nothing reaches the store before drain.`
  - `recall`: `Search a scope's claims in force for a query and return the matches. The result lands here as a tool result and changes nothing above it.`
  - `decisions_query`: `Search a scope's decisions in force for a query and return the matches with their ids. Use it for what is normal here before deciding anew.`
  - `decision`: `Fetch one claim by id from a scope, in full.`
  - `drain`: `Promote every candidate in this thread's quarantine into the store, once, when the work is done.`
  - `dispatch`: `Start a hand from a named agent definition with a self-contained brief and return its id at once; nothing blocks. The hand sees the brief and nothing else: not this window, not the store, not the state object. Its verdict, stop or exit arrives later as a [hand …] line with a trace handle. body, if given, must match the definition's.`
  - `tap`: `Read a hand's state, its brief, and the question it is stopped on, if any.`
  - `steer`: `Answer a stopped hand, or push a message into a running native hand. A claude-code or codex hand cannot take a message mid-turn; kill it and dispatch again.`
  - `kill`: `Tear the hand down. Its exit and trace handle arrive as a [hand …] line.`
- **The gate gap.** `subject_field` gains `"mcp__arx__read_file" => Some("path")` beside L2-03a's `edit_file` line, so a rule `mcp__arx__read_file(pattern)` parses.
- **What is not rendered.** The draft's `State object at respawn:` part has no producer until L3-02; it is not rendered and no code path exists for it. The standing prompt therefore carries no respawn line (the draft's band two already omits it).
- **Nothing else moves.** `KINDS`, `Cargo.toml`, the gate's semantics, the hands' bodies, the checkpoint ritual and its prompts, the nudge, `compose_store_grounding`'s claim and candidate line formats.
- **Codex gate:** behaviour-changing, so the gate runs at end of work; the provider limit holds until 2026-09-07 08:38, so one attempt, `gate_run: no` with the reason, and the reviewer gates on brief conformance.

## Chosen without a grill

- **The row keeps one `text` plus an offset**, rather than two text fields. The two tests that read `system_row["text"]` and the readers that walk rows keep working; the offset makes the split recoverable. Rejected: `static`/`dynamic` fields on the row; logging the marker string.
- **The workspace line is always present**, so the dynamic block is never empty and the array always has three elements. Rejected: a two-element array on an empty dynamic block (a second shape for the sidecar to handle).
- **The judge is not in the roster.** Main dispatches hands; the judge is arx's. Rejected: listing every definition.
- **The measurement is a report, not an assertion that can fail the card.** The numbers on the owner's login are the card's done-when, but if the cache does not hit as the draft predicts, that is a finding about the CLI's breakpoints, to be reported with the raw rows, not something this card can fix. Rejected: a test that greps a live number.

## Ground truth

Verified 2026-09-05 by a read-only trace at `8387b13` and re-read after L2-03a at `cf649cc` (✅ read, quoted). Line numbers move; re-anchor by symbol.

- ✅ `crates/arx/src/main.rs`: `DEFAULT_MAIN_INSTRUCTIONS` (one sentence, unreferenced by any test's literal); `compose_system_prompt(resolved) -> Result<String, String>` reading `resolved.instructions` or the default and appending `"\n\nRepo instructions ({path}):\n{text}"`; `compose_store_grounding(store, scope, config_dir) -> Result<Option<String>, String>` rendering `"\n\nStore grounding (scope {scope}):\n"` with `- [{label}] {question} — {answer}` lines and `"Quarantined this thread:\n"` with `- [{kind}] {question}` lines; the call site in `cmd_run` that pushes the grounding onto the same string, then `log.append_system(&system_prompt)` and `ClaudeCodeMain::spawn(SpawnOpts { model, system_prompt, cwd })`; `mod tests` at the file's tail (the place for this card's unit tests).
- ✅ `crates/arx/src/claude_code.rs`: `SpawnOpts { model, system_prompt: String, cwd }`; the `init` op built as `json!({"op": "init", "model": …, "systemPrompt": opts.system_prompt, "cwd": …})`. No `#[cfg(test)]` module; `tests/claude_main.rs` `#[path]`-includes it — no inline tests here.
- ✅ `crates/arx/src/transcript.rs`: `Line { kind, turn, text, role, usage, tool, verdict }`; `append_system(text)` writes `{kind: "system", text}`; one call site.
- ✅ `crates/arx/src/config.rs`: `RoleConfig` with `deny_unknown_fields` and fields `body, model, provider, instructions, repo_instructions, nudge, checkpoint, steps, window, write, env, image, tools`; `split_frontmatter` returns the YAML slice and discards the body; `parse_frontmatter`; `RoleRow`; `Resolved { main, judge, hands, instructions, repo_instructions, sidecar, checkpoint, store_scope, store_embedder, providers, gate }`; the `agents/main.md` guard (`ConfigError::MainOverride`, locked by `tests/check_config.rs::main_override_is_rejected`).
- ✅ `crates/arx/src/gate.rs`: `subject_field` with `read_file → path`, `recall → query`, and L2-03a's `mcp__arx__edit_file → path`; `parse_checked_rule` rejecting a patterned rule without a subject.
- ✅ `sidecar/claude-main.mjs`: `import { query, createSdkMcpServer, tool } from '@anthropic-ai/claude-agent-sdk'` at the top; the twelve registrations with their one-line descriptions and zod schemas (`save`: scope, kind, question, answer, why; `recall`/`decisions_query`: scope, query; `decision`: scope, id; `drain`: none; `dispatch`: agent, body optional, brief; `tap`/`kill`: hand; `steer`: hand, message; `read_file`: path; `edit_file`: path, old, new); the `init` handler passing `systemPrompt: op.systemPrompt` into `query({ options })` with `settingSources: []`, `tools: []`; the `usage` event forwarding `message.usage` verbatim.
- ✅ `sidecar/node_modules/@anthropic-ai/claude-agent-sdk` 0.3.251: `sdk.d.ts` declares `systemPrompt?: string | string[] | { type: 'preset', … }` and documents `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` ("Blocks before the marker are eligible for cross-session prompt caching; blocks after it are not"); `sdk.mjs` exports it (`zee as SYSTEM_PROMPT_DYNAMIC_BOUNDARY`, value `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__`).
- ✅ Stub sidecar `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`: `init` writes `{ev: 'ready'}` and ignores `op.systemPrompt`, `op.model`, `op.cwd`; `currentUsage()` returns fixed cache fields unless `ARX_STUB_TOKENS_PER_TURN` is set.
- ✅ Run fixture `crates/arx/tests/fixtures/run/`: `arx.toml` with `[main] body = "claude-code", model = "stub-model"` and no `instructions`; `agents/` holds `hand-an`, `hand-cc`, `hand-cw`, `hand-cx`, `hand-ds`, `hand-ex`, `judge`, each with a one-line body after the fence (`hand-cc.md`: `Read-only doer for deep trace and live repro.`). The repo's own `agents/` has the same body lines.
- ✅ `crates/arx/tests/claude_main.rs` (63): `run_cli_streams_stub_reply_and_writes_transcript_rows` locks `rows.len() == 4` and `rows[0]["kind"] == "system"`; `grounding_lands_in_the_system_row` reads `system_row["text"]` as a string and checks substrings. No test asserts the default sentence, `Repo instructions`, `Quarantined this thread`, or any tool description. `tests/layer_close.rs` asserts grounding in the system row by substring through the same path.
- ✅ Log plumbing for the measurement: the sidecar forwards `message.usage` whole; `append_usage` writes it as the `usage` row; `transcript::occupancy` sums `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`; `arx gauge` reads the last row. Nothing to add.
- ✅ Baseline at `cf649cc`, cold: `cargo test --workspace` 399 passed, 1 ignored. `cargo test -p arx` 355: unittests 195, check_config 9, checkpoint 6, claude_in_container 4, claude_main 63, container_runtime 9, events 3, gate 4, hands_in_containers 5, layer_close 12, native_in_container 7, state_object 7, store 3, transcript_log 28; arx-engine 44. Count first; a difference is a stop, not an adjustment.
- ✅ Hazards: no `lib.rs`; `#[path]` includes double-register inline tests; `store/` and `pid.txt` tracked live state; `KINDS` seventeen and locked; additive fields touch every struct literal (`Line` literals in `transcript.rs`, `RoleRow` literals in `config.rs`'s and `layer_close.rs`'s test modules — count them, insert `boundary: None` / `role_line: None`, delete nothing); a stray container from a concurrent run and a rare transcript-lock flake under full parallelism are known.
- Trap: the fixture `arx.toml` gaining `instructions` changes the static block for every stub-driven test; assertions are substrings and row counts, so nothing should move — if a test does fail on the prompt text, STOP and report which.
- Trap: the descriptions name arguments; if a description names one the schema lacks, STOP and report rather than editing either.

## Execution plan

### Scope 1 — the file, the two blocks, the wire, the row, the roster, the descriptions

- **Files:** Create `prompts/main.md`. Modify `arx.toml` (`[main] instructions` only), `crates/arx/src/main.rs`, `crates/arx/src/claude_code.rs`, `crates/arx/src/config.rs`, `crates/arx/src/transcript.rs`, `crates/arx/src/gate.rs` (the one `subject_field` line), `sidecar/claude-main.mjs`. Do not modify `events.rs`, `ledger.rs`, `native_hand.rs`, `container_hand.rs`, `state_object.rs`, any test file (Scope 2 owns tests), the checkpoint prompts.
- **Build:** the four bands live in the adapter.
- **Steps:**
  1. `prompts/main.md` from the draft's band two; `arx.toml` `[main] instructions = "prompts/main.md"`.
  2. `config.rs`: `role_line` on `RoleRow`, captured after the fence; every `RoleRow` literal in the crate's test modules gains `role_line: None`.
  3. `main.rs`: `SystemPrompt`; `compose_system_prompt` producing both blocks with the dynamic block rendered per Decisions locked (fold today's grounding renderer into it or call it; the claim and candidate line formats stay byte-identical); `cmd_run` passing `SystemPrompt` to the log and the spawn; unit tests in `mod tests`: `dynamic_block_renders_parts_in_order_and_omits_empty_ones` (a `Resolved` with two hands, one without tools and one without a body line, no repo instructions, empty grounding: the block is the workspace line, a blank line, `Hands available:` and two lines in the locked forms) and `static_block_is_the_instructions_file_byte_for_byte`. Two tests.
  4. `claude_code.rs`: `SpawnOpts.system_prompt: SystemPrompt`; the `init` op's `systemPrompt` object.
  5. `transcript.rs`: `append_system(static_block, dynamic)`; `boundary` on `Line`; every `Line` literal gains `boundary: None`.
  6. `gate.rs`: the `mcp__arx__read_file` subject line.
  7. `sidecar/claude-main.mjs`: the import, the three-element array, the ten descriptions.
- **Verify:** `cargo build -p arx` with no new warnings; `cargo test -p arx` → unittests 197 (195 + 2), every other binary at baseline; `cargo run -p arx -- check-config ./arx.toml` byte-identical to `main`'s output (if the `instructions` key changes the table, quote both outputs and continue; that change is the card's); `grep -c "SYSTEM_PROMPT_DYNAMIC_BOUNDARY" sidecar/claude-main.mjs` → 2 (the import and the array).

### Scope 2 — the done-when as data, the live measurement, the card

- **Files:** Modify `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs` (the `init` handler writes the whole `op` as JSON to `<cwd>/stub-init.json`; nothing else), `crates/arx/tests/fixtures/run/arx.toml` (`[main] instructions = "prompts/main.md"`), Create `crates/arx/tests/fixtures/run/prompts/main.md` (`You are the fixture main.\n`), Modify `crates/arx/tests/claude_main.rs` (four added tests; every existing test unchanged), `crates/arx/tests/check_config.rs` (one added test), `docs/wayfinder/maps/arx-v0/L2-03b-prompt-layout-and-cache.md`. Do not modify `map.md`, `tests/layer_close.rs`, `tests/scripts/l1-close.json`, `tests/gate.rs`, `tests/transcript_log.rs`, `tests/events.rs`; `tests/common/mod.rs` additive only if a helper is needed (say so).
- **Build:** the hermetic done-when, the live numbers, the card closed.
- **Steps:**
  1. Stub: `stub-init.json`.
  2. `tests/claude_main.rs`, each on its own fixture copy: `the_prompt_reaches_the_sidecar_as_static_and_dynamic_blocks` (`stub-init.json`'s `systemPrompt.static` equals the fixture `prompts/main.md` byte for byte; `.dynamic` starts with `Workspace ` and contains `Store scope arx`, `Hands available:` and a `- hand-cc: claude-sonnet-5, claude-code, tools none. Read-only doer for deep trace and live repro.` line; the `system` row's `text` equals static + `\n\n` + dynamic and `boundary` equals `static.len() + 2`); `two_spawns_share_the_static_block_and_a_store_change_moves_only_the_dynamic` (run once, plant a hot claim through the engine as the store tests do, run again: static identical, dynamic differs and the second carries the claim's question); `repo_instructions_render_in_the_dynamic_block_only` (fixture copy with `[main] repo_instructions = "REPO.md"` and a planted file: the dynamic block carries `Repo instructions (` and the file's text; the static block does not); `a_gate_rule_denies_read_file_by_path_for_main` (`[gate] deny = ["mcp__arx__read_file(docs/*)"]`; `file:read_file:{"path":"docs/x.md"}` is denied at the gate, a `gate` row says so, no `file` row).
  3. `tests/check_config.rs`: `read_file_rule_is_admitted_for_main` (`[gate] deny = ["mcp__arx__read_file(store/*)"]` loads).
  4. Live, on the owner's login, real sidecar, in a scratch workspace set up as L2-13's and L2-03a's live steps were (`git init` plus an empty commit, `sidecar` absolute in the copied `arx.toml`, `prompts/main.md` copied, the repo's `agents/` copied): run A, two operator turns within five minutes (`Reply with the word one.` then `Reply with the word two.`); then, within five minutes of run A's last turn, run B with `[main] repo_instructions = "REPO.md"` added and a fifty-line `REPO.md` planted, one turn (`Reply with the word three.`). From each `threads/main.jsonl` quote every `usage` row verbatim and the `system` row's `boundary` and `text` length in bytes. Report: run A turn 1 `cache_creation_input_tokens`; run A turn 2 `cache_read_input_tokens` against turn 1's creation; run B turn 1 `cache_read_input_tokens` and `cache_creation_input_tokens`. The draft's predictions: A2 read ≥ A1 creation; B1 read > 0 and of the order of the static block plus the tool schemas, B1 creation of the order of the dynamic block. If the numbers contradict the predictions, do not adjust anything: report them and continue to step 5; if the login is unavailable, STOP the step and report it blocked. Sweep the scratch workspace after.
  5. Close the card: `status: closed`, `assignee: spec-runner`, a `## Shipped` section in the voice of the last cards carrying the live numbers.
- **Verify:** `cargo test -p arx` → unittests 197, claude_main 67 (63 + 4), check_config 10 (9 + 1), every other binary at baseline; `cargo test --workspace` 406 passed, 1 ignored; `docker ps -a --filter label=arx.hand` empty; `/tmp/arx-hands` empty; `git status --short` clean under `store/` and for `pid.txt`.

## Out of scope

- Reset, respawn and the orientation turn (L3-02); the `State object at respawn` part of the dynamic block.
- The first cut's prefix check or hash; a tokenizer; cache breakpoints of arx's own (the CLI places them).
- A Codex-bodied main's prompt.
- Any change to what `save`, `recall`, `dispatch` and the rest do; only their descriptions.

Done when Scope 2's Verify holds, the reviewer's must_fix is empty, and the map's frontier entry is written by main.

## Guardrails

Never commit, never `git add`, never stash, never switch branches, never rebase; read-only git only (`git status`, `git diff`, `git log`, `git show <ref>:<path>`). Never write under `store/`, to `pid.txt`, or to `~/.claude`; never print or quote a credential. No inline comments unless one already exists at the site. Clean up every container, hand root and scratch workspace you start.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-05-l2-03b-prompt-layout-and-cache.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
