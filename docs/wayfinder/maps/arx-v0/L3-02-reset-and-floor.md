---
id: L3-02
layer: 3
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-close
ruled-by: D15 (`d65f8fe1`, `e2b0ceb2` as the native shape, `253334ad`), D16 (`952d3b69`, `e4510547` as superseded by `c474a7f4`), `31a20ff6`, D14 (`8b9f41ae` as the native ladder)
---

# L3-02 — reset and the CLI-main floor

## Deliverable

Reset against a reserve armed below the body's compaction threshold: the harness arms and tells the operator on the terminal; the operator fires from it; the harness fires alone only at the floor. Reset is kill-and-respawn — fresh session, no resume, the state object as orientation, the respawned main's first turn a mandated orientation report shown on the terminal before the operator types again. The CLI-main floor per `31a20ff6`: when the child rejects the ritual's authoring or truth-check turn, kill and respawn from the last checkpoint's state object plus the transcript handle, logged as its own event; with no checkpoint, the transcript handle alone.

## Done when

A thread driven past the reserve with synthetic load arms, the operator fires from the terminal, the respawn orients and reports; forcing a rejection mid-ritual (a stubbed body error) takes the floor path and the log shows the floor event with the distance past the last checkpoint; the body's own compaction never fires (its ceiling verified above arx's reserve in the spawn args).

## Ruled by

D15 (`d65f8fe1`, `e2b0ceb2`, `253334ad`), D16 (`952d3b69`, `c474a7f4`), `31a20ff6`, D14 (`8b9f41ae`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
