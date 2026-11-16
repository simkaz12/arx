---
id: B08
labels: wayfinder:task
status: open
assignee: none
blocked-by: B04, B05, B03
ruled-by: D15 (`adec3734`, `18fec68a`, `18b4928b`, `253334ad`, `b03f77fc`), D07 (`7dae481c`), D16 (`952d3b69` — ritual turns stay in the window under a CLI main), D05 (hands in flight run on across a checkpoint)
---

# B08 — state object and checkpoint

## Deliverable

The checkpoint on a token-delta clock between turns, or by operator command: the full ritual every time — authoring turn, harness-prompted truth-check turn, mechanical check on every write — each authored fresh from the window, never from the previous object; header-level provenance; `(thread, turn)` handles on the header. Under a CLI main the ritual turns stay in the window (no rewind); the mechanic that would rewind them out is a native-main concern and stays fog. The state object's contents: position, target, drift, hands in flight.

## Done when

A synthetic thread crosses the token-delta twice and produces two state objects, each passing the mechanical check; a deliberately false position in the authoring turn is caught by the truth-check turn; the object's header handle resolves in the transcript log; the checkpoint event is in the log.

## Ruled by

D15 (`adec3734`, `18fec68a`, `18b4928b`, `253334ad`, `b03f77fc`), D07 (`7dae481c`), D16 (`952d3b69` — ritual turns stay in the window under a CLI main), D05 (hands in flight run on across a checkpoint) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
