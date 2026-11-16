# L1-03 — claude-code main adapter

Card: `docs/wayfinder/maps/arx-v0/L1-03-claude-code-main-adapter.md`. The card is this brief's whole world.

**Ruled by:** D16 `705e4df2` (arx hosts main under every body), `e652ce3a` (one adapter per CLI kind; a Claude-Code-bodied main drives over the Agent SDK), `c474a7f4` (the compaction lever is the auto-compact window ceiling, superseding `e4510547`'s nonexistent boolean), `7b607c7e` (arx writes the transcript log from the body's stream; the body's session file is never read), `68c366dc` (arx's system prompt replaces the body's preset), `ceb0390f` (arx suppresses repo-instruction discovery and pushes the file itself), `96ce60c0` (a CC-bodied model may fill main), `890ebff2` (the host inversion is retired), `b8d59164` (the SDK sidecar affirmed over the http-hook route, borrowing t3code's embedding; hook route is the recorded fallback), R01 (`docs/wayfinder/maps/arx-v0/findings/R01-claude-code-drive.md`). If an edit below lands against one of these, STOP — the ruling reopens in the store first; do not bend the brief to fit.

## Problem / Solution

L1-01 shipped config, L1-02 shipped the log; no body runs yet. This card ships the claude-code adapter in the main role: arx spawns a Node sidecar it owns, the sidecar drives the `claude` binary through `@anthropic-ai/claude-agent-sdk` `query()` (t3code's production embedding — `arx-design/findings/t3code-claude-embedding.md`), and arx talks to the sidecar over a JSONL stdio protocol. arx replaces the body's system prompt, suppresses CLAUDE.md discovery, pushes the repo instruction file itself when config says so, strips every built-in tool, sets the auto-compact window ceiling, writes every turn into the L1-02 transcript log, reads usage off the stream into `arx gauge`, and can kill the child. L1-04 (terminal), L1-05 (gate), L1-06 (memory), L1-07 (dispatch) all hang off this adapter and none of them is here.

## Decisions locked

- arx hosts main; the Claude Code child is driven, never a host. (`705e4df2`, `890ebff2`)
- Drive surface for a CC-bodied main is the Agent SDK, via a sidecar process arx spawns and supervises; the arx-owned `PreToolUse` http hook is the declined fallback, not an option here. (`e652ce3a`, `b8d59164`)
- arx's own system prompt replaces the preset (`systemPrompt` string option); one author, recorded in the log. (`68c366dc`)
- CLAUDE.md discovery is off (`settingSources: []`); when config names a repo instruction file, arx reads it and appends it to the prompt it pushes. (`ceb0390f`, R01 switch (e))
- Built-ins are stripped: `disallowedTools: ["*"]`, no MCP mounted at this card, so no permission flow exists and no `canUseTool` is wired — the gate is L1-05's card. (`70260cad`, `e652ce3a`)
- The compaction lever is `CLAUDE_CODE_AUTO_COMPACT_WINDOW` on the child's environment, set to the 1M ceiling — arx has no reserve yet (L3-02), so the body's threshold goes as high as it can. (`c474a7f4`)
- arx writes the transcript log from the stream; the body's session file on disk is never opened by arx — and the body's own session persistence stays on precisely so the never-opened check is non-vacuous. (`7b607c7e`)
- How the gauge computes occupancy is build-settled, not ruled: at this card `arx gauge` displays the body's own usage numbers verbatim plus a labeled sum, no local estimate. (`5fadfcc0`)
- `TranscriptLog` enforces a single writer per thread file across processes, not just within one: `open` takes an exclusive advisory lock on the thread file and holds it for the handle's lifetime; a second concurrent writing open fails fast, naming the locked path, rather than blocking, corrupting the log, or silently proceeding. Reads (`resolve`, `last_usage`) are not locked — `arx gauge` reads live while a run holds the writer lock.
- Brief-fixed, not rulings (cheap to change later): the sidecar is plain Node ESM at `sidecar/claude-main.mjs` with `@anthropic-ai/claude-agent-sdk` pinned exactly `0.3.251` (R01's checked version); the stdio protocol is JSON lines — ops in (`init`, `prompt`, `shutdown`), events out (`ready`, `delta`, `turn`, `usage`, `turn_end`, `fatal`) — sidecar stdout is protocol-only, stderr inherited; main's thread id is `main`; the log root is `<config dir>/threads`; turn rows gain an optional `role` field (`operator` | `main`), the pushed system prompt lands as a `kind:"system"` row, usage as a `kind:"usage"` row — all skipped by L1-02's readers, whose tolerance for foreign kinds is already proven; config gains three optional fields (`instructions`, `repo_instructions` on a role block; top-level `sidecar` dir defaulting to `sidecar`), resolved relative to the config file's directory and checked for existence only at `run`, never at `check-config`.

## Execution plan

### Scope 1 — config widening

- **Files:** Modify `crates/arx/src/config.rs`.
- **Build:** the three fields `run` needs, with L1-01's strictness intact.
- **Steps:**
  1. `RoleConfig` gains `instructions: Option<PathBuf>` (file whose content is arx's system prompt for that role) and `repo_instructions: Option<PathBuf>` (repo instruction file arx pushes; presence means push). `StartupConfig` gains `sidecar: Option<PathBuf>`, default `sidecar`. `deny_unknown_fields` stays on every struct.
  2. `Resolved` carries the three paths resolved against the config file's directory (same rule as `agents`). No existence check here — `check-config` on the L1-01 fixtures must keep passing untouched.
  3. `#[cfg(test)]` unit tests: a config with all three fields resolves them relative to the config dir; a config without them yields `None`/`sidecar` default; L1-01's existing tests run unchanged.
- **Verify:** `cargo test -p arx` — new tests green, every pre-existing config and transcript test green.

### Scope 2 — transcript widening

- **Files:** Modify `crates/arx/src/transcript.rs`.
- **Build:** the three row shapes the adapter writes, through the single-writer `TranscriptLog` (`7b607c7e` — no second writer, ever).
- **Steps:**
  1. The turn line type gains `role: Option<String>`, serialized only when present; deserialization stays tolerant. `append_turn` keeps its signature (role absent); add `append_turn_as(&mut self, role: &str, text: &str) -> Result<Handle, TranscriptError>` — same numbering, same handle minting.
  2. `append_system(&mut self, text: &str) -> Result<(), TranscriptError>` — writes `{"kind":"system","text":…}`, no turn number, no handle.
  3. `append_usage(&mut self, usage: &serde_json::Value) -> Result<(), TranscriptError>` — writes `{"kind":"usage","usage":{…}}`, the body's numbers verbatim.
  4. Free `last_usage(root: &Path, thread: &str) -> Result<Option<serde_json::Value>, TranscriptError>` — scans the thread's file, returns the last usage row's `usage` value, `None` when no usage row exists, unknown-thread error when the file is absent.
  5. Unit tests: role serialized when present and absent from `append_turn` rows; system and usage rows skipped by `resolve` and by `next_turn` recovery; `last_usage` returns the latest of several; every L1-02 test runs unchanged — the L1-02 interface is widened, never moved.
- **Verify:** `cargo test -p arx` — all green, including L1-02's `transcript_log` integration suite untouched.

### Scope 3 — the sidecar and its stub

- **Files:** Create `sidecar/claude-main.mjs`, `sidecar/package.json`, `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`; modify `.gitignore` (add `sidecar/node_modules/`, `threads/`).
- **Build:** the SDK sidecar speaking the protocol, and a dependency-free stub speaking the same protocol for tests.
- **Steps:**
  1. `sidecar/package.json`: `"type": "module"`, one dependency `"@anthropic-ai/claude-agent-sdk": "0.3.251"`.
  2. `sidecar/claude-main.mjs` (one header comment naming what the file is — arx's claude-code main sidecar, `b8d59164` — and nothing else): read JSONL ops from stdin. On `{"op":"init","model":…,"systemPrompt":…,"cwd":…}`: build one long-lived `query()` — session-per-thread, t3code's shape — whose input is an async generator fed by a prompt queue; options: `model`, `systemPrompt` as the given string (replaces the preset), `settingSources: []`, `disallowedTools: ["*"]`, `includePartialMessages: true`, `cwd`; emit `{"ev":"ready"}`. On `{"op":"prompt","text":…}`: push a user message onto the queue; while consuming the stream, emit `{"ev":"delta","text":…}` for each text delta, and per assistant message `{"ev":"turn","text":…}` (its text blocks joined) then `{"ev":"usage","usage":…}` (the message's usage object verbatim); on the result message emit `{"ev":"turn_end"}`. On `{"op":"shutdown"}` or stdin EOF: end the input iterable and exit 0. Any thrown error: `{"ev":"fatal","message":…}` then exit 1. Nothing but protocol lines on stdout. No session options set — the body persists its own session file and arx never opens it.
  3. Stub `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`: no imports beyond node built-ins. `init` → `ready`. `prompt` with text `T` → `delta` `"stub:"`, `delta` `T`, `turn` `"stub:" + T`, `usage` `{"input_tokens":7,"cache_creation_input_tokens":0,"cache_read_input_tokens":40,"output_tokens":5}`, `turn_end`. Prompt exactly `hang` → one `delta` then sleep forever. Prompt exactly `die` → exit 1 mid-turn. `shutdown`/EOF → exit 0.
  4. `cd sidecar && npm install` — succeeds, lockfile and `node_modules` land (gitignored, uncommitted like everything else).
- **Verify:** `node crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs <<< '{"op":"init"}'` prints `{"ev":"ready"}`. `cd sidecar && node -e "import('@anthropic-ai/claude-agent-sdk').then(() => console.log('sdk ok'))"` prints `sdk ok`.

### Scope 4 — the adapter

- **Files:** Create `crates/arx/src/claude_code.rs`; modify `crates/arx/src/main.rs` (declare `mod claude_code`); create `crates/arx/tests/claude_main.rs`.
- **Build:** the adapter type later cards mount things onto: spawn, drive a turn, kill.
- **Steps:**
  1. `ClaudeCodeMain` in `claude_code.rs` (one module doc comment: the claude-code adapter in the main role, SDK sidecar route, `b8d59164`). `spawn(sidecar_dir: &Path, opts: SpawnOpts) -> Result<Self, ClaudeCodeError>` where `SpawnOpts { model, system_prompt, cwd }`: verify `<sidecar_dir>/claude-main.mjs` exists (error naming the resolved path otherwise); spawn `node` on it with stdin/stdout piped, stderr inherited, env var `CLAUDE_CODE_AUTO_COMPACT_WINDOW=1000000`; write the `init` op; wait for `ready` with a 10s deadline (timeout kills the child and errors).
  2. `run_turn(&mut self, prompt: &str, on_delta: impl FnMut(&str)) -> Result<TurnOutcome, ClaudeCodeError>` — writes the `prompt` op, reads events until `turn_end`: deltas go to the callback, `turn` text and the last `usage` value accumulate into `TurnOutcome { text: String, usage: Option<serde_json::Value> }`. A `fatal` event, EOF, or child death mid-turn is an error carrying the sidecar's message.
  3. `kill(&mut self)` — SIGKILL the child, reap it. `shutdown(mut self)` — write the `shutdown` op, give the child 2s to exit, then kill. `Drop` best-effort kills.
  4. `ClaudeCodeError` — one variant per failure with a Display message: sidecar entry missing (path), spawn io (source), ready timeout, protocol line malformed (the line), sidecar fatal (message), child died mid-turn.
  5. The adapter owns the process and the protocol only — it never touches the transcript log; the run loop wires the two (scope 5). The log's single writer stays single.
  6. Integration tests in `tests/claude_main.rs`, each spawning the stub sidecar dir with `tempfile` workspaces: one turn returns `stub:hello` with the stub's usage and the deltas seen in order; two prompts on one adapter both complete (session held open between turns); `hang` then `kill()` — the call returns, the child is gone; `die` mid-turn surfaces the child-died error.
- **Verify:** `cargo test -p arx` — all green: the new adapter tests, scopes 1–2's tests, every pre-existing suite.

### Scope 5 — `arx run`, `arx gauge`, the live done-check

- **Files:** Modify `crates/arx/src/main.rs`; create `arx.toml`, `agents/judge.md`, `agents/hand-ex.md` (repo root — the v0 working config, the owner's to edit); create `crates/arx/tests/fixtures/run/` (an `arx.toml` whose `sidecar` points at `../stub-sidecar`'s parent — i.e. a config tree wiring the stub — plus its `agents/`); extend `crates/arx/tests/claude_main.rs`.
- **Build:** the card's done-check, runnable.
- **Steps:**
  1. `run [PATH]` (default `./arx.toml`, same idiom as `check-config`): load config; error unless main's body is `claude-code` (`native`/`codex` mains are not-yet-specified on the map — say so in the message). Compose the system prompt: the `instructions` file's content when set, else a built-in two-sentence arx-main constant; when `repo_instructions` is set, append that file's content under a one-line header naming its path (`ceb0390f`). Open `TranscriptLog` on `<config dir>/threads`, thread `main` — numbering continues across runs by L1-02's reopen behaviour, the thread outliving the process. Append the composed prompt as a system row, spawn the adapter (`cwd` = config dir), then loop: read a line from stdin → append it as a turn row with role `operator` → `run_turn` streaming deltas to stdout → append the reply as a turn row with role `main`, append the usage row, print a newline. stdin EOF → `shutdown`, exit 0. Adapter error → one `error:` line to stderr, kill, exit 1.
  2. `gauge [PATH]`: load config, `last_usage` on thread `main`; no usage row → say so, exit 1; otherwise print each usage field and `occupancy <input+cache_creation+cache_read> / window 1000000 (body-reported)`.
  3. Root `arx.toml`: `[main] body = "claude-code"`, `model = "claude-sonnet-5"` (cheap for checks; the owner flips it at will). `agents/judge.md` body `codex` model `gpt-5.4-codex`; `agents/hand-ex.md` body `claude-code` model `claude-sonnet-5` — L1-01 validation demands all three roles.
  4. CLI tests against the stub fixture config with `assert_cmd`: piping one prompt line into `run` exits 0 with `stub:` in stdout, and the thread file then holds system, operator-turn, main-turn and usage rows in order; `gauge` on that config prints `occupancy 47`; `gauge` on a fresh copy with no threads dir exits 1.
- **Verify:**
  - `cargo build` — green. `cargo test -p arx` — everything green.
  - Live, requires the operator's own `claude` login on this machine — if auth is absent the runner STOPS and reports, it does not fake the check:
    - `printf 'reply with exactly the word turquoise\n' | cargo run -p arx -- run` — exit 0, stdout contains `turquoise`.
    - `cargo run -p arx -- gauge` — prints nonzero usage fields and the occupancy line.
    - `python3 -c "import json,sys; rows=[json.loads(l) for l in open('threads/main.jsonl')]; print([r['kind'] for r in rows])"` — shows `system`, `turn`, `turn`, `usage` from the live run.
    - `printf 'say ok\n' | strace -e trace=open,openat -o /tmp/l103-strace.txt ./target/debug/arx run` then `grep -c '\.claude/projects' /tmp/l103-strace.txt` — prints `0` (no `-f`: only the arx process is traced; the body writes its own session file, arx never opens it — `7b607c7e`).
  - Kill-leaves-log-intact is pinned by scope 4's `hang`/`kill` test; no live kill needed.

## Out of scope

Done means the Verify commands above pass — nothing more. No gate and no `canUseTool` wiring (L1-05), no MCP mounts, no memory (L1-06), no dispatch, hands or spawn ledger (L1-07), no terminal beyond line-buffered stdin/stdout (L1-04), no judge (L1-11). No mid-turn interrupt op, no resume of body sessions, no reserve or reset arming (L3-02, D15), no occupancy policy beyond displaying the body's numbers (`5fadfcc0` — build settles it later, by use), no event-line vocabulary beyond the three row kinds named here (L2-01 owns events), no codex or native main (not-yet-specified on the map), no prompt-layout or cache discipline (L2-03). The L1-02 interface is widened, never moved — existing tests prove it by running unchanged. No commits — the working tree is the deliverable. No inline comments; the two doc comments named in scopes 3 and 4 are the exceptions.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-02-l1-03-claude-code-main-adapter.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
