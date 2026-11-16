---
id: T04
labels: wayfinder:research
status: closed
assignee: none
blocked-by: none
research-dispatched: 2026-08-24
resolved: 2026-08-24
---

# T04 — Steering-queue semantics as the field actually builds them

## Question

How do shipping harnesses implement steer/queue/abort — where does the queue drain, what "safe boundary" did each pick, and what failure reports do their trackers carry? Sources: Goose in source; the open issues on pydantic/pydantic-ai-harness, anomalyco/opencode, tiann/hapi named in SURVEY.md; Claude Code's observed behaviour from the local transcript corpus. Deliverable is a cited grounding artifact plus the decision questions it exposes for arx's steering↔step-boundary seam — the one BUILD item that is also the owner's daily friction.

## Resolution

Findings: [findings/T04-findings.md](findings/T04-findings.md).

SURVEY §1's three-way steer/queue/abort table is a real field *proposal* (opencode #32157, open, unimplemented), not shipped behaviour anywhere: the shipping convention is a **two-way** split — immediate-boundary steer and hold-to-idle queue — with abort as an orthogonal control action. SURVEY's "abort clears the queue" is contradicted by Claude Code's own docs, Goose's cancel-preserves-queue test, and opencode's stated intent; where queues actually get cleared it is either a deliberate second, harder verb or an open bug (two independent opencode filings). The safe boundary divides the field silently: per tool call (Claude Code observed in the local corpus, pydantic-ai) vs per full turn (Goose) — described with identical prose on both sides. HAPI's protocol probe shows steer is not universally implementable (Claude's stream-json has no non-destructive mid-turn primitive; Codex has a first-class `turn/steer` RPC). Graduated into [D05](D05-steering-semantics.md).
