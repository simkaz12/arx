---
id: L1-11
layer: 1
labels: wayfinder:task
status: closed
assignee: none
blocked-by: L1-09
ruled-by: D16 (`1e6ad121`, `da3f5418`, `0bd4df2f`)
---

# L1-11 — the judge as a dispatch under `judge`

## Deliverable

The judge as a role in config and a hand at runtime: a dispatch under the reserved `judge` definition, resolved from config and never from a per-dispatch body, through L1-09's codex adapter, over a seeded diff with a read-only brief. The verdict is the hand's text. The verdict schema, the read-only clamp, the no-save rule and the same-family warning are L2-12; Codex's `review/start` is not used at any layer.

## Done when

Main dispatches the judge over a seeded diff and receives its text verdict at a step boundary; a dispatch that names a body other than the `judge` definition's is refused at the ledger; the judge's dispatch and verdict are log lines.

## Ruled by

D16 (`1e6ad121`, `da3f5418`, `0bd4df2f`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Debt inherited from L1-09

Seven fixtures carried `model: gpt-5.4-codex`, which this host's ChatGPT-authenticated Codex account rejects with a 400 — the blessed id is `gpt-5.6-sol` (owner ruling 2026-09-02, recorded in the L1-09 brief's Ground-truth). Five of the seven were named in the original L1-09 debt note (`valid`, `drive-surface`, `main-override`, `native-no-provider`, `provider-on-cli`, each `agents/judge.md`); the other two were missed — `native-unknown-provider/agents/judge.md` (added by L1-10, after the debt note was written) and `main-override/agents/main.md` (not counted the first time). L1-11 moved all seven to `gpt-5.6-sol` and added `codex_bodied_fixture_definitions_use_a_supported_model_id` in `check_config.rs`, which scans every `tests/fixtures/*/agents/*.md` for a codex body on an unsupported model id, so the drift fails at `cargo test` instead of at a live 400.
