# L3-05 — the store root outside the repository

Card: `docs/wayfinder/maps/arx-v0/L3-05-store-root-outside-the-repo.md`. Ruled by `57aa480a` (the owner's 2026-09-08 ruling: the live store is per user at `~/.local/share/arx/store`, named by `[store] root`; absent, the root stays `<config dir>/store`; the repository's `store/` is deleted and its two FileStore-era rows dropped; rejected: migrating `store/arx.jsonl` in place and untracking `store/`), `f09b9ed4` (a scope is a directory under the store root and nothing else; the owner registers it, the engine never mints one), `f89798aa` (the store root is its own git repository; `Engine::open` runs `git init` when it finds none). Branch `l3-05-store-root-outside-the-repo`, cut from `main` at `4f3a051` after L3-02 merged.

No grill: the owner ruled the one fork in session, the second of the two options the L2-05 owner item named, and said the old rows do not survive. Every other call is under "Chosen without a grill".

Amended 2026-09-08 after the runner's pass, on its own finding: nothing in the tree writes `pid.txt` (committed with L1-close; the only code reference is the edit guard that refuses it as arx state), so the Ground rules line, the CLAUDE.md sentence in Scope 4 and the `git status` expectation in Scope 5 no longer say a session rewrites it. The Scope 2 grep tripwire's exception list was also short at baseline (test-module hits in `native_hand.rs` and `main.rs` predate this card); the runner kept the locked sentence, "no `join("store")` outside test modules", by moving the two remaining production literals behind one `DEFAULT_STORE_DIRNAME` constant in `config.rs`.

## Problem / Solution

`arx run` with the repository's own `arx.toml` cannot start. The store root is `<config dir>/store`, which here is the tracked `store/` holding `store/arx.jsonl` in L1-06's FileStore shape; the engine git-inits `store/.git` inside the source tree and then refuses grounding because `store/arx/` does not exist. The L2-05 brief predicted this and left it as an owner item. This morning the owner hit it from a new `arx` terminal command and ruled: the store lives outside every repository, one per user.

After this card, `[store] root` in `arx.toml` names the store root and the repository's file sets it to `~/.local/share/arx/store`. Every place that derives the root today (grounding and the quarantine listing at spawn, the drain's quarantine read, the hand spec on both dispatch paths, `arx run`'s engine open, `arx ember`) reads the one resolved path instead, so `arx` from any directory opens the same store. An unregistered scope is still refused, and the refusal names the directory to create. The repository's `store/` is gone: `store/arx.jsonl` deleted from the tree, the nested `.git`, `.gitignore` and `quarantine/` that the failed run left behind removed from disk. CLAUDE.md's store hazard goes with it.

## Decisions locked

- `StoreConfig` gains `root: Option<PathBuf>` under `#[serde(default)]`, so `[store] root = "…"` parses and `deny_unknown_fields` stays.
- `Resolved` gains `pub store_root: PathBuf`, always absolute. Resolution at load: absent → `config_dir.join("store")`; a value equal to `~` or starting with `~/` → `HOME` in place of the tilde; any other relative value → `config_dir.join(value)`, the `agents` and `sidecar` pattern; then, when the result is still relative, `std::env::current_dir()` joined in front, which is what `cmd_run` does today. An empty or whitespace-only `root` is refused as `ConfigError::EmptyStoreRoot`, Display `store.root must not be empty`. A tilde with `HOME` unset is refused as `ConfigError::StoreRootHome`, Display `store.root: cannot expand ~, HOME is not set`. Nothing is created at load.
- `[store] root` without `[store] scope` is impossible today, since `scope` is the table's required field, and stays so.
- Every runtime derivation of the store root reads `resolved.store_root`: `compose_dynamic_block` and `on_store_drain` (quarantine listing under `<root>/quarantine/main`), `resolve_hand_spec` and `build_hand_spec` (`HandSpec.store_root`), `cmd_run`, `ember_inner`. No `join("store")` remains in `crates/arx/src` outside test modules.
- `StoreError::NoScope` carries the root: `NoScope { scope: String, root: PathBuf }`, Display `scope '{scope}' has no directory under the store root {root}; mkdir {root}/{scope} to register it`. Registration itself is unchanged: nothing creates the directory (`f09b9ed4`).
- The container hand's mask for an in-tree store (`plan` in `container_hand.rs`, the `spec.store_root.strip_prefix(toplevel)` branch) is untouched. A root outside the toplevel is not in the mounted tree, so no mask is pushed and the hand still never sees the store.
- The repository's `arx.toml` gains `root = "~/.local/share/arx/store"` under `[store]`.
- `store/` leaves the repository: `store/arx.jsonl` removed from the tree, `store/.git`, `store/.gitignore` and `store/quarantine/` deleted from disk, the directory gone. Nothing is added to the root `.gitignore`: a `store/` reappearing here means a config without the key was run in the repo, and that should show in `git status`.
- `check-config` output is unchanged. `arx events`, `arx gauge`, the sidecar and the standing prompt never learn the root and are untouched.
- Test fixtures and scratch workspaces never set the key, so they keep `<config dir>/store`. Locked counts move only where this brief adds tests.

## Ground rules

- Never commit, never switch branches, never stage anything. `pid.txt` at the repo root is a tracked leftover; leave it alone in the tree.
- No inline comments. Unslop applies to the verdict.
- If a locked count or a "byte-identical" clause cannot hold as written, STOP on that scope and report the exact conflict with evidence rather than bending the count or the test.
- `crates/arx` has no `lib.rs`; `config.rs` and the other modules are `#[path]`-included by test binaries, so an inline `#[cfg(test)]` module in any of them registers twice. Unit tests go in `main.rs`'s test module.
- Baseline first: on the untouched branch run `cargo test -p arx 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s}'`. It must print `453` over nineteen binaries (measured on `main` this morning). If it does not, STOP and report the number before touching anything.

## Execution plan

Line numbers are from `main` at `4f3a051`; locate by the function name given beside each.

### Scope 1 — config: the key and the resolved root

- **Files:** Modify `crates/arx/src/config.rs` (`StoreConfig` at 57-63, `ConfigError` from 96 with `EmptyStoreEmbedder` at 135 and its Display at 187, `Resolved` at 255-269, the `agents_dir` and `sidecar_dir` lines 476-477, the store block 638-643, the `Resolved { … }` literal at 662), `crates/arx/src/main.rs` (the two `Resolved { … }` test literals: one at 3969 inside the hand-spec test, one in `bare_resolved` at 4021; the unit tests below in the same test module)
- **Build:** `[store] root` loads into `resolved.store_root`, absolute, with its two refusals.
- **Steps:**
  1. `StoreConfig.root: Option<PathBuf>` with `#[serde(default)]`.
  2. `ConfigError::EmptyStoreRoot` and `ConfigError::StoreRootHome`, Display texts exact per Decisions locked.
  3. `pub fn resolve_store_root(config_dir: &Path, root: Option<&Path>) -> Result<PathBuf, ConfigError>` in `config.rs` implementing the cases in Decisions locked, the tilde read from `std::env::var_os("HOME")`, the final absolutising join against `std::env::current_dir()` (a `current_dir` failure leaves the path as it is, as `cmd_run` does). `load` calls it beside the store-scope block and puts the result in `Resolved.store_root`.
  4. Additive `store_root: PathBuf::from("store"),` in both test literals in `main.rs`; zero deletions.
  5. Three unit tests in `main.rs`'s test module, each writing an `arx.toml` into a `tempfile::tempdir` and calling `config::load` on it (copy the `[main]` lines from `tests/fixtures/run/arx.toml`, with `sidecar` pointing at any existing directory): `store_root_defaults_to_store_under_the_config_dir` (no `root`; the result is absolute and ends in `<config dir>/store`); `store_root_expands_tilde_and_takes_absolute_and_relative_paths` (`~/x/store` → `HOME` + `/x/store`; `/abs/store` → itself; `elsewhere/store` → absolute, ending in `<config dir>/elsewhere/store`); `empty_store_root_is_refused` (`root = "  "` → the exact Display text).
  6. Tripwire: `cargo run -q -p arx -- check-config ./arx.toml` byte-identical to `main` after this scope. Stop and report otherwise.
- **Verify:** the unit binary (`cargo test -p arx --bin arx`) at 246; `cargo test -p arx --test check_config` at 13.

### Scope 2 — one root for every consumer, and a refusal that names the directory

- **Files:** Modify `crates/arx/src/main.rs` (`compose_dynamic_block` at 181, its quarantine line 210; `on_store_drain` at 328, line 400; `resolve_hand_spec` at 527, lines 531-534; `build_hand_spec` at 567, lines 579-582; `cmd_run` at 2422, lines 2463-2466; `ember_inner` at 3116, lines 3118 and 3126-3129), `crates/arx-engine/src/lib.rs` (`StoreError::NoScope` at 124, its Display 148-151), `crates/arx-engine/src/engine/mod.rs` (`require_scope` 244-256; the `matches!` at 1066 keeps `{ .. }`)
- **Build:** the resolved root is the only root; the refusal prints the path.
- **Steps:**
  1. Replace each of the six derivations with `resolved.store_root.clone()`, joined with `quarantine/main` for the two listings. `compose_dynamic_block` and `on_store_drain` keep their `config_dir` parameter for the other paths they build. `ember_inner` binds `let resolved = config::load(path)…?;` where today it discards the result.
  2. `NoScope { scope, root }` with `root: self.root.clone()` at the `require_scope` site; Display exact per Decisions locked.
  3. Tripwire: `grep -n 'join("store")' crates/arx/src/*.rs` lists only the test-module lines in `ledger.rs` (677 and 695) and nothing else. The `/tmp/arx-*-hand-test/store` literals in `claude_hand.rs` and `codex_hand.rs` are test-only and stay. Anything else: stop and report.
  4. Tripwire: `grep -rn 'has no directory under the store root' crates/arx/tests crates/arx-engine`. If a test asserts the old text, update that assertion to the new text and list before and after in the verdict; nothing is deleted.
- **Verify:** `cargo test -p arx-engine` at 45 plus 1 ignored; `cargo test -p arx --test store --test claude_main --test checkpoint --test state_object` at 7, 73, 6 and 7.

### Scope 3 — the done-when as tests

- **Files:** Modify `crates/arx/tests/store.rs` (two tests after `save_returns_landscape_and_commits_nothing`; a variant of `store_workspace` that takes the root); `crates/arx/tests/common/mod.rs` only if a session runner returning exit status and stderr is missing
- **Build:** a root outside the workspace is used end to end; an unregistered scope names its directory.
- **Steps:**
  1. `a_root_outside_the_workspace_is_the_store_the_session_uses`: build the workspace as `store_workspace` does, with a second `tempfile::tempdir` as the root and `root = "<its absolute path>"` under `[store]`; `plant_engine_scope` there instead of under the workspace. Drive one session with the stub the way `save_returns_landscape_and_commits_nothing` does (one save-tool line) and assert: the `system` row's grounding names the planted hot claim's answer (`skylark`); `<root>/quarantine/main/` holds the one candidate; `<workspace>/store` does not exist; `head_count(&root)` is unchanged by the save.
  2. `an_unregistered_scope_is_refused_naming_the_directory_to_create`: the same workspace with the root a fresh empty `tempdir` path and no scope directory; run `arx run` and assert a non-zero exit and stderr containing `scope 'testscope' has no directory under the store root <root>; mkdir <root>/testscope to register it`; `<root>/.git` exists, since the open happened and the refusal came at grounding.
  Use whichever runner in `common` returns status and stderr. If none does, add `run_session_expect_failure(workspace, config, lines, env) -> (i32, String)` beside `run_session_with_stdout`, built the same way.
- **Verify:** `cargo test -p arx --test store` at 9; then `cargo test -p arx` → 458 over nineteen binaries, the baseline plus three in the unit binary and two in `store`, every other binary at its baseline count. `cargo test --workspace` green. `git diff --stat main -- crates/arx/tests/scripts/` prints nothing.

### Scope 4 — the repository's own store, and the docs

- **Files:** Modify `arx.toml`, `CLAUDE.md`, `CONTEXT.md` (the Store entry, line 179), `docs/wayfinder/maps/arx-v0/findings/carried-findings.md` (the `store/<scope>/` item under "Store, drain, judge"); Delete `store/` (the tracked `store/arx.jsonl` and the untracked `.git`, `.gitignore`, `quarantine/`)
- **Build:** the repository stops being a store; the docs say where the store is.
- **Steps:**
  1. `arx.toml`: under `[store]`, after `scope = "arx"`, add `root = "~/.local/share/arx/store"`.
  2. From the repo root: `rm -rf store`. Main stages the deletion of `store/arx.jsonl` at close; do not stage it yourself. `git status --short` then shows ` D store/arx.jsonl` and no other `store/` line; `ls store` fails.
  3. `CLAUDE.md`, Hazards: replace the first bullet (`store/` is the live file store…) with: `- The live store is not in the repo: \`[store] root\` in \`arx.toml\` puts it at \`~/.local/share/arx/store\`, its own git repository with a directory per scope, registered by hand with \`mkdir\`. A \`store/\` appearing in the repo means a config without the key was run here; delete it, never stage it. \`pid.txt\` at the root is a tracked leftover of the L1-close session: nothing writes it now, main's edit tool refuses it as arx state, and no change to it is ever staged. \`threads/\` is ignored.`
  4. `CONTEXT.md`, the Store entry: append one sentence to the end of the paragraph, before `_Avoid_`: `Its root is per user and outside any repository, \`[store] root\` in arx.toml (\`~/.local/share/arx/store\` on the owner's machine); a project's config names a scope inside it, never a store of its own.`
  5. `carried-findings.md`: tick the item beginning `A fresh workspace needs \`store/<scope>/\` before \`arx run\` starts` (`- [ ]` → `- [x]`) and append to it: ` Closed by L3-05: the refusal prints the absolute root and the \`mkdir\` line.`
  6. Tripwire: `cargo run -q -p arx -- check-config ./arx.toml` byte-identical to `main`; the new key prints nothing.
- **Verify:** `git status --short` lists ` D store/arx.jsonl`, the edited files, and no untracked path under `store/`; the tripwire holds.

### Scope 5 — live check on the owner's login

- **Files:** none in the repository. `~/.local/share/arx/store/arx/` is created on the owner's machine, the registration act `f09b9ed4` reserves for the owner, done here on the owner's word from this morning's ruling.
- **Steps:**
  1. `mkdir -p ~/.local/share/arx/store/arx`.
  2. From the repo root: `printf 'reply with the single word ARX-L3-05-OK and nothing else\n' | cargo run -q --release -p arx -- run`. Expect the real main's reply on the terminal and a clean exit when stdin closes. Then: no `store/` in the repo; `git -C ~/.local/share/arx/store log --oneline | wc -l` prints `1`; `~/.local/share/arx/store/quarantine/main/` exists; `threads/main.jsonl` gained rows; `git status --short` shows nothing under `store/`.
  3. From `/tmp`: `cd /tmp && printf 'reply with the single word ARX-L3-05-CWD and nothing else\n' | arx`. `arx` is the owner's `~/.local/bin/arx`, a wrapper that changes into the repo, builds the release binary quietly and execs `run`. Expect the reply, the same store (still one commit, no `/tmp/store`, no `store/` in the repo).
  4. Negative: `mv ~/.local/share/arx/store/arx ~/.local/share/arx/store/arx.off`, run step 2's command again, expect exit 1 and stderr `error: scope 'arx' has no directory under the store root ~/.local/share/arx/store; mkdir ~/.local/share/arx/store/arx to register it`, then `mv` it back.
- **Verify:** the verdict quotes both replies, the refusal line, the `git log` count and the `ls` results.

## Chosen without a grill

1. The default root stays `<config dir>/store` when the key is absent, rather than `~/.local/share/arx/store` for everyone: every fixture and scratch workspace relies on the in-tree default, and a per-user default would point tests at the owner's real store. The user-level default belongs to the config-discovery want (`60058bb2`), not here.
2. Tilde expansion by hand, `HOME` only: the tracked `arx.toml` must not carry `/home/user`, and one prefix check is smaller than a crate.
3. The path is made absolute at load with the current directory, as `cmd_run` did, so a bind mount or a `strip_prefix` against the git toplevel never meets a relative root.
4. The refusal prints `mkdir <root>/<scope>` rather than creating the directory: `f09b9ed4` holds (a typo from main must not mint a scope); `arx init <scope>` is the want's shape and waits for it.
5. `NoScope` widens by a field rather than growing a second variant: one refusal, one text.
6. No `.gitignore` entry for `store/`: a store inside the repo is now a mistake and should show.
7. `check-config` stays silent on the root: its output is locked byte for byte by two cards, and the refusal carries the path when it matters.
8. The two FileStore-era rows are dropped, not converted: the owner's word, and one was a probe row marked for deletion when it was planted.

## Out of scope

`arx init <scope>`, `run` as the default subcommand, config discovery from the current directory, a user-level `~/.config/arx`, the sidecar's own home, `cargo install`: the want `60058bb2`, for the polishing day. A root under `$XDG_DATA_HOME`. Printing the root from `check-config` or `gauge`. Any change to the engine's on-disk shape, `Engine::open`'s git init, scope registration or `RESERVED_SCOPES`. Editing `l1-close.json` or `l2-close.json`. Any event shape. Migrating anything from `store/arx.jsonl`.

Done when every Verify above holds, `cargo test --workspace` is green with no test deleted, `check-config ./arx.toml` is byte-identical to `main`, the repository has no `store/`, and the live check ran on the owner's login with the quoted evidence.

## Review handoff

solo: dispatch spec-runner on `docs/specs/2026-09-08-l3-05-store-root-outside-the-repo.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
