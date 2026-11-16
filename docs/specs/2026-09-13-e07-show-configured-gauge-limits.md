# E07 — Show configured window and reserve in gauge

Card: `docs/wayfinder/maps/arx-reliability/E07-show-configured-gauge-limits.md`
Base: `main` @ `7805ee6` · Branch: `e07-show-configured-gauge-limits`

## Problem — already traced

`cmd_gauge` (`crates/arx/src/main.rs:3840-3867`) calls `config::load(path)`, checks it for `Err`, and **discards the `Ok(Resolved)` value**. It then prints:

```rust
println!("occupancy {occupancy} / window {COMPACT_WINDOW} (body-reported)");
```

`COMPACT_WINDOW` is hardcoded at `main.rs:53` as `1_000_000` — the body's auto-compaction constant, pushed into the body env at `claude_code.rs:102`. It is not arx's configured main window, and no reserve line is printed at all.

The configured values already exist and are already computed correctly elsewhere: `config::main_window(row)` (`config.rs:577-578`) returns `(window, reserve)` with defaults `DEFAULT_MAIN_WINDOW = 800_000` and `DEFAULT_MAIN_RESERVE = 16_000` (`config.rs:454-455`).

**Only the CLI is wrong.** The panes gauge is already correct — `build_status` (`main.rs:1187-1198`) calls `config::main_window` and renders real values at `panes/ui.rs:212-213`. Do not touch the panes path.

## Deliverable

Print arx's own configured main window and reserve in `arx gauge`, with truthful labels. If the body-reported compaction number is retained, label it distinctly so it cannot be read as arx's window.

Use the existing `config::main_window()` — same pattern `build_status` already uses. Do not duplicate the defaults.

## Scope — owned paths

- `crates/arx/src/main.rs`
- `docs/wayfinder/maps/arx-reliability/E07-show-configured-gauge-limits.md` (frontmatter and Handoff only)
- this brief

Nothing else. Not `config.rs`, not `panes/`.

## Concurrent work — `main.rs` is contested, and one neighbour is CLOSE

Regions already claimed:

- PR #30 (open, unmerged): `@@ -1555`, `@@ -5091`
- `r18-record-family-after-reviewer-spawn`, running: ~`787`
- `e01-bound-hand-delivery`, running: ~`1054-1070`, ~`1703-1757`, ~`3400`, **~`3823`**

**E01's pump-site edit at ~3823 is only ~17 lines above your `cmd_gauge` at ~3840.** Git's default 3-line context means these should not collide, but the margin is thin. Keep your edit strictly inside `cmd_gauge`'s own body, do not reflow or reformat anything above line 3838, and do not touch the pump sites. Put your tests beside the existing gauge CLI tests, not at ~5091 and not at ~787.

## Order of work

1. Failing regression first: assert `arx gauge` prints the configured window (default 800000) and show it currently prints 1000000. Record that output.
2. Then thread `resolved.main` through.
3. Re-run.

## Required cases

- `arx gauge` prints arx's configured `[main]` window and reserve, from config, not a constant.
- A config with explicit non-default `window`/`reserve` is reflected — prove it with a fixture that sets them, or the test proves nothing.
- If the body-reported compaction number is kept, its label distinguishes it unmistakably from arx's window.

## Existing tests — none pin the window line

`crates/arx/tests/claude_main.rs:385-403` (`gauge_cli_prints_occupancy_from_last_usage_row`) and `:405-416` assert only `contains("occupancy 47")` and a failure exit code. `:491-581` checks `gauge` survives concurrent appends. `:599-601` checks only that `gauge` is listed in `--help`.

None constrain the `window ...` wording, so you have a clean seam. If you find yourself editing one of these literals, STOP and report.

## Verify

```
cargo test -p arx --test claude_main gauge
cargo test -p arx --bin arx
```

Do NOT run `cargo test --workspace` or the full Docker suite as evidence — flaky from unrelated container tests, and concurrent runners are using the daemon. Do NOT `docker rmi` any stub image.

## Constraints

- Do NOT commit, switch branches, or run ANY git write. No `git stash`, no `git checkout`. Use `git show main:<path>`.
- Work only inside `~/Projects/arx-wt/e07-show-configured-gauge-limits`.
- `prompts/main.md`: `arx gauge` is an operator command, not main's surface. Say which you concluded and why.

## Non-goals

R17 (panes startup status) is a different call site and is NOT fixed by this. E06's occupancy accounting is separate — you are changing the denominator's label, not the numerator's arithmetic. Do not fix E06 here.
