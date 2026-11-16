---
id: R02
labels: wayfinder:research
status: closed
assignee: none
research-dispatched: 2026-08-30
resolved: 2026-08-30
blocked-by: none
ruled-by: D16 (`e652ce3a`, `70260cad`, `68c366dc`, `1e6ad121`, `da3f5418`); `findings/codex-cli-drive-surfaces.md`
---

# R02 — Codex app-server and exec gaps the trace left

## Deliverable

The D16 trace covered Codex's drive surfaces; it left three gaps named in the D16 resolution: whether `-c project_doc_max_bytes=0` fully suppresses AGENTS.md discovery; usage events on the app-server stream and on `codex exec --json`; and `--output-schema` behaviour under `exec` for the judge's verdict shape. Primary sources only (openai/codex source, official docs). Return a cited findings file.

## Done when

`findings/R02-codex-drive-gaps.md` exists, answers all three with source citations, and names what B16 and B20 must do differently if any answer is no.

## Ruled by

D16 (`e652ce3a`, `70260cad`, `68c366dc`, `1e6ad121`, `da3f5418`); `findings/codex-cli-drive-surfaces.md` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Resolution

**Resolved 2026-08-30.** Findings in [`findings/R02-codex-drive-gaps.md`](findings/R02-codex-drive-gaps.md), source-cited against openai/codex `b8c86376` and codex-cli 0.147.0. All three answered: `-c project_doc_max_bytes=0` is a true off-switch (no filesystem walk at all), the only mechanism; app-server emits `thread/tokenUsage/updated` once per model round-trip carrying both running total and last-call usage, while `codex exec --json` emits usage once at `turn.completed`, cumulative, with no context-window field; `--output-schema` is independent of `--json`, maps straight to the Responses API's strict `json_schema` format, and Codex validates nothing afterward — a refusal is dropped like an unknown event. Consequence carried to B20: the judge driver parses the final message with an explicit failure path. B16 unchanged. Two items stay open and need a live API probe, not source reading.
