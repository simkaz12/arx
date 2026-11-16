---
id: L1-06
layer: 1
labels: wayfinder:task
status: closed
assignee: none
blocked-by: L1-03
ruled-by: D12 (`e79bed85`), D01 (`40ebd8d3`), `a82c08c5` (unticketed owner ruling — mid-session recall stays, results tail-appended), D16 (`e652ce3a` — main's mounted surface carries save and drain), D13 (`c775b5c2` — Claude Code stays on v1 and never sees this store; `da0b0d8b` — the trait carries the save lifecycle from the start)
---

# L1-06 — the memory seam with a file adapter

## Deliverable

The store interface main is mounted on — save, recall, decisions_query, decision, drain — as a trait in the engine crate, with one adapter over a flat file per scope. The trait carries the ruled save shape from the start so L2-04 swaps in behind an unchanged signature: save takes the intent, returns a landscape, and lands the record in a per-thread quarantine file — never the store directly (`da0b0d8b`); drain promotes quarantined records into the store file, main minting. In the thin adapter the landscape is a substring match and the promotion is mechanical. Grounding pushed into main's window at spawn as the per-thread section (the file's hot and ember entries); recall, decisions_query and decision mounted as ordinary tools whose results append at the tail. No retrieval beyond substring match, no lineage, no tiers beyond a label in the file. L2-04 puts Tabularium v2 behind the same trait and L2-05 swaps the adapter; nothing at the call sites moves.

## Done when

A claude-code main starts with grounding visible in its first turn; a recall call mid-thread returns file hits as a tool result; a save from main returns a landscape and lands in the quarantine file with the thread handle, leaving the store file untouched; a drain promotes it into the store file; a second stub adapter compiles against the trait with no change at main's call sites.

## Ruled by

D12 (`e79bed85`), D01 (`40ebd8d3`), `a82c08c5`, D16 (`e652ce3a`), D13 (`c775b5c2`, `da0b0d8b`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
