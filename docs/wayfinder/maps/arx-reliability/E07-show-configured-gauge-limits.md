---
id: E07
effort: arx-reliability
labels: wayfinder:task
status: open
stage: pr-open
assignee: none
blocked-by: none
priority: P3
next: owner-review
brief: docs/specs/2026-09-13-e07-show-configured-gauge-limits.md
verification: runner PASS; independent reviewer PASS with empty must_fix; verify PASS 2026-09-13
pr: https://github.com/simkaz12/arx/pull/32
verified-revision: 58a7ee0eab2aea8e8509cccaece4c775042d1c23
---

# E07 — Show configured window and reserve in gauge

[Board](map.md) · [Original record](../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L19)

## Existing point

arx gauge was observed printing the body's compaction constant as the window.

This is carried evidence, not a fresh claim that the current implementation still fails. Revalidate only the named gap before locking a repair.

## Deliverable and next flow step

Next activity: focused-trace.

Revalidate command output and display the effective configured main window/reserve with truthful labels.

A source-based implementation choice can be settled during briefing. A changed product promise or contradiction with an existing ruling returns to grill-me.

## Done when

- [x] Gauge values match the selected effective model and main configuration.
- [x] Body compaction metadata, if retained, is clearly distinguished from arx's configured limit. *(Distinguished from arx's window, yes — but see the named limit in the Handoff: the constant is Claude-Code-specific and is printed under every body.)*
- [x] The resolution is linked with evidence. The original point was revalidated as still true before the repair.
- [x] Any implementation has a linked brief, runner PASS, independent reviewer PASS and flow verify PASS before Done.

## Scope and delegation

Scheduling group: main, terminal. Confirm exact owned paths during the next step and coordinate with overlapping cards on the board. Keep the card to its named deliverable; do not combine unrelated old findings because they share a file.

Use scratch environments for failure checks. Follow the repository's branch, Docker fixture, prompt and verification rules. Creating this board authorizes planning only.

## Handoff

The regression was reproduced before any edit: with a fixture `[main]` window of 250000 (default 800000), `arx gauge` printed `occupancy 47 / window 1000000 (body-reported)` — the body's `COMPACT_WINDOW` constant, not arx's configured window.

`cmd_gauge` (`crates/arx/src/main.rs`) now keeps the resolved config and prints `config::main_window(&resolved.main)`'s window and reserve, labelled `(arx main, configured)`. The retained body constant is relabelled `body auto-compact threshold ... (body-reported, not arx's window)`. No change to `config.rs`, `panes/`, or occupancy's numerator.

Two tests added beside the existing gauge CLI tests in `crates/arx/tests/claude_main.rs`: one pins the default configured window (800000, not 1000000), one sets a non-default `window`/`reserve` fixture and asserts both are reflected. `cargo test -p arx --test claude_main gauge` (5 passed) and `cargo test -p arx --bin arx` (440 passed) are clean. The non-default fixture is synthesised at test time from a copied workspace, so the shared `tests/fixtures/run/` directory other lanes read concurrently was never modified on disk.

Runner PASS, independent reviewer PASS with an empty `must_fix`, and a separate verify gate PASS. The reviewer confirmed the diff is two hunks strictly inside `cmd_gauge` with nothing at or above line 3838 touched, keeping clear of the sibling card editing `~3823`. The verify gate re-ran the suite itself and additionally swept for other stale window/reserve displays, finding none: the reset-armed notice (`main.rs:3298-3301`) and the nudge notice (`main.rs:2176`) are already config-sourced.

**Named limit, pre-existing and deliberately not fixed here.** The retained third line prints `COMPACT_WINDOW` under every body, but that constant is Claude-Code-specific — `CLAUDE_CODE_AUTO_COMPACT_WINDOW`, set only at `claude_code.rs:102`. Running main under Codex or Native prints a number unrelated to that session, under a `(body-reported)` label implying otherwise. The new wording is honest about what the number is *not* (arx's window) and silent about what it *is*.

The diagnostic case for keeping the line is also weaker than it looks: `config::load` (`config.rs:744-757`) already refuses a Claude Code `[main]` window at or above 900000 with `ConfigError::MainWindowTooHigh`, re-enforced on model switch at `main.rs:3520`, so the divergence an operator might hope to catch is structurally unreachable for the one body the constant describes.

This behaviour predates the card — the old code printed the same constant unconditionally — so it is recorded rather than repaired. The honest fix is to print the line only under a Claude Code body, or to name the body it belongs to. Worth its own card; not folded in here, since the brief scoped this to truthful labelling of arx's own configured limits.

