---
id: B10
labels: wayfinder:task
status: open
assignee: none
blocked-by: B01
ruled-by: D13 (`c775b5c2`, `30e83788`, the D13 retrieval rulings in map.md's D13 bullet and `findings/debate/2026-08-27-arx-d13-recall-gate/ledger.md`), T08 (`findings/T08-findings.md` — text-only led at recall@1), D01 (`40ebd8d3`)
---

# B10 — the engine

## Deliverable

Tabularium v2 as a Rust crate over a file store that is a git repository: record kinds fact / decision / digest, scopes, supersession lineage with `supersedes` / `superseded_by`, lifecycle and validity, hot and ember tiers; retrieval text-first as T08 measured, brute-force in-process at this corpus size, no Qdrant; the verbs main will mount — save, recall, decisions_query, decision, drain — as library calls. Starts empty. Sync is clone, pull and merge. Claude Code stays on v1 and never sees this store.

## Done when

Over an empty store: save two decisions where the second supersedes the first, recall returns the successor and lists the predecessor as superseded, decisions_query finds both with `include_superseded`; a fact saved then invalidated is excluded by default and found with `include_invalid`; the store directory is a valid git repo with one commit per write; T08's recall fixture reproduces its recall@1 ordering.

## Ruled by

D13 (`c775b5c2`, `30e83788`, the D13 retrieval rulings in map.md's D13 bullet and `findings/debate/2026-08-27-arx-d13-recall-gate/ledger.md`), T08 (`findings/T08-findings.md` — text-only led at recall@1), D01 (`40ebd8d3`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
