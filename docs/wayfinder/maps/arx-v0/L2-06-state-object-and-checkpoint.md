---
id: L2-06
layer: 2
labels: wayfinder:task
status: split
assignee: none
blocked-by: L2-01
ruled-by: D15 (`adec3734`, `18fec68a`, `18b4928b`, `253334ad`, `b03f77fc`), D07 (`7dae481c`), D16 (`952d3b69` — ritual turns stay in the window under a CLI main), D05 (hands in flight run on across a checkpoint)
---

# L2-06 — state object and checkpoint, hands in flight

## Deliverable

The checkpoint on a token-delta clock between turns, or by the operator's checkpoint command on the terminal: the full ritual every time — authoring turn, harness-prompted truth-check turn, mechanical check on every write — each authored fresh from the window, never from the previous object; header-level provenance; `(thread, turn)` handles on the header. Under a CLI main the ritual turns stay in the window (no rewind); the mechanic that would rewind them out is a native-main concern and stays fog. The state object's contents: position, target, drift, hands in flight — L1-07's ledger survives the checkpoint and the object lists it. Held off the frontier by `needs-split`; cut into session-sized cards at claim time (S-SOL-7).

## Done when

A synthetic thread crosses the token-delta twice and produces two state objects, each passing the mechanical check; a deliberately false position in the authoring turn is caught by the truth-check turn; the object's header handle resolves in the transcript log; the checkpoint event is in the log; a fake hand in flight across the checkpoint is listed in the object and its verdict lands afterwards.

## Ruled by

D15 (`adec3734`, `18fec68a`, `18b4928b`, `253334ad`, `b03f77fc`), D07 (`7dae481c`), D16 (`952d3b69`), D05 — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Split

Cut on 2026-09-04 by main under the owner's overnight clearance into [L2-06a — the state object and its mechanical check](L2-06a-state-object-and-check.md) (the file, its header and handle, the parser, the check, the config key) and [L2-06b — the checkpoint ritual in the loop](L2-06b-checkpoint-ritual.md) (the clock, the operator command, the three-part ritual, the event, hands in flight). The deliverable and done-when above are met when both land; this card is not claimed on its own.
