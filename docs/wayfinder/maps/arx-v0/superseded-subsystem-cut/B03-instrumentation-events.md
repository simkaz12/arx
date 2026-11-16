---
id: B03
labels: wayfinder:task
status: open
assignee: none
blocked-by: B02
ruled-by: D09 (`340b3f2e`, `01647e3a`, `25664aa5`, `f11a9548`), D11 pull and echo counters, D14 (`3a5aefb6` miss-size event), D15 (`b03f77fc` falsehood residual signal)
---

# B03 — instrumentation events

## Deliverable

Event lines in the transcript log for the four policy constants — admission threshold, checkpoint token-delta, reset reserve, occupancy nudge — plus the pull and echo counters, the reserve-miss size, the floor event, and the wrong-state-object signal. Each event has one line shape; the owner reads counts, nothing scores them. No scorer, no model judge.

## Done when

A test emits every event kind through the public API and a `arx events <thread>` command prints them grouped by kind with counts; the nudge event is logged, never kills.

## Ruled by

D09 (`340b3f2e`, `01647e3a`, `25664aa5`, `f11a9548`), D11 pull and echo counters, D14 (`3a5aefb6` miss-size event), D15 (`b03f77fc` falsehood residual signal) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
