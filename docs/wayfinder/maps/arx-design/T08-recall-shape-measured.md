---
id: T08
labels: wayfinder:research
status: closed
assignee: none
blocked-by: none
research-dispatched: 2026-08-28
resolved: 2026-08-28
---

# T08 — The recall shape, measured on the owner's own corpus

## Question

Opened from the D13 grill and the 2026-08-27 debate (`findings/debate/2026-08-27-arx-d13-recall-gate/ledger.md`, S-SOL-4 agreed by both seats: no retrieval shape gets ruled without an arx-specific comparison; S-SOL-2 contested: whether BM25-only returns "a lot of similar stuff" is unmeasured). Against a read-only copy of Tabularium v1's live store (8,552 valid records): does the vector leg earn its place in recall at this corpus size? Compare text-only (BM25), vector-only, and v1's RRF hybrid as configured, on three query sets that each carry an answer column — the 591 `DistinctPair` rows (biased toward what the vector gate surfaced; say so), real `recall` queries mined from the session transcripts with the records the session went on to use, and self-retrieval from a record's own opening sentence. Report recall@1/5/10 and MRR per leg per set, the S-SOL-2 clutter count (same-subject hits per text-leg top-10 on real queries), and the load-all number (tokens of active records per scope). Say plainly what could not be measured.

## Resolution

Findings: [findings/T08-findings.md](findings/T08-findings.md). Read-only against an rsync copy of the live store, engine at Tabularium `294fa14d`, k=50, three query sets.

The vector leg earns nothing at this corpus. On the cleanest set (self-retrieval, n=100) text-only leads at recall@1 0.97 against vector 0.95 — and v1's RRF hybrid as configured drops to **0.85**, below both legs, because fusion plus the importance boost displaces an answer both legs independently ranked first. On the pairs set, built by vector's own cosine and so maximally favourable to it, vector reaches only 0.266 recall@10, tied with hybrid, text 0.223. Real queries sit around 0.2 recall@1 on weak labels for every leg, a floor the hand-judged cases put materially higher for both. S-SOL-2 settles: 29 of 30 real queries have zero same-subject pairs in the text-only top-10, so "BM25 would search through a lot of similar stuff" does not occur on this corpus. Load-all is on the table for 7 of 8 populated scopes today (scope+global under 200k); arx's own scope is ~8k tokens. Timing: every leg 10–14ms median locally; hybrid ~25ms. Feeds D13's last open bullet.
