# Roster B — `bolt` and `hand-do`, and the first hand that writes

## Problem / Solution

Card A made a position's model a runtime choice. It did not add any positions. arx still has one working hand, `hand-ex`, and nothing that writes — `main.rs:729` masks `write` off for the judge and the drain, every shipped agent file leaves the key absent, and the only thing that has ever set it is the `fake-write` string special-cased in `resolve_hand_spec` (`main.rs:656`). Meanwhile `prompts/main.md` already tells main to "start with the smallest hand the brief permits" and that "a write hand's work is the diff in its trace". Both sentences are false today: there is one tier, and no hand writes.

This card makes them true. `bolt` is the cheap tier for one-hop lookups; `hand-do` is the first position with `write: true`, which swaps its `/hand/repo` mount from the bare toplevel to a detached git worktree and captures a diff at teardown. Both land as ordinary hands — `config.rs:777-799` reserves only `main.md`, `judge.md` and `drain.md`, so any other file in `agents/` becomes a hand with no source change, and `render_hand_roster` (`main.rs:164-183`, called at `:227-229`) puts them in main's system prompt automatically.

Three gaps have to close for that to be worth shipping. A position's prose reaches main but never the hand, so two positions on one model are the same hand wearing different names. A read-only codex hand is launched with `--sandbox danger-full-access` regardless of its `write` flag, so the flag is a claim arx does not enforce. And a write hand's diff is written to `threads/<thread>.<hand>/worktree.diff` and then named by nothing — not the ledger row, not either front, not the trace viewer.

## Decisions locked

- **A position is a class: it carries tools, prose and a write flag; the model is the driver.** Card A settled that the body follows the model; this card settles that everything else follows the position.
- **`check-config` shows every resolved position, drain included.** Ruled mid-run 2026-09-09 after the table proved unable to satisfy this card's own Verify. `Resolved::rows()` dropped drain with no comment saying why and had exactly one caller, so it is deleted rather than kept in parallel with `positions()`. Drain does not leak into main's prompt as a side effect: `render_hand_roster` takes hands and judge as direct parameters and calls neither method.
- **`bolt` and `hand-do` are ordinary hands.** No new `HandRole` variant, no reserved filename, no special-casing. They land in the generic `hands` bucket like `hand-ex`.
- **Defaults**: `main = "claude-opus-5"`, `hand-ex = "claude-sonnet-5"`, `hand-do = "gpt-5.6-sol"`, `bolt = "gpt-5.6-luna"`. `judge` and `drain` stay on `gpt-5.6-sol`.
- **The registry gains `gpt-5.6-luna` and `claude-fable-5-1`.** Both ids verified against the live installs — luna from `~/.codex/models_cache.json`, fable from `~/.claude/settings.json` and `claude config`. Fable is registered but is nobody's default; it exists so `/model claude-fable-5-1` works on main.
- **`hand-do` sets `write: true`.** The worktree, mount and diff-capture machinery is real and tested end to end (`crates/arx/tests/hands_in_containers.rs:141-260`, including a killed hand whose diff still applies). This card is the first to point a shipped position at it.
- **A position's prose line reaches the hand.** `RoleRow.role_line` already exists and already reaches main's prompt; it now also reaches the hand's system text, appended to each body's `ARX_HAND_INSTRUCTIONS`. No new config key — the file's existing prose is the source.
- **Codex's sandbox follows `write`.** A read-only hand gets `--sandbox read-only`; a write hand keeps `danger-full-access`. Tightening write hands to `workspace-write` needs `writable_roots` covering `/hand/out` and `/hand/home` plus a network decision, and is deferred.
- **A tools list its body ignores is announced, not silently accepted.** Only claude-code honours `tools:`; codex never reads it (`codex_hand.rs` has no `spec.tools` reference) and native has only `read_file`/`recall`. Because `render_hand_roster` reports every position's tools to main as fact, an inert list is an active lie. Config load prints a notice; it never refuses.
- **A write hand's diff is named at every surface that already names its trace.** The ledger's verdict row, both fronts' delivery line, and the panes trace viewer.
- **arx never applies a diff.** Settled by `4f90f38f` and `d70f022c`: the diff is the deliverable, main or a hand it dispatches applies it as plain working-tree changes, never a rebase, never a commit on the hand's behalf. This card adds no apply path — it only makes the diff findable.
- **`hand-do` does not verify its own work.** Neither hand image carries a Rust toolchain (both are `FROM node:22` plus one npm install), so `cargo test` cannot run in a container. The position's prose says so, so main does not read its verdict as tested.

## Execution plan

### Scope 1 — the two positions and the registry entries

- **Files:**
  - Create `agents/bolt.md`, `agents/hand-do.md`
  - Modify `arx.toml`, `crates/arx/src/main.rs`, `crates/arx/src/config.rs`
- **Build:** six positions resolving, with the defaults above, and `check-config` showing all six.
- **Steps:**
  1. Add two `[[models]]` entries: `id = "gpt-5.6-luna"`, `body = "codex"`, `image = "arx-hand-codex:0.152.1"`; and `id = "claude-fable-5-1"`, `body = "claude-code"`, `image = "arx-hand-claude:2.1.258"`.
  2. Change `[roles].main` from `claude-sonnet-5` to `claude-opus-5`. Add `hand-do = "gpt-5.6-sol"` and `bolt = "gpt-5.6-luna"`. Leave `hand-ex`, `judge` and `drain` as they are.
  3. Create `agents/bolt.md` with frontmatter `tools: [Read, Glob, Grep, Bash]` and one prose line naming the class: a mechanical scout for one-hop lookups — a file, a symbol, a caller, a git fact — that returns "needs a deeper hand" rather than guessing.
  4. Create `agents/hand-do.md` with frontmatter `tools: [Read, Glob, Grep, Bash]` and `write: true`, and a prose line that says it implements a bounded brief and leaves the result as a diff in its trace, and that it does not verify its own work.
  5. Do not add `body`, `model`, `image` or `provider` to either file — card A retired those keys and `RETIRED_FRONTMATTER_KEYS` (`config.rs:635`) makes them a load error.
  6. `check-config`'s table cannot show six positions today: `render_table` (`main.rs:3981`) renders `Resolved::rows()` (`config.rs:395-399`), which is main + judge + hands and drops drain. This predates the card — on unmodified HEAD the table prints three rows while `arx.toml` configures four positions. Point `render_table` at `Resolved::positions()` (`config.rs:406-411`) instead, and delete `rows()`: `render_table` is its only caller, and nothing documents why it omits drain. If a test asserts the table's shape, update the expected rows; if any test's stated intent is that drain must NOT appear in the table, stop and report it rather than editing it away, because that would be the missing reason.
- **Verify:** `cargo run -p arx -- check-config ./arx.toml` succeeds and lists six positions — main, judge, bolt, hand-do, hand-ex, drain — each with its resolved body and model, main on `claude-opus-5`, bolt on `gpt-5.6-luna`, hand-do on `gpt-5.6-sol`. `cargo test -p arx config` green. A test asserting `Resolved::positions()` returns main and judge first, then the three hands alphabetically — bolt, hand-do, hand-ex — then drain.

### Scope 2 — the inert-tools notice

- **Files:** Modify `crates/arx/src/config.rs`, `crates/arx/src/main.rs`
- **Build:** config load announces a tools list the bound body will ignore.
- **Steps:**
  1. After resolution, walk every position with a non-empty `tools` list and compare against its effective body. Codex ignores the list entirely. Native accepts only `read_file` and `recall` — the same set `resolve_native_tools` (`main.rs:632-642`) enforces at dispatch, so a native position naming anything else is announced at load rather than only failing later.
  2. Emit one notice per offending position on the operator surface, reusing the path card A's dangling-pin notice writes to. Text: `[arx] tools: <position> lists tools, but <model-id> runs on the <body> body, which ignores them`. For native, name the offending tool instead: `[arx] tools: <position> lists <tool>, which the native body does not have`.
  3. This is a notice, never a refusal. `check-config` still exits zero.
- **Verify:** `cargo run -p arx -- check-config ./arx.toml` prints the notice for `bolt` and `hand-do` and exits zero. Unit tests: a codex position with a tools list produces exactly one notice; a claude-code position with the same list produces none; a native position listing `Bash` names `Bash` in its notice.

### Scope 3 — the position's prose reaches the hand

- **Files:** Modify `crates/arx/src/main.rs`, `crates/arx/src/claude_hand.rs`, `crates/arx/src/codex_hand.rs`, `crates/arx/src/native_hand.rs`
- **Build:** a hand is told which position it is.
- **Steps:**
  1. Carry `role_line` from `RoleRow` onto `HandSpec` in `build_hand_spec`. `RoleRow.role_line` already exists and is already populated — this only adds the field to the spec.
  2. In each of the three body adapters, append the role line to `ARX_HAND_INSTRUCTIONS` when it is non-empty, as a final line. Keep the constant itself unchanged; compose at use, so the shared instructions stay one text.
  3. `codex_hand.rs:build_argv` passes instructions through `-c instructions="…"`. Composing there changes the string the existing argv test asserts — update that assertion, do not weaken it.
- **Verify:** `cargo test -p arx` green. One test per body asserting a hand spawned for a position whose file says a known sentence has that sentence in the instructions it was launched with, and that a position with empty prose is launched with the constant unchanged. **Adding a field to `HandSpec` touches every test literal that constructs one — this is expected. The guard is that no test is deleted and the total strictly rises; do not promise the test files are unedited.**

### Scope 4 — codex's sandbox follows `write`

- **Files:** Modify `crates/arx/src/codex_hand.rs`, `crates/arx/src/main.rs`
- **Build:** `write: false` means something to codex, not only to the bind mount.
- **Steps:**
  1. `build_argv` (`codex_hand.rs:16-30`) currently hardcodes the literal `"danger-full-access"`. Branch on `spec.write`: `read-only` when false, `danger-full-access` when true. Keep the flag in its current argv position.
  2. The existing test `codex_argv_places_the_sandbox_clamp_right_after_skip_git_repo_check` (`main.rs:5700-5705`) locks the old value for both cases. Split it into two tests, one per branch, keeping the position assertion.
- **Verify:** `cargo test -p arx` green. A read-only codex hand's argv contains `--sandbox read-only`; a write-enabled one contains `--sandbox danger-full-access`. Live: dispatch `hand-ex` (read-only, but on a codex pin via `/model hand-ex gpt-5.6-sol`) and confirm from the trace that the process ran read-only.

### Scope 5 — the write hand's diff becomes findable

- **Files:** Modify `crates/arx/src/container_hand.rs`, `crates/arx/src/ledger.rs`, `crates/arx/src/main.rs`, `crates/arx/src/panes/state.rs`, `crates/arx/src/panes/ui.rs`, `crates/arx/src/panes/trace.rs`
- **Build:** every surface that names a hand's trace also names its diff.
- **Steps:**
  1. `container::sweep` (`container.rs:325-356`) already writes `worktree.diff` into the trace directory and records it in `trace.md`. Carry the fact that it wrote one back out, and through the `Delivery` that `container_hand.rs` produces.
  2. `ledger.rs:556` writes `{kind, hand, text, trace, usage}` for a verdict. Add `"diff"` carrying the path relative to the workspace — `threads/<thread>.<hand>/worktree.diff` — present only when a diff was captured. A read-only hand's row is unchanged.
  3. `format_delivery` (`main.rs:1027-1035`) renders `[hand {hand} verdict] {text} (trace: {trace})`. When a diff exists, name it in the same line so the line front shows it.
  4. Carry the diff path onto `HandRow` (`panes/state.rs`) and render it in the expanded verdict block beside `trace` and `bytes` (`panes/ui.rs:186-187`).
  5. `panes/trace.rs::read_trace` (`trace.rs:11-19`) handles `out/stdout.log`, `session.jsonl` and `transcript.jsonl`. Add a `worktree.diff` case so expanding the row can show the diff.
- **Verify:** `cargo test -p arx` green. A test asserting a completed write hand's verdict row carries `diff` with the expected relative path, and that a read-only hand's row has no `diff` key. Live, in a pty session: dispatch the `fake-write` fixture hand, confirm the verdict line names the diff, expand the row and confirm the diff renders.

### Scope 6 — main is told where a diff lives

- **Files:** Modify `prompts/main.md`
- **Build:** the standing prompt stops promising something it never explains.
- **Steps:**
  1. `prompts/main.md` says "A write hand's work is the diff in its trace; applying it is a decision you say out loud" without saying where the diff is or how to reach it. Extend that bullet: the diff is at `<trace>/worktree.diff`, `read_file` reaches it because `threads/` sits under the workspace root, and `list_files` will not show it because it filters arx state.
  2. Say that applying it is main's own move, as plain working-tree changes, never a rebase and never a commit — this is `4f90f38f` restated for the reader who only has this file.
  3. Do not enumerate the positions. `render_hand_roster` already injects them at startup with model, body, tools and prose; a hardcoded list would go stale on the next card.
- **Verify:** `cargo run -p arx -- check-config ./arx.toml` still succeeds (the file is loaded as main's instructions). Read the rendered system prompt from a live session's transcript and confirm the paragraph is present exactly once.

## Out of scope

Done means: six positions resolve from the registry, `bolt` and `hand-do` are dispatchable, a `hand-do` run leaves a diff that main and the operator can both find, and a read-only codex hand is launched read-only.

Not in this card:
- **Cross-body tool enforcement** — mapping one abstract tools list onto codex feature flags (`shell_tool`, `view_image`, `web_search`) and onto native's tool set. Card C. This card announces the gap rather than closing it.
- **Growing native's tools** — `grep` and `bash` as native tool defs. Card C. The four mechanical steps are known (`*_def()`, an `execute_tool` arm, a `ToolHost` method, the `resolve_native_tools` allowlist) but nothing here needs them.
- **Tightening write hands to `workspace-write`** — needs `writable_roots` for `/hand/out` and `/hand/home` and a decision on network access, tested against a hand that actually builds.
- **Container network and capabilities** — `container.rs:148-182` emits no `--network`, no `--cap-drop`, no read-only root filesystem. Every hand has full network. Untouched here.
- **The dead per-hand knobs** — `nudge`, `checkpoint` and `reserve` are parsed and validated for a hand and never read; `window` is reported but never enforced. Fixing them means deciding what happens when a hand hits a limit.
- **The `TraceFailed` hole** — `container_hand.rs:819-832` removes the worktree without capturing a diff when the host-side transcript fails to open. Reachable only with `write: true` on a native body, which no position has. Fix it when one does.
- **A build-capable hand image** — and the schema change it needs, since `id` is both the registry key and the string passed to `--model`, so one position cannot get its own image today.
- **The `glm5.3-flash` provider**, and any change to card A's registry, roster file, `/model` grammar or modal.
- **Applying a diff automatically.** Ruled out by `4f90f38f`.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-09-roster-b-positions-and-write-hand.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
