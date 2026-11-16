# CLAUDE.md

**arx** — an agent harness: a Rust shell that owns main, runners, reviewers and hands under swappable bodies (CLI or provider), with memory policy behind its own engine. Built layer by layer; layer 1 closed 2026-09-03, layer 2 closed 2026-09-07.

## Orientation

`CONTEXT.md` is authoritative on vocabulary, `docs/wayfinder/maps/arx-v0/map.md` on the delivery effort, and `docs/wayfinder/maps/arx-reliability/map.md` on the audit repairs and carried backlog. Cards sit beside their map; briefs live under `docs/specs/`. `POSITION.md` records how the design got here. The design map under `docs/wayfinder/maps/arx-design/` is closed; a ruling is reopened in the Tabularium store, never inside a card.

## Hazards (not derivable from one file)

- The live store is not in the repo: `[store] root` in `arx.toml` puts it at `~/.local/share/arx/store`, its own git repository with a directory per scope, registered by hand with `mkdir`. A `store/` appearing in the repo means a config without the key was run here; delete it, never stage it. `pid.txt` at the root is a tracked leftover of the L1-close session: nothing writes it now, main's edit tool refuses it as arx state, and no change to it is ever staged. `threads/` is ignored.
- `crates/arx` has no `lib.rs`. Integration tests `#[path]`-include `src/*.rs`, so an inline `#[cfg(test)]` module in an included file registers twice and breaks locked test counts. Such tests go in `main.rs`'s test module.
- `cargo test -p arx` needs the Docker daemon and builds the stub images `arx-claude-stub:test` and `arx-codex-stub:test` on first run. The build is skipped whenever a tag exists, so after editing a script under `crates/arx/tests/fixtures/*-stub/` run `docker rmi` on that tag first or the tests keep running the old stub.

## Commands

`cargo test -p arx` · `cargo test --workspace` · `cargo run -p arx -- check-config ./arx.toml` · `docker build -t arx-hand-claude:2.1.258 docker/claude-hand` · `docker build -t arx-hand-codex:0.152.1 docker/codex-hand` · `npm install` in `sidecar/` once.

## Repo deltas (global rules apply and are not restated)

- **Card branch.** Every card is its own branch from `main`, named after the brief without date or extension (`l2-01-instrumentation-events`). Main creates it before dispatching spec-runner; the brief and the map's "briefed" entry ride on it uncommitted. Runners, reviewers and hands never switch branches or commit.
- **Card close, the one override of the global never-commit rule.** Owner ruled 2026-09-03. When a card lands (spec-runner PASS, spec-reviewer PASS with empty `must_fix`, frontier entry written in `map.md`), main commits it, one commit, subject the branch name, staged by path; pushes plain; opens a PR to `main` whose body carries the brief path, gate status, test totals and the reviewer's verdict; merges it with a merge commit (`gh pr merge --merge --delete-branch`); then `git switch main && git pull` and `/digest`. The PR is the record, not a second review. No rebase, no force, no `git add -A`. If a push or merge fails, keep the commit, quote the error, stop. Anything outside a landed card stays owner-committed.
- **Standing prompt.** `prompts/main.md` is what main is told, byte for byte, and it says only what the binary does. A card that changes what main experiences edits it in the same card. It cannot live under `agents/`: that filename is refused at config load.
