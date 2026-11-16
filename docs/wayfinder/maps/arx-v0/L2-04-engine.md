---
id: L2-04
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L1-close
ruled-by: D13 (`c775b5c2`, `30e83788`, `7be68c4a` — the four-field claim, `da0b0d8b` — landscape and quarantine, `27e7cea9` — the drain's two pens, `896b62ad` — lexical recall; `../arx-design/findings/debate/2026-08-27-arx-d13-recall-gate/ledger.md`), `da0ccf24` (no digest kind, no narrative layer), D09 (`8915c49b` — the ember), T08 (`../arx-design/findings/T08-findings.md` — text-only led at recall@1), D01 (`40ebd8d3`)
---

# L2-04 — the engine

## Deliverable

Tabularium v2 as a Rust crate over a file store that is a git repository, behind L1-06's trait: record kinds fact / decision, each a claim of exactly four fields — question, answer, why, handle (`7be68c4a`) — plus the ember (`8915c49b`, owner-written, not a claim); no digest kind (`da0ccf24`); scopes, supersession lineage with `supersedes` / `superseded_by`, lifecycle and validity, hot and ember tiers. Save is not a store write: the intent returns a landscape and the record waits in the per-thread quarantine (`da0b0d8b`); the drain's promotion, minted by main (`27e7cea9`), is the write. Retrieval is lexical — BM25 over the claim files, no vector leg (`896b62ad`) — brute-force in-process at this corpus size, no Qdrant; the verbs main mounts — save, recall, decisions_query, decision, drain — as library calls. Starts empty. Sync is clone, pull and merge. Claude Code stays on v1 and never sees this store.

## Done when

Over an empty store: a save intent whose content contradicts a seeded claim returns a landscape naming that claim, and the store's git log stays unchanged until a drain promotion lands it as one commit; a claim missing any of its four fields is rejected; save two decisions where the second supersedes the first — recall returns the successor carrying its predecessor's id and lists the predecessor as superseded only under `include_superseded`, and decisions_query finds both the same way; a fact saved then invalidated is excluded by default and found with `include_invalid`; a ranking-sanity fixture holds for the lexical leg (exact phrase over partial match, rare term over common, no long-claim bias) — T08's measurement stands as the reason for the sole lexical leg and is not re-run, its fixture and scripts being gone from /tmp and the owner having ruled no export from Tabularium v1 (grill 2026-09-03); the crate implements L1-06's trait with the six shipped signatures unchanged, widened only by default-bodied methods carrying the flags and the landscape's legs.

## Ruled by

D13 (`c775b5c2`, `30e83788`, `7be68c4a`, `da0b0d8b`, `27e7cea9`, `896b62ad`), `da0ccf24`, D09 (`8915c49b`), T08, D01 (`40ebd8d3`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

The engine crate lives in `crates/arx-engine/src/engine/`: `mod.rs` (the `Engine` type and its `Store` impl), `claim_file.rs` (frontmatter and body), `git.rs` (the `git` binary calls), `bm25.rs` (the lexical index), and `embed.rs` (the `Embedder` trait, `OllamaEmbedder`, `HashEmbedder`). The ranking-sanity fixture that stands in for T08's now-gone corpus lives at `crates/arx-engine/tests/fixtures/recall-sanity.json`: forty-two hand-written claims about one invented project, ten queries covering exact-phrase, rare-term, length, case and no-overlap two ways each. The live check ran, not blocked: `nomic-embed-text` was present on the host's Ollama, and `cargo test -p arx-engine --test recall -- --ignored live_` passed, the seeded claim surfacing through the cosine leg for a paraphrased query. Two findings against layer 1 preceded the brief: `Store` couldn't express the flags or lineage this card's done-when names, so it gained three default-bodied methods while the six shipped signatures stayed byte-identical; and T08's own fixture and scripts no longer exist on this host, so its done-when clause became the ranking-sanity fixture above instead of a re-run.
