---
labels: wayfinder:map
effort: arx-reliability
charted: 2026-09-12
tracker: local-markdown
---

# arx reliability board

## Destination

Resolve the September 12 audit's failure paths and the existing unfinished work through bounded flow cards. Correctness work needs executable evidence; open product choices need an owner answer. Keep intentional deferrals visible without treating them as completed repairs.

This board is the local tracker for this effort. There are **41 arx cards and 3 external follow-ups**, plus the existing daily-driver gate linked below. The [audit snapshot](findings/project-review-2026-09-12.md) and [backlog coverage](findings/backlog-coverage.md) preserve the evidence and account for every carried point.

## Board

Ready means ready for the card's named next activity, which can be a focused trace or to-spec. It does **not** mean that an execution brief exists. Needs check contains older observations whose current status has not been established. Needs decision waits for an owner expectation. Blocked names a predecessor, time gate or missing evidence. Deferred preserves an explicit earlier deferral.

Current queue: 13 ready · 12 need checking · 8 need a decision · 6 blocked · 2 deferred. R02 implementation is authorized and awaits a coherent main base. No card is claimed.

| Ready | Needs check | Needs decision | Blocked | Deferred |
|---|---|---|---|---|
| — | [P1 · E01 — Bound hand verdicts before delivery](E01-bound-hand-delivery.md) | [P1 · R04 — Isolate quarantine by project and thread](R04-isolate-project-thread-quarantine.md) | [P1 · R01 — Reject output symlinks during capture](R01-reject-output-symlinks.md) | [P1 · E17 — Keep Codex child dispatch inside arx's ownership](E17-contain-codex-child-dispatch.md) |
| [P1 · R03 — Make interrupted drains recoverable](R03-recover-interrupted-drains.md) | [P2 · E04 — Align tap with its published contract](E04-align-tap-contract.md) | [P1 · R07 — Restore main from arx state after restart](R07-restore-main-after-restart.md) | [P1 · R05 — Reconcile orphaned hands after restart](R05-recover-orphaned-hands.md) | [P1 · E20 — Define container hardening for live hands](E20-define-container-hardening.md) |
| [P1 · R06 — Reopen transcripts with a torn final write](R06-reopen-torn-transcripts.md) | [P2 · E06 — Include the current reply in occupancy](E06-account-reply-occupancy.md) | [P2 · R09 — Keep quit controllable while work is active](R09-make-quit-controllable.md) | [P1 · E22 — Verify the real Claude and Codex handoff](E22-verify-live-cross-body-handoff.md) | — |
| [P1 · R08 — Reject duplicate supersession targets](R08-reject-duplicate-supersession-targets.md) | [P2 · E10 — Handle fenced Claude reviewer verdicts](E10-handle-fenced-reviewer-json.md) | [P2 · E09 — Make native cancellation interrupt silent I/O](E09-cancel-byte-silent-native-io.md) | [P2 · R16 — Isolate fallback traces by project and thread](R16-isolate-fallback-trace-paths.md) | — |
| [P2 · R10 — Bound file reads before admission](R10-bound-file-read-io.md) | [P2 · E11 — Resolve provider family independently of its config key](E11-resolve-provider-family.md) | [P2 · E13 — Make hand verification capabilities explicit](E13-verify-from-container-hands.md) | [P2 · E05 — Capture a real Claude full-window rejection](E05-observe-claude-window-rejection.md) | — |
| [P2 · R11 — Preserve claim markdown across reopen](R11-round-trip-claim-markdown.md) | [P2 · E23 — Diagnose intermittent test isolation failures](E23-diagnose-test-isolation.md) | [P2 · E18 — Agree and apply empty-tools semantics across bodies](E18-align-empty-tools-semantics.md) | [P1 · R02 — Authorized; base pending](R02-preserve-failed-hand-captures.md) | — |
| [P2 · R12 — Coordinate shared-store engines and caches](R12-coordinate-shared-store-engines.md) | [P3 · E02 — Distinguish trace-file counts from body counts](E02-clarify-trace-counts.md) | [P2 · E21 — Scope provider-backed main for wave 3](E21-scope-provider-main-wave-3.md) | — | — |
| [P2 · R13 — Invalidate vectors when claim content changes](R13-invalidate-stale-claim-vectors.md) | [P3 · E03 — Make arx_ping gating configuration truthful](E03-make-ping-gating-truthful.md) | [P3 · E08 — Settle the meaning of the reset fired event](E08-settle-reset-fired-event.md) | — | — |
| [P2 · R14 — Retain incomplete provider output in traces](R14-retain-partial-provider-output.md) | [P3 · E07 — Show configured window and reserve in gauge](E07-show-configured-gauge-limits.md) | — | — | — |
| [P2 · R15 — Keep recursive listing inside the workspace](R15-keep-file-listing-in-workspace.md) | [P3 · E12 — Persist coverage for invalid drain definitions](E12-cover-invalid-drain-body.md) | — | — | — |
| [P3 · R17 — Initialize panes with known session status](R17-initialize-panes-session-status.md) | [P3 · E14 — Refresh Docker fixture images when their content changes](E14-refresh-stub-images-by-content.md) | — | — | — |
| [P3 · R18 — Record reviewer family only after dispatch succeeds](R18-record-family-after-reviewer-spawn.md) | [P3 · E16 — Clean temporary roots owned by engine tests](E16-clean-engine-test-temp-roots.md) | — | — | — |
| [P3 · E15 — Correct the live event-kind documentation](E15-correct-event-kind-documentation.md) | — | — | — | — |
| [P3 · E19 — Resolve the unused gate rule writer](E19-resolve-unused-gate-rule-writer.md) | — | — | — | — |

## In flight

| Active | Review | Verify | PR open | Done |
|---|---|---|---|---|
| — | — | — | — | — |

Active includes the focused trace, grilling, briefing or authorized build named by the card's `next` field. Review and Verify are agent handoffs. PR open means those checks passed and the work awaits the owner's review; it is not Done. There are no completed flow cards in this new effort; the screenshot fixes shipped in the working tree before the board was created and are recorded only in the audit snapshot.

## How flow takes work

A named card is a direct target:

```text
flow docs/wayfinder/maps/arx-reliability/R02-preserve-failed-hand-captures.md
```

To authorize a complete implementation, say:

```text
Implement the next ready P1 card from docs/wayfinder/maps/arx-reliability/map.md through flow.
```

Without a named card, take the first unclaimed, unblocked eligible P1 card above, then P2/P3. R02 has been briefed and is waiting on its execution gates. The next ready card is [Make interrupted drains recoverable](R03-recover-interrupted-drains.md). During an AFK run, skip owner decisions, explicit deferrals, external follow-ups and unmet gates. Do not silently resolve their choices.

1. Read this map, the selected card and its linked evidence. Recheck only facts made stale by the chosen base or named as gaps.
2. Claim the card by setting `assignee` before work; set `stage: active` and keep `next` on the current flow activity. Update the board index in the same handoff.
3. Missing facts go to a bounded hand-ex trace. Open expectations go to grill-me. Settled work goes to to-spec. Record only settled choices and the next named gap in the card's Handoff.
4. Link the resulting brief in the card. With implementation authorized, main creates the task's own branch from `main` before dispatching a fresh spec-runner, then an independent spec-reviewer using the same agreement. Pass exact owned files and warn workers about concurrent edits.
5. Reviewer PASS advances to `stage: verify` and flow's verify skill. Link its verdict and evidence. Failed checks return only to the named fact, decision or local correction; do not repeat the whole audit.
6. After verification passes, commit the task's scoped changes, push its branch and open a PR targeting `main` under the processing rules below. Record the PR URL and verified revision in the card; set `status: open`, `stage: pr-open`, `next: owner-review`, and release the active claim. Continue with another eligible independent card; leave this PR open.
7. Move an implemented card to `status: closed`, `stage: done` only after the owner has merged its PR and that merge is confirmed. Add the resolution link below. A closed but unmerged PR is not a completed task. If a carried point needs no change because it is already resolved or refuted, link the proof and label that resolution instead of claiming a new implementation.

Card frontmatter owns status, stage, assignee, dependencies and artifact links. This board is their human index, updated with each transition; flow reads the card before claiming it if the index is stale. There is no separate flow log or runner queue.

This request authorizes creating the board. A later instruction to implement a card or an eligible set of cards authorizes execution at that scope; it need not be requested again at every handoff. A brief alone does not expand that scope.

## Task processing and PR handoff

Owner instruction, 2026-09-12: each task uses its own branch and a PR to `main`; the owner will review later. For this effort, these rules replace the repository's automatic card-landing workflow, including its automatic merge and branch deletion.

- Create one branch per task from `main`, named after its brief without the date or extension. Use an isolated worktree when needed to preserve other work. Runners, reviewers and hands do not switch branches or commit; main coordinates the branch and PR.
- Keep the branch and PR scoped to that task, its brief, tests and tracking updates. Stage by explicit path; do not sweep unrelated dirty files into a commit. Push normally, without force or history rewriting.
- Once the runner, independent reviewer and verify step pass, main may commit, push and open the PR without another permission request. The PR description includes the problem and resulting behavior, card and brief links, verification commands/results, reviewer verdict and any remaining limits.
- Record `pr` and `verified-revision` in the card alongside the existing brief and verification links. Keep it in PR open while the owner reviews. Later behavior changes require the affected review and verification checks again; update the same task PR.
- **Do not merge the PR, merge the task branch into `main`, enable auto-merge, or delete the branch.** Passing checks and an available next card do not authorize merging. The owner's review and merge remain the final handoff.
- A prerequisite PR being open does not unblock implementation that depends on it. Wait until the owner merges it into `main`, then branch the dependent task from the updated base. Continue independent cards meanwhile; do not merge or cherry-pick pending task branches to bypass the dependency.
- If push or PR creation fails, preserve the branch and commits, record the error and keep the card open. Do not mark it PR open without a real PR URL or Done without the required resolution.

## Dependencies and parallel work

Only four logical dependency groups are known:

- [Reject output symlinks during capture](R01-reject-output-symlinks.md) needs [Preserve hand work when capture fails](R02-preserve-failed-hand-captures.md).
- [Reconcile orphaned hands after restart](R05-recover-orphaned-hands.md) needs the same safe capture result.
- [Restore main from arx state after restart](R07-restore-main-after-restart.md) needs [orphan reconciliation](R05-recover-orphaned-hands.md) and [torn-tail recovery](R06-reopen-torn-transcripts.md) before implementation can complete. Its reopen semantics may be discussed earlier.
- [Isolate fallback traces by project and thread](R16-isolate-fallback-trace-paths.md) needs [project/thread quarantine identity](R04-isolate-project-thread-quarantine.md).

[Real Claude and Codex handoff verification](E22-verify-live-cross-body-handoff.md) is blocked until 2026-09-13 16:00 Europe/Vilnius and actual account availability. [Real Claude full-window rejection](E05-observe-claude-window-rejection.md) needs a safely reproducible vendor signal; do not create an expensive probe by default.

Shared files are scheduling constraints, not dependency edges:

| Scheduling group | Main overlap |
|---|---|
| Capture and cleanup | R01, R02, R05, R10; E13, E20 |
| Store | R03, R04, R08, R11, R12, R13; E16 |
| Main lifecycle and tools | R04, R07, R09, R10, R15, R16, R18; E01, E03, E06, E07 |
| Native and admission | R10, R14, R16; E01, E02, E09 |
| Panes and operator surface | R09, R17; E04, E07 |
| Roster and body interfaces | E10, E11, E17, E18, E21 |
| Test infrastructure | E12, E14, E16, E23 |

Independent read-only traces can run together. Serialize writers sharing files in one working tree, or use isolated card worktrees with a coherent base and planned integration. Do not use stash or revert another worker's work to make a dispatch fit.

## Baseline and repository rules

The audit describes a dirty tree based on `3c92977302a28a8accec18b79c5434a74a82b299` on main. That commit alone is not the audited implementation. Before an implementation branch is created, main must establish a coherent base containing the relevant roster/reviewer/Codex changes and preserve the owner's other uncommitted work. Do not treat a missing pre-existing change as a new defect.

This is an execution-map override of wayfinder's decision-only default: these cards can carry repairs through flow once implementation is authorized. They remain task cards until to-spec creates a bounded brief. Split a card if investigation shows it will not fit one build window; retain its ID as the parent and link the split.

The task processing and PR handoff rules above override automatic landing for this effort; the other repository rules still apply. A change to main's experience updates `prompts/main.md` in that same implementation. Respect Docker stub tag freshness and the `#[path]` test-module hazard. Failure probes use scratch stores, fixtures and workspaces, not the live user store. No new decision overrides a closed ruling merely because the audit recommends a fix.

## Existing gate and external follow-ups

[Daily-driver cutover](../arx-v0/L3-04-daily-driver-cutover.md) remains open and HITL in its existing map. It is not copied here. Its existing “no correctness finding remains” condition means applicable repair evidence must be accounted for before it closes; wants and intentional deferrals follow the owner's scope rulings. Flow can target that original card for the eventual live acceptance.

The following items belong in skillbook and are not arx implementation candidates:

| Follow-up | Next activity | Routing |
|---|---|---|
| [Check the reviewer's read-only workflow](X01-check-reviewer-read-only-workflow.md) | Focused trace | skillbook |
| [Make runner review polling self-contained](X02-make-runner-poll-its-review-round.md) | Focused trace | skillbook |
| [Clean up runner-owned wait processes](X03-clean-up-runner-wait-processes.md) | Focused trace | skillbook |

Each is a transfer pointer until the receiving project creates its canonical task. After transfer, the receiving tracker owns progress.

## Resolutions

None. Charting these cards does not resolve them.

## Not yet specified

The owner answers on project/thread identity, ordinary reopen behavior, shutdown policy, empty tools and provider main will determine any additional splits. Container hardening and Codex collaboration containment remain deferred pending explicit resumption. Their cards name the first question rather than invent a full implementation plan.

## Intentional limits and retired observations

The [coverage register](findings/backlog-coverage.md) accounts for already-closed incidents, likely stale observations and deliberate v1 constraints. They stay visible without entering the implementation frontier as fresh defects.
