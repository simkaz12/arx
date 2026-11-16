---
id: B02
labels: wayfinder:task
status: open
assignee: none
blocked-by: none
ruled-by: D15 (`eff1f01a`), D16 (`7b607c7e`), D04 files-only persistence (`ba6fd355` as grown by `eff1f01a`)
---

# B02 — transcript log

## Deliverable

The per-thread append-only transcript log: every turn of main lands as it streams, arx-written from the body's stream so the body's own session file is never read; `(thread, turn)` look-back handles minted on every write; independent of rewind — a rewind changes the window, never the log. Machine-local in v1. The third persisted file beside the state object and the archive.

## Done when

A synthetic stream of N turns produces a log with N entries; every handle resolves to its turn's text; truncating the in-memory window leaves the log byte-identical; a second thread writes to its own log and handles never cross threads.

## Ruled by

D15 (`eff1f01a`), D16 (`7b607c7e`), D04 files-only persistence (`ba6fd355` as grown by `eff1f01a`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
