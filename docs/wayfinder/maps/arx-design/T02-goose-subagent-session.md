---
id: T02
labels: wayfinder:research
status: closed
assignee: none
blocked-by: none
research-dispatched: 2026-08-24
resolved: 2026-08-24
---

# T02 — Goose's sessions, subagents, cancellation, persistence, read in source

## Question

How does Goose represent a session, spawn and supervise subagents, propagate cancellation, and persist/resume — what shape are the modules arx would lift, and what happens to a subagent's context when it ends? Deliverable is a cited grounding artifact (file:line) plus the decision questions it exposes for arx's subagent↔trace-archival seam and session persistence.

## Resolution

Findings: [findings/T02-findings.md](findings/T02-findings.md), from commit `8d844eec`.

Persistence is SQLite, three tables, rehydrate-on-demand resume — no serialized agent object; the sourcing map's "G (SQLite)" row holds. Subagent spawn is structurally a tool call: `summon` sits in the same platform-extension registry as memory and todo, so Goose has no harness-owned subagent seam distinct from tool dispatch — the memory-at-the-leaf arrangement arx rejects turns out to cover orchestration too. A finished subagent's transcript persists as an ordinary, fully assertable session row, merely type-filtered out of default listings — no evidence tier, nothing preventing trace content re-entering a window as fact. Cancellation lives in two disjoint registries (per-session tokens vs summon's own background map) that don't cross-reference, and subagent permission escalation to the parent doesn't exist, which is why subagents are forced to auto mode. Graduated into [D04](D04-subagent-trace-seam.md).
