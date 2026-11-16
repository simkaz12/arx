---
id: L1-04
layer: 1
labels: wayfinder:task
status: closed
assignee: none
blocked-by: L1-03
ruled-by: D16 (`705e4df2` — arx owns the operator surface), D15 (`18fec68a`, `d65f8fe1`, `253334ad` — the surfaces that arrive with layers 2 and 3)
---

# L1-04 — the terminal, thin

## Deliverable

A terminal front, and only a terminal: the streamed reply and operator input. Two of the five surfaces the design ruled. The other three arrive with the mechanism that needs them — the gate's `ask` prompt with L2-02, the checkpoint command with L2-06, the reset-armed notice and the orientation report with L3-02. The seam is what this card fixes: one channel from arx to the terminal and one back, typed so a later surface is a new message kind, not a new channel.

## Done when

A session runs from the terminal with no direct `claude` invocation; typed input reaches main and the reply streams back; both are transcript-log lines matching what the terminal showed; a second message kind added in a test renders without touching the two channels.

## Ruled by

D16 (`705e4df2`), D15 (`18fec68a`, `d65f8fe1`, `253334ad`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
