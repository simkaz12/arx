# R08 — Reject duplicate supersession targets

Card: `docs/wayfinder/maps/arx-reliability/R08-reject-duplicate-supersession-targets.md`
Board: `docs/wayfinder/maps/arx-reliability/map.md`
Base: `main` @ `7805ee6`
Branch: `r08-reject-duplicate-supersession-targets`

## Problem

Two promoted candidates in one drain can both supersede the same in-force claim. `validate_drain_plan` checks each plan against the pre-write cache independently, so neither sees the other. `apply_drain_plan` then writes both successors and sets the predecessor's singular `superseded_by` twice — the second write wins.

The commit succeeds and the store is left with contradictory lineage: two claims each assert they superseded the predecessor, while the predecessor points at only one of them.

## Deliverable

Reject duplicate scope/target supersession pairs during whole-plan validation, before any write occurs.

## Scope — owned paths

Only these files may change:

- `crates/arx-engine/src/engine/mod.rs`
- `crates/arx-engine/tests/engine.rs`
- `docs/wayfinder/maps/arx-reliability/R08-reject-duplicate-supersession-targets.md` (frontmatter and Handoff only)
- this brief

Anything else is out of scope. If the fix appears to require another file, stop and report.

## Orientation

- `validate_drain_plan` — `crates/arx-engine/src/engine/mod.rs:649`
- `apply_drain_plan` — `mod.rs:711`
- the offending predecessor write — `mod.rs:755` (`target_claim.superseded_by = Some(...)`)
- there is already a same-shaped per-candidate duplicate check near `mod.rs:606` (`"duplicate suggestion for candidate"`). Follow its error style and `StoreError` variant conventions rather than inventing a new reporting shape.

The check must be whole-plan, across candidates. A per-candidate check cannot catch this.

## Order of work

1. Write the failing regression first: a drain with two promotions targeting one claim. It must fail against current code by actually producing the contradictory lineage. Record that output.
2. Then add the whole-plan validation.
3. Re-run and show rejection.

## Required cases

- A drain containing two promotions for one scope/target is rejected **before any mutation**.
- After rejection, claim files, quarantine rows and git state are byte-identical to before the attempt. Assert this, do not assume it.
- A valid batch with two promotions against *different* predecessors still commits and preserves bidirectional lineage.

## Verify

```
cargo test -p arx-engine
cargo test --workspace
```

`arx-engine` needs no Docker. Passing the pre-existing suite alone does not prove this fixed; the new regression is the evidence.

## Constraints

- Scratch stores only. Never `~/.local/share/arx/store`.
- Do not commit, do not switch branches.
- Work only inside `~/Projects/arx-wt/r08-reject-duplicate-supersession-targets`.
- `crates/arx-engine` is a library crate, so inline `#[cfg(test)]` modules are fine there — the `#[path]` hazard applies to `crates/arx`, not here. Confirm before relying on it.

## Non-goals

Other audit findings. R11's claim-markdown round-trip, R12's shared-cache work and R13's vector invalidation all touch the same store area — do not drift into them.
