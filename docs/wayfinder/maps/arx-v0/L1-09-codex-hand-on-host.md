---
id: L1-09
layer: 1
labels: wayfinder:task
status: closed
assignee: none
blocked-by: L1-07
ruled-by: D16 (`e652ce3a`, `0bd4df2f` — hands over `codex exec`, `68c366dc`, `ceb0390f`), `../arx-design/findings/codex-cli-drive-surfaces.md`, R02 (`findings/R02-codex-drive-gaps.md`)
---

# L1-09 — codex hand on the host

## Deliverable

The codex adapter in the hand role: `codex exec --json` spawned by L1-07 on the host, no container, base instructions replaced by arx's, AGENTS.md discovery off per R02 (`project_doc_max_bytes=0`), the verdict captured from the JSON stream, usage read once at `turn.completed`. This adapter is also what the judge (L1-11) dispatches through. Clamp and container are L2-09.

## Done when

A dispatched codex hand completes a read-only brief and its verdict lands in main's window; an AGENTS.md placed in the working directory is verifiably not read; the usage line from `turn.completed` is in the log.

## Ruled by

D16 (`e652ce3a`, `0bd4df2f`, `68c366dc`, `ceb0390f`), `../arx-design/findings/codex-cli-drive-surfaces.md`, R02 — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
