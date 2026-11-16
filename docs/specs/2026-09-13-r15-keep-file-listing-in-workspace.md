# R15 — Keep recursive listing inside the workspace

Card: `docs/wayfinder/maps/arx-reliability/R15-keep-file-listing-in-workspace.md`
Board: `docs/wayfinder/maps/arx-reliability/map.md`
Base: `main` @ `7805ee6`
Branch: `r15-keep-file-listing-in-workspace`

## Problem

`list_files_call` canonicalizes only the starting path. The recursive descent then tests `entry.is_dir()` and walks into the result without repeating the root check. A directory symlink inside the workspace that points outside it is therefore followed, and the listing returns external filenames and file types.

This is **metadata disclosure only**. The ordinary read tool still refuses to read those contents.

## Scope boundary — do not conflate with R01

R01 covers a host-content capture escape and is a different card with a different severity. This card is the listing walk and nothing else. If you find yourself changing capture or output handling, you have left the card — STOP and report.

## Deliverable

Either do not descend symlinks at all, or canonicalize every directory and recheck it against the root before descending. Pick one, implement it, and say which you picked.

## Consistency requirement

The read side already guards this with `canonicalize_in_workspace` (`crates/arx/src/main.rs:1368-1385`), which resolves the path and confirms it sits under the workspace root. Your listing policy must be coherent with that guard — a path the read tool refuses must not be a path the list tool enumerates. State in your verdict how the two line up under the policy you chose.

## Scope — owned paths

Only these files may change:

- `crates/arx/src/main.rs`
- `docs/wayfinder/maps/arx-reliability/R15-keep-file-listing-in-workspace.md` (frontmatter and Handoff only)
- this brief

Anything else is out of scope. If the fix appears to require another file, STOP and report rather than widening.

## Orientation

- `list_files_call` — `crates/arx/src/main.rs:1518`
- the recursive `fs::read_dir` collection — `main.rs:1538-1539`
- the read-side guard to stay consistent with — `main.rs:1368-1385`
- tool dispatch — `main.rs:1599`

## Order of work

1. Write the failing regression first: a scratch workspace containing a directory symlink to an external directory. Listing must currently expose the external entries. Record that output.
2. Then implement the chosen policy.
3. Re-run and show the escape closed.

## Required cases

- A directory symlink escaping the workspace does not expose its entries.
- Ordinary nested directories remain listable, and every returned path stays under the declared root.
- A **dangling** symlink terminates cleanly — no traversal, no error cascade.
- A symlink **loop** terminates without hanging. Assert termination explicitly; a test that hangs is a failing test, so give this one a bound.

## Tripwire — test placement

`crates/arx` has no `lib.rs`; integration tests `#[path]`-include files from `src/`. Per the repo's CLAUDE.md, tests for `main.rs` belong in `main.rs`'s own test module — existing file-tool tests live there (see around `main.rs:6458`). Follow that placement.

Count the test total before and after. If the delta is not exactly the number you added, STOP and report — do not adjust a locked count to make it fit.

## Verify

```
cargo test -p arx list_files
cargo test -p arx
```

The second needs the Docker daemon. Do NOT `docker rmi` any stub image — this card touches no stub fixture and concurrent runners are building against those tags. Passing the pre-existing suite alone does not prove this fixed; the new regression is the evidence.

## Constraints

- Scratch workspaces only, created and removed by the test. Never the live store at `~/.local/share/arx/store`.
- Do NOT commit. Do NOT switch branches. Do NOT touch the main checkout at `~/Projects/arx`.
- Work only inside `~/Projects/arx-wt/r15-keep-file-listing-in-workspace`.
- Update `prompts/main.md` in this same change only if main's observable experience changes. Narrowing what `list_files` returns arguably does — decide, and justify it either way in your verdict.

## Non-goals

R10 (bounded reads) and R18 (reviewer family event) also target `main.rs` and are deliberately scheduled after this card. Do not pre-empt either. No other audit findings.
