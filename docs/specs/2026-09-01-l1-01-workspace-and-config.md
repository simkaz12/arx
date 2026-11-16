# L1-01 — workspace and config shape

Card: `docs/wayfinder/maps/arx-v0/L1-01-workspace-and-config.md`. The card is this brief's whole world.

**Ruled by:** D16 `0bd4df2f` (config shape per role), D02 `975f65cd` (body ≠ provider, no capability flags), `30e83788` (shell never leaves Rust), `c775b5c2` (engine is arx's own, in Rust). If an edit below lands against one of these, STOP — the ruling reopens in the store first; do not bend the brief to fit.

## Problem / Solution

The repo holds design docs and no code. Every later card (L1-03 main adapter, L1-07 dispatch, L1-10 provider trait) needs a workspace to land in and a config surface that already enforces the D16 role rulings. This card ships both: a two-crate Rust workspace — the arx shell and the memory engine linked in as a crate, no transport — and `arx check-config`, which resolves the three roles from startup config plus an agent-definition directory and refuses the two shapes the rulings forbid.

## Decisions locked

- Each role is configured by three fields: `body` (native | claude-code | codex), `model`, and `provider` for a native body only. (`0bd4df2f`)
- Main's block is startup config, fixed per run. A hand's block is its named agent definition. The judge's block is the reserved `judge` definition. (`0bd4df2f`)
- The drive surface is never configured — it follows from the role: main over the Agent SDK or Codex app-server, hand and judge over `claude -p` or `codex exec`, native over the provider loop. (`0bd4df2f`)
- A per-dispatch body override on main is rejected, not ignored. (`0bd4df2f`)
- A body is not a provider. `provider` on a CLI body is an error; at this layer `provider` is a free non-empty string, since adapter binding is spawn-time composition and L1-10's work. No capability flags anywhere in config. (`975f65cd`)
- The engine is a crate linked into the shell — a path dependency, no transport, no Python. (`c775b5c2`, `30e83788`)
- Brief-fixed, not rulings (cheap to change later): startup config is TOML (`arx.toml`); agent definitions are markdown with YAML frontmatter, matching the store's file idiom and the owner's daily agent files; crates are `arx` (bin) and `arx-engine` (lib) under `crates/`; edition 2024; parsing is strict — an unknown key is an error at this layer, and later cards widen the structs when their fields land.

## Execution plan

### Scope 1 — workspace skeleton

- **Files:** Create `Cargo.toml` (workspace root), `.gitignore`, `crates/arx/Cargo.toml`, `crates/arx/src/main.rs`, `crates/arx-engine/Cargo.toml`, `crates/arx-engine/src/lib.rs`.
- **Build:** the two-crate workspace with the dependency edge arx → arx-engine.
- **Steps:**
  1. Root `Cargo.toml`: `[workspace]` with `members = ["crates/arx", "crates/arx-engine"]`, `resolver = "3"`; `[workspace.package]` with `edition = "2024"`.
  2. `.gitignore`: `/target`.
  3. `crates/arx-engine/Cargo.toml`: lib crate, `edition.workspace = true`. `src/lib.rs`: empty except a doc comment naming what the crate will hold (the store's judging half — landscape lookup, lexical recall, lifecycle; L1-06 and L2-04 fill it).
  4. `crates/arx/Cargo.toml`: bin crate named `arx`, `edition.workspace = true`, `arx-engine = { path = "../arx-engine" }`. `src/main.rs`: a `fn main` stub; scope 3 replaces it.
  5. From the repo root: `cargo add -p arx serde --features derive`, `cargo add -p arx toml serde_yaml clap --features clap/derive`, `cargo add -p arx --dev assert_cmd predicates`.
- **Verify:** `cargo build` — green, both crates compile. `cargo tree -p arx | grep arx-engine` — prints the edge, proving linked-as-crate.

### Scope 2 — config module: parse and validate

- **Files:** Create `crates/arx/src/config.rs`; modify `crates/arx/src/main.rs` (declare `mod config`).
- **Build:** the config interface the CLI and later cards call: `config::load(path: &Path) -> Result<Resolved, ConfigError>`.
- **Steps:**
  1. Types, all `#[derive(Deserialize)]` with `#[serde(deny_unknown_fields)]`:
     - `Body` enum: `Native`, `ClaudeCode`, `Codex`, serde-renamed kebab-case so the wire values are exactly `native | claude-code | codex`.
     - `RoleConfig { body: Body, model: String, provider: Option<String> }` — shared by main's TOML block and agent frontmatter.
     - `StartupConfig { agents: Option<PathBuf>, main: RoleConfig }` — `agents` defaults to `agents`, resolved relative to the config file's directory.
  2. Drive-key tripwire: before typed deserialization, parse the TOML to `toml::Value` and walk every table for the keys `drive` and `drive_surface`. Hit → `ConfigError::DriveSurfaceConfigured` rendering as `error: 'drive_surface' in [main]: the drive surface is never configured — it follows from the role (0bd4df2f)`. This runs first so the targeted message wins over the generic unknown-key error.
  3. Frontmatter parsing: an agent definition is a file whose content starts with a `---` line; the block up to the next `---` line deserializes as YAML into `RoleConfig`; the prose after it is not read. No frontmatter → error naming the file.
  4. Semantic checks, each a distinct `ConfigError` variant with a Display message carrying the operative clause:
     - `provider` present with a non-native body → `provider is set but body is '<body>' — a body is not a provider; provider applies only to body = native (975f65cd)`.
     - `body = native` without `provider`, or with an empty one → `body 'native' requires a provider`.
     - `model` empty → error naming the role.
     - agents dir missing or unreadable → error with the resolved path.
     - no `judge.md` in the agents dir → `agents dir names no 'judge' definition — the judge role is unfilled (0bd4df2f)`.
     - no `.md` besides `judge.md` → `no hand definition — config must name all three roles`.
     - a `main.md` in the agents dir → `agents/main.md: main's block is startup config, fixed per run — no per-dispatch body override (0bd4df2f)`.
  5. `Resolved` holds the role rows: main from `[main]`, judge from `judge.md`, one hand row per remaining `.md`, hands sorted by name. Each row carries role, source, body, model, provider, and the derived drive surface: main → `agent-sdk` (claude-code) | `app-server` (codex) | `native loop` (native); judge and hand → `claude -p` | `codex exec` | `native loop`.
  6. `#[cfg(test)]` unit tests in `config.rs`: frontmatter split (valid, missing, unterminated), provider-on-CLI-body rejected, native-without-provider rejected, drive-key walk catches the key nested in `[main]`.
- **Verify:** `cargo test -p arx` — unit tests green.

### Scope 3 — `check-config` CLI, fixtures, integration tests

- **Files:** Modify `crates/arx/src/main.rs`; create `crates/arx/tests/check_config.rs` and the fixture tree under `crates/arx/tests/fixtures/`.
- **Build:** the card's done-check, runnable.
- **Steps:**
  1. `main.rs`: clap derive, one subcommand `check-config [PATH]`, `PATH` defaulting to `./arx.toml`. Success → print the resolved role table to stdout, exit 0. Any `ConfigError` → one `error:` line to stderr, exit 1.
  2. Table shape, fixed: header `role  source  body  model  provider  drive`, aligned columns, `-` for an absent provider, rows ordered main, judge, then hands by name.
  3. Fixtures, each directory self-contained (own `arx.toml` + `agents/`):
     - `valid/`: `arx.toml` with `[main] body = "claude-code"`, `model = "claude-fable-5"`; `agents/judge.md` (body `codex`, model `gpt-5.4-codex`), `agents/hand-ex.md` (body `claude-code`, model `claude-sonnet-5`), `agents/hand-native.md` (body `native`, model `claude-haiku-4-5`, provider `anthropic`). Each definition carries a line of prose after the frontmatter so the parse proves it skips prose.
     - `drive-surface/`: valid main block plus `drive_surface = "agent-sdk"` inside `[main]`; `agents/` with `judge.md` and `hand-ex.md`.
     - `main-override/`: valid `arx.toml`; `agents/` with `judge.md`, `hand-ex.md`, and a `main.md` (body `codex`, model `gpt-5.4-codex`).
     - `provider-on-cli/`: valid main; `agents/hand-ex.md` with `provider: anthropic` added to its claude-code frontmatter, plus `judge.md`.
     - `native-no-provider/`: valid main; `agents/hand-native.md` without `provider`, plus `judge.md`.
     - `no-judge/`: valid main; `agents/` with only `hand-ex.md`.
  4. `tests/check_config.rs` with `assert_cmd`: one test per fixture. `valid` asserts exit 0 and stdout containing the four role rows (match on `role`, `main`, `judge`, `hand-ex`, `hand-native`, `agent-sdk`, `codex exec`, `claude -p`, `native loop`). Each rejection fixture asserts exit 1 and the stderr substring from its scope-2 message.
- **Verify:**
  - `cargo build` — green on both crates.
  - `cargo test -p arx` — all unit and integration tests green.
  - `cargo run -p arx -- check-config crates/arx/tests/fixtures/valid/arx.toml` — exit 0, prints the role table: main/claude-code/agent-sdk, judge/codex/`codex exec`, hand-ex/claude-code/`claude -p`, hand-native/native/`native loop`.
  - `cargo run -p arx -- check-config crates/arx/tests/fixtures/drive-surface/arx.toml` — exit 1, stderr says the drive surface is never configured and follows from the role.
  - `cargo run -p arx -- check-config crates/arx/tests/fixtures/main-override/arx.toml` — exit 1, stderr says main's block is startup config, fixed per run.

## Out of scope

Done means the five Verify commands above pass — nothing more. No child process is spawned, no SDK touched, no engine behaviour (L1-06/L2-04 own the store), no transcript log (L1-02), no gate (L1-05), no dispatch (L1-07). Agent-definition prose is preserved and unread beyond the frontmatter. No provider-name enum and no adapter binding — L1-10. No config hot-reload; main's block is fixed per run by ruling. No commits — the working tree is the deliverable. No inline comments; the one doc comment named in scope 1 is the exception.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-01-l1-01-workspace-and-config.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
