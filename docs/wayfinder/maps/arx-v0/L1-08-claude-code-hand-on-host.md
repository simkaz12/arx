---
id: L1-08
layer: 1
labels: wayfinder:task
status: closed
assignee: none
blocked-by: L1-07
ruled-by: D16 (`e652ce3a`, `0bd4df2f` — hands over `claude -p`), D12 (`e79bed85` — grounding as pushed text only), `5f17e29e`
---

# L1-08 — claude-code hand on the host

## Deliverable

The claude-code adapter in the hand role: `claude -p` spawned by L1-07 as a child on the host, no container, no clamp — the brief in, grounding pushed as text inside the brief and nothing else, the verdict out through the ledger; the hand manages its own window. Its raw transcript lands on disk under the session on return, as the placeholder L3-03's sweep replaces. The clamp, the container and the fail-loud shortfall are L2-08.

## Done when

A dispatched claude-code hand completes a read-only brief and its verdict lands in main's window at a step boundary; the hand's raw transcript is on disk under the session before main sees the verdict; the hand never reads arx's store file.

## Ruled by

D16 (`e652ce3a`, `0bd4df2f`), D12 (`e79bed85`), `5f17e29e` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
