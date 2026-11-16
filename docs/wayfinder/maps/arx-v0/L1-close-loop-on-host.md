---
id: L1-close
layer: 1
labels: wayfinder:task
status: closed
assignee: none
blocked-by: L1-01, L1-02, L1-03, L1-04, L1-05, L1-06, L1-07, L1-08, L1-09, L1-10, L1-11
ruled-by: map Notes (layer gate); `58b34830` is tested only at L3-04 — this card rehearses the shape at layer-1 depth
---

# L1-close — the loop closes on the host

## Deliverable

The layer check — a rehearsal of the daily shape at layer-1 depth, not the use claim's test; that is L3-04. One scripted session, driven from the terminal, that crosses every layer-1 seam once: grounding in main's first turn, one hand of each body dispatched (claude-code, codex, native one-turn) plus the judge over a seeded diff, four verdicts landing at step boundaries, a save from main returning a landscape and landing in quarantine, a stop raised by L1-07's fake hand and answered by main — the fake is the layer's stop producer, since no real layer-1 body can raise one — and every tool call a gate line. The accumulated script is adapter-neutral: it asserts seam crossings as transcript-log lines only, so L2-close can run it unchanged over the real adapters; the flat-file assertions live in this card's own layer check beside it.

## Done when

The script runs green end to end with no bare `claude` launched; the transcript log covers the whole session and every event the script expects is a line in it; the layer check beside the script verifies the quarantine file holds the save with the thread handle; the script file is checked in beside the arx binary's tests.

## Ruled by

Map Notes (layer gate). `58b34830` names the map's one test and L3-04 runs it; this card only rehearses the loop at layer-1 depth. A card that lands against a ruling stops and reopens it in the store first.

## Shipped

The session lives at `crates/arx/tests/scripts/l1-close.json`, checked against the transcript log by `crates/arx/tests/layer_close.rs`. The stub sidecar (`crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`) now gates its own `hand:` and `store:` calls with a `mcp__arx__<verb>` event before each one, and the `claude` stub (`crates/arx/tests/fixtures/bin/claude`) appends every launch's argv to `claude-stub-argv`. Shared test fixtures moved to `crates/arx/tests/common/mod.rs`, which also carries the new script runner: `Script`, `Expect`, `load_script`, `check_script`, `run_session`.

`cargo test -p arx --test layer_close` runs the checked-in script hermetically and passes: grounding in the system row, five dispatches (claude-code, codex, native, the judge, and L1-07's fake) gated and delivered, the fake's stop answered by a gated steer, a save landing in `store/quarantine/main` with its thread handle, and exactly one `claude` launch carrying the `-p` flag. The live rehearsal on the owner's login passed: hand-ex over `claude -p`, hand-cx and the judge over `codex exec`, and hand-ds over DeepSeek each delivered a verdict, and the fake's stop was answered by a gated steer. The sampler caught two `claude` processes during the run and found the `arx run` pid an ancestor of both on every sample — no bare `claude`. Thirteen of the fourteen post-run checks were grepped; the system-row check was reported met but never run against the full text before the workspace was deleted, and stands on the hermetic script, which asserts the same substring on the same row written by the same `compose_store_grounding` path.
