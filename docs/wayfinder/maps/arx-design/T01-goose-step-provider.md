---
id: T01
labels: wayfinder:research
status: closed
assignee: none
blocked-by: none
research-dispatched: 2026-08-24
resolved: 2026-08-24
---

# T01 — Goose's step and provider layer, read in source

## Question

What does Goose's step actually do, in code — request construction, streaming, chunk-to-event translation, termination handling, context/window management, and the provider trait's surface? Specifically: who owns request construction (the provider adapter or something upstream), where the window is assembled before a request, and what the loop assumes about both. Deliverable is a cited grounding artifact (file:line) plus the list of decision questions it exposes for arx's provider↔assembler and pressure↔assembler seams.

## Resolution

Findings: [findings/T01-findings.md](findings/T01-findings.md), from commit `8d844eec` (2026-08-21).

The harness owns request construction — the adapter receives an already-assembled, already-repaired `&[Message]` slice and only translates wire formats. The provider trait is one required method (`stream`) returning semantically complete Message chunks, plus a `manages_own_context()` capability flag the caller must branch on at two sites — the trait does not donate cleanly. No `stop_reason` crosses the provider↔harness seam; termination is structural inspection of the assembled Message. Two context-management mechanisms ship as harness-core (not extensions), and tool-pair summarization is a default-on, continuously firing instance of exactly the splice arx's op-set excludes — built on a flag-invisible-and-append primitive that never deletes storage, a live precedent for question 7's removal-as-demotion. Graduated into [D02](D02-provider-assembler-seam.md), and feeds [D03](D03-admission-seat.md), [D07](D07-step-shape-and-clocks.md), [D08](D08-removal-as-demotion.md).
