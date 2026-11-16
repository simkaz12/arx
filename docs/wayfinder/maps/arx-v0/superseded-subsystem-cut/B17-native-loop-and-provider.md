---
id: B17
labels: wayfinder:task
status: open
assignee: none
blocked-by: B01, B12
ruled-by: D02 (`975f65cd`, `48af6ac7`), D14 (`3a5aefb6`, `5fadfcc0`), D07 (`7dae481c`, `e65b2a35`), T01 (`findings/T01-findings.md`), `6bbde714` as restated by `af68d429`
---

# B17 — native loop and provider trait

## Deliverable

The Goose lift: the native loop over a stateless provider trait — `(system, messages, tools) → stream`, no capability flags, nothing the loop branches on mid-flight — with an OpenAI-compatible adapter taking a base_url (OpenRouter is a base_url, not a third adapter) and an Anthropic adapter; the error enum by continuation class with Transient retried by the stream wrapper before the first item, Fatal surfaced, ContextWindowExceeded standing alone; the window gauge; append and rewind as the op-set with no splice; admission (B12) in the loop. Built for the hand role first — the main seat is fog. Likely two sessions; split at claim time.

## Done when

The loop completes a tool-using conversation against a live OpenAI-compatible endpoint and against Anthropic from one agent definition each; a stubbed 429 is retried before the first stream item; a stubbed 400 surfaces as ContextWindowExceeded with the overshoot parsed; a 12k tool result is externalized by B12 inside the loop.

## Ruled by

D02 (`975f65cd`, `48af6ac7`), D14 (`3a5aefb6`, `5fadfcc0`), D07 (`7dae481c`, `e65b2a35`), T01 (`findings/T01-findings.md`), `6bbde714` as restated by `af68d429` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
