# L2-06a — the state object and its mechanical check

Card: `docs/wayfinder/maps/arx-v0/L2-06a-state-object-and-check.md`, cut from L2-06 on 2026-09-04. Ruled by D15 (`18b4928b` — header-level provenance: checkpoint ordinal, timestamp, `(thread, turn)` handle; the object is authored fresh each time and the prior one is on disk, never in context; per-entry provenance is only what the mechanical check needs, path plus sha and decision id; `18fec68a` — the mechanical check is file exists, decision id resolves, path still carries that sha, harness-side on every state object write, blocking on failure; `eff1f01a` — files-only persistence, the transcript log is the address space for every handle and the harness mints them mechanically, on every checkpoint and in the state object's header; `b03f77fc` — the check covers paths, shas and decision ids and nothing else, the falsehood residual is accepted), D16 (`952d3b69` — the reset apparatus binds every body), D09 (`25664aa5` — the checkpoint token delta is config, its default set from the first real sizes, so no default is chosen here). Branch `l2-06a-state-object-and-check`, cut from `main` at `83afa6b` (L2-08 merged).

Written 2026-09-04 without a grill, on the owner's overnight rule. Every call main made alone is listed under "Chosen without a grill" so the morning read starts there.

## Problem / Solution

Nothing in the crate knows what a state object is. `grep -rni "state object\|state_object\|StateObject\|checkpoint" crates/` finds the reserved `Event::Checkpoint { check, delta }` shape and its rendering arm, and nothing else: no file format, no parser, no hash, no check, no config key. L2-06b's ritual has nothing to write until these exist.

This card is the artifact and its guard, with no loop change. A state object is one markdown file per checkpoint under `threads/<thread>.checkpoints/<ordinal>.md`: YAML front matter arx writes (`thread`, `checkpoint`, `at`, `turn`) and a body main authors in four `##` sections — Position, Target, Drift, Hands in flight — whose provenance entries are bullet lines of two shapes, `- file: <path> @ <hash>` and `- decision: <id>`, with hands listed as `- <hand id> <agent>`. The hash is git's blob id for the file, computed by `git hash-object`, so the crate takes no dependency and the value matches what `git ls-files -s` shows for a committed file. One module, `state_object.rs`, holds the types, the parser, the renderer, the hash, the check and a checked write that produces a file only when every probe passes: the file exists, the path still carries that hash, the decision id resolves through `Store::decision`. The check returns every failure, each naming its probe and its entry, so an authoring round can fix them all at once. `[main] checkpoint` joins the config as the token delta L2-06b will clock on, parsed and validated now, used by nothing yet.

## Decisions locked

- One new module, `crates/arx/src/state_object.rs`, importing only `std`, `serde`, `serde_json`, `serde_yaml` and `arx_engine` — never `crate::` — so `crates/arx/tests/state_object.rs` can `#[path]`-include it. No inline test module (the include hazard). `main.rs` gains `mod state_object;` and nothing else in this card.
- Location: `threads/<thread>.checkpoints/<ordinal>.md`, ordinal a positive integer without padding, the directory created on first write. `pub fn checkpoints_dir(threads_dir: &Path, thread: &str) -> PathBuf`; `pub fn latest(dir: &Path) -> io::Result<Option<(u64, PathBuf)>>` returns the highest ordinal by parsing file stems, ignoring anything that is not `<u64>.md`.
- Header, YAML front matter in this field order: `thread` (string), `checkpoint` (u64, the ordinal), `at` (RFC 3339 UTC, seconds), `turn` (u64, the handle's turn on `thread`). Types: `pub struct Header { pub thread: String, pub checkpoint: u64, pub at: String, pub turn: u64 }`, `pub struct StateObject { pub header: Header, pub body: String }`. `pub fn render(obj: &StateObject) -> String` writes `---\n<yaml>---\n<body>`; `pub fn parse(text: &str) -> Result<StateObject, String>` reads it back; `render` then `parse` is the identity on the header and the body text.
- Body grammar, all under `##` headings and all optional to the parser but required by L2-06b's prompt: `## Position`, `## Target`, `## Drift`, `## Hands in flight`. Provenance entries anywhere in the body are bullet lines matching exactly `- file: <path> @ <40 hex>` and `- decision: <id>`; a hands entry under `## Hands in flight` is `- <id> <rest of line>` with `<id>` matching `^h[0-9]+$`. `pub struct Entries { pub files: Vec<(String, String)>, pub decisions: Vec<String>, pub hands: Vec<String> }` and `pub fn entries(body: &str) -> Entries`. Anything else is prose and ignored by the check.
- Hash: `pub fn hash_file(path: &Path) -> Result<String, String>` runs `git hash-object <path>` (no repository needed; the command hashes any readable file) and returns the 40-hex blob id; `Err` carries git's stderr or the io error. Paths in entries are resolved against a root the caller passes, `config_dir` in practice.
- The check: `pub enum Probe { FileExists, FileHash, DecisionResolves }`, `pub struct CheckFailure { pub probe: Probe, pub entry: String, pub detail: String }`, `pub fn check(body: &str, root: &Path, store: &mut dyn Store, scope: &str) -> Result<(), Vec<CheckFailure>>` — runs every probe on every entry and returns all failures in body order (a missing file reports `FileExists` only, not also `FileHash`). `DecisionResolves` calls `store.decision(scope, id)`; `Ok(None)` is a failure, `Err(e)` is a failure whose `detail` is the error's Display. `pub fn write_checked(dir: &Path, obj: &StateObject, root: &Path, store: &mut dyn Store, scope: &str) -> Result<PathBuf, Vec<CheckFailure>>` runs `check` and writes `dir/<checkpoint>.md` only on `Ok`, creating `dir`; an existing file at that ordinal is refused as a `CheckFailure` with `probe: FileExists`, `entry` the target path and `detail` `already exists`, so an ordinal is never overwritten.
- Config: `RoleConfig` gains `checkpoint: Option<u64>` (additive; `deny_unknown_fields` stays; every literal takes `checkpoint: None`, zero deletions), `RoleRow` and `Resolved` carry it, validation refuses `0` with `main.checkpoint must be positive` in the same place `main.nudge must be positive` is raised. Absent means off. No default value is chosen (`25664aa5`). `check-config` output is byte-identical.
- The handle-resolves proof lives in `crates/arx/tests/transcript_log.rs`, which already includes `transcript.rs`; it gains `#[path = "../src/state_object.rs"] mod state_object;` and one test, so its count goes 26 → 27, stated in the verdict. No other locked count moves.
- Codex gate: behaviour-changing diff, the gate runs. The provider limit is in force until 2026-09-07 08:38: one attempt, `gate_run: no` with the reason, and the reviewer gates on brief conformance, as the last cards did.

## Chosen without a grill

- **Markdown with YAML front matter, not JSON.** The object is written for a main who "was there five minutes ago and lost their notes" (CONTEXT.md); a model authors it as prose with a few typed bullets, and the trace index and claim files already use the same front-matter shape. Rejected: a JSON schema the model must emit verbatim (fragile under a CLI body); a free-text file with no grammar (nothing to check).
- **`git hash-object` as the sha.** The ruling says "path still carries that sha"; git's blob id is a real SHA-1, matches what the repository already records for committed files, and costs no crate. Rejected: `sha2` (a dependency for one call); a non-cryptographic hash in std (not a sha, and no tool prints it).
- **Every failure returned, not the first.** The check is what L2-06b sends back for the second authoring round; one round should fix everything the check can see. Rejected: fail-fast.
- **Ordinal never overwritten.** `18b4928b` puts the prior object on disk as history; a rewrite would silently erase it. Rejected: overwrite on re-run.
- **Two entry shapes only.** `b03f77fc` limits the check to paths, shas and decision ids; hands are listed for the reader, not checked (the ledger is the truth for them and L2-06b supplies the list). Rejected: checking hand ids against the ledger (a second reader of ledger state with nothing to add).
- **The config key lands here, unused.** L2-06b then changes no schema, and `check-config`'s byte-identity is proven once. Rejected: leaving the key to L2-06b.
- **The `turn` in the header is the authoring reply's turn.** `eff1f01a` says the handle is minted at the checkpoint; L2-06b will pass the handle `append_turn_as("main", …)` returns for the reply it takes as the object. This card only stores and resolves it. Rejected: the operator's or arx's prompt turn.

## Ground truth

Verified by a read-only trace on 2026-09-04 against the tree at L2-07's merge (`aa75c86`), before L2-08's edits (✅ read by the trace, quoted here). Line numbers move; re-anchor by symbol.

- ✅ `Event::Checkpoint { check: String, delta: u64 }` at `events.rs:69-89` among six reserved kinds; doc comment names L2-06 as its emitter; `KINDS` 17. `event_fields` in `main.rs` (about :1306) renders it. No other hit for `checkpoint` in `crates/`.
- ✅ No `state object`, `StateObject`, `sha`, `reserve` in `crates/arx/src` (greps quoted in the trace).
- ✅ `transcript.rs`: `Handle { thread: String, turn: u64 }` :13-17; `TranscriptLog { thread, path, file, next_turn }` :138-143; `open(root, thread)` :146-183 with `File::try_lock` :161; `last_handle` :185-194; `append_turn_as(role, text) -> Handle` :196-262 region; free functions `occupancy` :282-287, `last_usage` :314-333, `resolve(root, &Handle) -> Result<String, TranscriptError>` :335-365, `events` :367-393, `kind_counts` :395-410. `recover_next_turn` :104-135. `arx_engine::ClaimHandle { thread, turn }` at `crates/arx-engine/src/lib.rs:44` is a separate type; main converts by field copy.
- ✅ `arx_engine::Store::decision(&mut self, scope: &str, id: &str) -> Result<Option<Claim>, StoreError>` at `lib.rs:188`; engine impl `engine/mod.rs:826-828`. `Engine::open(root, thread)` git-inits the root; scopes are directories under it, never auto-created; a claim is minted by `save_with` then `drain()` (the L2-05 fixture helper `plant_engine_scope` in `tests/common/mod.rs` does exactly this and returns the ids).
- ✅ Config: `RoleConfig { body, model, provider, instructions, repo_instructions, nudge: Option<u64>, write, env, image }` (`config.rs:35-46`, `deny_unknown_fields`; L2-08 adds `tools`); `RoleRow` :198-210; `Resolved` :212-224; `DEFAULT_NUDGE = 120_000` :234; `main.nudge must be positive` raised in `check_role` (search the string). `check-config` prints six columns.
- ✅ `threads/` layout: `threads/<thread>.jsonl` (transcript), `threads/<thread>.<hand>/` (container hand trace), `threads/<thread>.<hand>.jsonl` (codex host trace). `threads/` is gitignored.
- ✅ Baseline at `83afa6b` (L2-08 merged): 305 passed, 1 ignored (arx 261: unittests 140, check_config 8, claude_in_container 4, claude_main 49, container_runtime 8, events 3, gate 3, hands_in_containers 5, layer_close 12, store 3, transcript_log 26; arx-engine 44). `RoleConfig` now also carries `tools: Vec<String>` (L2-08); the trace's line numbers for `config.rs` predate it.
- ✅ `serde_yaml` is already a dependency of `crates/arx` (`Cargo.toml`), used by `parse_frontmatter`.
- ✅ Hazards: `crates/arx` has no `lib.rs`; `#[path]`-included files carry no inline tests; `transcript.rs`'s 16 inline tests already count in both `arx` unittests and `transcript_log`. `store/` and `pid.txt` are tracked live state. `git hash-object` on this host: `git version` ≥ 2.x, present.

## Execution plan

### Scope 1 — the module, the key, the tests

- **Files:** Create `crates/arx/src/state_object.rs`, `crates/arx/tests/state_object.rs`. Modify `crates/arx/src/main.rs` (`mod state_object;` and `RoleRow`/`Resolved` plumbing if any literal lives there), `crates/arx/src/config.rs`, `crates/arx/tests/transcript_log.rs` (one include line, one test). Do not modify `events.rs`, `transcript.rs`, `ledger.rs`, the sidecar, any fixture, `arx.toml`, `agents/*.md`.
- **Build:** everything under Decisions locked.
- **Steps:**
  1. `config.rs`: the `checkpoint` key, validation, plumbing; two unit tests in the existing module: `main_checkpoint_parses` and `main_checkpoint_zero_is_refused`; every `RoleConfig`/`RoleRow` literal takes `checkpoint: None`.
  2. `state_object.rs` per Decisions locked. `render` writes the header with `serde_yaml` in the fixed field order (a struct with fields declared in that order serialises in that order). `parse` splits on the first two `---` lines, deserialises the header, keeps the body verbatim. `entries` scans line by line with the three literal shapes; a `- file:` line whose hash is not 40 lowercase hex is not an entry.
  3. `crates/arx/tests/state_object.rs` with `#[path = "../src/state_object.rs"] mod state_object;`, using `tempfile`, `arx_engine::{Engine, Store, SaveIntent, ClaimHandle}`. Tests: `render_then_parse_is_identity` (header fields and body byte-equal); `entries_pick_files_decisions_and_hands_and_skip_prose` (two files, one decision, two hands, a bullet that is prose, a `- file:` with a short hash); `hash_file_matches_git` (write a temp file, compare with `git hash-object` run by the test); `check_passes_when_every_probe_holds` (temp root with two files hashed by `hash_file`; scratch engine at a temp root with `store/x` scope, one claim minted by `save_with` + `drain()`; body references both files and the claim id → `Ok(())`); `check_reports_every_failure_by_probe_and_entry` (one missing file → `FileExists`; one file rewritten after hashing → `FileHash`; one unknown decision id → `DecisionResolves`; all three in one body → three failures in body order with the entry text); `write_checked_writes_only_on_pass_and_never_overwrites` (a failing body leaves no file; a passing body writes `1.md`; a second passing write at ordinal 1 is refused with `already exists` and the file is unchanged); `latest_picks_the_highest_ordinal` (files `1.md`, `3.md`, `2.md`, `notes.txt` → `Some((3, …))`; empty dir → `None`).
  4. `tests/transcript_log.rs`: `a_state_object_header_handle_resolves_in_the_transcript_log` — open a log, append three turns as `operator`/`main`/`main`, take the handle of the second, build a `Header { thread, checkpoint: 1, at: "2026-09-04T00:00:00Z", turn: handle.turn }`, `render` + `parse`, then `transcript::resolve(root, &Handle { thread, turn: parsed.header.turn })` equals the second turn's text.
- **Verify:**
  - `cargo test -p arx` → unittests: the L2-08 number plus 2; check_config 8; state_object 7; transcript_log 27; every other binary unchanged from the L2-08 merge. State each.
  - `cargo run -p arx -- check-config ./arx.toml` stdout byte-identical to `main`; a scratch config with `[main] checkpoint = 50000` passes; with `checkpoint = 0` fails with `main.checkpoint must be positive`.
  - `cargo test --workspace` total stated.

### Scope 2 — the card

- **Files:** Modify `docs/wayfinder/maps/arx-v0/L2-06a-state-object-and-check.md`. Do not modify `map.md`.
- **Steps:** Close the card: `status: closed`, `assignee: spec-runner`, a `## Shipped` section in the voice of the last cards: the file's location and header, the body grammar and the two entry shapes, the hash, the check's three probes and its all-failures return, the checked write and the no-overwrite rule, the config key, and any deviation.
- **Verify:** `git status --short` shows no change under `store/` or to `pid.txt`; the workspace total from Scope 1 restated.

## Out of scope

- The clock, the operator command, the authoring and truth-check turns, the `Checkpoint` event's emitter, hands in flight in the prompt (L2-06b).
- Reset, respawn, orientation, rewind (L3-02).
- A default token delta (`25664aa5`).
- Checking hand ids against the ledger; any probe beyond the three ruled.
- An `arx checkpoints` listing command; `check-config` columns.

Done when Scope 2's Verify holds, the reviewer's must_fix is empty, and the map's frontier entry is written by main.

## Guardrails

- Never commit, never rebase, never stash, never switch branches, never `git add`. Read-only git toolkit on this repository: `git status`, `git diff`, `git log`, `git show <ref>:<path>`; `git worktree add` only for the scratch build of `main` (remove it after). Git writes inside temp directories the tests create are fine; `git hash-object` reads only.
- `store/` and `pid.txt` at the repo root are tracked live state: never write there, never stage them.
- No inline comments. Unslop applies to the card's Shipped paragraph and the verdict.
- If a locked count cannot hold as written, STOP on that scope and report the exact conflict with evidence rather than bending the count.

## Owner items (morning read)

- The two calls most worth a veto: `git hash-object` as the sha, and markdown with a bullet grammar as the object's shape.
- No default token delta is chosen; `[main] checkpoint` is off until you set it. L2-06b's first real objects are what size it (`25664aa5`).

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-04-l2-06a-state-object-and-check.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
