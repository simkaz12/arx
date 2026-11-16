---
id: B01
labels: wayfinder:task
status: open
assignee: none
blocked-by: none
ruled-by: D16 (`0bd4df2f`), D02 (`975f65cd`), `30e83788`, `c775b5c2`
---

# B01 — workspace and config shape

## Deliverable

A Rust workspace with two crates — the arx shell and the memory engine linked in as a crate, no transport — plus the configuration shape per role: `body` (native | claude-code | codex), `model`, and `provider` for a native body. Main's block is startup config, fixed per run; a hand's block is its named agent definition; the judge's block is the reserved `judge` definition. The drive surface is never configured — it follows from the role. Agent definitions live in a directory arx reads at startup.

## Done when

`cargo build` is green on both crates; `arx check-config` accepts a config naming all three roles with a claude-code main, a `judge` definition and one hand definition, rejects a config that tries to set a drive surface or a per-dispatch body override on main, and prints the resolved role table.

## Ruled by

D16 (`0bd4df2f`), D02 (`975f65cd`), `30e83788`, `c775b5c2` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
