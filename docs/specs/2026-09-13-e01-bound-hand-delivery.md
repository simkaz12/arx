# E01 — Bound hand verdicts before delivery

Card: `docs/wayfinder/maps/arx-reliability/E01-bound-hand-delivery.md`
Board: `docs/wayfinder/maps/arx-reliability/map.md`
Base: `main` @ `7805ee6`
Branch: `e01-bound-hand-delivery`

## Problem — already traced, do not re-derive

A hand's verdict text reaches main's window verbatim, unbounded, with no admission check anywhere on the path:

- `claude_hand::build_verdict_text` (`crates/arx/src/claude_hand.rs:122-135`) returns `result.to_string()` — the entire SDK result string, no length limit.
- It becomes `Delivery::Verdict { text, .. }` and is sent at `crates/arx/src/container_hand.rs:587` (native and codex paths are equivalent).
- `format_delivery` (`crates/arx/src/main.rs:1054-1070`) wraps it losslessly: `format!("[hand {hand} verdict] {text}")`. No truncation, no size check.
- Both pump sites (`main.rs:3400`, `main.rs:3823`) feed it to `after_arx_cycle!` (`main.rs:3311-3316`) → `run_turn_cycle` (`main.rs:1703`), which at `main.rs:1742` calls `log.append_turn_as(role, text)`.

`admission::admit` (`crates/arx/src/admission.rs:170`) **is** wired for tool results — call sites at `main.rs:1447, 1508, 1585, 1934` and `native_hand.rs:670` — but has **zero** call sites on the delivery path. A verdict is an injected message, not a tool result, and that is precisely the gap.

The only existing clamp is 500 bytes in `reviewer_verdict_event` (`main.rs:1092-1103`), and it applies solely to the *malformed-JSON reviewer* branch. A well-formed verdict of any size skips it.

The reserve check (`check_reserve!()` at `main.rs:3314`) runs *after* injection and reads `checkpoint.last_occupancy` from the prior turn, so it cannot stop an oversized verdict from landing.

## Deliverable

Put a hand verdict through the same admission ladder that already governs tool results: inline when under threshold, externalize with a preview plus a handle when over.

## Locked brief-time decision — the preview slice

Reuse the existing two-tier admission mechanism established by decision `e65b2a35` (raw stored whole, truncated preview plus handle). Do **not** invent a second mechanism.

One deviation from tool-result behaviour is deliberate and is settled here rather than left to judgement: **the preview must preserve the verdict's structured head — the fenced verdict block with `verdict:`, `gate:` and the start of `must_fix` — rather than an arbitrary leading byte slice.** A verdict's actionable content is its structure, and an arbitrary prefix systematically takes the wrong slice. If preserving that head is not achievable without parsing the verdict format, say so in your verdict and fall back to a leading slice, but state plainly that you did.

Reuse `admission.threshold_tokens` (default 1000, `admission.rs:15-22`). Do not add a new knob.

## Scope — owned paths

Only these files may change:

- `crates/arx/src/main.rs`
- `crates/arx/src/admission.rs` — only if the existing entry point genuinely cannot serve a delivery; justify in your verdict if you touch it
- `docs/wayfinder/maps/arx-reliability/E01-bound-hand-delivery.md` (frontmatter and Handoff only)
- `prompts/main.md` — see below
- this brief

If the fix appears to need `claude_hand.rs`, `container_hand.rs` or `native_hand.rs`, STOP and report. Bounding belongs at the delivery seam, not in each body.

## Concurrent work — do not collide

`crates/arx/src/main.rs` is contested. Regions already claimed:

- PR #30 (`r15-keep-file-listing-in-workspace`, open, unmerged): `@@ -1555` and `@@ -5091`
- `r18-record-family-after-reviewer-spawn`, **running right now** in a sibling worktree: ~`787`

Your regions are ~`1054-1070`, ~`1703-1757`, ~`3400` and ~`3823`. Stay in them. Put new tests near the delivery/admission tests, not near the file-tool tests at ~5091 and not near the dispatch tests at ~787.

## Order of work

1. Write the failing regression first: deliver an oversized verdict and show it lands in main's window whole, unbounded. Record that output.
2. Then route the delivery through admission.
3. Re-run and show the preview-plus-handle form, with the raw verdict still retrievable.

## Required cases

- An over-threshold verdict is externalized: main sees a bounded preview plus a handle, and the full text remains retrievable through the existing handle mechanism.
- The preview retains the verdict's structured head, per the locked decision above.
- A short verdict keeps its **exact** current delivery, byte for byte. This is load-bearing — see the tests below.
- The failure verdict path (a hand that failed) is bounded too, not just the success path.

## Tests that constrain you

These assert short-verdict delivery with exact or contains matching, and must keep passing untouched:

- `crates/arx/tests/reviewer.rs:226` — exact equality on `"[hand h1 verdict] stub verdict: Trace a bug (trace: threads/main.h1)"`
- `crates/arx/tests/layer_close.rs:134-152` and `crates/arx/tests/layer2_close.rs:324-335` — prefix/contains on `"[hand hN verdict]"`
- `crates/arx/tests/claude_main.rs:1669, 2123, 2625` — contains-checks on short stub verdicts

All fixtures are short, so gating on `threshold_tokens` should leave every one of them untouched. **If you find yourself editing any of these literals, STOP and report** — it means short verdicts changed, which the card forbids.

## prompts/main.md

Main's observable experience changes: it will sometimes receive a preview plus a handle where it previously received whole text. Per the repo rule, update `prompts/main.md` in this same card so what main is told matches what the binary does.

## Verify

```
cargo test -p arx --bin arx admission
cargo test -p arx --test reviewer
cargo test -p arx --bin arx
```

Do NOT use `cargo test --workspace` or the full Docker-backed integration suite as evidence — both are flaky from unrelated container/timing tests. Do NOT `docker rmi` any stub image.

## Constraints

- Do NOT commit, switch branches, or run ANY git write. No `git stash`, no `git checkout`. Two agents breached this tonight. Use `git show main:<path>` for base versions.
- Work only inside `~/Projects/arx-wt/e01-bound-hand-delivery`.
- Scratch workspaces and threads for failure tests. Never the live store.

## Non-goals

E02's trace-count clarification, E06's occupancy accounting and E07's gauge display are separate cards even though they touch nearby numbers. The occupancy-lags-a-turn defect is E06's, not yours — do not fix it here.
