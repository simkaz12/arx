---
id: L1-10
layer: 1
labels: wayfinder:task
status: closed
assignee: none
blocked-by: L1-01, L1-07
ruled-by: D02 (`975f65cd`, `48af6ac7`), D07 (`7dae481c`, `e65b2a35`), T01 (`../arx-design/findings/T01-findings.md`), `6bbde714` as restated by `af68d429`; owner ruling 2026-08-30 — one provider, one turn is layer 1's native width
---

# L1-10 — native: the provider trait, one adapter, one turn

## Deliverable

The stateless provider trait — `(system, messages, tools) → stream`, no capability flags, nothing the loop branches on mid-flight — with one OpenAI-compatible adapter taking a base_url, and a loop that completes exactly one turn: one request, its stream, its tool calls executed, the result appended, no second request. Run in the hand role through L1-07 on the host. Fatal surfaces; every other error class, the retry wrapper, the window gauge, rewind, the Anthropic adapter and multi-turn are L2-10. Sized for one gauntlet session by owner ruling.

## Done when

A native hand definition on a DeepSeek-class model over base_url completes one turn from a brief and its verdict lands in main's window; a second adapter stub compiles against the trait with the loop unchanged; the loop has no code path that inspects which adapter it holds.

## Ruled by

D02 (`975f65cd`, `48af6ac7`), D07 (`7dae481c`, `e65b2a35`), T01, `6bbde714` as restated by `af68d429` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
