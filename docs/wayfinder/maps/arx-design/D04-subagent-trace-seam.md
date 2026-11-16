---
id: D04
labels: wayfinder:grill-with-docs
status: closed
assignee: none
blocked-by: none
grounding: findings/T02-findings.md, findings/T06-findings.md, findings/fusion-harness-trace.md, findings/t3code-claude-embedding.md
---

# D04 — The subagent↔trace-archival seam, and session persistence

## Question

T02 shows the donor has no subagent seam at all — spawn is a tool call in the same registry as memory, and a finished subagent's transcript persists as an ordinary assertable session row. T06 shows delegation already does most of the mixed-tail-avoidance work in practice. To settle:

- **Where the trace archive lives.** Inside the same session store as ordinary sessions with a stricter read-path gate (Goose's shape plus enforcement), or a structurally separate store that recall/grounding cannot reach by construction? `061d7942`'s "never retrievable as an assertion" — is a listing filter enough, or must the read path itself refuse to hand trace content back as fact?
- **The capture hook.** Trace archival must fire at nested-session end, before discard, ordered against the parent's admission of the verdict (T07 C1). Where is that hook, and what exactly is captured — full transcript, or transcript plus the `(path, sha)` witnesses the freshness rules need?
- **Supervision.** Goose has two disjoint cancellation registries and a background subagent invisible to the generic supervisor. One registry by construction, or foreground/background as two named seams? And does the parent get a cancellation tree (`.child_token()`) or Goose's shared-token shape?
- **Permission escalation.** Goose forces subagents to auto mode because `ActionRequired` doesn't forward to the parent — a gap acknowledged in a comment. Does arx's seam carry a first-class child→parent approval path, and what does the parent's step do while a child waits on one?
- **Schema.** Recursion depth as a type-level property or a runtime check; lineage (`parent_session_id`) from schema v1 or retrofit-acceptable; SQLite as the donor gives it, or shaped by the trace/evidence split from the start?

**Gained from D07 (2026-08-24, decision `e65b2a35`):**

- **Delegate is this ticket's now.** The admission ladder shed its delegate tier — "run the suite, diagnose, fix" was never an admission decision, since no result exists at the point it is made. It is the model dispatching a subagent, which is a tool call. `3e2d9a86` partly existed to enforce the owner's delegation discipline in the harness instead of in the owner's memory; with the tier gone, that enforcement has to be argued here, or at D06's pre-dispatch guard, or conceded back to the prompt. Which of those is the question.
- **The bounded side-context shape.** Raised in grill 2, not ruled: observer, probe, and model-initiated subagent all reduce to *spawn a bounded context, hand it material and a question, get a small answer back* — one deep module with several adapters, or three things that merely rhyme? Two adapters would already have earned the seam.

**Gained from the seats/thread session (2026-08-25, rubber-duck, no ticket) — read `findings/fusion-harness-trace.md` and the twelve session decisions before grilling:**

- **Part-answered in advance.** Seat vs dispatch is ruled (`14aed109`): one spawn primitive, two lifetime classes — the bounded-side-context bullet below is settled at the class level, and what remains is mechanics. The removal law bullet below is spent for seats: removal is teardown plus trace capture, binding the crashed and cancelled child too. Supervision and cancellation should be argued as process lifecycle under `21e8f9a9` (fusion's process-group SIGTERM→SIGKILL shape is donor material; its normal-completion gap — nothing sweeps a backgrounded grandchild when the child returns cleanly — is the counter-example containers answer).
- **The delegate-discipline question inherits `3a873e9a`.** A seat's findings reach the store only through the worker's window; whatever enforcement the grill lands on must not reopen a direct seat→store write path.

**Gained from D08 (2026-08-24, closed without a grill):**

- **The removal law, and whether it binds here.** Every seam arx has ruled demotes rather than deletes: admission externalizes to a handle (`e65b2a35`), refusal is a bare handle (`ee932ea8`), a cut archives with a breadcrumb (`a8895a1e`), a trace is reachable by handle and never assertable (`061d7942`). Stating "nothing is removed, only demoted" as a law would restate all four, so its only unspent work is on seams not yet designed — and this ticket owns the largest one. A sub-loop's window is discarded at nested-session end, which is the one place in arx where a whole context disappears. The capture hook bullet above already assumes a trace is written; the law would make that mandatory instead of chosen, and would bind the cancelled and crashed child as well as the one that returns a verdict. Rule it, or leave capture as this seam's local decision and let the pattern stay a pattern.

> **Revised by D05, 2026-08-28.** `426670ee` (stop as a parent-side hold at the child's next step boundary) is superseded by `001823de`: steer — interrupt the in-flight step, append, continue — and kill are the two verbs on an arx-native hand; the child-initiated permission-wall stop (`5e8da52e`) is untouched; a foreign body has kill only. Dispatch is non-blocking (`23642b44`): the verdict or stub lands at main's next step boundary as an injected message, `6aab1d24`'s sweep-before-verdict invariant unchanged. Main can tap a running hand's live transcript log (`7213cb42`). The close stands; the records moved.
