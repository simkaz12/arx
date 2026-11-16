---
id: L1-07
layer: 1
labels: wayfinder:task
status: closed
assignee: none
blocked-by: L1-03, L1-05
ruled-by: D04 (`5e8da52e`, `001823de`, `6aab1d24`), D05 (`23642b44` non-blocking dispatch; steer / tap / kill semantics), D16 (`e652ce3a` — dispatch / tap / steer / kill on main's mounted surface), `3a873e9a`
---

# L1-07 — dispatch and the spawn ledger, with a fake hand

## Deliverable

Dispatch as a tool call on main's surface that spawns a hand and returns without blocking; the spawn ledger holding every hand in flight; verdicts landing at main's next step boundary; tap, steer and kill as main-initiated verbs; stop as the child-initiated hold that crosses to main as a small mechanical request. The body behind the ledger is a fake — a hand that returns a verdict after a delay, or raises a stop — so the seam is proven before any real body (L1-08 to L1-10) sits behind it. Hands crossing a checkpoint is L2-06.

## Done when

Main dispatches three fake hands and keeps taking turns while they run; each verdict lands at a step boundary and is in the log; a steer reaches a running fake; kill removes it from the ledger; a stop raised by a fake lands in main's window as a mechanical request and main's answer releases it.

## Ruled by

D04 (`5e8da52e`, `001823de`, `6aab1d24`), D05 (`23642b44`), D16 (`e652ce3a`), `3a873e9a` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
