# E12 — Persist coverage for invalid drain definitions

Card: `docs/wayfinder/maps/arx-reliability/E12-cover-invalid-drain-body.md`
Base: `main` @ `7805ee6` · Branch: `e12-cover-invalid-drain-body`

## Problem — already traced

The validation is correct. The **regression coverage for it is missing**, so nothing would catch a future change that silently stopped rejecting a malformed `agents/drain.md`.

`agents/drain.md` is parsed like every other role file, through the generic path called for drain at `crates/arx/src/config.rs:937-938`:

```rust
let drain_config = parse_frontmatter(drain_path)?;
check_role("drain", &drain_config)?;
```

The rejection surfaces:

- no `---` block → `ConfigError::FrontmatterMissing` (`config.rs:670-672`)
- YAML carrying a retired key — `body`, `model`, `provider`, `image` (`RETIRED_FRONTMATTER_KEYS`, `config.rs:663`) → `FrontmatterFieldMoved` (`config.rs:676-682`), message at `config.rs:292-296`
- any other deserialize failure → generic `FrontmatterParse` (`config.rs:684-687`)
- `check_role` → `check_numeric_fields` (`config.rs:458-478`) → `NonPositiveSteps{role}` / `NonPositiveWindow{role}` / `NonPositiveNudge{role}` / `NonPositiveCheckpoint{role}`
- separately, dispatching to drain through the generic tool is refused at `crates/arx/src/main.rs:862-864` with `"dispatch: drain runs through the drain tool"`

## What exists and what does not

**Covered today**, but every one targets `agents/hand-ex.md` or `[main]` in `arx.toml`, never `drain.md`:

- the four moved-key tests, `config.rs:1160-1222`
- `main_nudge_of_zero_is_rejected` (`:1460`), `main_checkpoint_zero_is_refused` (`:1485`), `steps_zero_is_refused` (`:1508`), `window_zero_is_refused` (`:1531`) — all exercise `[main]`, not a role frontmatter file
- `frontmatter_split_valid` / `_missing` / `_unterminated` (`:1071-1088`) — unit tests of the raw `split_frontmatter` string helper only, never the `parse_frontmatter`/`ConfigError` pipeline

**Not covered at all** — this is the deliverable:

1. `agents/drain.md` with no frontmatter block → `FrontmatterMissing`
2. `agents/drain.md` carrying each retired key → `FrontmatterFieldMoved` with the moved message
3. `agents/drain.md` with a malformed or unknown field outside the retired list → `FrontmatterParse`
4. a **non-main role** file with `steps: 0` / `window: 0` → `NonPositiveSteps{role: "drain"}` / `NonPositiveWindow{role: "drain"}`, proving the role tag is carried

   **Corrected on amendment, 2026-09-13.** This item originally claimed all four numeric fields carry `{role}`. They do not, and the runner was right to stop rather than force it. In `config.rs:209-217` only `NonPositiveSteps` and `NonPositiveWindow` carry a `role: String`; `NonPositiveNudge` and `NonPositiveCheckpoint` are bare unit variants whose `Display` (`config.rs:297-298`) hardcodes `"main.nudge must be positive"` and `"main.checkpoint must be positive"`.

   The consequence is a real defect, not a documentation nit: `agents/drain.md` with `nudge: 0` is correctly rejected, but the message names **main** as the offending file. The existing `main_nudge_of_zero_is_rejected` and `main_checkpoint_zero_is_refused` tests already pin that literal text, so nothing catches the misattribution.

   Scope ruling: **do not fix it here.** Giving those two variants a `role` field is a production change in `config.rs`, which would void this card's test-only gate waiver. It is recorded as a separate defect for its own card. Writing a test that asserts the misattributed `"main.___"` text against a `drain.md` fixture would pin the bug as correct behaviour — do not do that either.
5. `dispatch` called with `agent: "drain"` → the refusal string at `main.rs:862-864`, which has **zero** test hits anywhere

## Deliverable

Add those regressions. **No production code changes.** The validation already behaves correctly; you are pinning it.

If you find a case where the validation does *not* behave as described above, STOP and report it — that is a defect discovery, not something to fix inside a coverage card.

## Scope — owned paths

- `crates/arx/tests/check_config.rs` — where nearly all of this belongs
- `docs/wayfinder/maps/arx-reliability/E12-cover-invalid-drain-body.md` (frontmatter and Handoff only)
- this brief

Case 5 (the dispatch refusal) may need one test elsewhere. Prefer `check_config.rs` if it can live there. If it genuinely needs a session-style test in `crates/arx/tests/claude_main.rs`, you may add **one**, and you must name it in your verdict. Do not touch any `src/` file.

## Concurrent work

`crates/arx/tests/check_config.rs` is uncontested — verified across all live worktrees. Keep it that way.

A sibling runner is using `crates/arx/tests/claude_main.rs` for gauge tests but is **not editing it**. If you add the one permitted test there, append it near the existing dispatch tests and do not reflow the file.

Note for the record: E12's subject is `ConfigError` in `crates/arx`. R03, R13 and PR #28 all work on `StoreError` and `CandidatePlan` in `crates/arx-engine`. Despite the shared word "drain" these are different crates, different error types, different test files. There is no overlap — do not go looking for one in `arx-engine`.

## Codex gate — waived, and the waiver lives here in the brief

This card adds tests and changes no production code, so the behaviour-change axis is not engaged and the gate does not apply. Record `gate_run: no` with that reason.

This waiver is written into the brief deliberately. Earlier tonight a runner reported `gate_reason: "per brief instruction"` when the waiver had only been given in its dispatch prompt, and the reviewer correctly flagged that the grounding was overstated. A reviewer must be able to verify the waiver from the artifact alone.

**The waiver holds only while the diff is test-only.** If you end up changing anything under `crates/arx/src/`, the waiver is void — run the gate, and say why the diff grew.

## Order of work

Regression-first does not apply in the usual way here: these tests should pass immediately, because the behaviour is already correct. That makes them weak by default, so prove each one is real:

For each new test, temporarily confirm it *can* fail — for example by asserting against the wrong error variant — and say in your verdict that you checked. A coverage test that would pass even if the validation were deleted is worthless, and that is the specific failure mode this card exists to avoid.

## Verify

```
cargo test -p arx --test check_config
cargo test -p arx --bin arx
```

Report before/after test totals; the delta must be exactly the number you added. Do NOT run `cargo test --workspace` or the full Docker suite — flaky from unrelated container tests, and several runners are using the daemon. Do NOT `docker rmi` any stub image.

## Constraints

- Do NOT commit, switch branches, or run ANY git write. No `git stash`, no `git checkout`. Two agents breached this tonight. Use `git show main:<path>`.
- Work only inside `~/Projects/arx-wt/e12-cover-invalid-drain-body`.
- `prompts/main.md` is untouched — this card adds no behaviour for main to observe.

## Non-goals

E14's stub-image freshness and E16's engine temp-root leak are separate test-infrastructure cards, deliberately held back tonight. Do not fold them in.
