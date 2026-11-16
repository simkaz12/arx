# L1-11 — the judge as a dispatch under `judge`

Card: `docs/wayfinder/maps/arx-v0/L1-11-judge-as-dispatch.md`. Ruled by D16 (`1e6ad121`, `da3f5418`, `0bd4df2f`), with `e652ce3a` (one adapter per CLI kind, role-agnostic) as the in-force companion — if a step below lands against one of those, stop and report; do not bend the step or the ruling.

## Problem / Solution

Every body is behind the dispatch seam and the judge is still a dead name: `resolve_hand_spec` returns `'judge' is the reserved judge definition; judge dispatch lands at L1-11` (`main.rs:215-217`) before any ledger row is written. Config already demands `agents/judge.md` at load (`config.rs:374-378`, `ConfigError::NoJudgeDefinition`) and parses it through the same `RoleConfig` path as a hand, holding it in `Resolved.judge`; nothing reads that row at dispatch. The dispatch call carries `agent` and `brief` only (`sidecar/claude-main.mjs:110-115`), so today there is no way to name a body per dispatch, and no way to prove a smuggled one is refused.

This brief lets the reserved name resolve: a dispatch of `judge` reads `Resolved.judge`, takes the body that definition declares, and goes through L1-09's codex adapter like any codex hand. The dispatch call gains an optional `body` that exists only to be checked against the definition and refused at the ledger when it differs, which is the card's second done-when as something that runs. The spawn row records the role. A seeded diff in the working directory plus a read-only brief is what the judge is dispatched over, hermetically with the stub codex and once live on the owner's login. The verdict is the hand's text, landing at a step boundary exactly as a hand's does. The verdict schema, `--output-schema`, the read-only clamp, the no-save rule and the same-family warning are L2-12.

## Decisions locked

- The judge is a role in config and a hand at runtime (`1e6ad121`). Dispatch names the reserved definition `judge`; `resolve_hand_spec` resolves it from `Resolved.judge` and builds a `HandSpec` the same way it does for a row in `Resolved.hands`. No judge-specific spawn path, no judge-specific adapter: `spawn_codex_hand` is untouched and never learns which role called it (`e652ce3a`).
- Body, model and provider come from the definition, never from the call (`0bd4df2f`). The judge's block is `agents/judge.md`; under `body: native` it would resolve a provider row through the same code as `hand-ds`. Nothing in this card special-cases which body the judge has.
- A dispatch call may carry `body`; when it does and the value is not the definition's body, the ledger writes a refused `dispatch` row and the tool returns the refusal, with no spawn row and no child. When it matches, the dispatch proceeds unchanged. This is role-agnostic: `hand-cc@codex` is refused the same way `judge@claude-code` is.
- Refusal text is `dispatch of '<agent>' names body '<requested>' but its definition says '<body>'; the body is resolved from config, never per dispatch`. A `body` value that is not a known body name is a mismatch, with the raw string echoed.
- The spawn ledger row gains `role`, `judge` for the reserved definition and `hand` for everything else. Main's window line stays `[hand h<N> verdict] …`: the judge is a hand at runtime and the hand id is the join key to the ledger row that says it was the judge. The L1-05 gate row for the `dispatch` tool call already carries `agent: "judge"` in the transcript, so the dispatch itself is a transcript line without a new format.
- Verdict is the hand's text, whatever the model wrote; the arx-defined schema and the parse-with-explicit-failure wrapper R02 §3 calls for are L2-12 (`da3f5418`). Codex's `review/start` is not used at any layer.
- Read-only means the brief says so; no clamp is added here. If the live judge writes a file anyway, that is reported as a finding for L2-12, not fixed in this card.
- The seeded diff is a unified-diff file in the working directory that the brief names by path; the judge reads it with its own tools. The stub codex cannot read files, so the hermetic test proves the dispatch, the role row and the verdict path; the live scope proves the read by requiring the verdict to quote a canary line from the diff.
- `config_dir()` learns that `Path::new("arx.toml").parent()` is `Some("")`, not `None`, so a bare filename resolves to `.`. This is a ride-along fix for a pre-existing bug L1-10's runner hit live (`read_file error: No such file or directory` on the first attempt, worked around with `./arx.toml`). One function, one unit test; not a card.
- Six `judge.md` fixtures and `main-override/agents/main.md` move from `gpt-5.4-codex` to `gpt-5.6-sol`, and a test scans every codex-bodied fixture definition against an allowlist held in the test, so an unsupported id fails at `cargo test` rather than at a live 400. The product code names no model id; the allowlist is test data.

## Chosen without a grill

The owner asked for this brief overnight; these three calls are mine and are the first things to revisit if a scope reads wrong.

- `body` on the dispatch call is optional and tolerated when it matches. The done-when says a body *other than* the definition's is refused; a redundant restatement is not an override, and refusing it would make the tool's description lie about what the field is for.
- `role` is a field on `HandSpec` set by `resolve_hand_spec`, not a string comparison inside the ledger. The ledger stays ignorant of the reserved name.
- The `config_dir` fix rides along because the live scope of this card runs from the repo root the same way L1-10's did, and the bug is a one-liner with a test.

## Ground truth and tripwires

- Baseline: L1-10 left `cargo build --workspace` with 6 dead-code warnings and `cargo test --workspace` at 187 green (100 unit in `arx`, 7 `check_config`, 43 `claude_main`, 26 `transcript_log`, 11 `arx-engine`). The L1-10 spec-reviewer may have forced fixes since; record the actual numbers before touching anything, and anything red at the start is pre-existing: report it, do not fix it.
- The reservation is the only judge-specific code outside config: `main.rs:215-217`. `ledger.rs` contains no `judge` string (grep confirmed 2026-09-03). `Resolved.judge` is a `RoleRow` with `role == "judge"`, parsed by the same `parse_frontmatter` and `check_role` as a hand (`config.rs:390-399`).
- Dispatch wire shape: real sidecar `tool('dispatch', …, { agent: z.string(), brief: z.string() }, …)` at `sidecar/claude-main.mjs:110-115`; stub sidecar parses `hand:dispatch:<agent>:<brief>` at `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs:85-87`. `on_hand_call`'s dispatch arm (`main.rs:281-291`) reads `agent` and `brief` only.
- Spawn row today: `{"kind":"spawn","hand":…,"agent":…,"brief":…}` (`ledger.rs:350`). Steer refusal row: `{"kind":"steer","hand":…,"message":…,"refused":true}` (`ledger.rs:413`) with body labels built ad hoc at `ledger.rs:414-419`. `BodyKind` has no `Display` and no label method.
- `HandSpec` (`ledger.rs:43-52`) is constructed in `main.rs::resolve_hand_spec` and in unit tests inside `ledger.rs`, `claude_hand.rs`, `codex_hand.rs` and `native_hand.rs`. Adding a field touches every one of them; `cargo build` will list them.
- The stub codex (`crates/arx/tests/fixtures/bin/codex`) echoes `stub verdict: <first line of the brief>` and dumps its argv to `$CODEX_HOME/codex-stub-argv`. Keep the assertion key on the brief's first line. The `run` fixture's `agents/judge.md` is already `body: codex`, `model: gpt-5.6-sol`.
- `gpt-5.4-codex` hits in fixtures, counted 2026-09-03: `valid`, `drive-surface`, `main-override`, `provider-on-cli`, `native-no-provider`, `native-unknown-provider` (each `agents/judge.md`) plus `main-override/agents/main.md`, seven files. The card says five: it predates `native-unknown-provider` (added by L1-10) and did not count the `main.md`. Run `grep -rl 'gpt-5.4-codex' crates/arx/tests/fixtures | wc -l` first; seven is the expected answer, five is explained above, anything else means STOP and report rather than adjusting the sweep to fit.
- Model id for codex-bodied definitions is `gpt-5.6-sol` (owner ruling 2026-09-02, L1-09 brief). Never substitute another id; a live 400 on it is a STOP.
- Codex quota on this host is subscription-bound and was exhausted at least once on 2026-09-02. A usage-limit error on any live step is a STOP for that step, reported as blocked, never faked.
- `config_dir` bug: `main.rs:80-82`. Until scope 1 lands, any live `cargo run -p arx -- run` must be given `./arx.toml`, not `arx.toml`.
- The live drive shape from L1-09 scope 3 and L1-10 scope 5: pipe an operator turn into `cargo run -q -p arx -- run ./arx.toml` from the repo root; main is the real Claude Code sidecar. Main sometimes answers in prose instead of calling the tool; re-prompt more explicitly rather than faking the artifact.

## Execution plan

### Scope 1 — the judge resolves; the body is never per call

- **Files:**
  - Modify `crates/arx/src/ledger.rs` (`HandSpec`, `BodyKind`, spawn row ~350, steer refusal ~401-420, new `refuse_dispatch`)
  - Modify `crates/arx/src/main.rs` (`config_dir` 80-82, `resolve_hand_spec` 198-266, `on_hand_call` dispatch arm 281-291)
  - Modify `crates/arx/src/claude_hand.rs`, `crates/arx/src/codex_hand.rs`, `crates/arx/src/native_hand.rs` (test-only `HandSpec` constructions)
  - Modify `sidecar/claude-main.mjs` (dispatch tool schema and description)
  - Modify `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs` (dispatch payload parsing)
- **Build:** `judge` dispatches through its definition's body; a dispatch that names a different body is refused at the ledger with a row; the spawn row carries the role.
- **Steps:**
  1. `ledger.rs`: `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum HandRole { Hand, Judge }` with `pub fn label(&self) -> &'static str` returning `hand` / `judge`. `HandSpec.role: HandRole`. Every existing construction sets `role: HandRole::Hand`; `cargo build` names them.
  2. `ledger.rs`: `impl BodyKind { pub fn label(&self) -> &'static str }` returning `fake` / `claude-code` / `codex` / `native`. Replace the ad hoc `body_label` match in `steer` with `entry_kind.map(BodyKind::label).unwrap_or("claude-code")`; the refusal strings the L1-08/09/10 tests match are unchanged.
  3. `ledger.rs` spawn row: `json!({"kind": "spawn", "hand": hand, "agent": spec.agent, "role": spec.role.label(), "brief": brief})`. Existing tests that match `"kind":"spawn"` by substring keep passing; any that compare the row whole gain the field.
  4. `ledger.rs`: `pub fn refuse_dispatch(&mut self, agent: &str, requested: &str, definition: BodyKind) -> Result<String, LedgerError>` — `self.drain_available()?` first like `steer`, then `write_row(&json!({"kind": "dispatch", "agent": agent, "requested_body": requested, "body": definition.label(), "refused": true}))`, then return the refusal text from Decisions locked. No hand id is minted; the counter is untouched.
  5. `main.rs::config_dir`: `match path.parent() { Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(), _ => PathBuf::from(".") }`.
  6. `main.rs::resolve_hand_spec`: delete the reservation branch. Replace the row lookup with `let (row, role) = if agent == resolved.judge.role { (&resolved.judge, HandRole::Judge) } else { (hands.find(..)?, HandRole::Hand) }`, keeping the unknown-agent error and its known-definitions list exactly as they are. The `fake` branch sets `role: HandRole::Hand`. Body, model and provider resolution below the lookup is unchanged and now serves the judge row too.
  7. `main.rs::on_hand_call`, dispatch arm: read `let requested = call.args.get("body").and_then(|v| v.as_str())`. After `resolve_hand_spec` returns `Ok(spec)` and before `ledger.dispatch`, when `requested` is `Some(body)`: parse it as `config::Body` via `serde_json::from_value::<Body>(Value::String(body.into()))`, map through the same `Body → BodyKind` match `resolve_hand_spec` uses (extract that match into `fn body_kind(body: Body) -> BodyKind` so it exists once), and compare with `spec.kind`. Unparseable or unequal → `match ledger.refuse_dispatch(agent, body, spec.kind) { Ok(text) => json!({"error": text}), Err(err) => json!({"error": err.to_string()}) }`. Equal → fall through to `ledger.dispatch`.
  8. `sidecar/claude-main.mjs`: dispatch schema becomes `{ agent: z.string(), body: z.string().optional(), brief: z.string() }`; description becomes `Spawn a hand from a brief; returns its id at once, without blocking. The body is the agent definition's; body, if given, must match it.` The handler is unchanged, `args` already flows through whole.
  9. Stub sidecar: after splitting `agent` from the payload, `const at = agent.indexOf('@'); if (at >= 0) { args.body = agent.slice(at + 1); args.agent = agent.slice(0, at); }`. `hand:dispatch:judge@claude-code:brief` yields `{agent: "judge", body: "claude-code", brief: "brief"}`; a payload without `@` is unchanged.
- **Verify:** `cargo build -p arx` clean with the warning count no higher than baseline. Unit tests in `ledger.rs`: `refuse_dispatch_writes_a_refused_row_and_mints_no_hand` (row has `kind == "dispatch"`, `refused == true`, `requested_body == "claude-code"`, `body == "codex"`; a following `dispatch` still mints `h1`); `spawn_row_carries_the_role` (a `HandRole::Judge` spec's spawn row has `role == "judge"`, a `Hand` spec's has `role == "hand"`); every existing ledger test green. Unit tests in `main.rs`: `config_dir_of_a_bare_filename_is_dot` (`config_dir(Path::new("arx.toml")) == Path::new(".")`), `config_dir_of_a_nested_path_is_its_parent`. `grep -c judge crates/arx/src/ledger.rs` → `0` beyond the `HandRole::Judge` variant and its label; `grep -n 'L1-11' crates/arx/src` → nothing.

### Scope 2 — hermetic e2e

- **Files:**
  - Create `crates/arx/tests/fixtures/run/seeded.diff`
  - Modify `crates/arx/tests/claude_main.rs`
- **Build:** the card's done-when as tests with the stub codex on `PATH` and `CODEX_HOME` per test, the pattern of `codex_hand_dispatch_delivers_stub_verdict_with_trace_and_usage` (`claude_main.rs:1422`).
- **Steps:**
  1. `seeded.diff`, a unified diff against a file that does not exist in the fixture:
     ```diff
     --- a/src/count.rs
     +++ b/src/count.rs
     @@ -1,4 +1,5 @@
      // CANARY: ARX-L1-11-JUDGE-CANARY
      pub fn count_lines(text: &str) -> usize {
     -    text.lines().count()
     +    // off by one on purpose
     +    text.lines().count() + 1
      }
     ```
  2. Rename `dispatch_on_unknown_agent_and_on_the_reserved_judge_definition_are_refused_before_spawn` (`claude_main.rs:1385`) to `dispatch_on_unknown_agent_is_refused_before_spawn`: drop the `judge` stdin line and its assertions, keep the unknown-agent half, and add `assert!(unknown_reply.contains("judge"))` so the known-definitions list is proven to name the judge.
  3. Test `judge_dispatch_over_a_seeded_diff_delivers_a_verdict_through_the_codex_body`: stdin `hand:dispatch:judge:Review seeded.diff and return a verdict\n`. Assert: `run/threads/main.jsonl` has an `arx` turn whose text contains `[hand h1 verdict] stub verdict: Review seeded.diff and return a verdict` and `(trace: threads/main.h1.jsonl)`; that trace file exists and byte-matches the stub's rollout under `CODEX_HOME`; the ledger `spawn` row has `agent == "judge"` and `role == "judge"`; the ledger `verdict` row for `h1` carries the stub's usage numbers; `$CODEX_HOME/codex-stub-argv` contains `--model gpt-5.6-sol` and `project_doc_max_bytes=0`; `run/seeded.diff` exists in the workspace the child ran in.
  4. Test `dispatch_naming_a_body_other_than_the_definition_is_refused_at_the_ledger`: stdin three lines, `hand:dispatch:judge@claude-code:first\n`, `hand:dispatch:hand-cc@codex:second\n`, `hand:dispatch:judge@codex:third\n`. Assert: the first two `main` replies contain `names body 'claude-code'` / `names body 'codex'`, `definition says 'codex'` / `definition says 'claude-code'`, and `never per dispatch`; the ledger has exactly two rows with `kind == "dispatch"` and `refused == true`, with `agent` `judge` then `hand-cc`, before any `spawn` row; exactly one `spawn` row, `agent == "judge"`, `hand == "h1"`, `role == "judge"`; a `verdict` row for `h1` whose text starts `stub verdict: third`.
  5. Test `dispatch_naming_an_unknown_body_is_refused`: stdin `hand:dispatch:judge@gpt:x\n`. Assert the reply contains `names body 'gpt'` and the ledger has a refused `dispatch` row with `requested_body == "gpt"` and no `spawn` row.
  6. Every pre-existing test green, including the three codex-hand tests and `three_dispatches_keep_main_turning_and_verdicts_land_at_boundaries`.
- **Verify:** `cargo test -p arx --test claude_main` green, count baseline plus three; `cargo test --workspace` green.

### Scope 3 — fixture model ids and the scan test

- **Files:**
  - Modify `crates/arx/tests/fixtures/{valid,drive-surface,main-override,provider-on-cli,native-no-provider,native-unknown-provider}/agents/judge.md`
  - Modify `crates/arx/tests/fixtures/main-override/agents/main.md`
  - Modify `crates/arx/tests/check_config.rs`
- **Build:** the L1-09 debt cleared, and a test that keeps it cleared.
- **Steps:**
  1. Tripwire count from Ground truth, then `sed -i 's/gpt-5.4-codex/gpt-5.6-sol/' <the seven files>`. Re-run the grep: zero hits under `crates/arx/tests/fixtures`.
  2. `check_config.rs`: `const SUPPORTED_CODEX_MODELS: &[&str] = &["gpt-5.6-sol"];` and test `codex_bodied_fixture_definitions_use_a_supported_model_id`: glob `tests/fixtures/*/agents/*.md` from `CARGO_MANIFEST_DIR`, read each, take the text between the first two `---` lines, find `body:` and `model:` values by line prefix (no YAML crate; `RoleConfig` is private to the binary), collect every file whose body is `codex` and whose model is not in the allowlist, and `assert!(offenders.is_empty(), "codex-bodied fixtures on unsupported model ids: {offenders:?}")`. It must visit at least seven codex-bodied files (`assert!(visited >= 7)`), so an empty glob cannot pass vacuously.
  3. Confirm the seven `check_config` tests still assert what they did: none of them matches a model id.
- **Verify:** `cargo test -p arx --test check_config` green, eight tests.

### Scope 4 — one live judge turn

- **Files:** none kept. A temporary `l1-11-seeded.diff` at the repo root, removed in step 5.
- **Build:** the done-when against the real `codex` on the owner's login (subscription-billed, cents).
- **Steps:**
  1. Quota tripwire: in a fresh temp dir, `codex exec --json --model gpt-5.6-sol --skip-git-repo-check "Reply with exactly: OK"`; a usage-limit error or a 400 on the model id → STOP this scope, report it blocked with the output, leave the card open.
  2. Write `l1-11-seeded.diff` at the repo root with the scope-2 content, canary line included.
  3. From the repo root: `printf 'Use the dispatch tool with agent "judge" and this brief: Read l1-11-seeded.diff in the working directory. Do not modify or create any file. In one paragraph say whether the change is correct and quote the token that follows CANARY: in the diff. Then reply "dispatched".\n' | cargo run -q -p arx -- run ./arx.toml`. If main answers without calling the tool, re-prompt more explicitly.
  4. Inspect and record: `threads/main.jsonl` has the `[hand h<N> verdict]` row with `(trace: threads/main.h<N>.jsonl)`; `grep -c ARX-L1-11-JUDGE-CANARY` on the verdict text → 1 or more; the verdict mentions the off-by-one; `threads/main.ledger.jsonl` has the spawn row with `agent == "judge"` and `role == "judge"` and a verdict row with nonzero `input_tokens` and `output_tokens`; the trace file is a codex rollout; `git status --short` after the run shows no tracked file changed and no new file beyond `l1-11-seeded.diff` and `threads/` (a write by the judge is a finding for L2-12, reported in the verdict, not reverted by the runner beyond deleting what it created itself).
  5. Delete `l1-11-seeded.diff` and the step-1 temp dir.
- **Verify:** the paths, the canary count, the usage numbers and the `git status` line reported in the verdict.

### Scope 5 — card

- **Files:** Modify `docs/wayfinder/maps/arx-v0/L1-11-judge-as-dispatch.md`.
- **Build:** the card reflects reality once every Verify above passes; if scope 4 was quota-blocked the card stays `open` and the verdict says why.
- **Steps:** flip `status: open` → `status: closed` in the frontmatter. Amend the card's "Debt inherited from L1-09" paragraph in place: seven files, not five, naming `native-unknown-provider` and `main-override/agents/main.md`, and note the scan test's name. `map.md` stays untouched. Do not commit anything.
- **Verify:** `git diff --stat` shows no tracked file beyond the pre-existing dirty baseline; every path this brief touches is untracked or named above.

## Out of scope

- The verdict schema and `codex exec --output-schema`, the driver's own parse failure path (R02 §3), the read-only clamp (`sandbox_mode = read-only`, approval policy), the no-save rule, the same-family-as-main warning — L2-12.
- Container, credential isolation for the judge — L2-09.
- Codex `review/start` — never (`da3f5418`). The judge as a third participant — the named reopen on `1e6ad121`, owner's call, not a card.
- Any change to `spawn_codex_hand`, `ARX_HAND_INSTRUCTIONS`, the trace handle, `format_delivery` or main's window line.
- Wiring `RoleConfig.instructions` / `repo_instructions`; grounding pushed into the judge's brief (L2-05).
- A body override that is honoured. `body` on the call is a check, never a choice.

Done means: main dispatches `judge` over a seeded diff and the verdict text lands in main's window at a step boundary with a trace handle; a dispatch naming any body other than the definition's is refused at the ledger with a `refused: true` row and no spawn; the spawn row says `role: judge`; every codex-bodied fixture definition is on a supported model id and a test keeps it so; a bare `arx.toml` resolves its config dir to `.`.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-03-l1-11-judge-as-dispatch.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
