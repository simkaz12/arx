---
id: L1-03
layer: 1
labels: wayfinder:task
status: closed
assignee: none
blocked-by: L1-01, L1-02
ruled-by: D16 (`705e4df2`, `e652ce3a`, `e4510547` as superseded by `c474a7f4`, `7b607c7e`, `68c366dc`, `ceb0390f`, `96ce60c0`), `890ebff2`, `b8d59164` (the SDK sidecar affirmed, the hook route declined), R01 (`findings/R01-claude-code-drive.md`)
---

# L1-03 — claude-code main adapter

## Deliverable

The claude-code adapter in the main role, on the SDK sidecar route — `b8d59164` affirmed it over the `PreToolUse` http-hook alternative, borrowing the embedding t3code runs in production (SDK `query()`, `CanUseTool` forwarding, session-per-thread — `../arx-design/findings/t3code-claude-embedding.md`): arx spawns Claude Code as a child it owns, replaces the body's system prompt with arx's own, suppresses CLAUDE.md discovery and pushes the repo instruction file itself when the agent definition says so, sets the `--autocompact` window ceiling per `c474a7f4`, strips the body's built-in tools, streams every turn into the transcript log, reads usage from the stream into an occupancy gauge, and can kill the child. No hands, no gate, no memory — those are L1-07, L1-05, L1-06 and they all hang off this adapter.

## Done when

`arx run` starts a claude-code main; a typed prompt streams a reply; the transcript log holds it; `arx gauge` shows the body's own token count for the window; the body's session file on disk is never opened by arx (verified with strace or the equivalent); killing the child leaves the log intact.

## Ruled by

D16 (`705e4df2`, `e652ce3a`, `e4510547` as superseded by `c474a7f4`, `7b607c7e`, `68c366dc`, `ceb0390f`, `96ce60c0`), `890ebff2`, `b8d59164`, R01 — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
