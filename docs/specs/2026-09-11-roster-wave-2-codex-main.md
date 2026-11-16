# Roster wave 2 — Codex in main's seat

## Problem / Solution

The registry can bind hands to Codex, but main still requires Claude Code. Selecting a Codex model for
main is refused. Put both bodies behind the main-seat interface and let the existing `/model` command
select either. Same-body changes keep the conversation; a body change runs arx's checked handoff immediately.

Codex uses the same normal checkpoint and reset ritual as Claude Code. If Codex unexpectedly starts its
own compaction, arx discards that vendor session and recovers through its checked-state-object/transcript
fallback. Continuing from the vendor's summary is not the recovery mechanism.

## Inherited agreement

### Observable behavior and examples

- A configured or persisted Codex main starts directly under an arx-owned `codex app-server` stdio child.
  The operator keeps the existing terminal, commands, tool results, transcript and hand deliveries.
- `/model main <codex-id>` while Claude main is running performs the normal authoring and truth-check
  ritual, then starts Codex and runs orientation. The reverse change follows the same path. Running hands
  survive, and their later deliveries reach the new main through the existing ledger.
- Changing between two models on the same body retains its conversation. A resulting Codex compaction
  is handled by the fallback below, not accepted as an invisible replacement for arx's state object.
- `/reset`, automatic digestion and checkpointing retain their established meaning under either body.
  A fresh main never resumes vendor conversation history.
- Unexpected Codex compaction takes the existing exhaustion fallback from the latest checked object plus
  transcript, or transcript alone if no checked object exists. Partial output and the recovery cause remain
  visible in arx's record. An unfinished ritual never becomes the checked recovery object.
- Typed commands, `/model clear`, and modal Enter use the same body-selection behavior. Esc changes nothing.
  Failed selection must not be displayed or persisted as a successful model change.

### Evidence

- Arx baseline `c0567aa`, roster A/B/C landed [read]. Their briefs remain the compatibility contract:
  `2026-09-09-roster-a-registry-and-modal.md`, `2026-09-09-roster-b-positions-and-write-hand.md`,
  `2026-09-10-roster-c-tool-enforcement-and-ask-removal.md`, beside this file.
- Current turn, reset, model and floor paths in `crates/arx/src/main.rs`; current adapter in
  `crates/arx/src/claude_code.rs`; occupancy in `crates/arx/src/transcript.rs` [read at baseline].
- [R03](../wayfinder/maps/arx-v0/findings/R03-codex-runtime-probe.md) [ran, 2026-09-10]: stdio child
  lifetime, instruction injection, interrupt, model switching, token notifications and home isolation.
- [R04](../wayfinder/maps/arx-v0/findings/R04-codex-main-source-refresh.md) [read, 2026-09-11]: installed
  Codex 0.153.4 and matching official source `3d2ee51ca2d5db578f328aa75e20aa22c0197c9a`, including
  dynamic tools, usage fields, compaction limits and compaction notifications. No new live prompt was run.
- Owner agreement in this flow, 2026-09-11: immediate checked handoff on a body change; ordinary Codex
  digestion as on Claude Code; vendor compaction extends the exhaustion fallback.

### Uncertainty

- Scope verification must prove the new adapter's complete tool round trip, prompt isolation, interrupt and
  fresh-session recovery. Source/schema inspection alone is not that proof.
- Owner update during execution, 2026-09-11: real Claude ↔ Codex handoff testing is deferred to Sunday,
  2026-09-13, after Claude's weekly quota resets at 16:00 Europe/Vilnius. This live check is excluded from the
  current completion gate by the owner's instruction. Deterministic cross-body acceptance and live Codex
  adapter/product checks remain required. Record the deferred check explicitly; do not claim it passed.

## Decisions locked

- D1: The seam is the main role. Both Claude Code and Codex supply main-seat adapters. Shared knowledge
  belongs in a body module when both of that body's role adapters use it. The existing hand seam remains.
  Rejected: a single adapter per CLI kind spanning both roles.
- D2: Arx owns the process tree and operator surface. Codex runs as a direct stdio child; no shared daemon,
  remote listener or external Codex host is introduced.
- D3: A cross-body model change runs the ordinary checked reset ritual immediately. Arx's process, front,
  transcript, ledger and in-flight hands survive. Rejected: saving the choice for a later reset or launch.
- D4: Same-body model changes retain the current vendor thread and take effect for the next turn. Preserve
  the current immediate command behavior and truthful model/pin reporting; no dummy model turn is required.
- D5: Normal digestion/checkpoint/reset is shared across bodies. A Codex `contextCompaction` event is an
  additional exhaustion-floor trigger. Discard that vendor session and recover from arx's checked state and
  transcript. Rejected: continuing on the vendor summary or blocking Codex main until compaction is preventable.
- D6: Main is not asked for per-call approval. Retain arx's tool handling and admission semantics; do not
  rebuild the removed `Ask` path or make an approval RPC a requirement of the main-seat interface. Codex's
  retained built-ins remain a disclosed limitation of admission coverage. Preserve deterministic allow/deny
  evaluation in `handler_gate_call`/`gate_call`; removing the SDK approval bridge does not remove those rules.
- D7: Arx owns main's instructions. Use a fresh isolated `CODEX_HOME` for each main incarnation, supply
  authentication without copying ambient instructions/config, suppress project discovery, and send arx's
  composed prompt through `baseInstructions`. Never resume a previous vendor thread after reset.
- D8: `[main] window` remains authoritative. Codex's final `tokenUsage.last.inputTokens` supplies prompt
  occupancy once; it already includes cached input. Cumulative usage is separate. Do not silently adopt the
  reported model window or add cached input twice.

## Constraints

- Branch: `roster-wave-2-codex-main`, based on `c0567aa`. The brief, R04, map and CONTEXT changes already
  belong to this card. Do not revert them. Runners/reviewers never switch branches, stash or commit.
- Use a solo, scope-sequential runner: the scopes share the main loop and its tests. No parallel file owners.
- Preserve existing transcript readability, roster file format, typed/modal command forms, and provider/hand
  behavior. Native/provider main remains wave 3. Unsupported main bodies still refuse with a useful message.
- The effective registry entry determines the running main body. Existing `[main].body` compatibility and
  validation must not veto a valid cross-body pin or apply Claude-only limits to an effective Codex main.
  Existing configurations remain loadable; do not add another separately configured active-body selector.
- The common main interface hides transport and vendor message shapes. Callers need turns, tool round trips,
  text, usage, interruption, model selection and lifecycle outcomes. Do not export vendor approval machinery
  merely because the current Claude adapter has it.
- Generated Codex schemas/source are evidence, not vendored dependencies. Keep the wire types scoped to the
  implemented protocol. Preserve the package's current async/process conventions.
- Arx has no `lib.rs`; integration tests `#[path]`-include source files. Do not add inline test modules to
  included files. Put focused unit coverage in `main.rs` or integration tests.
- Baseline test totals must be measured before edits; old map counts are not an assertion to copy. Account for
  every removed/replaced test. A contradictory requirement is a tripwire: stop and report it instead of
  changing the expected behavior or count to fit.
- Isolate test/live `threads`, roster pins, gate configuration and Codex homes. Never run `arx run` from the
  owner's active repo directory. Keep the live store read-only during probes. Never print credentials.
- Docker is required by the existing suite. If an existing fixture script changes, remove its cached stub
  image tag before testing. Do not use stale images as verification.

## Execution plan

### Scope 1 — Main-seat interface

- **Files:** Create `crates/arx/src/main_body.rs`; Modify `crates/arx/src/main.rs`,
  `crates/arx/src/claude_code.rs`, `crates/arx/tests/claude_main.rs`.
- **Build:** The current Claude main operates behind the role interface; the turn/reset loop stops naming
  `ClaudeCodeMain` as its concrete dependency.
- **Steps:**
  1. Measure the existing suite baseline before implementation.
  2. Introduce the shared main handle and body-neutral turn/lifecycle results using the existing adapter's
     actual caller needs. Preserve async tool callbacks, streaming, stopped turns and error propagation.
  3. Move Claude-specific construction behind the main factory. Retain its existing sidecar behavior.
  4. Keep session replacement behind the handle so callers and running hands do not acquire stale adapters.
- **Verify:** Run `cargo test -p arx --test claude_main`, `cargo test -p arx --test checkpoint` and
  `cargo test -p arx --test reset`, plus the existing main-module stop tests. Preserve
  `kill_then_respawn_does_not_double_signal_and_the_new_child_answers`,
  `one_turn_returns_stub_reply_with_usage_and_ordered_deltas`, `two_prompts_on_one_adapter_both_complete`,
  and the operator/floor reset coverage. The production loop's tests exercise the common seam.

### Scope 2 — Codex main adapter

- **Files:** Create `crates/arx/src/codex_main.rs`, `crates/arx/src/main_tools.rs`,
  `crates/arx/src/codex_auth.rs`, `sidecar/main-tools.json`,
  `crates/arx/tests/fixtures/stub-codex-main/codex`, `crates/arx/tests/codex_main.rs`;
  Modify `crates/arx/src/main_body.rs`, `crates/arx/src/main.rs`, `crates/arx/tests/common/mod.rs`,
  `sidecar/claude-main.mjs`, `sidecar/test/drive.test.mjs`. If authentication logic is shared, Modify
  `crates/arx/src/container_hand.rs` only to extract/reuse its existing credential helpers without changing hand behavior.
- **Build:** A Codex main can complete a streamed turn, call arx tools and accept their results, stop a turn,
  change its model, and terminate without leaving a child or active home behind.
- **Steps:**
  1. Launch `codex app-server` over stdio with a fresh home, using the installed executable from PATH and
     explicit noninteractive approval behavior consistent with D6. Do not require Claude's sidecar to run it.
  2. Initialize with `capabilities.experimentalApi=true`; start a fresh thread with arx's composed
     `baseInstructions`, project discovery suppressed, and arx's existing schemas registered as `dynamicTools`.
     Today's schema authority is the manual Zod construction in `sidecar/claude-main.mjs::arxTools`; there
     is no existing Rust schema module or JSON export. Expose the same contract for Codex through
     `main_tools.rs`, with a shared source/export or a parity check against the sidecar definitions. Preserve
     tool names and argument meanings; do not invent a second vocabulary as roster C's first brief did.
  3. Map `item/tool/call` onto the existing Rust tool path and reply with `contentItems` and `success`.
     Correlate protocol request ids, tool call ids, thread ids and turn ids without conflating them.
  4. Map streamed text, final text, tool failures, turn completion, interrupt and child/protocol failures into
     the common interface. Do not duplicate streamed text in the transcript's canonical final reply.
  5. Apply same-body model selection through the next `turn/start.model`. Validate supported registry/body
     selection before changing the pin; surface an upstream model rejection without claiming it ran.
  6. Kill and reap on shutdown/error/reset, then remove the owned temporary home. Reuse only authentication
     input, not old vendor session state. Keep any shared body helpers small and used by real callers.
- **Verify:** `cargo test -p arx --test codex_main` and `npm test` in `sidecar/`. A deterministic fake stdio
  process exercises text → arx tool call → result → final reply, with
  interleaved notifications, unsuccessful tool results and malformed/closed transport. Stop leaves the thread
  usable; a model change affects the next turn without a new thread. Fresh starts have distinct homes and
  thread ids; ambient global/project instruction canaries and unrelated config do not enter the composed prompt.
  Cleanup leaves no running child. The fake validates actual request/response fields from R04.

### Scope 3 — Usage, digestion and recovery

- **Files:** Modify `crates/arx/src/main_body.rs`, `crates/arx/src/codex_main.rs`,
  `crates/arx/src/main.rs`, `crates/arx/src/transcript.rs`, `crates/arx/src/events.rs`,
  `crates/arx/tests/codex_main.rs`, `crates/arx/tests/reset.rs`, `crates/arx/tests/checkpoint.rs`,
  `crates/arx/tests/fixtures/stub-codex-main/codex`.
- **Build:** Codex drives the existing nudge, checkpoint, normal reset and exhaustion recovery correctly.
  Vendor compaction is visible and cannot silently become arx's continuity mechanism.
- **Steps:**
  1. Normalize the final usage notification of a completed turn. Preserve whole-turn/cumulative accounting
     separately from latest prompt occupancy; retain the raw evidence needed to diagnose a mismatch.
  2. Use the configured arx window and existing ritual thresholds. Set a Codex compaction limit only within
     the supported controls; do not assert that the setting disables compaction or guarantees precedence.
  3. Map window-exhaustion errors to the existing floor path. Map either `item/started` or `item/completed`
     for `contextCompaction` to a distinct recorded recovery cause and the same fallback mechanism.
  4. On compaction, stop accepting output/tool dispatch from the abandoned vendor session, kill/reap it,
     and start fresh. Handle duplicate/delayed compaction events once per abandoned incarnation.
  5. Use only the latest checked state object and transcript for fallback orientation. A compaction during
     authoring/truth-check cannot promote an unfinished object. Normal successful reset keeps its existing ritual.
- **Verify:** Run `cargo test -p arx --test codex_main`, `cargo test -p arx --test reset` and
  `cargo test -p arx --test checkpoint` after the added recovery cases. Feed several token notifications
  in one turn, including cached input; assert final prompt
  occupancy without cumulative inflation or double counting. Exercise normal checkpoint/reset under Codex,
  ordinary-turn and ritual exhaustion, compaction during an ordinary turn and during truth-check, no-checkpoint
  fallback, and started/completed arriving before the client handles either. Assert one recovery, a fresh
  thread/home, no vendor resume/summary, preserved transcript and an in-flight hand still delivering afterward.

### Scope 4 — Registry selection and cross-body handoff

- **Files:** Modify `crates/arx/src/main.rs`, `crates/arx/src/config.rs`, `crates/arx/src/roster.rs`,
  `crates/arx/src/terminal.rs`, `crates/arx/src/panes/app.rs`, `crates/arx/src/panes/state.rs`,
  `crates/arx/src/panes/ui.rs`, `crates/arx/tests/claude_main.rs`, `crates/arx/tests/codex_main.rs`,
  `crates/arx/tests/reset.rs`, `crates/arx/tests/fixtures/stub-sidecar/claude-main.mjs`,
  `crates/arx/tests/fixtures/stub-codex-main/codex`.
- **Build:** Codex can be the configured or pinned main; all existing model-selection entry points perform
  the agreed handoff consistently.
- **Steps:**
  1. Replace Claude-only main guards with the two supported main bodies. Keep provider main unsupported.
     Startup uses the effective binding; no hidden Claude dependency may prevent a Codex-only launch.
  2. Route typed selection, clearing a pin and modal application through one body-aware transition path.
     Same body changes the model in place; different body invokes the checked ritual under the outgoing main,
     then constructs the destination adapter and runs orientation as its first turn.
  3. Recompose the prompt for the selected binding. Preserve the Rust process, front, ledger, transcript and
     hands. Clear reset accounting only where the existing successful-reset/floor path does so.
  4. Commit and announce the resulting selection consistently. Preflight locally detectable launch/auth/config
     failures before replacing the active child. Failure must retain or recover the previous binding from the
     checked handoff; if recovery itself fails, show an explicit stopped/error state rather than a false success.
     For one main-selection command, success means the destination is running and its pin is durably saved.
     Prepare persistence before replacing the child where possible; a failed final save requires rollback or
     explicit recovery, not the current silent live/durable divergence. A failed same-body change follows the
     same reporting rule. This does not make all modal rows one transaction.
  5. Preserve roster modal all-table behavior, cancellation, clear/default semantics, dangling-pin fallback and
     judge-family warnings. Today Enter emits changed rows separately in order: main, judge, hands alphabetically,
     drain. Preserve that per-row application; a failed main change is reported as failed while later selections
     can still be processed. A multi-row rollback or a new modal transaction is outside this card.
- **Verify:** Run `cargo test -p arx model_command`, `cargo test -p arx --test codex_main`,
  `cargo test -p arx --test reset` and the affected config/roster/modal tests. Test Claude → Codex and Codex → Claude
  through typed command, modal Enter and clearing a pin
  across bodies. Assert outgoing ritual, fresh destination, orientation first, hands surviving, persisted pin
  and model event matching reality. Same-body changes do not reset. Exercise missing executable/start failure,
  upstream model refusal and roster-save failure; confirm no misleading persisted/printed successful switch.
  Cover a model present in the catalogue but refused by `turn/start`, followed by a usable turn on the prior
  binding. Selecting a valid same-body model and quitting before another model turn must still preserve that
  choice for restart. A replacement that fails validation or launch must preserve an already-saved pending
  selection and its rollback baseline. Esc remains byte-identical on disk. Restart with a Codex pin starts Codex directly.

### Scope 5 — Operator contract and acceptance

- **Files:** Modify `prompts/main.md`, `CONTEXT.md`, `POSITION.md` only where the delivered behavior changes
  their current statements, `docs/wayfinder/maps/arx-v0/map.md`, `crates/arx/tests/layer2_close.rs`,
  `crates/arx/tests/scripts/l2-close.json`; use the integration fixtures/tests listed above.
- **Build:** The standing prompt and operator surfaces describe the delivered behavior; the complete main
  path is verified under both bodies before review and flow verification.
- **Steps:**
  1. Update the standing prompt in the same card for any new main experience. Remove obsolete claims that
     only Claude main is supported. Explain body-switch handoff and compaction recovery in operator language.
  2. Keep normal Codex text, mounted-tool results, hand deliveries, model changes and recovery visible through
     existing fronts/events. Report retained built-in/admission limitations without inventing an approval UI.
  3. Run the integration commands below and record actual totals and any justified test replacement.
     The L2-close script's retired SDK gate requests must be replaced with real mounted-tool requests while
     retaining its explicit allow/deny assertions; do not retain a dead approval bridge just for that fixture.
  4. In an isolated scratch git directory, use the installed bodies for a short live acceptance pass: Codex
     starts with arx's prompt, performs one harmless mounted-tool round trip, stops and accepts another turn,
     changes models, and runs a checked handoff. The real Claude ↔ Codex switch check is owner-deferred until
     Sunday as recorded above; do not retry Claude before then. Use deterministic fixtures for cross-body
     handoff and compaction/floor failures; do not burn tokens filling a real window.
- **Verify:** `cargo test --workspace`, `npm test` in `sidecar/`, and
  `cargo run -p arx -- check-config ./arx.toml` pass. The workspace run supplies both arx and engine totals;
  do not rerun an unchanged green suite just to restate its count. Record the live request/result and front/transcript evidence
  with isolated paths and versions. A paid-body outage is an explicit verification limitation, never a claimed
  live PASS. Close all child processes and remove scratch credentials/homes after the probes.

## Out of scope

Provider/native main, redesigning the hand seam, container hardening, new position/image semantics,
enforcing Codex's unremovable collaboration tools, rewriting all gate rules, changing configured-window policy,
vendor-thread resume, a new terminal layout, and unrelated carried findings.

## Review handoff

Runner and reviewer receive this brief, R03/R04, the predecessor roster briefs only for relevant compatibility,
and the repository hazards. They do not repeat the grill. The reviewer checks the interface from the actual
main callers and tests, model/pin failure consistency, fresh-session isolation, and compaction recovery races.

A failed result returns as a local fix, missing fact, or a concrete contradiction requiring a reopened decision.
After runner PASS and independent reviewer PASS with empty `must_fix`, flow routes through `verify` against
this agreement before the card is marked landed or shipped. Record test totals and live limitations in the
handoff. Main alone performs the repository's authorized card-close sequence after all required gates pass.
