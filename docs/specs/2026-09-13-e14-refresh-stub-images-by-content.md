# E14 — Refresh Docker fixture images when their content changes

Card: `docs/wayfinder/maps/arx-reliability/E14-refresh-stub-images-by-content.md`
Base: `main` @ `7805ee6` · Branch: `e14-refresh-stub-images-by-content`

## Problem — already traced, do not re-derive

`crates/arx/tests/common/mod.rs:35-41` (claude) and `:71-77` (codex) each do:

```rust
let inspect = StdCommand::new("docker")
    .args(["image", "inspect", "arx-claude-stub:test"])
    .output()
    .expect("docker image inspect");
if inspect.status.success() {
    return;
}
```

The freshness test is "does a tag exist", never "does the tag's content match the fixture on disk". Each is wrapped in a `std::sync::Once`, which dedupes per process only, not across processes.

So editing a script under `crates/arx/tests/fixtures/*-stub/` silently keeps running the **old** image until someone manually `docker rmi`s the tag. The repo's CLAUDE.md documents this as a standing hazard operators must remember. It should not be a thing operators have to remember.

Introduced at `fadb615` and untouched since. Not currently mis-firing — both images happen to match disk right now — but nothing in the code guarantees that.

## Locked decision — content-hash tagging, and why the alternatives are out

The card's own text delegates the mechanism choice to briefing. This brief settles it, because the evidence eliminates two of the three candidates rather than merely disfavouring them:

- **mtime comparison is unreliable in this environment.** The trace found the on-disk `claude-stub/claude` mtime sitting *after* the image's `Created` timestamp while the content was byte-identical. Git checkouts and worktree operations do not preserve edit time, and this machine's clock is not monotonic across sessions. A freshness check that reports "stale" on identical content is worse than the bug.
- **Naive always-rebuild is an active regression.** `arx-claude-stub:test` is a single mutable tag. Several worktrees run Docker-backed suites against one daemon concurrently — that happened repeatedly tonight. If two worktrees hold different fixture content and both `docker build -t arx-claude-stub:test` unconditionally, the last build wins and the other worktree's containers silently resolve to the wrong image. That is worse than today's status quo, where the tag is at least a known fixed quantity.
- **Content-hash tagging removes the race by construction.** Each worktree hashes its own fixture directory and gets its own tag, so differing content cannot collide on one name. Identical content producing the same tag is benign: Docker's layers are content-addressed, so concurrent builds converge on the same image id and the only cost is duplicated work, never incorrect state.

Do **not** implement mtime or unconditional rebuild. If you believe content-hashing cannot work here, STOP and report rather than substituting an alternative.

## Deliverable

Derive each stub image's tag from a hash of its fixture inputs — the Dockerfile plus the script, at minimum. Inspect *that* tag and build only on a miss.

## Scope — owned paths

- `crates/arx/tests/common/mod.rs`
- `crates/arx/tests/fixtures/run/arx.toml`
- `crates/arx/tests/reviewer.rs`
- `docs/wayfinder/maps/arx-reliability/E14-refresh-stub-images-by-content.md` (frontmatter and Handoff only)
- this brief

**The literal tag string is baked into four places, not one.** A fix confined to `ensure_*_stub_image` will compile and silently do nothing, because the consumers still reference the old fixed name:

- `crates/arx/tests/common/mod.rs:35-41` and `:71-77` — the inspect/build functions
- `crates/arx/tests/common/mod.rs:189` — `run_fixture_toml`'s format string
- `crates/arx/tests/fixtures/run/arx.toml:14,19` — a **static file**, so the tag must be substituted at fixture-copy time rather than hardcoded
- `crates/arx/tests/reviewer.rs:162` — `rewrite_reviewer_definition(... "arx-claude-stub:test")`

Grep for both literals yourself before finishing and confirm none remain. If you find a fifth site the trace missed, name it in your verdict.

## Do NOT modify the shared fixture directory on disk

`crates/arx/tests/fixtures/run/` is read concurrently by other lanes. `fresh_run_fixture()` copies it into a tempdir — do the tag substitution on **the copy**, never the source. A sibling card did exactly this for a config value tonight and it is the pattern to follow.

## Concurrency requirement — this is the point of the card

Two worktrees running the suite simultaneously must not interfere. After your change:

- differing fixture content must produce differing tags, so neither can clobber the other
- identical content may build the same tag concurrently; that must be harmless, not a failure

You do not need a cross-process lock for correctness. Do not add one; if you think it is required, report why instead.

## Order of work

1. Demonstrate the defect first: with an image already tagged, modify a stub script, run a test that uses it, and show the old behaviour still runs. Record that.
2. Implement hash-derived tagging and thread the tag through all four sites.
3. Show the same edit now triggers a rebuild and the new behaviour takes effect.
4. Show an unchanged fixture does **not** rebuild — otherwise you have implemented always-rebuild.

## Verify

```
cargo test -p arx --test reviewer
cargo test -p arx --test container_runtime
```

These need the Docker daemon. **Do NOT `docker rmi` the existing `arx-claude-stub:test` or `arx-codex-stub:test` tags** — other agents may still be running suites against them. Your change should make those tags simply fall out of use, not require their removal.

Leave behind no images beyond what the tests build. Report which tags exist afterwards.

## Constraints

- Do NOT commit, switch branches, or run ANY git write. No `git stash`, no `git checkout`. Three agents breached this tonight. Use `git show main:<path>` from the start if you need a base version.
- Work only inside `~/Projects/arx-wt/e14-refresh-stub-images-by-content`.
- No production code under `crates/arx/src/`. This is test infrastructure. If the fix appears to need `src/`, STOP and report.
- `prompts/main.md` is untouched — this changes nothing main observes.

## Non-goals

E16's engine temp-root leak and E23's isolation diagnosis are separate cards in the same scheduling group. Do not fold them in. Do not attempt to fix the `/tmp/arx-hands` sharing that E23 owns.
