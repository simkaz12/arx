# R06 — Reopen transcripts with a torn final write

Card: `docs/wayfinder/maps/arx-reliability/R06-reopen-torn-transcripts.md`
Board: `docs/wayfinder/maps/arx-reliability/map.md`
Base: `main` @ `7805ee6`
Branch: `r06-reopen-torn-transcripts`

## Problem

The process dies mid-write to a transcript, leaving a final row that is unterminated JSON or partial UTF-8. The read helpers already tolerate that tail. Writer reopen does not: it parses every line through `BufRead::lines` and returns `MalformedLine`, so arx cannot reopen a transcript after a realistic crash even though the committed rows are intact and readable.

## Deliverable

Under the writer lock, truncate only the bytes after the last newline, then recover turn numbering from the committed rows. A malformed but newline-terminated row stays fatal, matching the ledger.

## Scope — owned paths

Only these files may change:

- `crates/arx/src/transcript.rs`
- `crates/arx/tests/transcript_log.rs`
- `docs/wayfinder/maps/arx-reliability/R06-reopen-torn-transcripts.md` (frontmatter and Handoff only)
- this brief

Anything else is out of scope. If the fix appears to require another file, stop and report rather than widening.

## Order of work

1. Write the failing regression first, at the real writer-reopen seam. It must fail against the current code for the stated reason, not by accident. Record the failure output.
2. Then implement the truncation.
3. Re-run and show the same test passing.

## Required cases

- Reopen after an unterminated final row: the torn bytes are removed, reopen succeeds.
- Reopen after a **partial UTF-8** final row specifically — not just truncated ASCII.
- The next append continues the committed turn numbering and does not join onto torn bytes.
- A malformed row that *is* newline-terminated still fails explicitly with `MalformedLine`.
- Tail repair happens under the writer lock; a concurrent writer is still refused.

## Tripwire — test placement

`crates/arx` has no `lib.rs`; integration tests `#[path]`-include files from `src/`. An inline `#[cfg(test)]` module in an included file registers twice and breaks locked test counts. `transcript.rs` already carries an inline test module (around line 479), which implies it may not be included — **verify this before adding a test**. Determine whether any integration test `#[path]`-includes `transcript.rs`. If it does, the new test goes in `main.rs`'s test module instead.

Count the test total before and after your change. If the delta is not exactly the number of tests you added, STOP and report it — do not adjust a locked count to make it fit.

## Verify

```
cargo test -p arx --test transcript_log
cargo test -p arx transcript
cargo test -p arx
```

The last needs the Docker daemon. Passing the pre-existing suite alone does not prove this fixed; the new regression must be the evidence.

## Constraints

- Scratch files and temp dirs for failure tests. Never the live store at `~/.local/share/arx/store`.
- Do not commit, do not switch branches, do not touch `pid.txt`.
- Work only inside `~/Projects/arx-wt/r06-reopen-torn-transcripts`.
- `prompts/main.md` changes only if main's observable experience changes — it should not here.

## Non-goals

Other audit findings. Read-path behaviour, which already works. Any broader transcript redesign.
