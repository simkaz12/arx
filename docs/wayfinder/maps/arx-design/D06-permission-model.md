---
id: D06
labels: wayfinder:grill-with-docs
status: closed
assignee: none
resolved: 2026-08-29
blocked-by: none
grounding: findings/T05-findings.md
---

# D06 — The permission model

> **Closed 2026-08-29** after one grill, two rounds, eight questions, sol silent on both — see Resolution. Round 2's last question fell outside the ticket: the owner's daily shape is Claude Code as the deciding loop with arx as its hands, which opens **D16** rather than reopening anything here.

## Question

T05 gives the taxonomy: schema-level (tool never enters the completion call), runtime gate (rule table + suspend/resume to a human — all three systems converge on it), OS-level (kernel isolation). Claude Code runs all three; the shell donor runs only the middle one, with `never_allow` tools still model-visible — the arrangement OpenDev argues against. To settle:

- **The layering.** Which of the three levels does arx build, and in what order of trust? OpenDev's actual lesson is narrower than SURVEY's paraphrase — schema separation for the read/write axis, runtime approval kept in parallel — so the question is which axes get structural enforcement and which get rules.
- **Schema ownership.** Is a tool's presence in the completion call a harness-owned protocol fact or an admission-owned worth judgment? Per-mode static schemas (a read-only mode whose interface contains no write tools) versus one registry with per-call classification — and does Rust's type system get to make illegal dispatches unrepresentable, or does MCP's runtime-built tool list make that over-fit?
- **The approval channel.** The suspend/resume shape (keyed one-shot, blocking prompt) is convergent across the field. Is it part of the tool-dispatch seam or a second control-inversion point — and how does it compose with the subagent escalation gap D04 carries?
- **Persistence.** Claude Code persists approvals per repo; Goose caches per session. Where does arx's permission memory live — its own artifact, or behind the memory trait as a specialized recall?
- **Precedence and the second model call.** Fixed deny→ask→allow (Claude Code) vs inspector-override (Goose); and both incumbents put a second model call inside the permission decision. Does arx ever call a model to judge a tool call, and does the no-LLM guardrail — scoped to a store — extend to dispatch?
- **Sandbox.** None (Goose), Bash-only (Claude Code), or wrapping subagent execution as the isolation unit? What does the daily-driver posture actually need on this machine?

**Gained from the seats/thread session (2026-08-25), reframed.** That session ruled in passing that isolation couples to the write axis (`f4e08417`) — argued at the time as structural per seat: write tools absent from a read-only seat's surface, worktree isolation demanded by write capability, writer exclusivity the degenerate case. Seats are gone (2026-08-26 trim; `map.md:156`), so "per seat" has no referent. The write-axis coupling itself does not obviously fall with it — the current participant set is main plus dispatched hands (`da0ccf24`, `749c8765`), and a dispatched hand is exactly the kind of thing a read/write split and a worktree-per-writer rule could still bind. Whether it does, and whether "structural" still means what it meant when a seat's tool surface was a static template rather than whatever a given dispatch composes, is this ticket's to re-argue rather than inherit. `findings/fusion-harness-trace.md` is still field evidence for the layering bullet: structural read-only allowlists beside an advisory-only no-background-jobs clause — the enforcement gap containers close by teardown.

## Resolution

One grill, two rounds, nine rulings, sol silent on both rounds. The feel-first input was the owner's own Claude Code configuration: eleven bare-name schema denies, `skipDangerousModePermissionPrompt: true`, no allow or ask block, and every Bash call chained through dippy (a parser-based allow-or-deny gate) before bashfilter's rewrite. What the owner runs daily is schema removal plus a deterministic deny table, with the interactive prompt off — and every bullet closed against that.

- **The layering** (`ce46b9ea`, `ebe94e38`, `cb4b1d07`). Main gets schema-level (its startup surface) plus one pre-dispatch gate returning allow / deny / ask; the ask branch is built and ships with no rules — owner: "its not needed but option is good" — so the daily posture is deny-only. No sandbox on main: its Bash runs on the host under the deny table, as Claude Code does today. Hands get all three levels: schema-level through a static surface per agent definition, the same gate, and a container for every hand regardless of the write axis (teardown is what closes fusion-harness's background-process hole), with a worktree only when write-enabled. The read/write axis is the only axis enforced structurally; anything finer is a rule. `f4e08417`'s seat-worded coupling is superseded by `ebe94e38`, its substance intact per hand.
- **Schema ownership** (`86875715`). Harness-owned configuration: startup config for main, the agent definition for a hand, prepended by the assembler as identity. Admission judges results, never tools. No Rust closed enum of permitted actions — the registry is MCP-built at runtime, so the surface is a set of names built once at spawn with no method that adds to it. Owner: "no need to overcomplicate here."
- **The approval channel** (`ce46b9ea`, `96d1ad3a`). One shape, two receivers: the gate's ask suspends the step for the party above — the operator for main, main for a hand (the child-initiated stop of `5e8da52e`). An arx-native hand's gate runs in arx on the host before the call executes in the container. A Claude-Code-bodied hand is clamped at spawn and its own gate runs inside; under the Agent SDK `CanUseTool` becomes the stop-shaped request, under plain `claude -p` there is no callback so a shortfall fails loud. The edge D05 left here dissolves: the reply to a stop was never a steer — it is the gate's own return, landing at the held boundary for a native hand and as the callback return for an SDK body.
- **Persistence** (`91d75ffe`). Rules live in arx's own config, user tier and repo tier with repo winning — never the store, because a rule has no question, answer or why. An in-session "always" appends a line to the repo file; a one-time answer is session-only.
- **Precedence and the second model call** (`799b0967`). Not-in-surface, then deny → ask → allow, first match wins, nothing overrides. No built-in danger list — the table ships empty. The gate speaks Claude Code's PreToolUse hook contract, so bashfilter-chain and dippy run unchanged as arx's Bash rule. No prompt-injection scanner and no model call in the decision: the no-LLM guardrail extends from the store to dispatch.
- **Sandbox.** Already ruled by `21e8f9a9` (arx on the host, hands in containers); this grill added only the main side (`cb4b1d07`, none) and the every-hand clause (`ebe94e38`).

Vocabulary in `CONTEXT.md`: Tool surface, Gate, Rule, Body, Judge added; Assembler corrected ("the mode's tool surface" → "the tool surface", mode having been retired 2026-08-26); Main amended to name itself a role filled by a body.

**Opened, not closed:** the owner's daily shape — "think/plan with CC CLI, arx as hands, Codex CLI as cross-model judge", each role configurable as to model and/or CLI (`568f864d`). It lands against `7d1f7da4`, `41bf6e4f`, `0be93430` and `ca15f9eb`, and by the map's own rule that re-opens as a fresh ticket, **D16**, rather than being absorbed here. Every D06 ruling binds arx's hand-runner under either shape; the main-side rulings apply when arx's own main is in the seat.
