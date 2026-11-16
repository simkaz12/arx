# R13 — Invalidate vectors when claim content changes

Card: `docs/wayfinder/maps/arx-reliability/R13-invalidate-stale-claim-vectors.md`
Board: `docs/wayfinder/maps/arx-reliability/map.md`
Base: `main` @ `7805ee6`
Branch: `r13-invalidate-stale-claim-vectors`

## Problem

Edit a persisted question or answer by hand while keeping its claim id and embedding model — a legitimate files-as-truth edit. Vector rows store only id, model and vector, so those keys still look current and the **old vector stays joined to the new text**. The cosine landscape can then retrieve the wrong neighbours.

BM25 uses current content, so this affects the cosine leg only, not every retrieval path. Do not overstate it.

## Deliverable

Store a hash of the embedded content alongside the vector row. Re-embed when the hash does not match the current text. Discard superseded vector rows.

## Scope — owned paths

Only these files may change:

- `crates/arx-engine/src/engine/mod.rs`
- `crates/arx-engine/tests/engine.rs`
- `docs/wayfinder/maps/arx-reliability/R13-invalidate-stale-claim-vectors.md` (frontmatter and Handoff only)
- this brief

If the vector row's schema lives outside these files, STOP and report rather than widening.

## Concurrent work — do not collide

Two open, unmerged PRs touch files near yours:

- PR #28 (`r08-reject-duplicate-supersession-targets`) added one hunk to this same file at `@@ -647,+18` in `validate_drain_plan`. **Stay out of that region.**
- `r03-recover-interrupted-drains` is being worked in parallel right now in a sibling worktree, in `apply_drain_plan` (~`mod.rs:711-787`). **Stay out of that too.**

Your work is the vector/index path around `mod.rs:103` and `mod.rs:185` and whatever `ensure_vectors` touches. Keeping to it is what lets all three merge cleanly.

## Orientation

- `mod.rs:103` and `mod.rs:185` — the card's named seams
- `ensure_vectors` — called after drain completes; the re-embedding entry point
- existing coverage to study first: `index_rows_with_another_model_are_reembedded` already handles the *model changed* case. The content-changed case is the same shape with a different key. Follow that test's pattern rather than inventing a new one.

## Order of work

1. Write the failing regression first: persist a claim, let it embed, then edit its question/answer text on disk while keeping id and model, and show the stale vector is still used. Record that output.
2. Then add the content hash and the mismatch check.
3. Re-run and show recomputation happening before cosine comparison.

## Required cases

- Changing embedded claim content without changing id or model forces recomputation **before** cosine comparison.
- A metadata-only change that does not affect the embedded text does **not** force unnecessary embedding. Define precisely which fields feed the embedded text and say so in your verdict.
- Old vector rows that predate the hash — i.e. have no hash stored — are rebuilt safely rather than crashing or being silently trusted.
- Embedding failure still falls back to BM25 as documented. Do not regress that.

## Verify

```
cargo test -p arx-engine
```

No Docker needed. Do NOT use `cargo test --workspace` as evidence — it is flaky from unrelated container tests in `crates/arx`, which you are not touching.

If any test needs a live embedder, note that `live_ollama_landscape_surfaces_paraphrase` is `#[ignore]`d in this suite. Do not un-ignore it and do not make your regression depend on a live model.

## Constraints

- Scratch stores only. NEVER `~/.local/share/arx/store` — it holds the owner's real claims.
- Do NOT commit, switch branches, or run any git write. Work only inside `~/Projects/arx-wt/r13-invalidate-stale-claim-vectors`.
- No `git stash` or `git checkout` — two agents breached that tonight.

## Non-goals

R03's drain recovery, R08's duplicate rejection, R11's claim-markdown round trip, R12's shared-engine cache and locking. R12 in particular also concerns staleness — it is about a *different* cache (the in-process `BTreeMap`), not vector rows. Do not merge the two.
