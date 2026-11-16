---
id: R13
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P2
next: owner-review
brief: docs/specs/2026-09-13-r13-invalidate-stale-claim-vectors.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
---

# R13 — Invalidate vectors when claim content changes

[Board](map.md) · [Audit finding R13](findings/project-review-2026-09-12.md#R13)

## Problem and evidence

Edit a persisted question or answer while preserving its claim id and embedding model. Vector rows store id/model/vector only. Those keys are considered current, so the old vector is joined to the new claim text.

The similarity landscape can retrieve the wrong neighbors after a valid files-as-truth edit.

Evidence strength: New. BM25 uses current content; this finding affects the cosine landscape, not every retrieval path. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Store a hash of the embedded content and re-embed on mismatch; discard superseded vector rows.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx-engine/src/engine/mod.rs](../../../../crates/arx-engine/src/engine/mod.rs#L103)
- [crates/arx-engine/src/engine/mod.rs](../../../../crates/arx-engine/src/engine/mod.rs#L185)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: store. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Use to-spec on this card and its linked evidence. Recheck the named failure seam against the chosen base and put the regression before the fix. The audit establishes the problem; the brief must lock the implementation scope.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [x] Changing embedded claim content without changing id/model causes recomputation before cosine comparison. Ordering confirmed in `landscape_for`: `ensure_vectors` runs to completion before the query is embedded or any cosine scoring happens.
- [x] Metadata-only changes that do not affect embedded text do not force unnecessary embedding. Embedded text is exactly `format!("{} {}", question, answer)`; every other `Claim` field is metadata. Proven by an inode check rather than byte equality, because a deterministic embedder would produce identical bytes even on a wrong rewrite.
- [x] Old vector rows without a content hash are rebuilt safely. A missing hash fails the match guard and is treated as stale, never trusted.
- [x] Embedding failure preserves the documented BM25 fallback. The degraded path is untouched and its test still passes.
- [x] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [x] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Change a claim's meaning without changing its id/model and verify recomputation. Keep label-only edits exempt.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Engine contract](../arx-v0/L2-04-engine.md) and [glossary](../../../../CONTEXT.md) define file truth, drain consistency and internal write coordination.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

Delivered and awaiting owner review. `VectorRow` gained `hash: Option<String>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`, so rows written before this change deserialize as `None` rather than failing to parse. `ensure_vectors` partitions existing rows into four cases: a different model is kept verbatim, a matching model with a matching hash is kept untouched, a matching model with a mismatched **or missing** hash is discarded and re-embedded, and rows for ids no longer in candidacy are carried forward unchanged. Embedded text is exactly `format!("{} {}", question, answer)`; every other `Claim` field is metadata and cannot trigger a re-embed.

The behaviour is asymmetric in the safe direction, which matches the standing position on content hashes: an unchanged hash is treated as proof, a changed or absent one only as suspicion. A missing hash is never trusted.

Regression-first, proven by temporarily reverting `ensure_vectors` and `VectorRow` to their pre-fix shape with the new tests in place. Two failed exactly as intended: the stale vector `[...0.4472136...]` survived a question edit because the old `current` check compared only id and model, and a fabricated legacy vector `[9.9, 9.9]` was silently trusted. `cargo test -p arx-engine` is 49 passed, 1 ignored.

**Two codex findings, both fixed rather than argued.** Round 1 caught a non-atomic index rewrite: the file is now written to `index_path.with_extension("jsonl.tmp")` and `fs::rename`d into place, with a test that forces the write to fail by pre-creating a directory at the tmp path and asserts the original index is byte-identical afterwards. Round 2 caught a complexity regression — an `O(N·M)` `fresh.iter().any()` scan — fixed with a `retained_ids: HashSet` built in the same single pass. The verify gate confirmed the replacement selects exactly the same set rather than merely running faster. Round 3 approved.

The metadata-only test proves its point by inode rather than by byte comparison, because `HashEmbedder` is deterministic: a wrong re-embed would produce identical bytes and the test would pass anyway. The verify gate confirmed both halves of that reasoning empirically, including that `fs::rename` onto an existing path always yields a new inode, so there is no path that rewrites the index while preserving it.

**Named gap the deliverable's wording oversells.** "Discard superseded vector rows" turns out to mean only that a changed claim does not accumulate a second row for the same id. Rows whose claims have fallen out of candidacy — invalidated, superseded, or a kind other than Fact or Decision — take the carry-forward arm and are kept forever, never hash-checked and never removed. This is pre-existing, matches the card's non-goals, and R12 owns the adjacent staleness concern, so it is recorded rather than repaired. A reader taking that phrase at face value would expect lifecycle garbage collection, and there is none.

Two smaller notes. A stale `.jsonl.tmp` left by an interrupted write is inert — the loader only ever reads the final name, and the next write truncates rather than appends — but if a scope's content never changes again, that orphan is never cleaned up. And the content hash is over the concatenation, so `question: "a b" / answer: "c"` and `question: "a" / answer: "b c"` collide; this is harmless, because the embedder only ever consumes the joined string, making such inputs genuinely equivalent from its point of view.

Remaining gap: none for this card.

