---
id: D12
labels: wayfinder:grill-with-docs
status: closed
assignee: none
closed: 2026-08-29
blocked-by: none
grounding: findings/debate/2026-08-26-arx-plans/ledger.md
---

# D12 — The capture sweep's rules

New ticket, off closed D04. D04 ruled that a child's trace is captured by a body-agnostic sweep of its capture surface, with no uniform shape promised across bodies (`7e5b09d7`), synchronous on the child's return path (`6aab1d24`). It never ruled what the sweep itself is bounded by. The map's own "Capture surface" vocabulary entry names the mechanism; this ticket is what governs it.

## Question

The plans debate found the sweep, as ruled, carries no size limit, no exclusion rule, no retention policy, and no secret-handling rule — and separately, that it sweeps CC session files as part of its capture surface (`S-SOL-6`, plans, agreed, major). Four open questions, none answered by D04:

- **Size.** A child's capture surface can be arbitrarily large — a large file read, extensive tool output, a long transcript. Does the sweep cap what it captures per child, or does the trace archive grow unbounded in proportion to whatever the child happened to touch?
- **Exclusion.** Does the sweep exclude any category of content by kind? The debate's finding specifically names Claude Code session files as something the sweep currently pulls in wholesale when a body reads them — is capturing another agent's full session transcript inside arx's own trace archive intended, or a consequence nobody named on purpose?
- **Retention.** D03's provenance model makes an externalized artifact write-once (`061d7942`); D04 gives a trace the same shape. Write-once says nothing about how long it lives. Is there an expiry or pruning rule, and if not, what is the growth curve once every dispatched hand produces one trace per spawn?
- **Secret handling.** Does the sweep run through the same admission path ordinary tool output does, or is it a raw copy of the capture surface that could carry credentials, tokens, or other material an ordinary admission threshold would have caught or redacted?

None of these were live questions when D04 closed, because D04's own scope was where the trace lives and how the hook fires, not what the sweep is allowed to hold.

## Resolution — 2026-08-29, one grill, three rounds, seven questions, sol silent throughout

Two premises in the charter fell first: the surface is what a child *leaves*, not what it read, and admission is a size threshold that never inspected content, so "admission would have caught a secret" was not a comparison the sweep could lose. With those gone, the four bullets closed against one rule — the designation at spawn is the sweep's only bound — and the secret question moved one seam earlier, to what the container is given.

- **Size.** No per-child cap; the designation is the bound. The trace's frontmatter index records byte counts per path, so growth is an instrumentation line, not a rule. A worktree contributes its diff against the spawn base, never a tree copy — git holds the tree, and the copy was the one genuinely unbounded case (`4e10940c`).
- **Exclusion.** A child's own transcript, in whatever format its body keeps it, is always on the surface: arx's transcript log for a native hand, the session `.jsonl` for a Claude-Code-bodied one, lifted out of the container before teardown. Nothing is excluded by kind because nothing undesignated is ever swept. `S-SOL-6` read the session file as an accident; it is the rule (`e2e92711`).
- **Retention.** No expiry in v1, the store's own no-cleaner posture. The one constraint on a future pruner: never remove a trace whose handle a claim or ember holds. Pruning is an operator-invoked hand ritual, never the harness's (`72f65636`).
- **Secrets.** No redaction; arx redacts nowhere. A trace's trust class is Claude Code's own session file — the owner's disk, outside git and the store, entering a window only by a sliced pull (`cd27a1f6`). What does the work is the container's allowlist, empty by default: no host env, no home; the worktree (read-only when not write-enabled), the output dir, what the agent definition names, the body's own auth as the one mandatory entry, and two pushes at spawn — the store's grounding for the task and the brief with its handles, nothing else. The worktree's gitignored `.env` is the accepted residual (`068d4cbf`, owner's addition: "it should get the context of the store + context for the task").
- **The diff is the deliverable.** Hands never commit and the worktree dies with the container, so the trace's diff is the only copy of the work; main, or the hand main dispatches, applies it from the handle as plain working-tree modifications, never a rebase, never a commit on the hand's behalf. The trim's "hands return worktrees" now reads as "return a trace whose diff is the worktree's" (`4f90f38f`).
- **Store access from a hand.** Native: grounding pushed at spawn, plus `recall` as an ordinary tool on its surface when the agent definition lists it, served by arx's engine on the host through the gate, read only, never a mount. Claude-Code-bodied: the push alone in v1 — a store-reaching MCP surface is D16's seam question (`e79bed85`).

Debate row `S-SOL-6` (plans, agreed, major) retires on the first four bullets. Vocabulary: **Capture surface**, **Trace**, **Dispatch**, **Store** amended in `CONTEXT.md`. Ticket impact: **D16** inherits one edge — whether arx exposes an MCP surface so a Claude-Code-bodied hand can reach the store mid-loop — and D04's close stands, its sweep now bounded.
