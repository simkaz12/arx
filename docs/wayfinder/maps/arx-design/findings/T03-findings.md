# T03 findings — Tabularium's call profile under per-turn duty

Ticket: [T03](../T03-tabularium-call-profile.md) · resolved 2026-08-24 · produced by a hand-ex research agent from `~/Projects/Tabularium` source, read-only. Live probes: five throwaway-string embed calls each against local Ollama (`localhost:11434`) and the production Pi's Ollama (`memory.example.internal:11434`, over the existing WireGuard MCP link); nothing persisted.

Grounding only, per the ticket. No verdict on the transport ruling.

## 1. Call surface inventory — cost anatomy per call

Tabularium's MCP surface, from `src/tabularium/server.py:614-1348` (16 `@app.tool` registrations + one `@app.resource`): `save`, `ember`, `recall`, `decision`, `decisions_query`, `set_validity`, `bury`, `revoke`, `set_hot`, `set_audience`, `context`, `digest`, `sessions`, `feedback`, `set_shelved`, `unsupersede_fact`, plus `decision://{id}`.

Only four touch the embedder. Every other tool is a pure local-store read or write with no network call outside the process.

| Call | Embeds? | Retrieval shape | Dominant cost |
|---|---|---|---|
| `recall` (`engine/recall.py:66-362`) | Yes, once, unconditionally (`embedder.embed(query)`, line 135) | `vector_search` + `text_search` gathered concurrently (158-161), `k = max(limit, candidate_k)`, `candidate_k` default 50 (`config.py:52-57`); possible second gather on archived fallback if primary hits `< min_active_results` (default 3, `config.py:67-72`) — reuses the same vector, no re-embed | embed, by measurement below |
| `save` (`engine/facts.py:1340-1348`, gate in `engine/gate.py:103-152`) | Yes, once, before the write lock is taken (deliberate — `facts.py:1318-1326`, so a degraded embedder can't stall the lock) | Same hybrid probe shape as `recall`, RRF-fused, best candidate by raw cosine | embed |
| `decisions_query` (`engine/decisions.py:837-888`) | Yes, once (line 878) | Same hybrid shape | embed |
| `ember` | Yes — thin wrapper over `save` | Same gate | embed |
| `context` (`engine/context.py:218-340`, `server.py:1112-1145`) | **No** | `adapter.scan_records(lifecycle_in=...)` — one round trip per tier (hot, ember), no query string at all, filtered/sorted in Python, packed by `pack()` | local scan + in-memory sort/pack |
| `sessions` (`engine/sessions.py:143-183`) | **No** — module docstring line 37: "Nothing on this path embeds" | `adapter.search_sessions`: BM25/substring only, no vector leg | local BM25, negligible |
| `decision`, `set_hot`, `set_audience`, `set_validity`, `bury`, `revoke`, `set_shelved`, `unsupersede_fact`, `feedback`, `digest` | No | Direct `get_by_id` / row mutation, no search | local, negligible |

**The retrieval leg is deliberately non-indexed.** `vector_search` forces `bypass_vector_index()` (`storage/lancedb_adapter.py:685-740`) — exact brute-force cosine, not ANN. ADR `0019` measured an approximate index at ~3000 real rows: 1.01-1.05x speedup (none), plus a real correctness cost (mean top-10 overlap 0.918 vs exact). Brute force stays; at current corpus scale it isn't the bottleneck.

### Measured, not guessed

- **`context()` end-to-end** (no embed, real Pi store, 7801 records, MCP-over-WireGuard): `docs/superpowers/plans/2026-07-31-m2-grounding-slice-c/findings-context-latency.md`, re-measured 2026-08-01 — median **0.108-0.136s**, worst **0.120-0.150s** across `client-a`/`tabularium`/`mobile`. MCP handshake alone: median 34ms, one-time per session.
- **Embed call alone**, against the Pi's Ollama over the same WireGuard link (`nomic-embed-text`, throwaway string, five calls): **150-185ms**, median ~152ms.
- Same call against local Ollama on this dev machine: **11.6-12.8ms** — confirms ADR `0019`'s finding that Pi hardware, not corpus size, is the constraint.

**Read together:** on the real deployment, a single embed call (~150ms) costs about as much as `context()`'s entire round trip (~110-150ms) — a call that does two local scans, sorts, and packs, and touches no embedder. `recall`/`save`/`decisions_query` pay the embed cost sequentially before the retrieval legs (`recall.py:135` then `:158`), so their floor is `embed_time + max(vector_leg, text_leg)`. `recall()` was not timed end-to-end on the live store (it writes a read event and can trigger revival on every call — outside read-only scope), so that comparison is inference from the embed number plus ADR `0019`, not a same-call measurement.

## 2. What each per-step shape would require, and which parts embed

**`assemble(window, budget)` does not exist in Tabularium.** No file, function, or MCP tool matches the signature — it is arx-side vocabulary from decision `dc433ba5`, not a Tabularium concept. The nearest analogue is `context()`, and the shapes differ on the property the discriminator turns on: `context()` is query-less (fixed hot/ember tiers, no window, no per-turn state), called once at session start; a per-step `assemble` would need to see the window's current content every step, which nothing on this surface does today. Whether that signature "comes out chatty" cannot be settled by reading Tabularium's code — the operation doesn't live there yet, in either direction.

**Per-tool-result admission** — per POSITION §6, not built either, but its shape as argued is a size/interaction heuristic, not obviously a store query at all on the common path. The place a store call is explicitly implied is narrower: the refusal carve-out (`80bffad0`) and its residue registry (`79da6421`), where POSITION §6 q7's own caveat says matching a new question against the stored set is semantic matching — embeddings — a cosine threshold. That is the one admission-adjacent lookup the design itself already says would embed, if built as described.

**Does anything non-embedding serve it today?** Two calls never touch the embedder — `context()` (no query capability at all; can't answer "is this known") and `sessions()` (BM25-only, but scoped to session-narrative rows, not Facts/Decisions). The engine's `text_search` leg is architecturally embed-free — `recall()`'s degraded path (`recall.py:149-155`) runs BM25-only when the embedder is *unavailable* — but that path fires only on outage, not on caller choice. Nothing on the current surface lets a caller deliberately request "search Facts, BM25-only, skip the embed" as a normal mode. A per-tool-result admission check wanting a cheap relevance probe over Facts has no exposed non-embedding path today; it would need new surface.

## 3. Where latency actually sits

- **Embedding** — real, Pi-measured, the single largest per-call cost among the four calls that pay it (~150-185ms, against ~12ms for the same model on stronger hardware).
- **Retrieval** (vector + BM25) — deliberately non-indexed and, per ADR `0019`'s measurement, cheap at current scale (~3000-7800 rows). `context()`'s two-scan, no-embed round trip at ~110-150ms total is the closest available bound on what pure local retrieval + transport costs.
- **Transport** (JSON-RPC over stdio, or in this deployment streamable-HTTP over WireGuard) — the store's own findings doc could not isolate it cleanly from retrieval variance, but bounds it tightly by inclusion: handshake ~34ms one-time, and `context()`'s full call sits at ~110-150ms doing genuinely more work than a bare round trip. Transport is small relative to either embedding or the Pi's own compute.

**One correction to the framing this ticket was issued against.** The sentence "a recall is dominated by the embedding step" (`dc433ba5`) is not a measurement recorded anywhere in Tabularium's docs — searched, no such benchmark exists. What the store's latency work *did* measure was the opposite shape: `context()`'s original 8.2s failure was dominated by retrieval (N+1 `get_by_id` round trips, 94-100% of engine time — `findings-context-latency.md:42-59`), not embedding, because `context()` never embeds. The embedding-dominance claim about `recall` is directionally supported by today's probe (embed ~150ms vs a same-shape non-embedding call at ~110-150ms total), but it rests on inference across two different calls. "Supported by inference" and "measured" are different epistemic states, and the decision text reads as the latter.

## 4. Mismatches with POSITION.md / the brief

- **"A recall is dominated by the embedding step"** (`dc433ba5`) — no corresponding measurement exists for `recall()` specifically. Supported by inference, not refuted; but not measured.
- **`assemble(window, budget)`** — confirmed unproven in the strongest sense: no Tabularium code exists in its shape to read. The open question isn't "will it come out chatty" but "what would it even call."
- No contradiction found in `8a3b81f7`'s core claim — admission firing per tool result would, if it needs the applicability/residue lookup, touch the embedder. What this trace adds: the *ordinary* admission decision (not the refusal carve-out) may not need a store call at all, which narrows the pitfall's "hot path of every turn" framing — the hot-path cost may be concentrated in the refusal path specifically.

## 5. Decision questions this exposes

- Does the ordinary admission path (not the refusal carve-out) need a Tabularium round trip at all, or is it a harness-local size/type heuristic with zero store calls? The call-profile answer depends entirely on which.
- If only the refusal/residue path embeds, does that path fire often enough per session to matter for the transport discriminator — or is it rare enough that the discriminator was never really about admission at scale?
- Is `context()` — measured, embed-free, ~110-150ms on real hardware — a legitimate proxy for what a non-embedding per-step call would cost, or does its query-less fixed-tier shape make it structurally too different from a windowed `assemble` to bound anything?
- Given the embed call itself costs ~150ms on the Pi regardless of transport, does moving Tabularium in-process change the embed-dominated calls' latency at all — or does the discriminator only bite for the embed-free calls, where transport is a larger fraction of a much smaller total?
- Is the Pi's own hardware (the constraint ADR `0019` already identified) a confound in the whole discriminator — would the question read differently against a daemon on the harness's own host rather than a Raspberry Pi over WireGuard?

## Confidence

High on the call-surface inventory and the embed/no-embed split (code-cited). Medium on the cross-call latency comparison (real numbers on both sides, but not the same call measured two ways). Low on anything about admission's actual call shape — that mechanism isn't built; every claim there traces to POSITION's own unresolved argument, not to code.

Files read: `src/tabularium/server.py`, `engine/recall.py`, `engine/context.py`, `engine/gate.py`, `engine/facts.py`, `engine/decisions.py`, `engine/sessions.py`, `embedding.py`, `config.py`, `storage/lancedb_adapter.py`, `docs/adr/0019-approximate-vector-index-dropped.md`, `docs/superpowers/plans/2026-07-31-m2-grounding-slice-c/findings-context-latency.md`, `STACK.md`.
