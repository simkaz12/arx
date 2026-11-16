---
id: T03
labels: wayfinder:research
status: closed
assignee: none
blocked-by: none
research-dispatched: 2026-08-24
resolved: 2026-08-24
---

# T03 — Tabularium's call profile under per-turn duty

## Question

If admission fires per tool result and window assembly per step, what calls would the store actually see, and what does each cost today? Which operations touch the embedder and which do not; what a non-embedding admission check would run through; where the latency actually sits (embedding, retrieval, transport). Read from Tabularium source — no writes to the live store. Deliverable is a cited grounding artifact plus what it establishes about the transport question: whether `assemble(window, budget)` comes out chatty, and whether the transport-is-cheap argument (recall dominated by embedding) holds for the per-tool-result path.

## Resolution

Findings: [findings/T03-findings.md](findings/T03-findings.md).

Only 4 of the 16 store calls embed (`recall`, `save`, `decisions_query`, `ember`); everything else is local reads/mutations. Measured: an embed call costs ~150-185ms on the Pi (~12ms for the same model locally — hardware, not corpus, is the constraint), about the same as the *entire* embed-free `context()` round trip (~110-150ms); transport itself is small against either. Two corrections to the standing frame: `dc433ba5`'s "recall is dominated by the embedding step" was never measured anywhere — it is supported by cross-call inference, not benchmark — and `assemble(window, budget)` exists nowhere in any shape, so its chattiness is unproven in the strongest sense. The sharpest narrowing: the *ordinary* admission decision may need no store call at all — the one admission-adjacent lookup the design says would embed is the refusal/residue path — so the hot-path cost may concentrate there rather than on every Admit. Graduated into [D01](D01-transport-and-trait.md) (blocked on [D03](D03-admission-seat.md), which owns the "does ordinary admission call the store" question).
