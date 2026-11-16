# T06 findings — the numbers the design owes, mined from the transcript corpus

Ticket: [T06](../T06-transcript-numbers.md) · resolved 2026-08-24 · produced by a hand-ex research agent over the live corpus at `~/.claude/projects/*/`, read-only, via scratch scripts under `/tmp/t06/`.

## Corpus inventory

270 session files across 9 Claude Code project scopes (no subagent sub-transcripts, no externalized `tool-results/*.txt` — those don't feed the model's context, so they're out of scope for occupancy). Tabularium 99, client-a-be 89, side-project-a 18, client-b-be 14, arx 14, skillbook 13, pi-assistant 12, bare-home 10, client-a-fe 1. Date range 2026-07-26 to 2026-08-23 (~4 weeks). 80,814 total JSON entries, 11,304 `tool_result` blocks, 246/270 sessions (91%) carry usable `usage` data (24 near-empty/aborted sessions have none). Model mix: `claude-opus-5` dominant (23,183 assistant entries), with `claude-opus-4-8`, `claude-fable-5`, `claude-sonnet-5` in smaller numbers.

Schema matched the brief's assumption exactly: each non-sidechain `assistant` entry carries `message.usage.{input_tokens, cache_read_input_tokens, cache_creation_input_tokens}`, and their sum on the last such entry before any point is that point's context occupancy — cross-checked against raw line counts (80,814 parsed vs 80,824 `wc -l`; the gap is blank-line artifacts) and against stored memory `f0d99a3e`, which states this exact method. One correction to a naive read: `type: "attachment"` → `total_tokens_reminder` is an account-level weekly-quota counter, not per-turn occupancy — plausible at first glance and the wrong field.

## 1. Occupancy trajectories

Peak occupancy per session (min/p25/median/p75/p90/max, tokens): 19,777 / 88,776 / **136,226** / 210,955 / 264,159 / 564,753. Crossing counts: 80k → 188/270 (70%), **120k → 150/270 (56%)**, 160k → 93/270 (34%), 200k → 63/270 (23%). Within sessions occupancy is strictly monotonic — `last_occ == peak_occ` in all 270 files: no mid-session eviction ever happens in this corpus, consistent with the no-splice/no-compaction ritual already on record.

The ~120k figure reads as a **soft aim, routinely overshot**, not a boundary sessions stop at: of the 150 sessions that reach 120k, 93 (62%) continue to 160k and 63 (42%) to 200k+. The direct `mcp__tabularium-mcp__digest` call (175 invocations) fires at a **median occupancy of 161,841**, not ~120k; only 75% of digest calls happen at or above 120k at all, and 51% above 160k. Peaks to 564,753 tokens confirm this account/model combination runs with a context ceiling well past the classic 200k, which is itself why no session was ever forced to stop — see measurement 3.

## 2. Tool-result size distribution

Token proxy = chars/4 (the transcript carries no per-`tool_result` token count, only aggregate occupancy at the next turn — an approximation, stated as such; the occupancy numbers above are exact API tokens). Overall: p50 108, p75 410, **p90 1,156, p95 1,818, p99 4,606**, p99.9 ≈12,375, max ≈18,172. Sharply Pareto: results ≥1,000 tokens are 11.7% of calls but **62.7% of all admitted tokens**; ≥3,000 tokens are 2.1% of calls but 26.7% of tokens. By tool (median/p90/p99): `Bash` 147/836/2,986 (4,032 calls, largest by count), `Read` 690/4,062/12,546 (729 calls, heaviest per-call tail), `Agent` (subagent verdict returns) 264/1,464/3,568 (1,301 calls — compact, corroborating that delegated verdicts come back small), `mcp__tabularium-mcp__recall` 1,678/3,361/5,289, `mcp__pi-jira__jira_tool` 72/2,600/9,162.

This corroborates POSITION §5/§6's guess of low single-digit thousands for the crossover: the elbow — where a shrinking share of calls starts carrying the majority of volume — sits around 1,000-3,000 tokens. The corpus shows *where the tail starts dominating*; it cannot say where inlining stops *paying*, because that depends on the round-trip cost of a deferred pull, which doesn't exist yet to measure.

## 3. Compaction/overflow events

None exist in this corpus, in the technical sense. Zero `PreCompact`/auto-compact markers, zero `compactMetadata` fields (the one grep hit was not a compaction event), zero `/compact` invocations. `last_assistant_stop_reason` across all 270 sessions: `end_turn` 241 (89%), `None` 25 (parse-boundary gaps, not truncation), `tool_use` 3, `stop_sequence` 1 — **zero occurrences of `model_context_window_exceeded` or `refusal`**. Sessions end because the user ends them.

The ending mechanism, confirmed from the data: 179 `/clear` invocations, each appearing near the *start* of a fresh session file (idx 3-21, median 5) rather than mid-file — `/clear` rotates to a new file, it doesn't reset in place, so **file boundary and work-session boundary coincide**, which is why per-file peak occupancy is the right unit in measurement 1. Only 11 `/digest` *slash-command* invocations against 175 `mcp__tabularium-mcp__digest` *tool* calls — the digest step runs almost entirely as a direct MCP call, not the slash command. A real divergence from the described ritual, flagged rather than folded in.

## 4. Mixed-tail frequency

Sampled 8 sessions (one per project, weighted toward the two largest — Tabularium ×2, client-b-be, skillbook, client-a-be, pi-assistant, side-project-a, arx), 2 windows each (~25 entries centered on the first 120k crossing and on session end), 16 windows total. Classification rule: a window counts as mixed if it visibly holds *both* a recently-abandoned/failed direction *and* continued live forward progress, co-present and unresolved — the case POSITION §6 names as splice-worthy.

Positive match: **1/16** — `client-b-be/6c8f92c3-...jsonl` entries 165-167, a P2 critique round-2 FAIL verdict sitting beside `SendMessage` resuming the builder with a fix list, both live in the same window. One borderline — `Tabularium/16567da1-...jsonl` entries 190-213: an explicit user correction ("That's wrong — untagged does not mean universal"), fixed via `Edit`, immediately followed by rejecting a subagent's bare PASS verdict and re-verifying with `git diff` + `pytest` — but it resolves within the same short window rather than sitting unresolved. The other 14/16 windows are clean: single-threaded work, or — heavily represented at session end — the digest/handoff ritual itself, whose only friction is dedup-gate bounces, a different phenomenon from a failed task branch.

Structural read worth carrying: high-occupancy sessions in this corpus are disproportionately **delegation-heavy** — `Agent` fan-out, critique loops, orchestration. That may be *why* the mixed tail is rare here: the "half failed" content lives in the subagent's own window and only a compact verdict crosses into the main thread — exactly `061d7942`'s "traces are evidence, not knowledge" already working in practice. The sample is small (N=8/16) and drawn from the top of the occupancy distribution, so this is weak evidence against urgency for splice, not proof of absence — and it may under-represent mixed-tail risk for a harness that delegates less than these sessions do.

## What is not measurable, and why

The ~120k figure as a *quality* boundary cannot be established from this corpus — occupancy says how full the window got, not whether reasoning degraded. Two proxies were tried; both are too confounded to report as evidence either way. Tool-result **error rate by occupancy bucket falls** with occupancy (6.98% at 0-40k down to 1.64% at 200k+) — the wrong direction for a degradation story, almost certainly because early-session calls are exploratory (wrong greps, wrong paths) and later calls are precision-guided. A keyword-heuristic **correction-language rate** in user turns rises with occupancy, 1.8% at 0-40k to a 17.2% peak at 120-160k and 21.2% above 200k — directionally consistent with degradation, but confounded by task difficulty (harder tasks run longer *and* need more correction), by sample size, and by the crudeness of keyword matching. Neither is offered as evidence for or against. What would make the claim measurable: a labelled judgement of "was this turn's output actually wrong," which only `Corrected:` lines in digests or a purpose-built eval harness could supply.

## Decision questions this exposes

- **Admission ladder.** The observed Pareto elbow (1,000-3,000 tokens) matches the design's guess; the open piece is the deferred-pull round-trip cost, unmeasurable until the ladder exists.
- **Eval apparatus.** `last_occ == peak_occ` for every session means the transcripts cannot retroactively simulate *eviction* decisions (nothing was ever evicted to compare against) — but they *can* directly simulate *admission* decisions, since every `tool_result`'s size and downstream handling is recorded. The 175 real digest calls are a ready-made labelled set for what a real close-out looks like.
- **Open question 7.** The delegation pattern in measurement 4 is a working instance of exactly the mechanism question 7 proposes generalizing — subagent dispatch already demotes bulky/failed material to a compact verdict rather than dropping it, and may be doing more of the mixed-tail-avoidance work than the design has credited.
- **The digest/clear gap.** 11 slash-command digests vs 175 tool-call digests, and a median digest point of 162k against a stated ~120k target, are facts about actual practice diverging from the described ritual — a session with the owner, not a constant to fold in silently.

## Evidence

Corpus at `~/.claude/projects/-home-user-Projects-{client-a-be,client-a-fe,side-project-a,client-b-be,arx,skillbook,pi-assistant,Tabularium}/*.jsonl` and `~/.claude/projects/-home-user/*.jsonl`. Key cited entries: `client-a-be/3fbfe2f7-d1d8-4384-8be6-adf22b24775a.jsonl` idx 744 (stale-memory correction); `client-b-be/6c8f92c3-6145-41c3-b790-7128b47ec7c6.jsonl` idx 165-167 (mixed-tail positive); `Tabularium/16567da1-468b-471f-84c0-95e67673821a.jsonl` idx 190-213 (borderline).

## Unknowns and confidence

Whether the correction-language rise reflects real degradation, task-difficulty confound, or sampling artifact — unresolved, needs labelled ground truth. Whether the digest/clear gap reflects a UI change, habit drift, or two rituals the owner treats as distinct — a question for the owner, not the corpus. Whether mixed-tail rarity (1/16) generalizes beyond this delegation-heavy sample — unresolved; the sample is small and non-neutral by construction.

Confidence: high on measurements 1-3 (direct field extraction, cross-checked); medium on measurement 4 (small manually-classified sample, explicit rule, judgement-based); low on the degradation proxies, reported as exactly that.
