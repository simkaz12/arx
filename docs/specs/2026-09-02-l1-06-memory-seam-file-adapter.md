# L1-06 — the memory seam with a file adapter

Card: `docs/wayfinder/maps/arx-v0/L1-06-memory-seam-file-adapter.md`. Ruled by D12 (`e79bed85`), D01 (`40ebd8d3`), `a82c08c5`, D16 (`e652ce3a`), D13 (`c775b5c2`, `da0b0d8b`), the claim schema `7be68c4a`, handle minting `eff1f01a`. A scope that lands against any of them stops and reopens the ruling in the store first.

## Problem / Solution

Main has a gate and a transcript but no memory: nothing grounds it at spawn, and no verb on its surface reaches a store. The design ruled a store interface on main's mounted surface — save, recall, decisions_query, decision, drain — with the save lifecycle (landscape → quarantine → drain) carried by the trait from the start, so L2-04's engine swaps in behind an unchanged signature and nothing at the call sites moves.

This card fixes the seam and ships the thin adapter: a `Store` trait in `crates/arx-engine` (whose lib.rs doc comment already names this card), a `FileStore` over a flat JSONL file per scope, grounding pushed into main's window at spawn, the five verbs mounted as sidecar tools crossing a new `store`/`store_result` protocol exchange, and a `NullStore` stub proving the call sites are trait-shaped. Retrieval is substring match; promotion is mechanical; the real engine is L2-04/L2-05.

## Decisions locked

- The trait lives in `crates/arx-engine` and carries the ruled save shape from the start (card; `da0b0d8b`): save takes the intent, returns a landscape (the N nearest in-force same-scope claims, content included), and lands the candidate in a per-thread quarantine file — never the store file. The landscape is not a gate: the store refuses nothing and bounces nothing.
- A claim is the four-field schema `7be68c4a`: `question` (a future condition, not the current task), `answer`, `why` (for a decision, the rejected alternative — `"none"` refused), `handle`. Record plumbing (`id`, `kind`, an optional `label`) rides beside the four; no fifth content field. Derivability is checked at the drain by a hand with repo access — that is L3-03; this card's drain promotes mechanically and checks nothing.
- The handle is minted by the harness, mechanically, at save time (`eff1f01a`): cmd_run supplies the most recent committed turn row's `(thread, turn)` to the save call. The adapter never invents a handle — a seam invariant in the L1-05 mold (the caller writes the gate line; here the caller mints the handle).
- Grounding is pushed into main's window at spawn (`e79bed85`, `a82c08c5`) as a section appended to the system prompt: the scope file's `hot` and `ember` labeled entries, plus this thread's staged quarantine candidates (`da0b0d8b`: every main in the thread sees what earlier mains staged). Pushed only when non-empty; a config with no `[store]` table pushes nothing, so existing sessions stay byte-identical.
- recall, decisions_query and decision are ordinary tools whose results append at the tail (`a82c08c5`); results are inlined whole — the memory seam is exempt from admission (`40ebd8d3`). Admission itself does not exist yet (L3-01); the exemption is recorded here so nothing later wraps these returns.
- The scope is explicit on every verb that takes one (save/recall/decisions_query/decision); drain takes none and drains this thread's quarantine into each candidate's own scope file, main minting (card; `3a873e9a`). Grounding's scope comes from config: a new optional `[store]` table in `arx.toml` with one key, `scope`. `check-config` output is unchanged; the table is not a role.
- Wire shape follows the gate precedent: sidecar tool handlers emit `{ev:'store', id, verb, args}` and await `{op:'store_result', id, result}`, correlated by id from the existing readline listener; `run_turn` grows `on_store: impl FnMut(&StoreCall) -> serde_json::Value` beside `on_delta`/`on_gate`, so `claude_code.rs` carries the protocol exchange and no store knowledge. The store verbs are tools, so every call also crosses the gate (`canUseTool`) and gets its gate row for free — no new transcript rows in this card.
- This store is arx's own; Claude Code stays on Tabularium v1 and never sees it (`c775b5c2`). Nothing here touches Tabularium.
- No retrieval beyond substring match, no lineage, no tiers beyond the label (card). Matcher, pinned: case-insensitive; query tokens are whitespace-split words of length ≥ 3 (for save's landscape, tokens come from the intent's question + answer); a claim matches when its question, answer or why contains any token; rank by distinct-token hits, ties in file order; landscape N = 5, recall/decisions_query default limit 5.
- File discipline is the transcript's: append-only, one `write_all` of the fully serialized row plus newline, then flush. Store root is `<config-dir>/store/`, a sibling of `threads/`: scope files `store/<scope>.jsonl`, quarantine `store/quarantine/<thread>/<seq>-<id>.json`, one file per candidate. Scope names validate like thread names. The store never opens `threads/main.jsonl` — the transcript's writer lock stays single-owner.
- A store write failure inside the `on_store` closure fails the session by the L1-05 `gate_err` pattern (record, kill via the `ManuallyDrop` clone, answer the tool with an error result).
- Sibling card L1-07 also grows `run_turn` a callback and the sidecar a tool set. The steps here are additive: if L1-07 landed first, add the `store` arm and `on_store` parameter beside its `hand` counterparts rather than assuming their absence.
- Piped-mode stdout stays byte-identical for a session whose config has no `[store]` table and that makes no store call. Pre-existing test assertions change only by threading the new `on_store` argument through direct `run_turn` callers.
- `terminal.rs`, `transcript.rs`, `gate.rs`, `check-config` behavior, `gauge` untouched. Store results never render at the terminal — they are tool results inside the body's turn.
- No commits, no git writes. Prior state is inspected read-only: `git status` / `diff` / `log`, and `git show <ref>:<path>` to read a file at a ref.

## Execution plan

### Scope 1 — the trait and the file adapter

- **Files:** Modify `crates/arx-engine/src/lib.rs`, `crates/arx-engine/Cargo.toml` (add `serde` with derive, `serde_json`).
- **Build:** The seam's types, the `Store` trait, the `FileStore` adapter, the `serve` dispatcher — nothing wired.
- **Steps:**
  1. Types: `ClaimKind { Fact, Decision }` (kebab-case wire form), `ClaimHandle { thread: String, turn: u64 }`, `SaveIntent { scope, kind, question, answer, why }`, `Claim { id, kind, label: Option<String>, question, answer, why, handle: ClaimHandle }`, `DrainReport { promoted: usize }`, and a `StoreError` enum in `TranscriptError`'s mold (`Io { path, source }`, `BadScope { scope }`, `MalformedRow { path, line }`, `WhyRefused`).
  2. The seam:
     ```rust
     pub trait Store {
         fn save(&mut self, intent: &SaveIntent, handle: &ClaimHandle) -> Result<Vec<Claim>, StoreError>;
         fn recall(&mut self, scope: &str, query: &str, limit: usize) -> Result<Vec<Claim>, StoreError>;
         fn decisions_query(&mut self, scope: &str, query: &str, limit: usize) -> Result<Vec<Claim>, StoreError>;
         fn decision(&mut self, scope: &str, id: &str) -> Result<Option<Claim>, StoreError>;
         fn drain(&mut self) -> Result<DrainReport, StoreError>;
         fn grounding(&mut self, scope: &str) -> Result<Vec<Claim>, StoreError>;
     }
     ```
  3. `FileStore::open(root: &Path, thread: &str)` creating `store/` and `store/quarantine/<thread>/` as needed. save validates (`why` non-empty; a decision's `why` never `"none"`, case-insensitive → `WhyRefused`), mints the id (`<unix-millis-hex>-<in-process-seq>`), writes the candidate JSON file carrying intent + handle + id, computes the landscape from the scope file, and never writes the scope file. drain reads the thread's quarantine files in `<seq>` order, appends each as a row to its scope file, deletes each candidate file only after its row is flushed, returns the count. grounding filters the scope file to `label` `hot`/`ember`. Scope-file row shape, exact: `{"id":...,"kind":"fact","label":"hot","question":...,"answer":...,"why":...,"handle":{"thread":"main","turn":3}}`, label omitted when absent.
  4. `pub fn serve(store: &mut dyn Store, verb: &str, args: &serde_json::Value, handle: &ClaimHandle) -> serde_json::Value` — parses args, dispatches to the trait, returns the result serialized (claims whole), or `{"error": <display>}` on any parse or store error. This is the one call site main wires; the stub adapter proves it in Scope 3.
  5. Unit tests in-module: `save_returns_landscape_and_lands_in_quarantine` (planted scope file; landscape contains the token-overlapping claim; candidate file exists with handle and id; scope file bytes unchanged), `drain_promotes_in_order_and_clears_quarantine`, `grounding_returns_hot_and_ember_only`, `recall_matches_substring_case_insensitive_ranked`, `decision_why_none_is_refused`, `serve_unknown_verb_returns_error`.
- **Verify:** Count first: `cargo test -p arx` baseline is 84 (35 unit, 6 check_config, 17 claude_main, 26 transcript_log). If it is not 84, STOP and report — unless the delta is exactly L1-07's landed additions, recorded in the verdict. Then `cargo test -p arx-engine` — the six new tests green.

### Scope 2 — the crossing, wired end to end

- **Files:** Modify `sidecar/claude-main.mjs`, `sidecar/package.json` (only if zod must be added), `crates/arx/src/claude_code.rs`, `crates/arx/src/main.rs`, `crates/arx/src/config.rs`, `arx.toml` (repo root), `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`.
- **Build:** Five tools mounted; the `store`/`store_result` exchange; grounding into the system prompt; `[store]` config.
- **Steps:**
  1. `config.rs`: optional top-level `[store]` table with one key `scope: String`; surfaced on `Resolved` as `store_scope: Option<String>`. All existing fixtures lack the table and must load unchanged; `check-config` output unchanged.
  2. Sidecar: add `save`, `recall`, `decisions_query`, `decision`, `drain` to the existing `arx` server's tool list beside `arx_ping` (which stays). Parameter schemas via zod — import from the SDK's own dependency tree; TRIPWIRE: if `zod` is not importable, `npm install zod` in `sidecar/` and record the dependency in the verdict. Each handler assigns `id = nextStoreId++`, emits `{ev:'store', id, verb, args}`, awaits its `{op:'store_result', id, result}` through a `pendingStores` map resolved from the existing readline listener (the `gate_verdict` pattern), and returns the result JSON as the tool's text content.
  3. `claude_code.rs`: `run_turn` takes `on_store: impl FnMut(&StoreCall) -> serde_json::Value` (define `StoreCall { verb: String, args: serde_json::Value }` in arx-engine or beside `GateCall` — put it in arx-engine so the sidecar exchange types sit with `serve`). New `"store"` match arm: parse id/verb/args, call `on_store`, write `{op:'store_result', id, result}` re-taking the stdin lock, as the gate arm does. No store knowledge in this file.
  4. `main.rs` `cmd_run`: open `FileStore` at `<config-dir>/store` for thread `main` before the loop. Track the most recent committed turn `Handle` in a variable updated at every `append_turn`/`append_turn_as` call site; the `on_store` closure converts it to a `ClaimHandle` and calls `arx_engine::serve`. A `StoreError::Io` surfacing from serve inside the closure follows the `gate_err` pattern (record, kill via a dedicated `ManuallyDrop` clone, return `{"error":"store write failed"}`); `serve`'s ordinary error JSON just returns to the tool.
  5. `main.rs` grounding: when `store_scope` is `Some`, read `store.grounding(scope)` and the thread's quarantine listing; if either is non-empty, append to the system prompt, after the repo-instructions section:
     ```
     Store grounding (scope <scope>):
     - [hot] <question> — <answer>
     Quarantined this thread:
     - [fact] <question>
     ```
     Empty grounding and empty quarantine append nothing.
  6. Root `arx.toml`: add `[store]` with `scope = "arx"`.
  7. Stub sidecar: a prompt starting `store:` is `store:<verb>:<json-args>` — emit one `{ev:'store', id, verb, args}`, await its result, then the normal delta/turn/usage/turn_end with turn text `stub-store:<result-json>` so tests read the round-trip. Existing `hang`/`die`/`tool:`/plain triggers unchanged.
  8. Thread the new `on_store` argument through existing direct `run_turn` callers in tests with a closure returning `{"error":"no store"}`; assertions unchanged.
- **Verify:** `cargo test -p arx` — pre-existing tests green with no edits beyond argument threading. `printf 'reply with exactly the word turquoise\n' | cargo run -q -p arx -- run` prints turquoise, exits 0 — grounding from the root config's empty store adds nothing, stdout byte-identical.

### Scope 3 — the done-when, proven

- **Files:** Modify `crates/arx/tests/claude_main.rs`.
- **Build:** The done-when's five checks as integration tests plus live checks.
- **Steps:**
  1. Test setup helper: a tempdir workspace — `arx.toml` with `[main]` copied from the run fixture, `sidecar` pointing at the stub-sidecar fixture by absolute path, `[store] scope = "testscope"`, and a planted `store/testscope.jsonl` (one `hot`-labeled claim, one unlabeled). Hermetic; nothing writes into the fixture tree.
  2. `grounding_lands_in_the_system_row` — run a plain prompt; assert the transcript's `{"kind":"system"}` row contains the hot claim's question and answer and not the unlabeled claim's.
  3. `recall_returns_file_hits_as_tool_result` — prompt `store:recall:{"scope":"testscope","query":"<planted token>","limit":5}`; assert the main turn row's text carries the planted claim's answer (the stub echoed the result).
  4. `save_quarantines_and_leaves_the_store_file_untouched` — prompt `store:save:{...}` with a question sharing a token with the planted claim; assert the result echo contains the planted claim (the landscape), exactly one file in `store/quarantine/main/` carrying `"thread":"main"` and a turn number, and the scope file's bytes unchanged.
  5. `drain_promotes_into_the_store_file` — save then `store:drain:{}` across two prompts; assert the scope file gained exactly the candidate row, the quarantine dir is empty, and the result echo says `"promoted":1`.
  6. `null_store_compiles_against_the_trait_at_the_same_call_site` — a `NullStore` (empty returns) defined in the test, driven through `arx_engine::serve` with the same verbs; asserts empty-shape results. This is the done-when's second-adapter proof: same trait, same call site, no arx edits.
  7. Live checks, real body, root config, output quoted in the verdict:
     - Plant one row labeled `hot` in `store/arx.jsonl` (create the file; note it in the verdict — the store starts empty by ruling, this is a probe row the owner can delete). `printf 'what does your store grounding say?\n' | cargo run -q -p arx -- run` → the reply names the planted content.
     - `printf 'call the save tool with scope arx, kind fact, question ..., answer ..., why ...\n' | ...` → reply describes a landscape (or its emptiness), `store/quarantine/main/` gains one candidate, `store/arx.jsonl` unchanged.
     - `printf 'call the drain tool, then tell me what it returned\n' | ...` → reply mentions promoted 1, the row is in `store/arx.jsonl`, quarantine empty.
- **Verify:** `cargo test -p arx` and `cargo test -p arx-engine` — full suite green, the five new integration tests included. All three live commands with the quoted expected shapes.

## Out of scope

- Real retrieval (BM25, `896b62ad`), lineage and supersession, dedup, tiers beyond the label — L2-04/L2-05 behind this trait.
- The doneness drain by a dispatched hand, derivability checking, capture sweep — L3-03 (`da0b0d8b`'s open item stays open).
- Admission (L3-01); this card only records the store's exemption.
- Store git sync (`095a930e`), the state object, checkpoint.
- Tabularium v1 and its MCP surface — untouched, permanently (`c775b5c2`).
- `arx_ping` removal; hand-side grounding (a hand's brief text, `e79bed85`) — no hands exist until L1-07/L1-08.

Done when the three scopes' Verifies pass: a claude-code main starts with grounding visible in its first turn, a mid-thread recall returns file hits as a tool result, a save returns a landscape and lands in quarantine with the thread handle leaving the store file untouched, a drain promotes it, and `NullStore` compiles against the trait with no change at main's call sites.

## Review handoff

dispatch spec-runner on docs/specs/2026-09-02-l1-06-memory-seam-file-adapter.md (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
