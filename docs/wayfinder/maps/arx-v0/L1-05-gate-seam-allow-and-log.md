---
id: L1-05
layer: 1
labels: wayfinder:task
status: closed
assignee: none
blocked-by: L1-03
ruled-by: D06 (`ce46b9ea`, `86875715`), D16 (`70260cad`, `96d1ad3a`)
---

# L1-05 — the gate seam, allow-and-log

## Deliverable

The pre-dispatch hook seam wired into the body's approval callback, with the rule contract's return type in full — allow / deny / ask in Claude Code's `PreToolUse` hook JSON shape — and an adapter that returns allow for every call and writes the crossing to the transcript log. Main's tool surface is arx-owned: the body's built-ins stripped, arx's mounted tools the only surface. The point of the card is that every tool call already crosses arx before it executes; deny and ask are L2-02's adapter behind the same signature.

## Done when

A scripted session making N tool calls produces N gate lines in the log, each naming the tool and the verdict; a call to a stripped built-in never executes; the gate's return type carries deny and ask as variants (a test constructs each and the adapter's caller compiles against all three) even though only allow is ever returned here.

## Ruled by

D06 (`ce46b9ea`, `86875715`), D16 (`70260cad`, `96d1ad3a`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
