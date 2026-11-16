---
id: T06
labels: wayfinder:research
status: closed
assignee: none
blocked-by: none
research-dispatched: 2026-08-24
resolved: 2026-08-24
---

# T06 — The numbers the design owes, mined from the transcript corpus

## Question

From the local Claude Code `.jsonl` corpus: (1) context-window occupancy over sessions — where pressure actually accumulates, and whatever the corpus can say about the claimed ~120k degradation boundary; (2) the size distribution of tool results — where the admission-ladder crossover sits; (3) how often mid-task overflow arrives with a mixed tail (half failed branch, half live progress) — the case that would earn splice back; (4) compaction/overflow events and what preceded them. Report what is measurable and say plainly what is not.

## Resolution

Findings: [findings/T06-findings.md](findings/T06-findings.md). 270 sessions, 4 weeks, 80,814 entries.

Median session peak is 136k tokens; 56% of sessions cross 120k and 23% cross 200k — the ~120k figure is a soft aim routinely overshot, and digests actually fire at a median occupancy of 162k. Occupancy is strictly monotonic in every one of 270 files: no mid-session eviction has ever happened, and zero compaction/overflow/refusal events exist in the corpus — sessions end because the owner ends them. The tool-result distribution is sharply Pareto with its elbow at 1,000-3,000 tokens (results ≥1k are 11.7% of calls but 62.7% of admitted tokens), matching the design's guessed admission crossover. The mixed tail that would earn splice back showed up in 1 of 16 sampled high-pressure windows — rare, plausibly *because* these sessions delegate heavily and failed material stays in subagent windows, which is question 7's mechanism already working undesigned. The ~120k *quality* boundary is not measurable from this corpus; both proxies tried are confounded and reported as such. Two practice-vs-record gaps flagged for the owner: digest fires via direct MCP call 175:11 over the slash command, and at 162k not 120k. Graduated into [D08](D08-removal-as-demotion.md) and [D09](D09-eval-apparatus.md); feeds [D03](D03-admission-seat.md).
