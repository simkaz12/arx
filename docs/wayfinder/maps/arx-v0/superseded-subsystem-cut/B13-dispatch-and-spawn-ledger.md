---
id: B13
labels: wayfinder:task
status: open
assignee: none
blocked-by: B04, B07
ruled-by: D04 (`5e8da52e`, `001823de`, `6aab1d24`), D05 (`23642b44` non-blocking dispatch; steer / tap / kill semantics), D15 (`adec3734` — hands in flight cross a checkpoint and a reset), D16 (`e652ce3a` — dispatch / tap / steer / kill on main's mounted surface), `3a873e9a`
---

# B13 — dispatch primitive and spawn ledger

## Deliverable

Dispatch as a tool call on main's surface that spawns a hand and returns without blocking; the spawn ledger holding every hand in flight; verdicts and stubs landing at main's next step boundary; tap, steer and kill as main-initiated verbs; stop as the child-initiated hold that crosses to main as a small mechanical request; hands surviving a checkpoint and a reset with the state object listing them in flight. Bodies are stubbed — a fake hand that returns a verdict after a delay — until B15 to B18 land.

## Done when

Main dispatches three stub hands and keeps taking turns while they run; each verdict lands at a step boundary and is in the log; a steer reaches a running stub; kill removes it from the ledger; a checkpoint mid-flight lists the three as in flight and the respawned main receives their verdicts after its orientation turn.

## Ruled by

D04 (`5e8da52e`, `001823de`, `6aab1d24`), D05 (`23642b44` non-blocking dispatch; steer / tap / kill semantics), D15 (`adec3734` — hands in flight cross a checkpoint and a reset), D16 (`e652ce3a` — dispatch / tap / steer / kill on main's mounted surface), `3a873e9a` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
