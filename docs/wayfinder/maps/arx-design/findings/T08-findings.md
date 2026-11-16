---
ticket: T08
gathered: 2026-08-28
method: read-only, against an rsync copy of the live v1 store (/tmp/t08-store), engine at Tabularium commit 294fa14d, embedder ollama/nomic-embed-text, k=50, rrf_k=60, beta=0.25
status: evidence only, nothing ruled
---

# T08 findings — recall shape measured on the owner's corpus

**method:** Store copied read-only via `rsync -a user@memory.example.internal:/home/user/tabularium-store/ /tmp/t08-store/` (8.4GB, byte-identical `du` on both ends verified). Engine = `~/Projects/Tabularium` at commit `294fa14d73d7cf38af15c8f2e56ae99ba2b81ffd` (clean tree, editable-installed in its own `.venv`). Embedder = `ollama/nomic-embed-text` (dim 768, confirmed running locally). Config = v1 defaults: `candidate_k=50` (used as `k` for every leg), `rrf_k=60`, `beta=0.25` (`~/Projects/Tabularium/src/tabularium/config.py`).

Legs invoked directly, no engine-level wrapper with side effects:
- **text-only** = `adapter.text_search(query, filters, k=50)` — the exact call `engine/recall.py:150-155` makes on the degraded (embedder-unavailable) path.
- **vector-only** = `adapter.vector_search(vector, filters, k=50)` alone (LanceDB exact brute-force cosine, `bypass_vector_index()`).
- **hybrid** = both legs via `asyncio.gather`, then `_fuse_rrf(v_hits, t_hits, k=60)` (`engine/scoring.py`) plus the importance boost `_importance()`/`beta=0.25`, reproducing `_recall()`'s ranking formula exactly. The top-level `_recall()` was not called — it also does archived-fallback widening, revival transitions, and read-event log writes, none of which this measurement needs and none of which should touch even a scratch copy needlessly.

`Filters` built via `_primary_recall_filters(_read_span(scope), include_invalid=False)` — same scope+global span and validity as production `recall()`. Scripts: `/tmp/t08/build_datasets.py`, `/tmp/t08/run_measurements.py`, `/tmp/t08/build_real_query_labels.py`, `/tmp/t08/clutter.py`.

## Results

**Self-retrieval** (100 random active Facts, seed=42; query = first sentence, ≤30 words; n=100, clean single-answer labels):

| leg | recall@1 | recall@5 | recall@10 | MRR | median ms | p95 ms |
|---|---|---|---|---|---|---|
| vector | 0.95 | 0.96 | 0.98 | 0.955 | 13.7 (+embed 12.3) | 15.3 |
| text | 0.97 | 0.98 | 0.99 | 0.976 | 11.1 | 12.3 |
| hybrid | **0.85** | 0.98 | 0.99 | 0.911 | 26.1 | 28.1 |

**Real queries** (30 most recent distinct (query, scope) from `~/.claude/projects/*/*.jsonl` tool_use blocks named `mcp__tabularium-mcp__recall`, out of 153 total calls found; weak label = record id/60-char-verbatim cited in the 20 transcript entries following the call, excluding that call's own tool_result; 22/30 labelled, 8 judged by hand):

| leg | recall@1 | recall@5 | recall@10 | MRR | median ms | p95 ms |
|---|---|---|---|---|---|---|
| vector | 0.167 | 0.267 | 0.333 | 0.214 | 11.6 (+embed 12.3) | 13.9 (embed p95 1808, cold reload) |
| text | 0.200 | 0.233 | 0.300 | 0.222 | 10.2 | 11.2 |
| hybrid | 0.167 | 0.300 | 0.300 | 0.212 | 24.0 | 1819 (inherits embed cold spike) |

**Pairs** (625 `DistinctPair` rows, `source="save"`, in this copy today — up from 591 in `tabularium-in-practice.md` dated one day earlier; both directions run; 1204/1250 directional queries resolved, 46 dropped because one side's id no longer exists — buried since judged, consistent with the registry being append-only and never pruned):

| leg | recall@1 | recall@5 | recall@10 | MRR | median ms | p95 ms |
|---|---|---|---|---|---|---|
| vector | 0.043 | 0.239 | **0.266** | 0.131 | 12.1 (+embed 14.1) | 14.7 |
| text | 0.027 | 0.173 | 0.223 | 0.094 | 12.7 | 15.8 |
| hybrid | 0.058 | 0.233 | **0.266** | 0.133 | 27.3 | 36.2 |

`ReinforcementEvent.matched_from_id`: 0 of 4283 events non-null — confirmed unusable in one query, matching the doc's exhaustive trace.

## Clutter

Rule applied (automated, spot-checked, not a full manual read of all 300 slots): two hits in the same text-leg-only top-10 flagged as same-subject if 5-word-shingle Jaccard ≥ 0.20, OR one content's first 200 chars is a substring of the other, OR they share a ticket-key/8-hex/UUID token. Result: **29 of 30 real queries have zero same-subject pairs in their top-10.** One query (`pstack skills repo source pull`, skillbook) flagged 2 hits — manually confirmed as a genuine near-duplicate (two records stating the same command-shadowing fact in nearly identical wording). Distribution: 0→29, 1-2→1, 3-4→0, 5+→0. Mean 0.067 flagged hits per query.

## Load-all per scope

Active Facts only, chars/4 estimator (`/tmp/t08/load_all_per_scope.json`):

| scope | count | chars | est. tokens |
|---|---|---|---|
| client-a | 1911 | 785,991 | ~196,498 |
| global | 317 | 225,541 | ~56,385 |
| tabularium | 258 | 174,871 | ~43,718 |
| client-b | 73 | 62,362 | ~15,590 |
| arx | 37 | 32,554 | ~8,138 |
| side-project-a | 21 | 8,996 | ~2,249 |
| skillbook | 12 | 7,899 | ~1,975 |
| pi-assistant | 4 | 3,654 | ~914 |
| scope-x / ai-assistant / mobile | 0 | 0 | 0 (bulk-shelved at seed) |

A real session's working set is scope+global (recall's own span): client-a+global ≈ 253k tokens, tabularium+global ≈ 100k, everything else ≤72k. "Just load it all" fits inside a 200k window for 7 of 8 populated scopes today; only client-a's combined span exceeds it.

## Caveats and bias

- **Pairs are confirmed-DISTINCT judgments, not duplicates** — the "answer" is a record a human/agent explicitly ruled a different fact, merely close enough in embedding space to collide at save time. The set is constructed by vector's own cosine, so it should be maximally favorable to vector; despite that, vector recall@10 tops out at 26.6%. Read this as the strongest single number in the report.
- Pairs queries are full record contents (sometimes hundreds of words), unlike real recall traffic's short phrases — not representative of production query shape.
- Real-query weak labels undercount true hits. Manual judgment of the 8 unlabelled queries found the true top-1 hit for BOTH legs in 5 of 8 cases (event-modal, prospect-banner, PROJ-332, pstack, agent-fleet-roster-partial) that the citation-based label missed because the session paraphrased the answer afterward instead of quoting or citing its id. The reported real-query recall@1 (text .20, vector .167) is a floor; the true figure is materially higher for both legs and the judged cases were not folded back into the quantitative table because mixing label types changes what the metric measures.
- Some weak labels went the other way — one query's answer set grew to 40-58 ids because a later, broader `recall`/`decisions_query` call in the 20-entry window dumped many hits' full content into the transcript; those are legitimate citations under the ticket's own label definition, not noise, but they make recall@k on those specific queries close to automatic for any leg (spot-checked, confirmed genuine).
- Self-retrieval is the cleanest measurement (single unambiguous answer) but also the easiest possible case — query and answer share most of the same words. 95-99% ceilings describe "can a leg find the document it was extracted from," not headroom for paraphrase retrieval.
- 3 of 11 registered scopes (scope-x, ai-assistant, mobile) hold zero active records — bulk-shelved at seed, per `tabularium-in-practice.md` §2 — excluded from every dataset here structurally, not by choice.
- Timing measured against a warm local Ollama; a fresh process's first call pays ~2.7-2.9s model-load, visible as isolated p95 spikes (real-query embed p95 1808ms) rather than in medians (~11-14ms steady state). Legs run sequentially here rather than via `asyncio.gather` (production's actual concurrency) — rankings are identical either way; hybrid's absolute latency here is a slight overestimate of production.

## Could not measure

- Bounce-abandonment rate (bounces shown but never resolved) — permanently unrecoverable per `tabularium-in-practice.md` §1, not re-attempted; out of this ticket's scope.
- No file-agent/grep retrieval leg was implemented or timed — only its proxy, load-all token size, per the ticket's own scope.
- Exhaustive manual read of all ~300 clutter top-10 slots — used the automated rule plus targeted spot-checks instead (documented above).
- SSH and Ollama were both available, so no hard "could not measure" applies to store access or embedding.

## Read against the debate rows

- **S-SOL-1** (vector-first false negatives unrecoverable): not directly testable — v1 runs both legs in parallel (`asyncio.gather`), never a sequential funnel; the underlying worry gets indirect support — vector-only recall@10 is only 26.6% even on the pairs set built by vector's own cosine.
- **S-RUNE-4** (no sequential funnel; vector at most an expansion leg): the code confirms v1's current shape already matches this — `recall.py`'s normal path is parallel `gather`, not a funnel; the debated "funnel" describes a proposed shape, not what v1 runs.
- **S-RUNE-6 / S-SOL-3** (RRF hybrid's weight is unmeasured; vector's influence needs a measured win): measured, and it is a loss — on the cleanest labelled set (self-retrieval, n=100), hybrid recall@1 (.85) is below BOTH single legs (.95, .97). Ten of the misses are cases where vector and text independently rank the true answer #1 and the fused+importance-boosted score still displaces it.
- **S-SOL-2** (BM25-alone clutter unmeasured / contested): measured on the exact degraded-path code (`text_search` alone) over the real-query set — 29/30 queries have zero same-subject duplicate pairs in their top-10. Supports rune's contest, sharper than the withdrawal already on record.
- **S-RUNE-5** (withdrawn: BM25 would search "a lot of similar stuff," refuted by 0.14% lexical near-twin rate): consistent with the clutter measurement above — same near-absence of same-subject duplication, independently arrived at.
- **S-SOL-6** (surfacer's interruption cost still live post-`df14a2bc`): out of this ticket's scope — needs bounce-frequency data, not retrieval-shape data; not addressed here.
