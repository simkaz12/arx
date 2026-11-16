---
id: L2-01
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L1-close
ruled-by: D09 (`340b3f2e`, `01647e3a`, `25664aa5`, `f11a9548`), D11 pull and echo counters, D14 (`3a5aefb6` miss-size event), D15 (`b03f77fc` falsehood residual signal)
---

# L2-01 — instrumentation events

## Deliverable

Event lines in the transcript log for the four policy constants — admission threshold, checkpoint token-delta, reset reserve, occupancy nudge — plus the pull and echo counters, the reserve-miss size, the floor event, and the wrong-state-object signal. Each event has one line shape; the owner reads counts, nothing scores them. No scorer, no model judge. The emitters for admission, reset and the floor land with L3-01 and L3-02; this card ships the kinds, the writer and the reader.

## Done when

A test emits every event kind through the public API and an `arx events <thread>` command prints them grouped by kind with counts; the nudge event is logged, never kills; L1-close's script produces its gate and dispatch events through the same writer.

## Ruled by

D09 (`340b3f2e`, `01647e3a`, `25664aa5`, `f11a9548`), D11, D14 (`3a5aefb6`), D15 (`b03f77fc`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

`crates/arx/src/events.rs` defines `Event`, seventeen kinds tagged by `kind`. `TranscriptLog::append_event` writes each as one JSON line through the transcript's existing single-buffer path; `append_gate` now delegates to it, byte-identical. `TranscriptLog::events` and `TranscriptLog::kind_counts` read the committed prefix back, and `arx events [--path] [thread]` prints the count table and one heading per kind present.

`gate`, `spawn`, `steer`, `tap`, `kill`, `delivered` and `nudge` have live emitters, wired at main's call sites in `main.rs`. The rest ship as shapes only: `wall` waits for L2-08/L2-09, `admission`/`pull`/`echo` for L3-01, `checkpoint` for L2-06, `rewind`/`reset`/`miss`/`floor`/`orientation` for L3-02. All seventeen round-trip through JSON now, so each later card adds an emitter, not a kind.

The nudge fires once per thread. The main seat crosses `[main] nudge` (default 120000) after a usage row, logs one `nudge` event, prints one operator notice, and arms the next inbound turn's text with the window line; the latch survives a reopened session by scanning `events()` for an existing main-seat `nudge`. The hand seat is judged once per delivery, from the verdict's own usage, with no latch — nothing is ever killed for crossing the line.

Dispatch lifecycle events land in the transcript at main's call sites, not by touching `ledger.rs`. The ledger keeps its own `<thread>.ledger.jsonl` as the record `steer` and `kill` resolve hands through; "the same writer" the card asked for is satisfied at the transcript, not by merging the two files.
