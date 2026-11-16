---
id: B19
labels: wayfinder:task
status: open
assignee: none
blocked-by: B13, B10
ruled-by: D12 (the capture sweep rules — `D12-capture-sweep-rules.md` and its ids), D04 (`6aab1d24` — synchronous on the child's return path), D13 (`27e7cea9` — drain is two pens), `3a873e9a` — durable claims minted only by main
---

# B19 — capture sweep and drain

## Deliverable

The sweep that runs on a hand's return path before the verdict lands: what is captured from the hand's transcript, where it lands (material on disk under the session, never the store), and what the sweep may never do; the drain as two pens — a dispatched hand of a different family proposing, main minting — replacing B11's stub.

## Done when

A returning hand's transcript is on disk under the session with provenance before main sees the verdict; the sweep never writes to the store (verified by the store's commit log); a drain run proposes candidates from a seeded transcript and only main's confirmation writes a record.

## Ruled by

D12 (the capture sweep rules — `D12-capture-sweep-rules.md` and its ids), D04 (`6aab1d24` — synchronous on the child's return path), D13 (`27e7cea9` — drain is two pens), `3a873e9a` — durable claims minted only by main — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
