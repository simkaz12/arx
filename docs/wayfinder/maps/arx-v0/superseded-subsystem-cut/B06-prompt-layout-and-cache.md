---
id: B06
labels: wayfinder:prototype
status: open
assignee: none
blocked-by: B04
ruled-by: `bb81d2c6` (first section of the V0 brief), D16 (`68c366dc`), D03 (`a82c08c5` recall appends at the tail, cache breaks only before the divergence point), D02 (`975f65cd`)
---

# B06 — system prompt layout and cache discipline

## Deliverable

A draft of arx's system prompt for a claude-code main, laid out for the prefix cache: the fixed prefix (identity, tool surface, the standing rules), the per-thread section (grounding push, the state object at respawn, the repo instruction file), and the rule for what may never move once a thread starts. The owner reacts to the draft; the resolution is the layout and the discipline, written into the adapter.

## Done when

The prompt runs in B04's adapter; two consecutive turns show a cache hit on the prefix in the body's usage stream; a grounding push or a respawn changes only the per-thread section; the layout is documented in one file the adapter reads.

## Ruled by

`bb81d2c6` (first section of the V0 brief), D16 (`68c366dc`), D03 (`a82c08c5` recall appends at the tail, cache breaks only before the divergence point), D02 (`975f65cd`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
