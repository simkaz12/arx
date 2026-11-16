# R14 — Retain incomplete provider output in traces

Card: `docs/wayfinder/maps/arx-reliability/R14-retain-partial-provider-output.md`
Board: `docs/wayfinder/maps/arx-reliability/map.md`
Base: `main` @ `7805ee6`
Branch: `r14-retain-partial-provider-output`

## Problem

A provider streams some text, then disconnects, times out, or emits malformed SSE. The accumulated response blocks stay local to the loop until a terminal chunk arrives. On the error path the loop writes an error row and drops those blocks, so the trace loses the evidence of what the hand actually produced before it failed.

## Deliverable

Record the received blocks as explicitly incomplete output before error handling runs. They must not be promoted to a successful verdict or an accepted state object.

## Scope — owned paths

Only these files may change:

- `crates/arx/src/native_hand.rs`
- `docs/wayfinder/maps/arx-reliability/R14-retain-partial-provider-output.md` (frontmatter and Handoff only)
- this brief

Plus, only if the test seam genuinely lives there, one existing test file under `crates/arx/tests/` — name it in your verdict if you use it. Anything else is out of scope; stop and report rather than widening.

## Orientation

- block accumulation: `push_block` at `native_hand.rs:134`, loop accumulation at `:562`-`:568`
- the failure that discards them: `"stream ended without a terminal chunk"` at `:589`
- the success path that keeps them: `content_blocks = blocks` at `:595`, `trace_write!` at `:600`
- a second, earlier accumulation path at `:272`-`:293`. Determine whether it has the same defect; if it does, fix both and say so; if it does not, say why.

## Order of work

1. Write the failing regression first: drive content-then-error through the native loop and assert the trace currently loses the partial content. Record the failure.
2. Then implement the incomplete-output row.
3. Re-run and show it passing.

## Required cases

- Content followed by **timeout**, by **disconnect**, and by **malformed stream** is preserved as explicitly incomplete evidence in the trace.
- The hand still fails. Partial content is not promoted to a success verdict or an accepted state object.
- A successful terminal stream keeps its existing outcome unchanged.
- An error arriving *before* any content keeps its existing outcome unchanged.

The incomplete row must be distinguishable from a successful assistant row by an explicit marker, not by the reader inferring it from surrounding rows. Choose the marker to match existing trace-row conventions in this file.

## Tripwire — test placement

`crates/arx` has no `lib.rs`; integration tests `#[path]`-include files from `src/`. An inline `#[cfg(test)]` module in an included file registers twice and breaks locked test counts. `native_hand.rs` already has inline tests (around `:1392`, `:1446`), which suggests it may not be included — **verify before adding**. If it is included, the test belongs in `main.rs`'s test module.

Count the test total before and after. If the delta is not exactly what you added, STOP and report — do not bend a locked count to fit.

## Verify

```
cargo test -p arx native_hand
cargo test -p arx
```

The second needs the Docker daemon. Do not `docker rmi` the stub images — this card touches no stub fixture, so the existing tags are correct and another card may be building against them concurrently.

## Constraints

- Scratch workspaces for failure tests. Never the live store.
- Do not commit, do not switch branches.
- Work only inside `~/Projects/arx-wt/r14-retain-partial-provider-output`.
- Do not make real provider calls. Drive the loop with scripted/stubbed streams as the existing tests do.
- `prompts/main.md` changes only if main's observable experience changes.

## Non-goals

R10's bounded-read work also touches `native_hand.rs` and is deliberately scheduled after this card — do not pre-empt it. No other audit findings.
