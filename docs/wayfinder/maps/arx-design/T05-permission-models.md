---
id: T05
labels: wayfinder:research
status: closed
assignee: none
blocked-by: none
research-dispatched: 2026-08-24
resolved: 2026-08-24
---

# T05 — Permission models as actually built

## Question

What do Goose (in source) and Claude Code (official docs plus observed local configuration) actually enforce at tool dispatch — where the check runs (schema-level tool visibility vs runtime gate), what the sandbox story is, how approvals reach the human, and what the OpenDev paper's schema-level lesson looked like in practice? Deliverable is a cited grounding artifact plus the decision questions it exposes for arx's permission model, currently empty of positions.

## Resolution

Findings: [findings/T05-findings.md](findings/T05-findings.md).

The taxonomy that falls out is three enforcement points: schema-level (the tool never enters the completion call), runtime gate (rule table + human suspend/resume — a shape all three systems converge on independently), and OS-level (kernel isolation). Claude Code runs all three: bare-tool-name denies are genuine schema removal (eleven live instances in this machine's own settings), scoped rules gate at runtime, and a bubblewrap/Seatbelt sandbox wraps Bash only. Goose runs only the middle one — no sandbox at all, extension-level schema granularity, and a `never_allow` tool is still model-visible and declined post-hoc: the exact arrangement OpenDev's lesson argues against, sitting inside the shell donor. OpenDev's lesson itself is narrower than SURVEY's paraphrase — schema separation is trusted for the read/write axis via a distinct Planner schema, while approval stays a runtime concern inside a five-layer, deliberately overlapping stack. Graduated into [D06](D06-permission-model.md).
