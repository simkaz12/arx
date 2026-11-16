---
id: D03
labels: wayfinder:grill-with-docs
status: closed
assignee: none
blocked-by: none
grounding: findings/T01-findings.md, findings/T06-findings.md, findings/T07-findings.md, findings/T03-findings.md
---

# D03 — Admission's seat in the step

**Closed 2026-08-24** after one grill. Every charted bullet resolved; two of them dissolved rather than answered. D01 is unblocked.

Decisions stored: `bb4099ad` (no store call, artifacts to disk), `0bec7b13` (trace layout, write-once, preview shape, no pull exemption), `3d559805` (result cache, supersedes `ee932ea8`), `32809bf6` (the threshold), `1fa2bae0` (Goose rejected, write failure mode). Outbound to scope `tabularium`: `5c562cff`.

## Resolution, bullet by bullet

**Does ordinary admission call the store?** No — in either direction, on either branch. Tabularium holds claims: facts, decisions, digests. A raw tool artifact is not a claim, so it was never the store's to hold, and the only existing write verb embeds and runs the dedup gate. Externalized bodies land on disk instead. Per-step context assembly went with it: an `assemble(window, budget)` seeing the window every turn was ruled process that stalls the loop for little benefit. `dc433ba5`'s discriminator now has no claimant on the per-step path, which is D01's to finish.

**Tier placement.** Already answered by `e65b2a35` before the grill.

**Refusal timing.** Dissolved. Post-dispatch substitution is incoherent — if the read ran, admitting the older attempt's handle is strictly worse — and a pre-dispatch veto is memory overriding the model, which `0aa87641` forecloses. Both collapse once you notice nothing in the surviving mechanism is memory: exact match on the tool call plus the sha check the harness already does. It is a session-scoped result cache, the word *refusal* leaves the vocabulary, and `3d559805` supersedes `ee932ea8`.

**The filter tier's write.** The store half is void — the write is local. Blocking the step is microseconds and not a question. The failure mode is: session dir, then `/tmp`, and the tool call fails only if both fail. arx exhausts its own remedies before bothering the model.

**Provenance.** Dissolved. T07's A3/E1 wanted explicit model reads kept distinguishable from memory's own retrievals, and with per-turn assembly gone there are no in-loop retrievals to confuse them with. What survives of the requirement is `061d7942`'s labelling guard, met by the artifact carrying its own frontmatter — source path, sha at capture or `none: always recheck`, producer, timestamp. Since the preview is the head, the preview leads with provenance for free.

**The crossover constant.** 1,000 tokens by default, plus a per-tool override table that ships empty. The cut deciding where a fix belongs is composing vs relaying, not owned vs foreign: composing tools can be fixed at source, relaying tools cannot be trimmed at all because trimming raw material is filtering. Calibration is per-tool handle pull-rate, which D09 inherits.

**Goose's counter-evidence.** Rejected on one ground, not three: tool-pair summarization is a splice, and `9a764559` refuses splices. The LLM objection was withdrawn — arx has a worker in the loop and an observer and probe off it. The residual it would have addressed is sub-threshold junk with no splice to remove it, and the cut answers that as part of the loop design.

## What this opened

- **D01 is unblocked** and lands in a different position than charted. The transport discriminator `dc433ba5` named has lost its per-step claimant entirely.
- **D09 gains a line item**: per-tool handle pull-rate as the threshold's calibration signal, and optionally the share of session occupancy sitting in sub-1k tool results.
- **Tabularium's closed surface has its first exception**, and it is the allowed kind: an optional budget parameter on `recall`, additive, defaulting to today's envelope so Claude Code and the Pi hosts see no change. Recorded in scope `tabularium` as `5c562cff`.
- **`bb4099ad` carries one stale word.** It lists "refused prior attempts" among the trace producers; `3d559805` renamed that to cached prior results. The glossary is correct.
