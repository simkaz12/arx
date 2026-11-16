---
id: R06
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P1
next: owner-review
brief: docs/specs/2026-09-13-r06-reopen-torn-transcripts.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
pr: https://github.com/simkaz12/arx/pull/29
verified-revision: 1dd2eb232bebbb6fa4187efcd4fde97db6191471
---

# R06 — Reopen transcripts with a torn final write

[Board](map.md) · [Audit finding R06](findings/project-review-2026-09-12.md#R06)

## Problem and evidence

The process dies during a transcript write, leaving an unterminated JSON or partial UTF-8 row. Read-side helpers ignore the unterminated tail, but writer reopen parses it through BufRead::lines and returns MalformedLine.

arx cannot reopen the transcript after a realistic crash, even though its readers can recover the committed rows.

Evidence strength: New. High-confidence source trace; existing read-side tests do not cover reopening that file for writing. The audit was a source review; a new brief must name the executable failure check.

## Deliverable

Under the writer lock, truncate only the bytes after the last newline before recovering turn numbering. Keep malformed newline-terminated rows fatal, as the ledger already does.

This is a task card, not an execution brief. The board's planning authorization and flow handoff rules apply.

## Scope and ownership

Likely source files:

- [crates/arx/src/transcript.rs](../../../../crates/arx/src/transcript.rs#L108)
- [crates/arx/src/transcript.rs](../../../../crates/arx/src/transcript.rs#L326)
- [crates/arx/tests/transcript_log.rs](../../../../crates/arx/tests/transcript_log.rs#L168)

Add the regression tests at the real caller seam and list exact owned paths in the brief. Shared-file scheduling groups: transcript. Coordinate these with the board before dispatch; do not revert another worker's changes.

## Next flow step

Use to-spec on this card and its linked evidence. Recheck the named failure seam against the chosen base and put the regression before the fix. The audit establishes the problem; the brief must lock the implementation scope.

No logical prerequisite is currently known. Shared-file overlap is a scheduling constraint, not a dependency.

## Done when

- [x] Writer reopen tolerates and removes only an unterminated final row, including partial UTF-8.
- [x] The next append continues the committed turn numbering without joining onto torn bytes.
- [x] A malformed newline-terminated row still fails explicitly.
- [x] Tail repair happens under the writer lock; concurrent writers remain refused.
- [x] An agreed execution brief is linked above; its runner and independent reviewer pass.
- [x] Flow's verify step passes against the implemented agreement, with evidence linked above. Only then may the card enter Done.

## Verification starting point

Reopen after a partial UTF-8 tail, append, and verify numbering. Separately verify rejection of a malformed committed row.

Select focused commands after locating the test seam. Run the applicable regression checks from the brief; passing the old workspace suite alone does not prove this failure is fixed. Follow the repository's Docker fixture rebuild and included-module test rules.

## Constraints and exclusions

- Preserve the existing dirty work and live store. Use scratch stores/workspaces for failure tests.
- Do not broaden this card into other audit findings or the old polish backlog.
- A new product expectation or contradiction with a ruling returns to grill-me; an implementation choice derivable from the contract does not need a new approval.
- Update prompts/main.md in the same implementation if main's observable experience changes.

## Contract references

- [Transcript module](../../../../crates/arx/src/transcript.rs) already distinguishes committed rows from an unterminated tail on reads.

Read the linked contract before briefing. If it contradicts a stored ruling, reopen that ruling through the project's registered memory scope; do not rewrite a closed design card.

## Handoff

Delivered and awaiting owner review. `TranscriptLog::open` now calls `truncate_torn_tail` after `try_lock()` succeeds and before `recover_next_turn`. The repair byte-scans for the last `0x0A` via `rposition` and `set_len`s to `pos + 1`, so it removes only bytes after the last newline and never touches a row that already ends in one. Byte-scanning rather than decoding is what makes a torn multi-byte codepoint safe: `0x0A` can never appear as a UTF-8 lead or continuation byte.

Five regressions in `crates/arx/tests/transcript_log.rs`: unterminated ASCII tail, partial UTF-8 tail (a 4-byte emoji torn at two bytes, a genuinely incomplete codepoint), append continuing committed numbering with byte-exact file assertions, a newline-terminated malformed row still raising `MalformedLine`, and a concurrent open refused with `Locked` while a torn tail awaits repair. `cargo test -p arx --test transcript_log` is 33 passed, 0 failed — baseline 28, delta exactly 5, none double-registered.

Test placement note. `transcript.rs` *is* `#[path]`-included by `transcript_log.rs`, so its inline `mod tests` already double-registers. The new tests are plain top-level functions in `transcript_log.rs`, which is itself never included anywhere, so they register once. The brief's tripwire said to fall back to `main.rs`'s test module, which was unreachable here because `main.rs` sits outside owned paths; this placement satisfies the same anti-doubling intent.

Named remaining gap: a **fully torn file** — one containing no newline at all, from a crash before the first row ever terminated — has no test. The implementation handles it correctly by inspection (`rposition` returns `None`, the file truncates to empty, numbering restarts at 1), and that is consistent with the committed/uncommitted invariant, since a row that never terminated was never promised durable. Every current fixture seeds at least one well-formed row first. Codex raised this as an advisory in round 1; it was judged non-material and left for a follow-up rather than expanded into this card.

