# L2-12 — the judge: schema, clamp, family warning

Card: `docs/wayfinder/maps/arx-v0/L2-12-judge-schema-and-clamp.md`. Ruled by D16 (`84c34f6a`, superseding `1e6ad121` — the judge is a role in config and a hand at runtime, a dispatch under the reserved definition `judge`: read-only clamp, no save verb, no third lifetime class; under a Codex body `codex exec --json --output-schema`, one turn, approvals never asked; the clamp is the container mount, not Codex's sandbox; the different-family-from-main rule is checked at dispatch against main's configured body and logged as a warning on the operator surface when they match, never refused; `da3f5418` — the judge returns an arx-defined verdict schema, body-independent; a Codex judge fills it through `--output-schema`; `review/start` is never used; `0bd4df2f` — body, model and provider are the only per-role fields, the drive surface follows from the role; `e652ce3a` — one adapter per CLI kind, role-agnostic, the role decides what arx mounts into the child), R02 (`--output-schema` is OpenAI's strict structured output and Codex checks nothing afterwards; the driver needs its own parse-with-explicit-failure wrapper). Branch `l2-12-judge-schema-and-clamp`, cut from `main` after L2-09 merged.

Written 2026-09-07 without a grill, on the owner's clearance to finish layer 2 today. Every call main made alone is listed under "Chosen without a grill".

## Problem / Solution

After L2-09 the judge runs in the container with the repository mounted read-only, so the clamp is done. What is missing is the return: the judge's verdict is whatever prose the model wrote, main has no schema to hold it to, and a bad verdict is indistinguishable from a good one until main reads it. Nothing checks at dispatch whether the judge shares main's model family, so a Claude main can be judged by a Claude judge without anyone noticing. And the L2-09 live check found that a real Claude Code main never dispatches the judge at all: `render_hand_roster` lists `resolved.hands` only, so the name `judge` is absent from the one prompt section that tells main what it can dispatch, and main correctly refused to guess.

This card completes the dispatch. arx ships one verdict schema, writes it into the judge's `/hand/out` before the container starts, and passes `--output-schema /hand/out/verdict-schema.json` on a codex-bodied judge; every judge, whatever its body, receives the schema at the top of its brief. At delivery, a judge's verdict is validated against the schema by arx's own validator: a conformant verdict reaches main verbatim and a `schema` event records it; a malformed one reaches main marked as malformed with the reason first, the raw text kept in the ledger row, and the `schema` event records the reason. At dispatch, the judge's family is compared with main's; a match writes a `family` event, says so on the terminal, and lets the dispatch proceed. The roster lists the judge. A judge's write attempt is refused by the read-only mount and, because the schema forces it to report, arrives in the verdict's summary and therefore in the transcript log.

## Decisions locked

- **The schema.** One arx-defined verdict schema, a `pub(crate) const JUDGE_SCHEMA: &str` in a new `crates/arx/src/judge.rs`, body-independent:
  ```json
  {"type":"object","additionalProperties":false,"required":["verdict","summary","findings"],
   "properties":{
     "verdict":{"type":"string","enum":["pass","fail"]},
     "summary":{"type":"string"},
     "findings":{"type":"array","items":{"type":"object","additionalProperties":false,
       "required":["severity","claim","evidence"],
       "properties":{"severity":{"type":"string","enum":["high","medium","low"]},
                     "claim":{"type":"string"},"evidence":{"type":"string"}}}}}}
  ```
  Strict-mode shape throughout: every property required, no additional properties.
- **Where the schema file lands.** For `spec.role == HandRole::Judge && spec.kind == BodyKind::Codex`, `container_hand::prepare` writes `JUDGE_SCHEMA` to `<root>/out/verdict-schema.json` right after the credential step. It is swept into the trace as `out/verdict-schema.json` like everything under `/hand/out`; that is wanted, it is the evidence of what the judge was held to.
- **The flag.** `codex_hand::build_argv` stays role-blind. `container_hand::build_argv`'s codex arm appends `--output-schema /hand/out/verdict-schema.json` when `spec.role == HandRole::Judge`; the role-conditional part lives where the mounts are decided (`e652ce3a`). No other argv change.
- **The brief every judge receives.** At the point the wire writes the brief to the child, for `spec.role == HandRole::Judge` of any body, the text is `Return your verdict as a single JSON object matching this schema, and nothing else:\n<JUDGE_SCHEMA>\n\n<the brief main wrote>`. The ledger's `spawn` row keeps the brief main wrote, unprefixed.
- **Validation at delivery, in main.** `judge::validate_verdict(text: &str) -> Result<(), String>` checks, in this order, stopping at the first failure: parses as JSON (`not JSON: <serde error>`); is an object (`not an object`); has no key outside `verdict`, `summary`, `findings` (`unexpected field <name>`); has every one of those keys (`missing field <name>`); `verdict` is `"pass"` or `"fail"` (`verdict not one of pass|fail`); `summary` is a string (`summary not a string`); `findings` is an array (`findings not an array`) of objects each with exactly `severity`, `claim`, `evidence` (`findings[<i>] unexpected field <name>` / `findings[<i>] missing field <name>`), `severity` in `high|medium|low` (`findings[<i>].severity not one of high|medium|low`), `claim` and `evidence` strings (`findings[<i>].<name> not a string`). When a `Delivery::Verdict` arrives for a hand whose ledger entry has `role == Judge`, main validates before formatting the window line. `Delivery` structs are unchanged; the role is read from the ledger entry before it is retired.
- **What main sees.** Conformant: the window line is `[hand hN verdict] <text verbatim> (trace: …)` as today. Malformed: `[hand hN verdict] judge verdict malformed: <reason>. Raw: <text, first 500 bytes> (trace: …)`. The ledger `verdict` row keeps the raw text in `text` in both cases and gains no field.
- **Two event kinds.** `Event::Schema { hand, outcome, reason }` serialising as `{"kind":"schema","hand":"h1","outcome":"ok"}` or `{"kind":"schema","hand":"h1","outcome":"malformed","reason":"missing field summary"}` (`reason` omitted when `None`), appended right before the verdict's window turn, judge deliveries only. `Event::Family { agent, family }` serialising as `{"kind":"family","agent":"judge","family":"claude"}`, appended at dispatch when the judge's family equals main's, before the `spawn` row. Both added to `KINDS`, `Event::kind()`, `every_kind()` and `event_fields` so `arx events` prints them (`schema`: hand, outcome, reason; `family`: agent, family).
- **Family.** `judge::family(row: &RoleRow, providers: &BTreeMap<String, ProviderConfig>) -> String`: `Body::ClaudeCode` → `claude`; `Body::Codex` → `gpt`; `Body::Native` → `claude` when the named provider's `kind` is `Anthropic`, else the provider's config key verbatim (`deepseek`, `openai`, …). The check runs in `on_hand_call`'s dispatch arm after `resolve_hand_spec` succeeds and only when the role is `Judge`: equal strings → `TerminalMsg::Notice` with text `[arx] judge 'judge' is <family>, the same family as main; its verdict is not independent`, then the `family` event, then the dispatch proceeds unchanged. Never refused.
- **The roster.** `render_hand_roster` takes the judge row too and renders it last as `- judge: <model>, <body>, read-only; returns a JSON verdict under arx's schema (verdict pass|fail, summary, findings)`. Hands render as today. `prompts/main.md` gains one sentence after the dispatch bullet: `The judge is a hand named judge: dispatch it over a diff or a trace when you want a verdict you did not write; its reply is JSON under arx's schema, and a malformed one arrives marked as such.` Nothing else in the standing prompt changes.
- **The write refusal in the log.** The mount refuses; the schema makes the judge report it; the `summary` carries the refusal; the ledger `verdict` row and the window turn carry the summary. No new event for it, and `walls` stays empty for codex (the codex wire has no `permission_denials` to read).
- **Stub.** `tests/fixtures/codex-stub/codex`: when argv carries `--output-schema <path>`, the rollout gains a third line `{"stub":"schema","path":"<path>","present":true|false}` (whether the file exists at that path inside the container) and the `agent_message` becomes JSON: by default `{"verdict":"pass","summary":"stub verdict: <first brief line>","findings":[]}`; a brief whose first line (after arx's prefix, so the stub scans for the first line starting with `probe-`) is `probe-malformed:` yields `{"verdict":"maybe"}`; `probe-write: <path>` yields the conformant object with the write result in `summary` (`wrote /hand/repo/<path>` or `write refused: <error>`). Without the flag the stub behaves exactly as L2-09 left it.
- **No save verb, no third lifetime class.** Mechanically true already: the judge's container home has no MCP configuration, so no `save` tool exists in it; nothing to build, stated in the card's Shipped section with the evidence.
- **Nothing else moves.** No change to `codex_hand::build_argv`, the claude-code or native wires, `format_delivery`'s shape for hands, `check-config`, the `[gate]` machinery, or any `Delivery` variant.
- **Gate amendment, 2026-09-07, sanctioned by main after the codex round.** `resolve_hand_spec` resolves a judge's `write` to `false` whatever its definition says (`write: role != HandRole::Judge && row.write`), so a `write: true` on `agents/judge.md` cannot mount a writable worktree; the dispatch is never refused. Two unit tests in `main.rs`'s test module cover the judge and an ordinary hand. This is the read-only clamp of `84c34f6a` enforced at the one place the spec is built.

## Chosen without a grill

- The schema's three fields. `da3f5418` says "the brief names the schema" and names none; `pass|fail` plus a summary plus severity-tagged findings with evidence is the smallest shape both the drain's reviewer and a gauntlet critic can fill. The drain's promote/drop per claim goes in `findings`; a later card can widen the enum.
- The schema file under `/hand/out`, swept and visible, rather than under the hand's home, hidden.
- The flag appended in `container_hand::build_argv`, not in `codex_hand::build_argv`, to keep the CLI adapter role-blind.
- The schema prefixed to every judge's brief, so a claude-code or native judge is at least asked for the shape this card cannot enforce on it.
- Validation in main at delivery, ledger row untouched, `Delivery` structs untouched: the judge logic sits in one module and one delivery site.
- Family for an OpenAI-compatible provider is its config key: a native main on a provider named `gpt` would match a codex judge, one named `openai` would not. Disclosed rather than solved.
- Two event kinds rather than one with a stage field, because `arx events` groups by kind and the two have different fields.
- The write refusal recorded through the verdict rather than a synthetic `wall` event parsed out of codex's command items.

## Ground truth

Verified by read-only traces on 2026-09-07 against the L2-09 landed tree.

- `crates/arx/src/codex_hand.rs:16-29`: `build_argv(spec: &HandSpec)` returns `exec --json --model <m> --skip-git-repo-check --sandbox danger-full-access -c instructions="…" -c project_doc_max_bytes=0`; `spec.role` is in scope and unread. No `output-schema` anywhere in the tree.
- `crates/arx/src/container_hand.rs`: codex `build_argv` arm 290-294; `prepare` 134+ with the codex credential step at 149-155 (`inject_codex_credential(home)` writes `<root>/home/.codex/auth.json`, `0700`/`0600`); `plan` mounts 229-245 (`/hand/repo` `readonly: !spec.write`, `/hand/home` and `/hand/out` always rw); masks 218-227; the codex `Wire` arm 507-523 builds `Delivery::Verdict { hand, text, trace, bytes, usage, walls: Vec::new(), miss: None }` (563-573); the brief is written to stdin at 407-410.
- `crates/arx/src/container.rs:308-336`: `sweep` copies every file under `out/` into `<trace>/out/` and lists it in `trace.md`.
- `crates/arx/src/ledger.rs`: `HandRole { Hand, Judge }` (67-70, `PartialEq`); `HandSpec` (88-109) with `role`, `write`, `tools`, `model`, `image`, `env`; `handle_delivery` (533-557) writes `{"kind":"verdict","hand","text","trace","usage"}` at 545.
- `crates/arx/src/main.rs`: `render_hand_roster(hands: &[RoleRow])` 156-174, called at 216 from `compose_dynamic_block` with `&resolved.hands`, row shape `- {role}: {model}, {body}, tools {tools}` plus `. {role_line}`; `resolve_hand_row` 329-342 returns `(&resolved.judge, HandRole::Judge)` for the judge name; `resolve_hand_spec` 379-451; `on_hand_call` dispatch arm 469-515 with the body check at 480-491 and no comparison against `resolved.main`; `format_delivery` 611-620; verdict deliveries reach `cycle!("arx", &text)` at 1858 and 1967; `Event::Wall` emitted at 1925 and 1932; `TerminalMsg` (`terminal.rs:3-8`) has `Chunk`, `TurnEnd`, `Notice{text}`, `Ask{tool,subject}`, `Notice` rendered by a bare `writeln!` at `terminal.rs:60-62`; `cmd_events` 2088-2129 with `event_fields` at 2011+ as a hand-written per-variant match. `grep -rn family crates/arx/src` is empty.
- `crates/arx/src/config.rs`: `Body { Native, ClaudeCode, Codex }` 12-16; `RoleRow { body, model, provider: Option<String>, … }` 214-230; `Resolved { main, judge, hands, providers: BTreeMap<String, ProviderConfig>, … }` 233-245; `ProviderConfig.kind: ProviderKind` at 66; `ProviderKind { OpenAiCompatible, Anthropic }` at `provider.rs:167-171`.
- `crates/arx/src/events.rs`: 17 variants 13-94, `KINDS` 96-114, `Event::kind()` 117-137, `every_kind()` 141-201. `tests/events.rs::every_kind_writes_and_arx_events_counts_it` iterates `every_kind()` and asserts each kind's heading and a count of 1, so two new kinds are covered without editing that file.
- `agents/judge.md`: `body: codex`, `model: gpt-5.6-sol`, `image: arx-hand-codex:0.152.1`, one line of instruction; no `write:`. `tests/fixtures/run/agents/judge.md` identical with `image: arx-codex-stub:test`. `tests/check_config.rs:13` `SUPPORTED_CODEX_MODELS = ["gpt-5.6-sol"]`, test at 127-168 needs `visited >= 7`.
- `tests/fixtures/codex-stub/codex` (66 lines): records argv and pid under `$CODEX_HOME`, reads the brief from stdin, honours `[sleep:N]`, writes the rollout with the `argv` and `auth` lines, answers `probe-write:` on the brief's first line, else emits `stub verdict: <first line>`, then the fixed four-event stream with usage `{11,3,0,7,1}`.
- Tests touching the judge: `tests/claude_main.rs` 1295-1355 `judge_dispatch_over_a_seeded_diff_delivers_a_verdict_through_the_codex_body` (asserts the window text `[hand h1 verdict] stub verdict: Review seeded.diff and return a verdict (trace: threads/main.h1)`, `trace.md`, `session.jsonl` argv line, spawn row `agent=="judge"`/`role=="judge"`, usage), 1261-1293 `dispatch_on_unknown_agent_is_refused_before_spawn` (refusal text lists `judge`), 1357-1424 `dispatch_naming_a_body_other_than_the_definition_is_refused_at_the_ledger`, 1426-1449 `dispatch_naming_an_unknown_body_is_refused`; `tests/layer_close.rs` dispatches `judge` as h4 and asserts its delivery text contains `stub verdict:`; `tests/scripts/l1-close.json` predicate `judge-verdict` is `contains "[hand h4 verdict]"`.
- Counts on the L2-09 tree: arx 381 over fifteen binaries (unittests 207, check_config 10, checkpoint 6, claude_in_container 4, claude_main 72, codex_in_container 4, container_runtime 9, events 3, gate 4, hands_in_containers 5, layer_close 12, native_in_container 7, state_object 7, store 3, transcript_log 28); arx-engine 44 passed, 1 ignored. Container tests share one Docker daemon and assert on global `docker ps -a` filters; a second `cargo test` running at the same time makes them flake. Run the suite alone.
- The L2-09 live check: a real Claude Code main declined to dispatch `judge` because the name was absent from its "Hands available" listing.

## Execution plan

### Scope 1 — judge module, events, roster, family, delivery

- **Files:** Create `crates/arx/src/judge.rs`; Modify `crates/arx/src/main.rs`, `crates/arx/src/events.rs`, `prompts/main.md`.
- **Build:** the schema constant and validator; the family function; the two event kinds; the roster line; the family check at dispatch; validation at delivery.
- **Steps:**
  1. `judge.rs`: `JUDGE_SCHEMA`, `validate_verdict`, `family` per Decisions locked. `mod judge;` in `main.rs`. No inline test module in `judge.rs` (integration tests `#[path]`-include src files); tests go in `main.rs`'s test module.
  2. `events.rs`: `Schema { hand, outcome, reason: Option<String> }` and `Family { agent, family }`, in `KINDS` (append at the end), `kind()`, `every_kind()`.
  3. `main.rs`: `render_hand_roster` takes the judge row as a second argument and renders it last per Decisions locked; `compose_dynamic_block` passes `&resolved.judge`. In the dispatch arm, after `resolve_hand_spec` and before `ledger.dispatch`, the family check for `HandRole::Judge`. At verdict delivery, look up the entry's role, validate for judges, emit the `schema` event, format the window line per Decisions locked. `event_fields` arms for both kinds.
  4. `prompts/main.md`: the one sentence.
  5. Unit tests in `main.rs`'s test module: `validate_verdict` on a conformant object, an extra top-level field, a missing field, a bad `verdict`, a finding with a bad severity, a finding with an extra field, non-JSON, a non-object; `family` for each of `ClaudeCode`, `Codex`, `Native`+`Anthropic`, `Native`+`OpenAiCompatible`; `render_hand_roster` output ends with the judge line.
- **Verify:**
  - `cargo build -p arx` → clean; warning count equal to the count on a worktree of `main` (no new warnings). Any new warning is a stop.
  - `cargo test -p arx --bin arx` → state the number and the delta from 207.
  - `cargo test -p arx --test events` → 3 passed, the new kinds counted by `every_kind_writes_and_arx_events_counts_it`.
  - `grep -n 'output-schema' crates/arx/src/codex_hand.rs` → empty.
  - Do not modify any file under `crates/arx/tests/` in this scope.

### Scope 2 — the container path and the stub

- **Files:** Modify `crates/arx/src/container_hand.rs`, `crates/arx/tests/fixtures/codex-stub/codex`.
- **Build:** the schema file in `/hand/out`, the argv flag, the brief prefix, the stub's schema behaviour.
- **Steps:**
  1. `prepare`: the schema write for a codex judge, after the credential step. `build_argv` codex arm: the flag for a judge. The wire: the brief prefix for a judge of any body.
  2. Stub per Decisions locked. Rebuild is automatic: `ensure_codex_stub_image()` builds from the fixture directory; if its `Once` caches a stale image across runs, `docker rmi arx-codex-stub:test` once before the suite and say so.
- **Verify:**
  - `cargo test -p arx` → run once, alone on the daemon, before any test edit: state exactly which tests fail and the assertion each fails on. Expected, two tests in `claude_main.rs`, each on its verdict-text assertion, because the stub now returns JSON for a judge: `judge_dispatch_over_a_seeded_diff_delivers_a_verdict_through_the_codex_body` (line 1322) and `dispatch_naming_a_body_other_than_the_definition_is_refused_at_the_ledger` (line 1423, whose third dispatch `judge@codex` names the definition's own body and so runs for real). `layer_close.rs` is expected to stay green (its judge assertion is a `contains` on `stub verdict:`, which the JSON summary still carries). If anything else fails, STOP and report. (Amended 2026-09-07 after the runner stopped on the second test: the brief had authorised re-pointing only the first.)
  - `docker image ls arx-codex-stub:test` → present.

### Scope 3 — tests, card, live check

- **Files:** Create `crates/arx/tests/judge.rs`; Modify `crates/arx/tests/claude_main.rs` (one test, one assertion), `crates/arx/tests/common/mod.rs` (a helper that rewrites a workspace's `agents/judge.md` to a given body/model/image), `docs/wayfinder/maps/arx-v0/L2-12-judge-schema-and-clamp.md` (`## Shipped`, status `closed`).
- **Build:** the hermetic proof of every done-check, and the live one.
- **Steps:**
  1. `claude_main.rs` keeps 72 tests, every name and order. Two tests re-point one verdict-text assertion each: `judge_dispatch_over_a_seeded_diff_delivers_a_verdict_through_the_codex_body` asserts the window text contains `[hand h1 verdict] {"verdict":"pass","summary":"stub verdict: Review seeded.diff and return a verdict","findings":[]} (trace: threads/main.h1)`; `dispatch_naming_a_body_other_than_the_definition_is_refused_at_the_ledger` asserts its third verdict text starts with `{"verdict":"pass","summary":"stub verdict: third"` (amended 2026-09-07; the test's `judge@codex` dispatch matches the definition and runs for real). Every other assertion in both and every other test in the file unedited. The `main.rs` unit test `dynamic_block_renders_parts_in_order_and_omits_empty_ones` takes the roster's new trailing judge line in its expected string; that is the signature change's own consequence, disclosed.
  2. `tests/judge.rs`, six tests over the run fixtures with the codex stub image and planted stub credential, serialised on the existing docker-state mutex:
     - `a_judge_verdict_conforms_to_the_schema_and_is_logged`: dispatch `judge` with brief `Judge the seeded diff` → window line as in step 1's shape with that summary; transcript has `{"kind":"schema","hand":"h1","outcome":"ok"}` before the verdict turn; the trace has `out/verdict-schema.json` byte-equal to `JUDGE_SCHEMA`; `session.jsonl` line 1 argv contains `--output-schema` and `/hand/out/verdict-schema.json`, line 3 is the stub's `schema` line with `"present":true`.
     - `a_malformed_judge_verdict_takes_the_failure_path`: brief first line `probe-malformed:` → window text starts `[hand h1 verdict] judge verdict malformed: missing field summary. Raw: {"verdict":"maybe"}`; the `schema` row has `outcome":"malformed"` and `reason":"missing field summary"`; the ledger verdict row's `text` is exactly `{"verdict":"maybe"}`.
     - `a_judge_write_is_refused_and_the_refusal_is_in_the_log`: brief first line `probe-write: VERDICT.md` → the verdict JSON is conformant (`schema` ok), its `summary` contains `write refused`, no `worktree.diff` in the trace, the scratch repo unchanged, and the ledger verdict row's `text` contains `write refused`.
     - `a_judge_in_mains_family_warns_and_still_runs`: rewrite the workspace's `agents/judge.md` to `body: claude-code`, `model: stub-model`, `image: arx-claude-stub:test` with a planted claude stub credential; dispatch `judge` → captured stdout contains `[arx] judge 'judge' is claude, the same family as main; its verdict is not independent`; transcript has `{"kind":"family","agent":"judge","family":"claude"}` before the `spawn` row; a `spawn` row for `judge` and a verdict delivery exist; the `schema` row for that hand has `outcome":"malformed"` with `reason` starting `not JSON` (the claude stub does not emit JSON).
     - `a_hand_dispatch_carries_no_schema_flag_and_no_schema_event`: dispatch `hand-cx` → its `session.jsonl` argv line has no `--output-schema`; no `schema` row in the transcript; its verdict is the plain stub text.
     - `the_roster_lists_the_judge`: the transcript's `system` row text contains `- judge: gpt-5.6-sol, codex, read-only; returns a JSON verdict under arx's schema` and the `prompts/main.md` sentence.
  3. Live check, on the owner's login, in a scratch git repository with one commit holding `README.md`, `src/lib.rs` and a `seeded.diff` that adds a function to `src/lib.rs`; definitions: the real `agents/judge.md` and `agents/hand-cx.md`, main as the repo's real Claude Code main. Operator line: `Dispatch the judge over seeded.diff: ask it whether the diff is safe to apply, and to attempt to write its verdict to VERDICT.md at the repository root and report what happened.` Expected: main dispatches `judge` without being told the name is valid (the roster now carries it); the window line carries a conformant JSON verdict; `arx events` shows `## schema (1)` with `outcome=ok`; the verdict's `summary` reports the write refused on a read-only file system; the trace has `out/verdict-schema.json`. Then edit the scratch workspace's `agents/judge.md` to `body: claude-code`, `model: <the model agents/hand-ex.md uses>`, `image: arx-hand-claude:2.1.258`, repeat the operator line → the terminal shows the family notice, `arx events` shows `## family (1)` with `family=claude`, a verdict arrives (conformant or malformed; quote it and the `schema` row either way). `docker ps -a` shows no hand container after; `/tmp/arx-hands` empty.
  4. Card: `## Shipped` with every deviation, including the evidence that the judge's container has no MCP configuration (no `config.toml` in its home) so no save verb exists; status `closed`.
- **Verify:**
  - `cargo test -p arx` alone on the daemon → every binary green; counts stated with deltas: `claude_main` 72, `judge` 6, `events` 3, `layer_close` 12, `check_config` 10 with `visited >= 7`.
  - `cargo test --workspace` → every count restated cold.
  - `cargo run -p arx -- check-config ./arx.toml` → byte-identical to `main`'s (a worktree, removed after).
  - Live-check transcripts quoted verbatim: both operator lines, both window lines, the `arx events` output for both runs, the terminal notice.
  - `git status --short` → only the files this brief names plus the brief and the map; nothing under `store/`, no `pid.txt`.
  - Do not modify `tests/layer_close.rs`, `tests/codex_in_container.rs`, `tests/claude_in_container.rs`, `tests/native_in_container.rs`, `tests/container_runtime.rs`, `tests/hands_in_containers.rs`, `tests/gate.rs`, `tests/events.rs`, `tests/transcript_log.rs`, `tests/store.rs`, `tests/state_object.rs`, `tests/checkpoint.rs`, `tests/check_config.rs`, `tests/scripts/*`, the repo-root `agents/*.md`, `CLAUDE.md`.

## Out of scope

- Schema enforcement for a claude-code judge (structured output) or a native judge (the provider's JSON mode); they get the brief prefix only.
- The judge as a third participant with its own machinery (`84c34f6a`'s named reopen).
- `review/start`, ever (`da3f5418`).
- A `wall` event for codex command failures; `walls` stays empty on the codex wire.
- Any schema field beyond the three, any per-brief schema.
- `elapsed_ms`/`lag_ms` on the new events (none of the existing events carry them).

Done means: main dispatches the judge over a seeded diff and receives a verdict matching the schema with a `schema` row saying so; a malformed verdict reaches main marked and reasoned, with the `schema` row carrying the reason; a judge in main's family produces the terminal notice and the `family` row and still runs; a judge's write is refused by the mount and the refusal sits in the verdict row of the ledger and the window turn; the roster names the judge; every test green with counts stated.

## Guardrails

- Never commit, never rebase, never stash, never switch branches, never `git add`. Read-only git toolkit on this repository: `git status`, `git diff`, `git log`, `git show <ref>:<path>`; `git worktree add` only for the scratch build of `main` (remove it after). Git writes inside temp directories the tests and the live check create are fine.
- `store/` and `pid.txt` at the repo root are tracked live state: never write there, never stage them.
- Never print, log, or quote the contents of `~/.codex/auth.json`, `~/.claude/.credentials.json` or any token.
- Inline `#[cfg(test)]` modules in files that integration tests `#[path]`-include register twice; new unit tests go in `main.rs`'s test module.
- No inline comments. Unslop applies to the card's Shipped paragraph and the verdict.
- If a locked count or a "keeps its assertions" clause cannot hold as written, STOP on that scope and report the exact conflict with evidence rather than bending the count or the test.
- Run the test suite alone on the Docker daemon; if a container test fails, check `docker ps -a` for a stray container before re-running and say what you found.
- Leave no container running, no `/tmp/arx-hands` entry, no scratch directory or worktree when a scope ends. Built images stay.
- The codex gate: run it; if it hits a limit, log it and say so.

## Owner items

- The three calls most worth a veto: the schema's shape (`pass|fail`, `summary`, severity-tagged `findings`); the family of an OpenAI-compatible provider being its config key; the schema prefixed to every judge's brief.
- A judge's write refusal is in the log only through its own verdict; the codex wire has no denial list to turn into `wall` events.
- The roster now names the judge, so a real main will dispatch it on its own judgement; the standing prompt tells it when.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-07-l2-12-judge-schema-and-clamp.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
