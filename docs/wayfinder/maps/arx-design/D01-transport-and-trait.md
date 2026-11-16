---
id: D01
labels: wayfinder:grill-with-docs
status: closed
assignee: none
blocked-by: none
grounding: findings/T03-findings.md, findings/T07-findings.md
---

# D01 — Transport and the memory trait's call shape

## Question

The live contradiction (`8a3b81f7` vs `dc433ba5`) with measured numbers finally under it. Blocked on D03 because the trait's chattiness depends on whether ordinary admission calls the store. To settle:

- **One call or a fan-out.** T07 I3: the rulings collectively describe several distinct memory-facing judgments per step — worth, ladder tier, defer/admit, refusal/residue, applicability. Does the trait collapse them into one `assemble(window, budget)` or expose several calls — and that answer, not transport, is what "chatty" turns on.
- **Re-examine `dc433ba5` on the measurements.** Its "recall is dominated by embedding" was never measured (T03); what T03 did measure says the embed (~150ms on the Pi) dwarfs transport, and the whole embed-free `context()` round trip costs about the same as one embed. If only the refusal path embeds, does the discriminator ever fire — or was it never about admission at scale?
- **The hardware confound.** ADR 0019 already found the Pi, not the corpus, is the constraint (~12ms same embed locally). Is the right response to the transport question a daemon on the harness's own host rather than any in-process move — and does that make `dc433ba5`'s third branch (rewrite in Rust) even less earned?
- **The embed-free surface.** No exposed call today does a deliberate BM25-only probe over Facts (T03). If the residue/applicability gate needs one, is that new Tabularium surface a prerequisite this map owes the other scope?

Output: `dc433ba5` reaffirmed, amended, or superseded — with the pitfall `8a3b81f7` resolved rather than standing.

**Ruled since (2026-08-24):** the contingency is designated — if in-process is ever demanded, a Rust copy of Tabularium lands in arx's root and the Python service keeps serving other hosts (`30e83788`); flip-to-Python is retired. And the residue strip (`ee932ea8`) removed the one admission-adjacent path that embedded, narrowing this ticket further toward "does the discriminator ever fire at all."

## Resolution — closed 2026-08-25, one grill

Three of the four bullets were hollow on arrival, which the re-test at the top of the grill established before any of them was answered:

- **One call or a fan-out** — dead. T07's five memory-facing judgments per step (worth, tier, defer/admit, refusal/residue, applicability) are all harness-local now: `bb4099ad` bars admission from calling the store, `3d559805` turned refusal into a sha-keyed result cache, `e65b2a35` made the tier a size threshold. Nothing left to fan out.
- **`dc433ba5` on the measurements** — its named discriminator was per-turn assembly that must see the whole window. D03 deleted it. It cannot fire.
- **The embed-free Facts probe** — existed only to serve the applicability gate, which no longer exists.
- **The hardware confound** — the one that survived, and it turned out to carry the whole latency argument.

What the grill found instead:

1. **Mid-loop recall exists**, and nothing had named it. The model calls `recall` and `decisions_query` as ordinary tools, the way the owner does by hand in Claude Code — model-initiated, not harness policy. Grounding stays a session-start push.
2. **The memory seam is exempt from admission** (`40ebd8d3`). Store returns inline whole whatever their size, because the caller already bounds them. The owner first preferred a general "bounded-return tools skip admission" rule; every member that survived inspection was a Tabularium verb, so the narrower seam-keyed rule is the honest one. `CONTEXT.md` carries **memory seam** as a term.
3. **The in/out question was malformed** (`d1ff8175`, superseding `dc433ba5`). Both old rulings treat Tabularium as indivisible. Qdrant splits it: engine as a Rust crate inside arx with no transport, Qdrant as a localhost container with its own volume, Ollama already local at ~12ms. `30e83788` survives narrowed — it describes the engine, not the store.
4. **arx builds Tabularium v2** (`dbbe749c`) — Rust, on Qdrant, starting empty; Claude Code stays on v1 forever. The speed argument for Rust failed under inspection (the embed is Ollama's, LanceDB is already Rust); what justifies v2 is that v1 was shaped for a host the owner does not control. Outbound record in scope `tabularium` as `dcda1d0c`.
5. **arx runs on the host, its hands run in containers** (`21e8f9a9`) — sandcastle's shape. Containerizing arx *and* letting it spawn containers would need the Docker socket, which is root on the host, making the boundary decorative. D06 territory, ruled in passing; the rest of that ticket stands.

The pitfall `8a3b81f7` is resolved: the discriminator it caught firing four sections from where it was defined has no claimant at all now, and the ruling it was aimed at has been superseded on other grounds.

Left open, deliberately: single-writer-at-a-time across the three machines is assumed true by construction and never ratified — concurrent writes would clobber, since a push is a snapshot and not a merge. And `b0683132`'s cross-architecture store-copy claim is moot for v2 but still untested for v1.
