---
id: T07
labels: wayfinder:research
status: closed
assignee: none
blocked-by: none
research-dispatched: 2026-08-24
resolved: 2026-08-24
---

# T07 — The loop commitments the memory rulings already made

## Question

POSITION.md §8's finding, executed: for each in-force arx ruling, what must be true of the turn cycle for the ruling to hold? Enumerate the silent assumptions, tag each with the ruling that makes it and whether SURVEY §1's field step shape satisfies it, and report conflicts without adjudicating.

## Resolution

Findings: [findings/T07-findings.md](findings/T07-findings.md), extracted from the nine rulings' stored text, not POSITION's gloss.

The ruling set implies twelve step-shape requirements SURVEY's skeleton doesn't name — among them provenance metadata surviving dispatch→admit, a task-boundary tier the session▸turn▸step nesting lacks, freshness `(path, sha)` lookups needed at four distinct points and never budgeted collectively, and a non-blocking admission path for saves beside the blocking one for reads. "Different clocks" comes out as at least **three** clocks, not two: admission per tool result, eviction armed-per-step/fired-at-a-seam, and the dedup fold's save-triggered batch — with questions 6 and 7 inseparable, both turning on whether the pressure check is one clock or two. Discrepancies between the store and POSITION, reported not resolved: the write→cut→append rewind order exists **only** in POSITION prose and an unlinked fact, in none of the nine decisions; the arm/fire split likewise; `80bffad0` is genuinely ambiguous between post-dispatch substitution and pre-dispatch veto (the latter in tension with `0aa87641`); `3e2d9a86`'s delegate tier straddles dispatch and admit; and `dc433ba5`'s cost model is directly contradicted by the call frequency the other rulings impose. Graduated into [D07](D07-step-shape-and-clocks.md), which blocks [D03](D03-admission-seat.md) and [D08](D08-removal-as-demotion.md).
