---
id: B16
labels: wayfinder:task
status: open
assignee: none
blocked-by: B13, B14, R02
ruled-by: D16 (`e652ce3a`, `0bd4df2f` — hands over `codex exec`, `68c366dc`, `ceb0390f`), `findings/codex-cli-drive-surfaces.md`, R02 findings
---

# B16 — codex hand

## Deliverable

The codex adapter in the hand role: `codex exec --json` in B14's container, base instructions replaced by arx's, AGENTS.md discovery suppressed per R02, same clamp and same fail-loud contract as B15, verdict captured from the JSON stream.

## Done when

A dispatched codex hand completes a read-only brief with its verdict in main's window; AGENTS.md in the worktree is verifiably not read; a clamp shortfall returns as the verdict.

## Ruled by

D16 (`e652ce3a`, `0bd4df2f` — hands over `codex exec`, `68c366dc`, `ceb0390f`), `findings/codex-cli-drive-surfaces.md`, R02 findings — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

**Gained from R02 (2026-08-30):** `-c project_doc_max_bytes=0` suppresses AGENTS.md at the filesystem-walk level — no read, no path discovery (`findings/R02-codex-drive-gaps.md` §1); usage arrives once, at `turn.completed`, cumulative, with no context-window field (§2). The hand's usage line is that one event; nothing mid-turn.
