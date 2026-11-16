---
id: E14
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P3
next: owner-review
brief: docs/specs/2026-09-13-e14-refresh-stub-images-by-content.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
pr: https://github.com/simkaz12/arx/pull/39
verified-revision: 6992daac1cd9517d763bd1b7d999535b352e1e2c
---

# E14 — Refresh Docker fixture images when their content changes

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L35)

## Existing point

Test fixture tags are reused regardless of script content; manual docker rmi is currently required.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: focused-trace.

Confirm the cache/build helper and design content-based invalidation or an equally reliable automatic freshness check.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [x] Changing a fixture script and running the relevant suite executes the new bytes without manual tag deletion. *(True for every input inside the fixture directory. The base image the Dockerfiles pull from is outside it — see residual 1.)*
- [x] Unchanged fixtures do not rebuild unnecessarily, and parallel test invocations do not race image preparation. Verified by an actual concurrent two-build race: a tag reads absent until a build completes, then appears whole.
- [x] The resolution is linked with evidence. The original point was confirmed live before any change.
- [x] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: test infrastructure. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

Briefed and run 2026-09-13: `docs/specs/2026-09-13-e14-refresh-stub-images-by-content.md`. The original point was confirmed live before any change — with `arx-claude-stub:test` already built, editing `tests/fixtures/claude-stub/claude`'s verdict text and re-running `claude_code_hand_dispatch_delivers_stub_verdict_with_trace` still passed on the old text, and `docker run --rm arx-claude-stub:test cat /usr/local/bin/claude` showed the pre-edit script.

Fix: each stub's tag is now a content hash (sha256 over the sorted filenames and bytes of its fixture directory, first 12 hex chars) instead of the fixed `:test` suffix, threaded through the four sites the trace named — `ensure_claude_stub_image`/`ensure_codex_stub_image`, `run_fixture_toml`'s format string, `rewrite_reviewer_definition`'s call site in `reviewer.rs`, and `tests/fixtures/run/arx.toml`'s two `image =` lines (substituted on the tempdir copy in a new `copy_run_fixture` helper, the static fixture file itself untouched). Both directions verified live: the same script edit now misses the new hash tag and rebuilds, with the container immediately reflecting the new text; re-running with unchanged content hits the tag and skips the build (confirmed by an unchanged image `Id`/`Created` across two separate process invocations). No cross-process lock added — differing content yields differing tags by construction, and identical content converging on the same tag under concurrent builds is accepted as harmless per the locked decision.

Verify: `cargo test -p arx --test reviewer` (7 passed) and `cargo test -p arx --test container_runtime` (9 passed), both green. Codex adversarial review round 1: `approve`, no findings. `arx-claude-stub:test` and `arx-codex-stub:test` left in place, untouched, alongside the new hash-tagged images the fixed code now builds.

Independent reviewer PASS with an empty `must_fix`, and a separate verify gate PASS. Neither took the runner's word for the mechanism: the reviewer recomputed the tag derivation independently in Python and got `d83048f73d17` / `8b264b47c9ce`, matching the images actually on the daemon, and proved the Dockerfile feeds the hash by copying a fixture to `/tmp`, editing only the copy's Dockerfile, and watching the digest move. The verify gate ran both suites again and confirmed all four tag sites are threaded, with `tests/fixtures/run/arx.toml` still carrying its static literals on disk.

**Residual 1 — the hash does not cover the whole build context.** Both `Dockerfile`s begin `FROM alpine:3.20`, and `hash_fixture_dir` covers only the files inside the fixture directory. The base image is outside it. This is dormant rather than broken today: `alpine:3.20` is already cached locally and `docker build` without `--pull` resolves against the local store, so current builds are deterministic. But if that local cache ever advances while a fixture's content stays byte-identical, a cache-cold rebuild would bake a different base layer under an unchanged hash — the same "an input is not in the hash, so a stale image is reused" failure this card exists to close, moved one level down. `codex-stub`'s `RUN apk add --no-cache bash` is a network fetch that is likewise uncaptured, though Docker's layer cache suppresses it except on a genuinely cold build. Closing this means folding the base image's resolved digest into the hash, or pinning `FROM` by digest rather than tag. Not attempted here; the brief scoped the hash to fixture inputs.

**Residual 2 — unbounded tag accumulation, small magnitude.** Every fixture edit mints a new tag and nothing removes the old ones. The cost per edit is roughly 2 to 4 kB, not a whole image: `docker history` shows the 9.44 MB alpine base layer is shared byte-for-byte across every `arx-*-stub:*` tag, and only the `COPY` layer is new. The `docker images` SIZE column is misleading here because it reports full logical size. Hundreds of edits would be needed to reach a megabyte of genuinely new data. It is the same shape as [E16](E16-clean-engine-test-temp-roots.md)'s leaked temp roots but a far smaller per-event cost, so it is recorded rather than fixed.

**Residual 3 — a cosmetic loss.** The `.expect(...)` message on the `docker build` `Command::output()` call no longer carries the tag; it reads `"docker build arx-claude-stub"` where it once read `"docker build arx-claude-stub:test"`. The `assert!` immediately after still interpolates `{tag}` correctly. This only surfaces if `docker` fails to spawn at all, not on a build failure.

One nuance recorded for accuracy rather than as a defect: under a concurrent race, two builds of identical content produce differing *manifest* digests, because BuildKit attaches provenance metadata carrying build-specific timestamps even when the rootfs layers are cache-hit identical. Whichever build wins the tag pointer, a container run against it executes the same script, and nothing in this suite inspects manifest digests. So "the only cost is duplicated work" is very slightly understated — duplicated work plus a harmless digest mismatch.

**Follow-up this card creates.** `CLAUDE.md`'s Hazards bullet instructs operators to `docker rmi` a stub tag before re-running after a fixture edit. Content-hash tagging removes that requirement, so the line becomes stale prose the moment this merges. It is outside this card's owned paths and was deliberately left alone — with several worktrees live, an unscoped edit to the repo's instruction file is a needless collision surface. Worth a one-line card once this lands.

Remaining gap: residual 1 is the only one that touches correctness, and only in a state that cannot arise while the base image stays cached.

