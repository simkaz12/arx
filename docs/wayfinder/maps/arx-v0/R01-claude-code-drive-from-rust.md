---
id: R01
labels: wayfinder:research
status: closed
assignee: none
research-dispatched: 2026-08-30
resolved: 2026-08-30
blocked-by: none
ruled-by: D16 (`e652ce3a`, `96d1ad3a`, `e4510547`, `68c366dc`, `ceb0390f`), D06 (`ce46b9ea`)
---

# R01 — driving Claude Code from Rust with a CanUseTool callback

## Deliverable

The design assumes a Claude-Code-bodied main runs over the Agent SDK because only the SDK exposes the `CanUseTool` callback the gate needs. arx is Rust and the SDK is TypeScript and Python. Primary sources only: which surface gives a Rust parent that callback — `claude -p --output-format stream-json --input-format stream-json` with `--permission-prompt-tool`, a thin SDK sidecar process arx owns, or the CLI's MCP permission hook — and for each: autocompact off, per-message usage on the stream, `disallowedTools: ["*"]` with mounted tools, system prompt replacement, CLAUDE.md discovery suppressed, and `interrupt()` or its equivalent. Return a cited findings file naming the route B04 should take and what each alternative loses.

## Done when

`findings/R01-claude-code-drive.md` exists, cites official docs or the CLI/SDK source for every claim, and ends with one recommended route and the switches it verified.

## Ruled by

D16 (`e652ce3a`, `96d1ad3a`, `e4510547`, `68c366dc`, `ceb0390f`), D06 (`ce46b9ea`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Resolution

**Resolved 2026-08-30.** Findings in [`findings/R01-claude-code-drive.md`](findings/R01-claude-code-drive.md), against Claude Code 2.1.240 and the live docs. Three routes give a Rust parent a per-call allow/deny verdict: an MCP `--permission-prompt-tool`, an SDK sidecar hosting `canUseTool`, and a `PreToolUse` hook with an `http` handler arx serves itself. All seven switches verified for each, with one correction: there is no `autoCompactEnabled: false` — the lever is `--autocompact <tokens>` / `CLAUDE_CODE_AUTO_COMPACT_WINDOW`, a ceiling up to 1M, not a boolean (`c474a7f4` supersedes `e4510547`). Recommendation for B04: the SDK sidecar, the ruled route (`e652ce3a`, `0bd4df2f`), at the cost of a process class arx must supervise and a second runtime. Surfaced for the owner, not decided here: the hook route is materially cheaper (an HTTP server in Rust, no sidecar, no MCP handshake) and `96d1ad3a`'s rejection of hooks was about a hand's body arx does not own, which does not transfer to main — a design reopen if wanted, never a quiet substitution. B04's blocking edge on R01 is released; its route is the SDK sidecar unless the owner reopens.
