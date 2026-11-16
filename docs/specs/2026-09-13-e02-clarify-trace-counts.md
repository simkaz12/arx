# E02 — Distinguish trace-file counts from body counts

Card: `docs/wayfinder/maps/arx-reliability/E02-clarify-trace-counts.md`
Base: `main` @ `7805ee6` · Branch: `e02-clarify-trace-counts`

## Problem — already traced

Two numbers describe different quantities and are labelled as if they were the same one.

The echo branch (`crates/arx/src/admission.rs:196-208`) counts the **whole sealed file**, header included:

```rust
let body_bytes = req.body.len();
let body_lines = req.body.lines().count();
let text = format!("[arx] this result is the whole of {handle}; the handle is unchanged ({body_bytes} bytes, {body_lines} lines)\n{head}");
```

When the path read is the trace file itself, `req.body` is the entire file from disk — `main.rs:1403-1441` does `fs::read` of the whole file and passes the unsliced content through.

The frontmatter inside that same head (`admission.rs:157-161`, populated at `:237-251`) reports the **original body's** size, excluding the header it sits in.

Measured gap for a 48000-byte/1200-line file: the echo preview says `(48218 bytes, 1211 lines)` directly above a frontmatter reading `bytes: 48000` / `lines: 1200`. The file never changed — it is written once with `create_new` (`admission.rs:166`).

This is not hypothetical. `docs/wayfinder/maps/arx-v0/map.md:232` records the L3-01 live check: main "read the frontmatter's body byte count against the preview's file byte count and told the operator the file had grown."

## Deliverable — labelling only, not arithmetic

**Do not change what either number counts.** In-force decision `8098d8ee` (D11 round 2) characterises the echo preview's count as a provable fact about "the file just read" — the current measurement is intended. Changing it to match the frontmatter would contradict a live ruling.

Add words so the two cannot be read as the same quantity. For example: the echo line naming its count as the file including the header below, and/or the frontmatter naming its own as body-only excluding the header. Which side carries the words is yours to settle — the card calls this a source-based implementation choice.

## Scope — owned paths

- `crates/arx/src/admission.rs`
- `docs/wayfinder/maps/arx-reliability/E02-clarify-trace-counts.md` (frontmatter and Handoff only)
- this brief

If a change appears to need `main.rs`, STOP and report — the labels live in `admission.rs`.

## Concurrent work

`e01-bound-hand-delivery` is running now and **may** touch `admission.rs`, but only at its entry point for delivery admission, and only if it cannot reuse what exists. Your work is the echo branch at ~`196-208` and the frontmatter builder at ~`157-161`/`237-251`. Stay in those. Do not refactor shared helpers.

## Order of work

1. Failing regression first: echo a real trace file and show the preview's number and the frontmatter's number differ with nothing distinguishing them. Record it.
2. Then add the labels.
3. Re-run.

## Required cases

- After the change, the two numbers are unambiguously different quantities to a reader seeing only the echo output.
- The numbers themselves are unchanged — assert the same values as before.
- The first-time externalize path, where preview and frontmatter legitimately agree, stays coherent and is not made confusing by the new wording.

## Tests that constrain you

- `crates/arx/src/admission.rs:542` (`a_whole_read_of_a_trace_over_the_threshold_is_an_echo`) — a unit test whose synthetic body has no embedded frontmatter; it locks the preview's exact format string at `:574`. Wording changes require editing it. That is expected and allowed **for this test only**; say so in your verdict.
- `crates/arx/tests/admission.rs:76` (`a_whole_read_of_the_handle_is_an_echo_and_mints_nothing`) — asserts only `starts_with` up to the opening parenthesis (`:94`). It should keep passing untouched. If it breaks, you changed more than wording.

## Verify

```
cargo test -p arx --bin arx admission
cargo test -p arx --test admission
```

Do NOT run `cargo test --workspace` or the full Docker suite as evidence. Do NOT `docker rmi` any stub image.

## Constraints

- Do NOT commit, switch branches, or run ANY git write. No `git stash`, no `git checkout`. Use `git show main:<path>`.
- Work only inside `~/Projects/arx-wt/e02-clarify-trace-counts`.
- `prompts/main.md`: main reads these lines and drew a wrong conclusion from them once. If the wording change alters what main should understand, update it here. Decide and justify.

## Non-goals

Do not reopen `8098d8ee`. Do not change the echo threshold, the handle-minting rule, or E01's verdict-bounding work.
