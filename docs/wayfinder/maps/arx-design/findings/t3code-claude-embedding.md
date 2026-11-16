---
labels: wayfinder:findings
source: hand-ex trace of github.com/pingdotgg/t3code at 5d7665396083d285132d67038813862a93337ca5 (2026-08-25, shallow clone, deleted after reading)
provenance: ad-hoc — no research ticket; produced mid-D04-grill on an owner question
grounds: D04 (child→parent approval path, capture/lifecycle as counter-example), D06 (permission modes, containment)
---

# t3code — how it embeds Claude Code

T3 Code is not a Claude Code fork. It is a multi-agent coding-tool host (web/desktop/mobile clients plus a server) that plugs five agent runtimes — Claude, Codex, Cursor, Grok, OpenCode — behind one internal driver interface. Every claim below is code-verified by a hand-ex at the recorded commit.

## Integration mechanism

Claude Code enters as a **library**: `@anthropic-ai/claude-agent-sdk`, pinned `0.3.170` (`pnpm-lock.yaml:460-462`, `apps/server/package.json:25`). The server imports the SDK's `query()` and drives one long-lived async session per thread. The SDK still forks the local `claude` binary under the hood (`pathToClaudeCodeExecutable`), but T3 Code's own code never touches CLI flags or stdout parsing — prompts in, typed `SDKMessage`s out, permission callback, session id, all across an SDK object interface.

Flow: `ClaudeDriver.create` (`apps/server/src/provider/Drivers/ClaudeDriver.ts:111-221`) → `makeClaudeAdapter` → `createQuery` wrapping SDK `query()` (`Layers/ClaudeAdapter.ts:1719-1728`) → per-turn options built at `ClaudeAdapter.ts:4307-4347`, consumed by a forked fiber (`4481-4502`) that translates each `SDKMessage` into T3 Code's canonical `ProviderRuntimeEvent`s.

## Auth

The user's own Claude Code login — the operator runs `claude auth login` normally and T3 Code points the SDK-spawned CLI at that account via `CLAUDE_CONFIG_DIR` (`Drivers/ClaudeHome.ts:17-35`; `HOME` is deliberately not overridden because that breaks the macOS keychain OAuth lookup). Alternative: API-key/router passthrough via `ANTHROPIC_BASE_URL`/`ANTHROPIC_AUTH_TOKEN`/`ANTHROPIC_API_KEY` as per-provider-instance env vars. Credentials never proxy through T3 Code's servers.

## Control surfaces

- **Per-tool veto**: the SDK's `CanUseTool` callback (`ClaudeAdapter.ts:4253-4254`, ~`4090-4251`) maps every tool call to T3 Code's own approval RPC; a human answers allow / allow-session / deny / cancel and a `PermissionResult` returns into the SDK. This is the real gate, independent of permission mode.
- **Permission mode**: four T3 modes map onto the SDK's `PermissionMode` (`4284-4289`); full access sets `bypassPermissions` plus `allowDangerouslySkipPermissions` (`4321-4323`). "Supervised" leaves mode unset and rides entirely on the callback.
- **System prompt**: stock Claude Code preset, kept not replaced (`4311`).
- **Settings**: the CLI still reads its normal `user`/`project`/`local` settings and skills hierarchy (`4312`, `1243-1247`).
- **MCP**: exactly one server mounted — T3 Code's own, over HTTP with a bearer token, per-thread (`4334-4345`).
- **Containment**: none beyond the callback plus a directory allowlist (`cwd` + attachments dir, siblings like `secrets/` excluded, `4303-4306`). No container, no worktree machinery; docs note Codex, by contrast, gets sandbox-level translation.

## Lifecycle

Session-per-thread, not spawn-per-turn: one `query()` call, later turns pushed onto a `promptQueue` into the same iterable, multi-turn state inside one child process. Resume across host restarts stores only the Claude session UUID plus a turn count and hands the id back to the SDK — **T3 Code never reads or parses the CLI's on-disk transcript**. Interrupt is a hard boundary: `interruptTurn` → `stopSessionInternal` (`4622-4630`, `3690-3790`) closes the query and lets the SDK escalate SIGTERM→SIGKILL; then pending approvals/tasks are cancelled and a scope finalizer (`4719-4727`) sweeps live sessions on shutdown. **There is no capture step after death** — everything T3 Code keeps came off the live message stream while the process was alive.

## Output path

Typed `SDKMessage`s, no stream-json parsing: (a) translated to canonical events streamed to clients; (b) optionally mirrored raw to a rotating NDJSON file per thread (`Layers/EventNdjsonLogger.ts`, 10 MiB/file, 512 MiB total, 14-day cap) — T3 Code's own observability log, separate from the CLI's session files, which it never opens.

## What arx takes from this

- **The child→parent approval path exists and ships**: the SDK's `CanUseTool` callback is exactly the forwarding Goose lacks (T02's `ActionRequired` gap). A CC-bodied child's veto can be surfaced to the parent rather than forced to auto — but only via the SDK interface, not the bare CLI. D04's escalation bullet now has a working production shape to argue against on both sides (Goose: none; t3code: callback-RPC-human).
- **The opposite capture philosophy**: T3 Code keeps only what crossed the live stream and never sweeps the corpse; its NDJSON mirror is retention-capped observability, not evidence. arx's capture-surface sweep is the inverse commitment — the trace outlives the process precisely because it is taken from disk at teardown, crash included.
- **Session-per-thread with SDK resume** is what "the hand keeps its own state" looks like when the host refuses to parse the body's formats — same non-parsing stance as the sweep ruling (`7e5b09d7`), opposite persistence rail.
- **Containment corroborates D06's layering**: approval callback plus directory allowlist and nothing structural underneath — the same advisory-hole shape fusion-harness had, which `21e8f9a9`'s container-per-hand closes.

## Unknowns and confidence

The `t3-code` MCP server's actual tool surface was not opened; the SDK's internal spawn/PATH resolution lives outside the repo and was not inspected; the other four drivers were not traced. Confidence: high on mechanism, auth, lifecycle (source-cited, doc-corroborated); medium on the full control-surface inventory.
