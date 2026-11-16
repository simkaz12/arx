# arx project review — 2026-09-12

The two reported model-selection symptoms are fixed in the working tree. The broader review found serious gaps in work preservation, store transactions, and recovery after process death. I would fix R01–R06 before relying on unattended runs, and R04/R08 before using the store across projects.

The broad findings below are source-reviewed, not failure-injected. They describe concrete paths through current code; they do not establish that your live work or store has already been corrupted. The inspected store was clean and its quarantine was empty.

## Delivered screenshot fixes

- Codex main now uses a private, unique runtime/cache home outside the temporary directory that Codex rejects for helper aliases.
- The child no longer inherits terminal stderr. arx drains it, retains a bounded 16 KiB diagnostic tail, and surfaces it on transport failure.
- An outgoing model's quota failure uses `outgoing_body_unavailable` recovery rather than a false floor/overshoot event. Rollback to a previous body also has its own recovery reason.
- Checkpoint/transcript fallback, the selected destination, and queued role pins remain intact.

The weekly-limit message in the screenshot came from outgoing Claude while it was asked to author the switch checkpoint. Codex activation succeeded. The warning in the input row was direct child stderr output, not text stored in the input state.

Changed files: [Codex adapter](../../../../../crates/arx/src/codex_main.rs#L54), [reset handling](../../../../../crates/arx/src/main.rs#L2451), [integration tests](../../../../../crates/arx/tests/codex_main.rs#L260), [test launcher](../../../../../crates/arx/tests/common/mod.rs#L566), and [Codex stub](../../../../../crates/arx/tests/fixtures/stub-codex-main/codex#L15).

**Verification:** `cargo test --workspace`: 755 passed, 2 ignored, 0 failed. Targeted Codex integration: 39 passed, 1 paid/live test ignored. Configuration validation, `cargo build -p arx`, and `git diff --check` passed. The two workspace skips are the paid installed-Codex check and live Ollama check.

Tests first caught the old false-floor behavior. Independent review then found two paths that lost captured diagnostics; added tests reproduced bare BrokenPipe and ReadyTimeout errors before their fix. Both now preserve stderr, while empty-stderr cases retain their original error types. The independent recheck passed. Added product checks cover a 1 MiB stderr stream without terminal leakage or deadlock, useful bounded diagnostics on startup failure, cleanup after a failed start, and diagnostics on write failure and initialization timeout. An installed-Codex `--version` probe reproduced the exact warning under `/tmp`; the selected runtime path emitted none and created the aliases.

`target/debug/arx` has been rebuilt. The running owner process was not restarted. Nothing was staged or committed. Project-wide formatting is not a clean baseline because the pre-existing working tree has formatting changes; the edited Codex product file was formatted and diff whitespace checks pass.

**Test reliability note:** One full rerun failed in transcript::tests::second_open_of_locked_thread_fails_then_succeeds_once_dropped at crates/arx/src/transcript.rs:657 with Locked on its temporary alpha.jsonl. It passed immediately in isolation and in the next complete workspace run. This is an observed intermittent failure; its cause and whether it predates these changes were not established.

## Archive context

This report was copied from the session artifact into the reliability effort on 2026-09-12. It describes a dirty working tree based on commit `3c92977302a28a8accec18b79c5434a74a82b299`; that commit alone does not contain every reviewed change. The task board tracks later work. This report remains the audit snapshot, not a second status ledger.

## Scope and evidence

Reviewed the current working tree, including the existing uncommitted roster/reviewer work, rather than only the changes made in this session. Coverage includes main lifecycle and reset, model persistence, panes, spawn ledger, container capture, reviewer dispatch, memory/quarantine/drain, claim serialization, indexing, file tools, admission, native execution, and provider failures.

The review followed real callers and compared behavior with `CONTEXT.md`, the build map, specifications, carried findings, and existing tests. The broad review ran no tests or formatters; runtime verification above applies to the screenshot fixes and regression suite. No paid model calls were made. File references describe this working tree and can move with later edits.

Priority expresses repair order: **P1** protects isolation, work, or recoverability; **P2** addresses narrower correctness or liveness failures; **P3** repairs operator information. **Strong** means a concrete structural defect; **Worth-flagging** includes narrower defects and cases with an unresolved behavior choice.

## Repair order

1. Close the host-read escape and preserve failed captures together (R01, R02). Reject unsafe output and retain the worktree when capture fails.
2. Make drains recoverable, reject duplicate supersession targets, and separate project/thread identities (R03, R08, R04). Add fault and multi-project tests.
3. Finish process recovery: tolerate torn transcript tails, reconcile orphan hands, then define startup orientation and quit semantics (R06, R05, R07, R09).
4. Bound file I/O and make stored content/indexes coherent (R10–R13). Then close the narrower trace and UI gaps.

## Findings

| ID | Priority | Classification | Finding |
|---|---|---|---|
| R01 | P1 | Strong | Output capture follows container-created symlinks on the host |
| R02 | P1 | Strong | Capture failure can delete a write hand's only deliverable |
| R03 | P1 | Strong | A drain retry can erase its recovery marker before the store is consistent |
| R04 | P1 | Strong | Different projects share one quarantine directory |
| R05 | P1 | Strong | Restart does not reconcile orphaned hands |
| R06 | P1 | Strong | A torn transcript tail prevents reopening the thread |
| R07 | P1 | Worth-flagging | Checkpoint recovery across process restart is missing |
| R08 | P1 | Strong | One drain can create two successors for a singular predecessor |
| R09 | P2 | Worth-flagging | Quit can leave an invisible process waiting for hands |
| R10 | P2 | Worth-flagging | Bounded read requests still perform unbounded blocking I/O |
| R11 | P2 | Worth-flagging | Allowed markdown fields change meaning after reopen |
| R12 | P2 | Worth-flagging | Shared-store writes and caches coordinate only inside one process |
| R13 | P2 | Worth-flagging | Editing claim content leaves a stale cosine vector |
| R14 | P2 | Worth-flagging | Stream failure discards already received native output |
| R15 | P2 | Worth-flagging | Recursive listing walks outside the workspace through symlinks |
| R16 | P2 | Worth-flagging | Fallback trace names collide across projects and restarts |
| R17 | P3 | Worth-flagging | Panes starts with blank session status despite known state |
| R18 | P3 | Worth-flagging | A failed reviewer preparation still records a family collision |

<a id="R01"></a>

### R01 — Output capture follows container-created symlinks on the host

P1 · Strong. New. High-confidence source trace; no exploit was executed.

**Trigger:** A hand places an absolute symlink under /hand/out, pointing to a file readable by the host arx process.

**Failure and impact:** copy_dir_all treats a symlink as a file and calls host-side fs::read on it. The target is resolved on the host during sweep, outside the container's mounts, and copied into the persistent trace. The capture module defeats the filesystem isolation of the container. Relative links can also copy credentials from the sibling hand home. Directory symlinks error rather than recurse; regular-file targets are the confirmed source path.

**Fix:** Reject symlinks throughout output capture using symlink_metadata. Treat rejection as a capture failure and preserve the hand root for inspection.

**Missing check:** Exercise container::sweep with a symlink to an external sentinel file. Assert that its contents are neither copied nor hashed and that capture fails explicitly.

Source: [crates/arx/src/container.rs](../../../../../crates/arx/src/container.rs#L274).

<a id="R02"></a>

### R02 — Capture failure can delete a write hand's only deliverable

P1 · Strong. New. High-confidence source trace. Existing tests cover successful capture.

**Trigger:** git add/git diff fails during collection, or writing the captured trace fails.

**Failure and impact:** Worktree::diff().ok() discards the diff error. Sweep failure becomes absent optional fields. The worktree and root are removed anyway, and delivery still advertises a trace handle. The only copy of a hand's edits can disappear, or main receives a handle to an incomplete trace without a mechanical failure.

**Fix:** Return a typed capture outcome. Only delete the worktree/root after a sealed capture succeeds. On failure, preserve them and deliver an explicit error with their paths.

**Missing check:** Inject diff failure and trace-write failure independently; assert retained work, an error delivered to main, and no claimed valid trace/diff handle.

Source: [crates/arx/src/container_hand.rs](../../../../../crates/arx/src/container_hand.rs#L485), [crates/arx/src/container_hand.rs](../../../../../crates/arx/src/container_hand.rs#L566).

<a id="R03"></a>

### R03 — A drain retry can erase its recovery marker before the store is consistent

P1 · Strong. New. High-confidence source trace; exact filesystem failure interleavings are not reproduced.

**Trigger:** A write or git commit fails after at least one successor claim has been written.

**Failure and impact:** Successors and predecessors are changed before commit. Retry sees an existing successor file, skips the entire plan including predecessor repair and commit accounting, then deletes the durable quarantine row. Claims can remain untracked, staged for an unrelated later commit, or linked to a predecessor that is still in force. The retry can remove the candidate needed to finish recovery.

**Fix:** Make the drain a recoverable transaction, with staged state and a durable completion record. File existence alone must not mean that predecessor update and commit succeeded.

**Missing check:** Fault-inject successor write, predecessor write, git add, git commit, and quarantine deletion. Restart and retry; assert complete lineage and one recorded result. The existing all-or-nothing test only rejects invalid plans before writing.

Source: [crates/arx-engine/src/engine/mod.rs](../../../../../crates/arx-engine/src/engine/mod.rs#L711), [crates/arx-engine/src/engine/git.rs](../../../../../crates/arx-engine/src/engine/git.rs#L49).

<a id="R04"></a>

### R04 — Different projects share one quarantine directory

P1 · Strong. New, latent. The inspected live store was clean, with one scope and empty quarantine; no existing corruption was observed.

**Trigger:** Project A saves a candidate, then project B runs with the same per-user store root.

**Failure and impact:** All runs name the engine thread main. Quarantine is root/quarantine/main, with no project or scope segment. B lists and drains A's candidates while its drain hand sees B's repository and B's threads/main.jsonl. A claim can be judged against the wrong source transcript. Different scopes collide too because scope is only a field in the candidate row. Partner repositories sharing a scope are also affected.

**Fix:** Persist a globally unique project/thread identity and carry it through save, quarantine listing, drain inputs, and transcript resolution.

**Missing check:** Run two workspaces against one store root, both with identical and different scopes. Each must list and drain only its own candidates and resolve their source handles correctly.

Source: [crates/arx-engine/src/engine/mod.rs](../../../../../crates/arx-engine/src/engine/mod.rs#L138), [crates/arx/src/main.rs](../../../../../crates/arx/src/main.rs#L221), [crates/arx/src/main.rs](../../../../../crates/arx/src/main.rs#L416).

<a id="R05"></a>

### R05 — Restart does not reconcile orphaned hands

P1 · Strong. New. The glossary explicitly promises this recovery; implementation is absent.

**Trigger:** arx crashes or is killed while a container hand and its worktree are active.

**Failure and impact:** SpawnLedger::open recovers the next identifier but initializes an empty hands map. There is no startup reconciler that finds, kills, and sweeps nonterminal hands. Containers, detached worktrees, and evidence can survive outside the restarted process. Main never receives the promised recovered terminal delivery.

**Fix:** Persist cleanup metadata before launch. Reconcile nonterminal ledger rows with labeled containers and hand roots on startup, then record and seal the recovered result before accepting dispatches.

**Missing check:** Kill arx during a delayed write hand, restart, and assert no orphan container/worktree remains and a terminal ledger row plus captured trace exists.

Source: [crates/arx/src/ledger.rs](../../../../../crates/arx/src/ledger.rs#L311), [CONTEXT.md](../../../../../CONTEXT.md#L121).

<a id="R06"></a>

### R06 — A torn transcript tail prevents reopening the thread

P1 · Strong. New. High-confidence source trace; existing read-side tests do not cover reopening that file for writing.

**Trigger:** The process dies during a transcript write, leaving an unterminated JSON or partial UTF-8 row.

**Failure and impact:** Read-side helpers ignore the unterminated tail, but writer reopen parses it through BufRead::lines and returns MalformedLine. arx cannot reopen the transcript after a realistic crash, even though its readers can recover the committed rows.

**Fix:** Under the writer lock, truncate only the bytes after the last newline before recovering turn numbering. Keep malformed newline-terminated rows fatal, as the ledger already does.

**Missing check:** Reopen after a partial UTF-8 tail, append, and verify numbering. Separately verify rejection of a malformed committed row.

Source: [crates/arx/src/transcript.rs](../../../../../crates/arx/src/transcript.rs#L108), [crates/arx/src/transcript.rs](../../../../../crates/arx/src/transcript.rs#L326), [crates/arx/tests/transcript_log.rs](../../../../../crates/arx/tests/transcript_log.rs#L168).

<a id="R07"></a>

### R07 — Checkpoint recovery across process restart is missing

P1 · Worth-flagging. Missing capability, not a model-switch regression. The glossary promises crash recovery, but L3-02 only specified in-process reset. Absence is confirmed; ordinary reopen semantics need an owner decision.

**Trigger:** Restart arx after a checkpoint and later work have been recorded.

**Failure and impact:** Startup opens the old log and checkpoint metadata, creates a fresh vendor session, resets last_handle to turn zero, and accepts input without orienting main from the checkpoint or transcript. The operator's next line reaches a main with no restored work state while the durable transcript continues its old numbering.

**Fix:** Define whether every reopen resumes, or distinguish crash recovery with a clean-close marker. For recovery, seed the handle from the log and run orientation before accepting operator input.

**Missing check:** Use two arx processes: the first records a checkpoint and subsequent turns; the second must orient on that state and its transcript gap before its first operator turn.

Source: [crates/arx/src/main.rs](../../../../../crates/arx/src/main.rs#L2990), [crates/arx/src/main.rs](../../../../../crates/arx/src/main.rs#L3135), [CONTEXT.md](../../../../../CONTEXT.md#L132).

<a id="R08"></a>

### R08 — One drain can create two successors for a singular predecessor

P1 · Strong. New. High-confidence source trace; existing supersession tests use only one successor per target.

**Trigger:** Two promoted candidates in one drain both supersede the same in-force claim.

**Failure and impact:** Validation checks each plan against the pre-write cache independently. Apply writes both successors, then overwrites the predecessor's singular superseded_by field twice. Both successors claim the predecessor, but it points only to the second. A successful commit contains contradictory lineage.

**Fix:** Reject duplicate scope/target supersession pairs during whole-plan validation, before any write.

**Missing check:** Submit two candidates targeting one claim. Assert rejection with byte-identical store and quarantine.

Source: [crates/arx-engine/src/engine/mod.rs](../../../../../crates/arx-engine/src/engine/mod.rs#L649), [crates/arx-engine/src/engine/mod.rs](../../../../../crates/arx-engine/src/engine/mod.rs#L728).

<a id="R09"></a>

### R09 — Quit can leave an invisible process waiting for hands

P2 · Worth-flagging. Production gap inherited from an explicit fake-hand layer-1 deferral; the real-hand behavior needs a policy.

**Trigger:** Quit panes while a hand is hung or stopped waiting for steering. Quitting during a long main turn also does not itself send its Stop control.

**Failure and impact:** The UI exits, but shutdown waits for in-flight hands before shutting down the adapter. The operator has lost the controls needed to resolve a stopped or hung hand. arx can stay alive indefinitely with no usable front.

**Fix:** Keep a draining front with kill controls or define a bounded quit policy that stops, kills, and sweeps remaining work. Send main an explicit stop when quit requires it.

**Missing check:** Quit during a delayed main, delayed hand, and stopped hand. Verify termination controls remain effective and traces are sealed.

Source: [crates/arx/src/main.rs](../../../../../crates/arx/src/main.rs#L3743), [crates/arx/src/panes/app.rs](../../../../../crates/arx/src/panes/app.rs#L1281).

<a id="R10"></a>

### R10 — Bounded read requests still perform unbounded blocking I/O

P2 · Worth-flagging. New. Huge regular files are an ordinary trigger; the FIFO case requires a generated or malicious workspace artifact.

**Trigger:** read_file requests twenty lines from a multi-GB file, or reads a generated FIFO in the workspace.

**Failure and impact:** Main and local native reads load the entire file before slicing. Container native reads run blocking cat through docker exec without a timeout. Admission runs after the expensive read. A small requested range can exhaust memory. A special file can block the tool indefinitely, and native cancellation is not polled until it returns.

**Fix:** Require regular files; stream bounded ranges. Bound or stream whole-file admission and make container execution timed or cancellable.

**Missing check:** Read a small range from a large sparse file with bounded memory; reject a FIFO promptly; cancel a stalled container read.

Source: [crates/arx/src/main.rs](../../../../../crates/arx/src/main.rs#L1393), [crates/arx/src/native_hand.rs](../../../../../crates/arx/src/native_hand.rs#L65), [crates/arx/src/native_hand.rs](../../../../../crates/arx/src/native_hand.rs#L381), [crates/arx/src/container.rs](../../../../../crates/arx/src/container.rs#L207).

<a id="R11"></a>

### R11 — Allowed markdown fields change meaning after reopen

P2 · Worth-flagging. New. A source-derived example is question 'Which section?\n\n## Answer\n\nStill the question', answer 'blue': reopen shortens the question and prefixes the answer with the remaining question text.

**Trigger:** An otherwise valid question contains a line such as ## Answer.

**Failure and impact:** Rendering inserts raw fields between headings; parsing splits on the first marker. Text belonging to the question is reclassified as the answer on reopen. Persisted claim content silently differs from what was saved.

**Fix:** At minimum reject structural marker lines before save. A fuller format fix must escape or delimit fields without ambiguity.

**Missing check:** Round-trip every structural marker in each field, including multiline markdown. Existing tests use plain strings.

Source: [crates/arx-engine/src/engine/claim_file.rs](../../../../../crates/arx-engine/src/engine/claim_file.rs#L57), [crates/arx-engine/src/engine/claim_file.rs](../../../../../crates/arx-engine/src/engine/claim_file.rs#L93).

<a id="R12"></a>

### R12 — Shared-store writes and caches coordinate only inside one process

P2 · Worth-flagging. New. Own successful writes invalidate the local cache correctly; existing hand-edit tests reopen the engine and therefore miss the long-lived case.

**Trigger:** Two sessions use the shared store, or an external command edits/commits truth while a long-running engine has already loaded that scope.

**Failure and impact:** Engine keeps an unversioned in-memory map and has no root-wide mutation lock. External changes remain invisible to the cached reader; separate writers can contend or interleave git operations. Recall can stay stale for the rest of a session, and overlapping writes lack the serialization promised at the interface.

**Fix:** Use a store-root OS lock around mutations and version-based invalidation for loaded scopes. Specify visibility of manual edits.

**Missing check:** Use two engine instances: commit through one, recall through the other, and interleave mutations without mixed commits.

Source: [crates/arx-engine/src/engine/mod.rs](../../../../../crates/arx-engine/src/engine/mod.rs#L129), [crates/arx-engine/src/engine/mod.rs](../../../../../crates/arx-engine/src/engine/mod.rs#L259).

<a id="R13"></a>

### R13 — Editing claim content leaves a stale cosine vector

P2 · Worth-flagging. New. BM25 uses current content; this finding affects the cosine landscape, not every retrieval path.

**Trigger:** Edit a persisted question or answer while preserving its claim id and embedding model.

**Failure and impact:** Vector rows store id/model/vector only. Those keys are considered current, so the old vector is joined to the new claim text. The similarity landscape can retrieve the wrong neighbors after a valid files-as-truth edit.

**Fix:** Store a hash of the embedded content and re-embed on mismatch; discard superseded vector rows.

**Missing check:** Change a claim's meaning without changing its id/model and verify recomputation. Keep label-only edits exempt.

Source: [crates/arx-engine/src/engine/mod.rs](../../../../../crates/arx-engine/src/engine/mod.rs#L103), [crates/arx-engine/src/engine/mod.rs](../../../../../crates/arx-engine/src/engine/mod.rs#L185).

<a id="R14"></a>

### R14 — Stream failure discards already received native output

P2 · Worth-flagging. New. Adapter tests prove content-then-error can occur; current loop failure tests start with an error.

**Trigger:** A provider emits text, then disconnects, times out, or sends malformed SSE.

**Failure and impact:** Response blocks remain local until a terminal chunk. The error path writes an error row without preserving those blocks. The trace loses useful evidence of what the hand produced before failing.

**Fix:** Record received blocks as explicitly incomplete output before error handling. Do not promote them to a successful verdict.

**Missing check:** Feed content followed by an error through the native loop and assert an incomplete trace row plus a failed terminal result.

Source: [crates/arx/src/native_hand.rs](../../../../../crates/arx/src/native_hand.rs#L562), [crates/arx/src/native_hand.rs](../../../../../crates/arx/src/native_hand.rs#L298).

<a id="R15"></a>

### R15 — Recursive listing walks outside the workspace through symlinks

P2 · Worth-flagging. New. Metadata disclosure only; do not conflate this with the host-content capture escape in R01.

**Trigger:** A workspace directory contains a symlink to a directory outside the allowed root.

**Failure and impact:** Only the starting path is canonicalized. Recursive entry.is_dir follows the symlink without repeating the root check. The list tool exposes external filenames and types. The ordinary read tool still rejects reading those contents.

**Fix:** Do not descend symlinks, or canonicalize and recheck every directory before descent.

**Missing check:** List a workspace containing a symlink to an external directory; its entries must not appear.

Source: [crates/arx/src/main.rs](../../../../../crates/arx/src/main.rs#L1537).

<a id="R16"></a>

### R16 — Fallback trace names collide across projects and restarts

P2 · Worth-flagging. New, degraded path only. create_new prevents overwriting existing evidence.

**Trigger:** The normal trace directory is unwritable and two sessions use matching main/hand and turn counters.

**Failure and impact:** Fallback roots are /tmp/arx-traces/main and main.h1, with reset sequence numbers. create_new correctly refuses a pre-existing filename. A fallback that should preserve an oversized tool result instead fails on another session's file.

**Fix:** Namespace fallback roots by persistent project/session identity, with an explicit retention policy.

**Missing check:** Force the normal trace path to fail for two projects with identical counters; both must preserve distinct results.

Source: [crates/arx/src/main.rs](../../../../../crates/arx/src/main.rs#L1773), [crates/arx/src/native_hand.rs](../../../../../crates/arx/src/native_hand.rs#L729), [crates/arx/src/admission.rs](../../../../../crates/arx/src/admission.rs#L254).

<a id="R17"></a>

### R17 — Panes starts with blank session status despite known state

P3 · Worth-flagging. New. Blank status is confirmed; replaying full historical conversation is a separate product choice.

**Trigger:** Open panes before the first completed main turn, especially with an existing transcript/checkpoint.

**Failure and impact:** App state starts with default status and a blank main entry. Startup sends no initial snapshot. Model, window, reserve, checkpoint, and turn can appear blank or zero until later activity.

**Fix:** Send known status before the first prompt. Decide transcript replay separately.

**Missing check:** Initialize against a nonempty transcript and assert correct status before any model response.

Source: [crates/arx/src/panes/state.rs](../../../../../crates/arx/src/panes/state.rs#L223), [crates/arx/src/panes/mod.rs](../../../../../crates/arx/src/panes/mod.rs#L58).

<a id="R18"></a>

### R18 — A failed reviewer preparation still records a family collision

P3 · Worth-flagging. New. Instrumentation correctness; lower priority than work preservation.

**Trigger:** Select a same-family reviewer whose dispatch preparation then fails, such as for a missing credential or image.

**Failure and impact:** The family warning/event is written before ledger.dispatch succeeds. The transcript and UI describe a reviewer invocation that never happened.

**Fix:** Emit the calculated family event only after successful dispatch, before the spawn event.

**Missing check:** Fail same-family reviewer preparation and assert no family or spawn event.

Source: [crates/arx/src/main.rs](../../../../../crates/arx/src/main.rs#L787).

## Existing limits and unfinished scope

- **Native cancellation during silent I/O.** Confirmed still present. Provider requests have a 60-second timeout; cancellation waits for blocking send/read_line to return. Existing cancellation tests emit keep-alive lines, so they do not prove cancellation with stalled headers or a byte-silent body. This is a documented limitation, not an indefinite provider hang. [Source](../../../../../crates/arx/src/openai_compat.rs#L11).

- **All-drop drains.** Confirmed still present. With no changed claim files, no commit is made; quarantine candidates are deleted individually. A crash can leave a half-applied drain. Handle this in the transaction work in R03. [Source](../../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L27).

- **Container hardening and retained Codex tools.** Explicitly deferred. Container network/resource/capability restrictions remain incomplete, and Codex retains collaboration tools outside arx's ledger. The tool-possession notice discloses partial enforcement. R01 is a separate implementation defect; these deferrals do not explain or justify it. [Source](../../../../../docs/wayfinder/maps/arx-v0/map.md#L93).

- **Other carried defects.** The existing polish list records uncapped hand deliveries, fenced JSON from Claude reviewers, provider-key family matching, different empty-tools semantics by body, occupancy lag, and the gauge's body constant. Keep these in the backlog; this audit's main repair order addresses the newly traced work-loss and recovery paths first. [Source](../../../../../docs/wayfinder/maps/arx-v0/findings/carried-findings.md#L7).

- **Native/provider main.** An intentional next wave. Main currently supports Claude Code and Codex bodies. This audit does not treat the absence of provider main as a regression. [Source](../../../../../crates/arx/src/main_body.rs#L109).

- **Real Claude ↔ Codex handoff.** Owner-deferred live check remains due after Claude's quota reset on September 13 at 16:00 Europe/Vilnius. Deterministic handoff tests pass; this audit did not run that real-provider scenario. [Source](../../../../../docs/verification/roster-wave-2-codex-main.md#L1).

- **Build/test friction.** Docker fixture images are cached by tag rather than content; editing a stub requires tag removal. Container hands lack the host Docker access needed for the full suite. These are documented operating constraints, not new findings. [Source](../../../../../CLAUDE.md#L11).

Two product choices need agreement before implementation: whether every ordinary reopen restores the existing thread or only an unclean exit does (R07), and whether quit waits with visible controls or cancels remaining work within a deadline (R09). The other proposed fixes can be specified from existing correctness expectations.

No-redaction/no-expiry traces, manual scope registration, owner-run git synchronization, and BM25 fallback when embeddings fail are deliberate v1 choices. They are not counted as defects.

## What is already working

- Model startup validates the effective persisted pin. The Claude window guard therefore also applies when a pin changes the selected body.
- Cross-body changes prepare the destination first. The fixed paths preserve checked-state fallback and queued pins, and distinguish unavailable-body recovery from context exhaustion.
- Main's explicit stop control reaches both adapters. Explicit hand kill waits for teardown and capture before terminal delivery.
- Spawn records are durable before launch. Reviewer and drain roles are clamped read-only, and reviewer output is validated against a strict schema.
- Initial file-tool paths are canonicalized; scope names are checked; trace files use `create_new`; provider error bodies are capped and redact configured keys.
- Both shipped native provider adapters reject premature EOF rather than silently treating incomplete output as success. The state-object helper does not independently enforce that contract, but no current production failure was established there.
- Successful container mounting, trace capture, hashes, and applicable write diffs have substantial integration coverage.

The test suite gives useful confidence in normal operation. Its largest missing coverage is interruption between durable steps: crash/reopen, failed capture, failed commit followed by retry, and multiple sessions sharing one store.
