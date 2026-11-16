# Backlog coverage — 2026-09-12

This is the charting inventory, not a status ledger. Follow each linked card for current stage, claim and evidence. Historical unchecked bullets are not automatically current defects, and an unchecked bullet whose prose says fixed is not reopened by this board.

## September 12 audit

Every R finding has a corresponding task. The original report remains in [project-review-2026-09-12.md](project-review-2026-09-12.md).

| Finding | Task |
|---|---|
| R01 | [Reject output symlinks during capture](../R01-reject-output-symlinks.md) |
| R02 | [Preserve hand work when capture fails](../R02-preserve-failed-hand-captures.md) |
| R03 | [Make interrupted drains recoverable](../R03-recover-interrupted-drains.md) |
| R04 | [Isolate quarantine by project and thread](../R04-isolate-project-thread-quarantine.md) |
| R05 | [Reconcile orphaned hands after restart](../R05-recover-orphaned-hands.md) |
| R06 | [Reopen transcripts with a torn final write](../R06-reopen-torn-transcripts.md) |
| R07 | [Restore main from arx state after restart](../R07-restore-main-after-restart.md) |
| R08 | [Reject duplicate supersession targets](../R08-reject-duplicate-supersession-targets.md) |
| R09 | [Keep quit controllable while work is active](../R09-make-quit-controllable.md) |
| R10 | [Bound file reads before admission](../R10-bound-file-read-io.md) |
| R11 | [Preserve claim markdown across reopen](../R11-round-trip-claim-markdown.md) |
| R12 | [Coordinate shared-store engines and caches](../R12-coordinate-shared-store-engines.md) |
| R13 | [Invalidate vectors when claim content changes](../R13-invalidate-stale-claim-vectors.md) |
| R14 | [Retain incomplete provider output in traces](../R14-retain-partial-provider-output.md) |
| R15 | [Keep recursive listing inside the workspace](../R15-keep-file-listing-in-workspace.md) |
| R16 | [Isolate fallback traces by project and thread](../R16-isolate-fallback-trace-paths.md) |
| R17 | [Initialize panes with known session status](../R17-initialize-panes-session-status.md) |
| R18 | [Record reviewer family only after dispatch succeeds](../R18-record-family-after-reviewer-spawn.md) |

## Existing carried findings

Source: [the original polish list](../../arx-v0/findings/carried-findings.md). Every bullet, including the uncheckboxed roster notes and explicitly closed items, is accounted for below.

| Original point | Board disposition |
|---|---|
| [Uncapped hand verdict delivery](../../arx-v0/findings/carried-findings.md#L9) | [Bound hand verdicts before delivery](../E01-bound-hand-delivery.md) |
| [Preview counts differ from trace-body counts](../../arx-v0/findings/carried-findings.md#L10) | [Distinguish trace-file counts from body counts](../E02-clarify-trace-counts.md) |
| [arx_ping gate configuration is inert](../../arx-v0/findings/carried-findings.md#L11) | [Make arx_ping gating configuration truthful](../E03-make-ping-gating-truthful.md); recheck after tool-possession changes. |
| [Native roster hides its default tools](../../arx-v0/findings/carried-findings.md#L12) | [Agree and apply empty-tools semantics across bodies](../E18-align-empty-tools-semantics.md) includes a current roster disclosure check. The named hand-ds definition no longer exists; likely superseded, not declared fixed without evidence. |
| [Tap promise differs from delivered result](../../arx-v0/findings/carried-findings.md#L13) | [Align tap with its published contract](../E04-align-tap-contract.md) |
| [Real Agent SDK full-window rejection unseen](../../arx-v0/findings/carried-findings.md#L17) | [Capture a real Claude full-window rejection](../E05-observe-claude-window-rejection.md); obtain actual evidence through a bounded authorized opportunity. |
| [Occupancy lags a long reply](../../arx-v0/findings/carried-findings.md#L18) | [Include the current reply in occupancy](../E06-account-reply-occupancy.md) |
| [Gauge prints body constant as window](../../arx-v0/findings/carried-findings.md#L19) | [Show configured window and reserve in gauge](../E07-show-configured-gauge-limits.md) |
| [Reset fired emission meaning is unsettled](../../arx-v0/findings/carried-findings.md#L20) | [Settle the meaning of the reset fired event](../E08-settle-reset-fired-event.md) |
| [PID-reuse guard regression cannot force the race](../../arx-v0/findings/carried-findings.md#L21) | Retained evidence limitation of the primitive. No new fix is claimed or speculative race test required; revisit only if a stronger observable check is identified. |
| [Native cancellation waits on blocking reads](../../arx-v0/findings/carried-findings.md#L22) | [Make native cancellation interrupt silent I/O](../E09-cancel-byte-silent-native-io.md); current provider timeout bounds silent network I/O to 60 seconds. |
| [Fresh workspace scope-refusal text](../../arx-v0/findings/carried-findings.md#L26) | Already closed by L3-05 in the original record. Manual scope registration remains intentional. |
| [All-drop drain deletes candidates without a commit](../../arx-v0/findings/carried-findings.md#L27) | [Make interrupted drains recoverable](../R03-recover-interrupted-drains.md) owns the shared drain recovery path; no duplicate card. |
| [Claude reviewer fences its JSON](../../arx-v0/findings/carried-findings.md#L28) | [Handle fenced Claude reviewer verdicts](../E10-handle-fenced-reviewer-json.md) |
| [Provider family varies with config key](../../arx-v0/findings/carried-findings.md#L29) | [Resolve provider family independently of its config key](../E11-resolve-provider-family.md); separate from R18's event-ordering fix. |
| [Bad drain definition lacks persisted refusal check](../../arx-v0/findings/carried-findings.md#L30) | [Persist coverage for invalid drain definitions](../E12-cover-invalid-drain-body.md) |
| [Hands cannot execute the Docker-backed suite](../../arx-v0/findings/carried-findings.md#L34) | [Make hand verification capabilities explicit](../E13-verify-from-container-hands.md) includes checking actual image toolchains and choosing the verification route. |
| [Docker stub images remain stale after script edits](../../arx-v0/findings/carried-findings.md#L35) | [Refresh Docker fixture images when their content changes](../E14-refresh-stub-images-by-content.md); the documented manual rebuild rule still applies until changed. |
| [Event-kind enumeration omits miss](../../arx-v0/findings/carried-findings.md#L36) | [Correct the live event-kind documentation](../E15-correct-event-kind-documentation.md) |
| [Bare config_dir path bug](../../arx-v0/findings/carried-findings.md#L37) | The original record explicitly says fixed, despite leaving its checkbox unchecked. No new card. |
| [Reviewer used stash while read-only](../../arx-v0/findings/carried-findings.md#L41) | [Check the reviewer's read-only workflow](../X01-check-reviewer-read-only-workflow.md); canonical correction belongs in skillbook. |
| [Runner stalls without explicit review polling](../../arx-v0/findings/carried-findings.md#L42) | [Make runner review polling self-contained](../X02-make-runner-poll-its-review-round.md); canonical correction belongs in skillbook. |
| [Runner leaves wait/poll shells alive](../../arx-v0/findings/carried-findings.md#L43) | [Clean up runner-owned wait processes](../X03-clean-up-runner-wait-processes.md); canonical correction belongs in skillbook. |
| [Engine tests leak temporary roots](../../arx-v0/findings/carried-findings.md#L44) | [Clean temporary roots owned by engine tests](../E16-clean-engine-test-temp-roots.md); revalidate against current helpers. |
| [Codex can dispatch outside arx's ledger](../../arx-v0/findings/carried-findings.md#L46) | [Keep Codex child dispatch inside arx's ownership](../E17-contain-codex-child-dispatch.md) preserves the explicit deferral. |
| [Empty tools means different things per body](../../arx-v0/findings/carried-findings.md#L47) | [Agree and apply empty-tools semantics across bodies](../E18-align-empty-tools-semantics.md) |
| [gate::append_rule has no production caller](../../arx-v0/findings/carried-findings.md#L48) | [Resolve the unused gate rule writer](../E19-resolve-unused-gate-rule-writer.md) |

## Current map frontier and other unfinished work

| Point | Board disposition |
|---|---|
| Killed write-hand diff is not named on the operator surface | [Preserve hand work when capture fails](../R02-preserve-failed-hand-captures.md) now includes making its sealed diff findable from terminal delivery. [Original note](../../arx-v0/map.md#L260). |
| Container network/capability/resource restrictions | [Define container hardening for live hands](../E20-define-container-hardening.md) preserves the explicit deferral. It is distinct from R01's host-read escape. |
| Retained Codex collaboration tools | [Keep Codex child dispatch inside arx's ownership](../E17-contain-codex-child-dispatch.md); listed once even though it occurs in both the map and carried findings. |
| Provider-backed main, wave 3 | [Scope provider-backed main for wave 3](../E21-scope-provider-main-wave-3.md); intentional next feature, not a defect in Codex main. |
| Real Claude and Codex handoff acceptance | [Verify the real Claude and Codex handoff](../E22-verify-live-cross-body-handoff.md); verification-only, after the recorded quota reset and actual account availability. |
| Intermittent test failures | [Diagnose intermittent test isolation failures](../E23-diagnose-test-isolation.md); the observed lock failure is not treated as fixed because it passed on reruns. |
| Daily-driver cutover | [L3-04 remains the canonical open HITL gate](../../arx-v0/L3-04-daily-driver-cutover.md). No second cutover card is created. |

## Retired incidents and stale roadmap prose

- Fresh-scope refusal text and the bare-config-path bug are explicitly recorded fixed above.
- The old native default-tool disclosure observation is checked under E18, rather than declared current based on a removed agent definition.
- Old fog entries for Codex main and a richer terminal are superseded by the landed Codex main and panes work. They do not create new feature cards.
- The sidecar dependency and accidental sparse-checkout incidents were repaired in their original session. Retained Codex child dispatch is still represented by E17; the repaired incidents are not reopened.
- The screenshot warning leak and false-floor reporting were fixed before this board was charted. Their verification is in the audit snapshot; they are not new Done cards claiming to have traversed flow.

## Deliberate constraints, not missing implementations

No trace redaction/expiry, manual scope registration, owner-run store git synchronization and BM25 fallback on embedding failure are existing v1 choices. They remain documented in the [glossary](../../../../../CONTEXT.md) and audit. A future owner request can reopen their scope; this inventory does not do so.

The PID-reuse test's limited power is retained as an evidence constraint above. No current production failure was established for the state-object helper's missing explicit terminal check because both shipped provider adapters reject premature EOF.

## Source and authority

The inventory used the current carried list, map tail, open L3-04 card, Codex-main verification record and audit snapshot. The new cards preserve proposed checks and named uncertainties. They do not silently decide product policy, erase previous deferrals or authorize building every listed item.

