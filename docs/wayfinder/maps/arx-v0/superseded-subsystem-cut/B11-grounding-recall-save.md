---
id: B11
labels: wayfinder:task
status: open
assignee: none
blocked-by: B10, B04
ruled-by: D12 (`e79bed85`), D01 (`a82c08c5`, `40ebd8d3`), D16 (`e652ce3a` — main's mounted surface carries save and drain), D13 drain rulings (`27e7cea9`)
---

# B11 — grounding push, recall tool, save and drain on main

## Deliverable

Grounding pushed into main's window at spawn — hot plus ember for the thread's scope — as part of B06's per-thread section; recall, decisions_query and decision mounted as ordinary tools whose results append at the tail; save and drain on main's mounted surface, drain as two pens (a dispatched hand of a different family, per `27e7cea9`) — the drain's hand side waits on B13 and is stubbed until then.

## Done when

A claude-code main starts with grounding visible in its first turn; a recall call mid-thread returns store hits as a tool result; a save from main lands as a record in B10's store with the thread handle; the B06 cache check still passes after a recall.

## Ruled by

D12 (`e79bed85`), D01 (`a82c08c5`, `40ebd8d3`), D16 (`e652ce3a` — main's mounted surface carries save and drain), D13 drain rulings (`27e7cea9`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
