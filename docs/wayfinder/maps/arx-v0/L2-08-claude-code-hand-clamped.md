---
id: L2-08
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-07
ruled-by: D16 (`e652ce3a`, `0bd4df2f`), D06 (`96d1ad3a`, `068d4cbf`), D04 (`5e8da52e` — a clamped foreign hand fails loud), D12 (`e79bed85`), `5f17e29e`
---

# L2-08 — claude-code hand in the container, clamped

## Deliverable

L1-08's adapter moved into L2-07's container and clamped at spawn by allowlist and permission mode, its credential injected off the container's designated paths with no host env and no home (`068d4cbf`), its own gate inside, a shortfall failing loud as the hand's verdict so main re-dispatches wider.

## Done when

L1-08's done-check passes with the hand in a container; the hand authenticates with the credential present off the designated paths and no host environment or home; the same hand with a brief needing a tool outside its clamp returns the shortfall as its verdict; the hand's raw transcript is still on disk under the session on return.

## Ruled by

D16 (`e652ce3a`, `0bd4df2f`), D06 (`96d1ad3a`, `068d4cbf`), D04 (`5e8da52e`), D12 (`e79bed85`), `5f17e29e` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

`BodyKind::ClaudeCode` now runs the container path exactly as `Fake` does — `SpawnLedger::dispatch` and `SpawnLedger::kill` route on a new `BodyKind::in_container()` (`Fake | ClaudeCode`), and the host-executed `claude_hand.rs` — `spawn_claude_hand`, `run_claude_hand`, `wait_for_exit`, `kill_and_reap`, `find_session_file`, `locate_and_copy_session`, `claude_config_dir`, the stdout/stderr readers — is gone. `claude_hand.rs` keeps `build_argv`, `build_verdict_text`, `generate_uuid_v4` and gains `permission_denials`, `CLAUDE_TOOLS` (13 names), `WRITE_TOOLS` (4) and `effective_tools`.

Before the container starts, `container_hand::prepare` reads `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/.credentials.json` from arx's own environment, takes the `claudeAiOauth` value alone (the file's other `mcpOAuth` block is dropped) and writes `{"claudeAiOauth": <value>}` to `<root>/home/.claude/.credentials.json`, mode 0600, `.claude/` at 0700 — off every path the sweep designates. Missing file, bad JSON or no `claudeAiOauth` key refuses dispatch with `dispatch: no Claude Code credential at <path>; log in on the host first`, root removed, no container started. `docker image inspect <image>` gates every body kind before any root exists; a claude-code definition with no `image:` key is refused at `resolve_hand_spec` before that.

The clamp compiles once at spawn from the definition's `tools` key (default empty → `Read,Glob,Grep,Bash`, plus `Write,Edit,MultiEdit` when write-enabled) into `--allowedTools <effective>` and `--disallowedTools <CLAUDE_TOOLS minus effective>`, `--permission-mode default`; the write axis strikes `Write`, `Edit`, `MultiEdit`, `NotebookEdit` from the effective set whatever the definition says. The image's own `claude --help` documents a third flag, `--tools <tools...>`, that specifies the available tools from the built-in set — the clamp rides a third time as `--tools <effective>`, beside the two flags, not instead of them.

`container_hand.rs` picks a `Wire` from the body kind — `Lines` for `Fake` (today's `stop:`/`verdict:` reader), `Json` for `ClaudeCode` (the brief written to stdin then the pipe closed before the control loop starts; stdout buffered whole, capped at `MAX_STDOUT_BYTES`, parsed once by the same `build_verdict_text` L1-08 wrote). After `wait()` and before `root.remove()`, the session `.jsonl` is globbed out of every subdirectory of `<root>/home/.claude/projects/` and swept into the trace as `session.jsonl`; not found appends `(capture failed: session file not found)` to the verdict text but still leaves a trace. `container::sweep` gained an `extra: &[(PathBuf, String)]` parameter for exactly this — `home/` is never swept wholesale, `.credentials.json` is never copied anywhere. `permission_denials` in the result becomes one `Event::Wall` per tool name, emitted before `Event::Delivered` at both drain sites; `Delivery::Verdict` carries the walls additively, `KINDS` stays 17.

The image is `docker/claude-hand/Dockerfile` — `FROM node:22`, `ARG CLAUDE_CODE_VERSION=2.1.258`, one `npm install -g`, nothing else — tagged `arx-hand-claude:2.1.258`, matching the host's own pinned version (`claude --version` inside the image reads `2.1.258 (Claude Code)`). `agents/hand-ex.md` carries `image: arx-hand-claude:2.1.258` and `tools: [Read, Glob, Grep, Bash]`; `check-config` stays byte-identical to `main`, the two keys add no column.

The test stub moved the same way: `tests/fixtures/bin/claude` (bash, host-executed) is gone, replaced by `tests/fixtures/claude-stub/{claude,Dockerfile}` — a 59-line POSIX `sh` script on `alpine:3.20` — built on demand into `arx-claude-stub:test` by `common::ensure_claude_stub_image()`, a `docker image inspect` behind a `std::sync::Once`. `common::plant_stub_credential(workspace)` writes the stub credential every test that dispatches a claude-code hand now needs. The three L1-08 tests in `claude_main.rs` kept their names and meaning, re-pointed at the trace directory (`session.jsonl`'s three lines, `trace.md`'s `paths`, `(trace: threads/main.h1)`) instead of the deleted host artifacts; `claude_in_container.rs` adds four: the credential present in the container and absent from the trace, the clamp reaching the child argv and striking the write tools for a read-only hand, a write-enabled hand's diff applying, and `permission_denials` becoming `Wall` events alongside both refusal shapes (no credential, no image).

Deviations, all disclosed to the reviewer as they happened:

- `common::claude_hand_path`/`stub_claude_bin_dir` were not deleted — `tests/fixtures/bin/` still holds the `codex` stub that several other tests (three `codex_hand_*` tests in `claude_main.rs`, `layer_close.rs`) still need on `PATH`; only `tests/fixtures/bin/claude` itself is gone.
- `codex_hand.rs` carries its own private `wait_for_exit`/`kill_and_reap`/`ExitOutcome`/`read_tail` now — it depended on these as shared infrastructure from `claude_hand.rs`, which this card empties of everything but the claude-code-specific clamp.
- `ledger.rs`'s `handle_delivery` widened its `already_recorded` check from "kind is not `Fake`" to `!kind.in_container()`, the same widening `kill()` needed, so the killed row still lands once for a container-bodied hand.
- `container_hand.rs`'s `Wire::Json` arm needed an explicit `drop(stdin)` after writing the brief — without it the pipe stayed open past the write and the stub's `cat` blocked forever, caught live against a real container before it ever reached a test run.
- `claude_main.rs`'s three claude-code tests mint the same `arx-h1-*` container name concurrently (default parallel test threads, no serialization, unlike `hands_in_containers.rs`) — a `CLAUDE_CODE_DOCKER_SERIAL` mutex and a 5s poll on the kill test's final `docker ps` check replace the single racy check.
- The write-diff test's target file is `claude-write-probe.txt`, not the card's literal `probe.txt` — `tests/fixtures/run/probe.txt` already exists, an unrelated tracked L1-10 fixture.
- `layer_close.rs`'s `l1_close_loop_on_host` plants the stub credential and ensures the stub image before its session now, per the amendment; its `claude-stub-argv` read became a read of `session.jsonl`'s `argv` line, its `(trace: threads/main.h1.jsonl)` assertion became `(trace: threads/main.h1)`. All twelve tests and every predicate name held.

No `.claude.json` fallback was needed anywhere — the live check's containers authenticated and ran on the copied `.credentials.json` alone.

The live check ran on the owner's login against the real sidecar and a real Claude Code main, in a scratch repository with one commit. The session directory under `/hand/repo`'s mount encodes as `-hand-repo`. `hand-ex` listing the top level named `README.md` and `src` among the mounted tree's other real entries, `session.jsonl` landing 7820 bytes, `trace.md` listing it. `hand-ex` asked to write `probe.txt` reported both halves of the clamp independently — the Write tool absent from its schema, then `Bash`'s own `printf`/`touch` failing on `/hand/repo`'s read-only mount — no `probe.txt` anywhere in the scratch tree. `hand-wr` given the same brief wrote it; `threads/main.h3/worktree.diff` applied cleanly, `probe.txt` reading `hello`, `base` matching the scratch repo's own `HEAD`. `sha256sum ~/.claude/.credentials.json` read identical before and after all three dispatches — no token write-back reached the host. `docker ps -a --filter label=arx.hand` and `/tmp/arx-hands` were empty after every dispatch and at the end; the scratch repository was removed.
