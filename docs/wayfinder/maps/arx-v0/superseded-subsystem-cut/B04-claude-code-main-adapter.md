---
id: B04
labels: wayfinder:task
status: open
assignee: none
blocked-by: B01, B02, R01
ruled-by: D16 (`705e4df2`, `e652ce3a`, `e4510547`, `7b607c7e`, `68c366dc`, `ceb0390f`, `96ce60c0`), `890ebff2`
---

# B04 — claude-code main adapter

## Deliverable

The claude-code adapter in the main role, on the route R01 recommends: arx spawns Claude Code as a child it owns, replaces the body's system prompt with arx's own, suppresses CLAUDE.md discovery and pushes the repo instruction file itself when the agent definition says so, turns autocompact off, strips the body's built-in tools, streams every turn into the transcript log, reads usage from the stream into an occupancy gauge, and can kill the child. No hands, no gate, no memory yet — those are B07, B11, B13.

## Done when

`arx run` starts a claude-code main; a typed prompt streams a reply; the transcript log holds it; `arx gauge` shows the body's own token count for the window; the body's session file on disk is never opened by arx (verified with strace or the equivalent); killing the child leaves the log intact.

## Ruled by

D16 (`705e4df2`, `e652ce3a`, `e4510547`, `7b607c7e`, `68c366dc`, `ceb0390f`, `96ce60c0`), `890ebff2` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

**Gained from R01 (2026-08-30):** the route is the SDK sidecar — a Node or Python process arx spawns, supervises (health, restart) and bridges over stdio JSON, hosting the Agent SDK with `canUseTool` forwarded to arx (`findings/R01-claude-code-drive.md`, Candidate 2). The sidecar is part of this card's deliverable. Verified switches: `systemPrompt` replaces outright; `settingSources: []` stops CLAUDE.md loading; `--tools ""` / tool presets strip built-ins while MCP-mounted tools stay; `usage` rides on every `SDKAssistantMessage`; `Query.interrupt()` plus SIGINT; a fresh session by omitting resume, `--no-session-persistence` for no artifact. Compaction is not a boolean: set the `--autocompact` / `CLAUDE_CODE_AUTO_COMPACT_WINDOW` ceiling above arx's own limit (`c474a7f4`, superseding `e4510547`). Open before the brief: whether the `control_request`/`control_response` envelope under `canUseTool` is a stable wire protocol a Rust process could speak on plain stdio — if yes, the sidecar disappears; the hook route (Candidate 3) is the owner's call, not this card's.
