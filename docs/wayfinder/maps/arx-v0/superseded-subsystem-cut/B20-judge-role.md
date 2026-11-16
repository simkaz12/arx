---
id: B20
labels: wayfinder:task
status: open
assignee: none
blocked-by: B16
ruled-by: D16 (`1e6ad121`, `da3f5418`, `0bd4df2f`), R02 findings on `--output-schema`
---

# B20 — the judge role

## Deliverable

The judge as a role in config and a hand at runtime: a dispatch under the reserved `judge` definition, read-only clamp, no save verb, no third lifetime class; the return an arx-defined verdict schema filled through `codex exec --output-schema`; a same-family-as-main check at dispatch that warns and never refuses. Codex's `review/start` is not used.

## Done when

Main dispatches the judge over a seeded diff and receives a verdict matching the schema; a judge definition set to main's own family logs the warning and still runs; a judge attempting a write is refused by its clamp and the refusal is in the log.

## Ruled by

D16 (`1e6ad121`, `da3f5418`, `0bd4df2f`), R02 findings on `--output-schema` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

**Gained from R02 (2026-08-30):** Codex enforces nothing on `--output-schema` — the strict `json_schema` format goes to the Responses API and the final message is written verbatim, a refusal dropped like an unknown SSE event (`findings/R02-codex-drive-gaps.md` §3). The judge driver wraps the final message in its own parse with an explicit failure verdict; "schema was set" is not a guarantee. Done-when gains: a stubbed empty final message yields a parse-failure verdict, never a panic or an empty pass.
