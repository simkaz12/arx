---
id: B05
labels: wayfinder:task
status: open
assignee: none
blocked-by: B02
ruled-by: D16 (`705e4df2` — arx owns the operator surface), D15 (`18fec68a` operator surface before teardown, `d65f8fe1` harness arms and tells the operator, `253334ad` orientation report surfaced), D06 (`64c789d0` ask lands on the operator surface)
---

# B05 — operator surface v0

## Deliverable

A terminal front, and only a terminal: the streamed reply, operator input, an `ask` from the gate landing as a prompt with allow / deny and a note, the reset-armed notice with the operator's fire command, the checkpoint command, and the respawned main's orientation report shown before the operator types again. Everything richer is fog.

## Done when

A full session runs from the terminal with no direct `claude` invocation; each of the five surfaces above is driven once in a scripted session and its transcript-log line matches what the terminal showed.

## Ruled by

D16 (`705e4df2` — arx owns the operator surface), D15 (`18fec68a` operator surface before teardown, `d65f8fe1` harness arms and tells the operator, `253334ad` orientation report surfaced), D06 (`64c789d0` ask lands on the operator surface) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
