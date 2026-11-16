# E15 — Correct the live event-kind documentation

Card: `docs/wayfinder/maps/arx-reliability/E15-correct-event-kind-documentation.md`
Board: `docs/wayfinder/maps/arx-reliability/map.md`
Base: `main` @ `7805ee6`
Branch: `e15-correct-event-kind-documentation`

## Problem — already confirmed on this base

The module doc comment on `crates/arx/src/events.rs` (lines 3-12) narrates the live event kinds across three sentences. `Event::kind` (`events.rs:153-173`) has **21** match arms:

gate, spawn, steer, tap, kill, delivered, nudge, wall, admission, pull, echo, rewind, checkpoint, reset, **miss**, floor, orientation, schema, family, drain, model

The doc narration names 20 distinct kinds (it repeats admission/pull/echo in a second clause). The one omitted is **`miss`**.

## Deliverable

Add `miss` to the narration so the documented kinds match the definitions. Documentation only.

## Tripwire — do not "fix" the count

The doc says "`KINDS` lists all twenty-one in declaration order." **Twenty-one is correct** — there really are 21 arms. The defect is the narration, not the number. If you find yourself changing "twenty-one", STOP and report: you have misread the defect.

Likewise, verify `KINDS` itself actually contains 21 entries in declaration order. If it does not, that is a *different* and larger defect — STOP and report it rather than absorbing it into this card.

## Placement

Do not simply append `miss` to the nearest list. Find the live emitter of `Event::Miss` in the source and place it in the sentence whose provenance actually matches (the existing sentences group kinds by where they landed: own cards / main's file and hand closures and the native loop / the reset path in L3-02 / the `/model` command). State in your verdict which emitter you found and why you placed it where you did.

## Scope — owned paths

Only these files may change:

- `crates/arx/src/events.rs` — **the doc comment only**
- `docs/wayfinder/maps/arx-reliability/E15-correct-event-kind-documentation.md` (frontmatter and Handoff only)
- this brief

No other file. No code change of any kind: no enum variants, no `kind()` arms, no `KINDS`, no serde attributes.

## Required outcome

- The documented kind names match the current event definitions exactly.
- No runtime behaviour changes. The diff touches comment lines only — confirm this by reading your own diff before finishing.

## Verify

```
git diff
cargo test -p arx --test events
cargo build -p arx
```

`git diff` must show only comment-line changes in `events.rs` plus the card and brief. Paste it in your verdict — for this card the diff *is* the evidence.

## Constraints

- Do NOT commit. Do NOT switch branches. Do NOT touch the main checkout at `~/Projects/arx`.
- Work only inside `~/Projects/arx-wt/e15-correct-event-kind-documentation`.
- Other runners are working concurrently in sibling worktrees. Do NOT `docker rmi` any stub image.
- No regression test is required: a comment cannot be regression-tested, and inventing one would be noise. Say so plainly in your verdict.

## Non-goals

Any behaviour change. Any other carried finding that happens to live in `events.rs`. Rewriting the doc comment's prose style beyond inserting the missing kind.
