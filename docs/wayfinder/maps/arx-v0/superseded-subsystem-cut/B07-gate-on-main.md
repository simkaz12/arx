---
id: B07
labels: wayfinder:task
status: open
assignee: none
blocked-by: B04, B05
ruled-by: D06 (`ce46b9ea`, `86875715`, `64c789d0`), D16 (`70260cad`, `96d1ad3a`), D04 (`5e8da52e`), `ebe94e38`
---

# B07 — the gate on main

## Deliverable

Main's runtime gate as a pre-dispatch hook seam running in the body's approval callback: allow / deny / ask, with the rule contract in Claude Code's PreToolUse hook JSON shape; ask lands on the operator surface and the answer is the gate's own return. Main's tool surface is arx-owned — the body's built-ins stripped, arx's mounted tools the only surface. Adapters narrow a body's richer answers, never widen them.

## Done when

A scripted session drives one call through each of allow, deny and ask; the ask reaches the terminal and the operator's answer reaches the body; a call to a stripped built-in never executes; every gate decision is a transcript-log line.

## Ruled by

D06 (`ce46b9ea`, `86875715`, `64c789d0`), D16 (`70260cad`, `96d1ad3a`), D04 (`5e8da52e`), `ebe94e38` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
