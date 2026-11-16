---
id: E02
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P3
next: owner-review
brief: docs/specs/2026-09-13-e02-clarify-trace-counts.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13 with a tie-break ruling that removed the key rename
pr: https://github.com/simkaz12/arx/pull/36
verified-revision: 77a12d8f4321426854b9e85b535df54cb39a02f2
---

# E02 — Distinguish trace-file counts from body counts

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L10)

## Existing point

The echo preview counts the complete trace file while frontmatter counts its body; the difference has been mistaken for growth.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: focused-trace.

Check current output, then name the quantities consistently wherever the operator or main compares them.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [x] The preview and frontmatter identify exactly which bytes/lines they count. The echo line names its own count as the whole file, header included; the frontmatter's body-only figure is left labelled as it was, which is unambiguous once the echo says what it is not.
- [x] A captured example explains the difference without suggesting that the sealed file changed. The regression uses a real 48000-byte/1200-line file and asserts both numbers unchanged.
- [x] The resolution is linked with evidence. The original point was revalidated as still true before the repair.
- [x] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: admission. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

Briefed and run: `docs/specs/2026-09-13-e02-clarify-trace-counts.md`. Confirmed the carried point still holds: a regression test built a real 48000-byte/1200-line trace file and its echo readback, and failed against the unfixed code because the frontmatter still said bare `bytes`/`lines` — no wording told a reader the echo's count (whole sealed file) and the frontmatter's count (original body only) were different quantities.

Fix, labelling only, arithmetic and the frontmatter's `bytes:`/`lines:` keys unchanged: trace files are write-once (`create_new`, `admission.rs:166`) and never expire (decision 72f65636), so a key rename would permanently split the corpus into two schemas with no version marker for no reader benefit — the frontmatter's own `bytes:`/`lines:` keys stay as they were on `main`. The words that actually resolve the L3-01 misread live on the echo side, which is schema-agnostic: the echo branch's misleadingly-named locals are now `file_bytes`/`file_lines` (they always held the whole sealed file, header included, not the body) and the preview reads "... the handle is unchanged (N bytes, M lines including the header below)" (`crates/arx/src/admission.rs:196-208`). The externalize path's own preview, which already reports the same body-only figure as the frontmatter, is untouched and still coherent. `prompts/main.md`'s echo line note now says its counts are of the whole file, header included, not the frontmatter's body-only counts, since main's own misread of these two numbers was the original evidence for this card.

Tests: the locked format-string test (`crates/arx/src/admission.rs:542`) was updated for the new echo wording only, as authorized; the handle-echo integration test (`crates/arx/tests/admission.rs:76`) passed unedited. Added `echo_and_frontmatter_counts_are_labelled_as_different_quantities`, which failed pre-fix and passes post-fix, asserting the frontmatter still reads `bytes: 48000`/`lines: 1200` while the echo distinguishes its own count with "including the header below". `cargo test -p arx --bin arx admission`: 11 passed. `cargo test -p arx --test admission`: 6 passed. Codex adversarial review, one round, verdict approve, no findings (`~/.claude/codex-logs/2026-09-13-e02-clarify-trace-counts.md.codex-review.r1.log`, predates this amendment; the amendment removed a rename codex had not flagged, so nothing in its verdict needs re-litigating).

All three gates passed. The independent reviewer returned PASS with an empty `must_fix`, and the separate verify gate returned PASS.

**The key rename was reverted after review, on the verify gate's ruling.** The runner originally renamed the frontmatter keys to `body_bytes:`/`body_lines:`. The reviewer would not have shipped that but judged it inside the brief's letter and declined to make it a `must_fix`; the verify gate was asked to break the tie and ruled against it, holding the fact that settled it — trace files are write-once and never expire, so the corpus would have split permanently into two schemas rather than migrating. Both earlier gates had assumed old files would age out. They do not. The runner was resumed, endorsed the reasoning rather than merely complying, and reverted the keys while keeping the echo-side wording, the local rename, the `prompts/main.md` clause and the regression.

Verify also confirmed the externalize path stays coherent, that `8098d8ee` is not contradicted (the echo still counts the whole sealed file and the new wording recommends no slice), and that this lane never crossed `e01-bound-hand-delivery`, which is editing `admission.rs` at its delivery-admission entry point.

Remaining gap: none for this card.

