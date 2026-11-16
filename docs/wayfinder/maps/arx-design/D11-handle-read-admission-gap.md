---
id: D11
labels: wayfinder:grill-with-docs
status: closed
assignee: none
resolved: 2026-08-28
blocked-by: none
grounding: findings/debate/2026-08-26-arx-plans/ledger.md
---

# D11 — Handle reads and admission's missing case

> **Closed 2026-08-28** after one grill, three rounds, three rulings, sol silent on all three — see Resolution. The body below is kept as the ticket stood when opened.

New ticket. D03 (admission) closed 2026-08-24 and did not carry this question — per the map's own charter, a given that a ticket's answer lands against reopens as a fresh ticket rather than reopening the closed one (`map.md`, "Givens, defeasible"), so this is D11 and not a reopened D03.

## Question

D03 ruled that an externalized body lands on disk as a filesystem path, and the ordinary read tool pulls it — no retrieval verb exists, and no admission exemption was built for pulling a handle back (`0bec7b13`, "no pull exemption," on the record). The plans debate found the gap this leaves open, at blocker severity: a trace-handle read has no admission exemption, so pulling an externalized body's full content back through the ordinary read path re-applies admission to what the read tool returns (`S-SOL-1`, plans, agreed, blocker). If that pulled content is still over the 1k threshold (`32809bf6`), does the read tool externalize it *again* — producing a second handle for the same body, then a third on the next pull, with no exit named anywhere for a handle that keeps re-qualifying for exactly the treatment it was already given?

Bearing on this:

- **`bb4099ad`** — admission makes no store call in either direction; a handle is a filesystem path, not a claim the store holds. Nothing in this ruling addresses what happens when the path *itself* is the thing being read.
- **`0bec7b13`** — the ruling that declined a pull exemption. Was that a considered rejection of the recursive case this ticket names, or a decision made before the recursive case was visible? The ticket text as it stands does not distinguish the two.
- **`3d559805`** — the session-scoped, sha-keyed result cache that replaced refusal. If a handle's target content and sha are unchanged between pulls, does the cache dedupe the second admission decision the way it dedupes an identical tool call — or does the cache key on tool-call identity in a way that a bare read-the-handle-path call never matches, leaving re-externalization uncaught regardless?

A related, more minor gap the same debate run surfaced and did not resolve: the result cache's own producer taxonomy lists "a cached prior result" as a trace producer distinct from externalization (`S-RUNE-10`, plans, agreed, minor) — whether an under-threshold repeat read is itself a case this ticket's answer needs to cover, or a separate loose end, is open rather than assumed either way.

## Resolution

Decisions stored, scope `arx`: `21f24656` (the echo — a whole-body pull mints nothing), `2248d3ac` (the cache holds externalized results only; three trace producers), `8098d8ee` (the echo is its own event, off the pull counter). Vocabulary in `CONTEXT.md`: Pull and Echo added; Trace, Result cache, Externalize, Transcript log amended. Sol ran every round from `~/.claude/discuss/2026-08-28-d11-handle-reads-grill/`: no flags.

- **`0bec7b13` was a considered rejection, not a blind one.** Its own text names the recursive case — "pulling a large body back returns a preview and a handle to *the file just asked for*, so the model slices or greps instead" — so the ticket's second bullet closes on the record as it stood. What the ruling never said is whether that pull writes a new artifact; that is the whole of the gap `S-SOL-1` found, and it is closed below.
- **A whole-body pull mints nothing** (`21f24656`). When a read's source path is under the trace root and the result covers the whole file (result sha equals the trace's sealed sha), externalize returns the trace's own handle and the same head-plus-counts preview, writing nothing. A path-prefix test plus one sha compare on state the harness already holds — no tracked set, so `0bec7b13`'s "tracks no set of externalized paths and grants no exemption" stands as written. The recursion is a fixed point, not a regress: the model gets back exactly what it already had and its only way forward is a slice or a grep, which was `0bec7b13`'s intended exit all along. Claude Code does the same thing today — it externalizes an over-threshold command output to a tool-results file, and the model slices it. Rejected: an admission exemption for trace-rooted reads (an 18k body into the window with no ceiling; already refused on record); a `Read` entry in the override table (the same exemption with a number on it). An over-threshold *slice* externalizes ordinarily — new artifact, producer "externalized tool result", source path plus range, its own sha — bounded because the range was the model's choice made with the counts in hand.
- **The result cache is downstream, never the exit.** `3d559805` keys on the exact tool call, and the first `Read(T1)` is a different call from whatever minted T1, so the cache cannot catch the first pull — only the second identical one. The ticket's third bullet closes as "never matches the first pull, always matches the second"; nothing to build.
- **The cache holds externalized results only** (`2248d3ac`). A sub-threshold repeat re-runs — there is no trace to hand back, and the re-run costs under 1k tokens. The glossary's fourth trace producer, "a cached prior result", was a phantom: the cache returns the *first run's* trace, whose producer is already "externalized tool result", and mints nothing. Trace has three producers. Closes `S-RUNE-10`. Deferred, not refused: answering an unchanged sub-threshold repeat with a `(thread, turn)` transcript-log handle (~20 tokens instead of ~900) — the waste is unmeasured, and D09's sub-1k occupancy share is the count that would reopen it.
- **The echo is its own event, owner ruling over the recommendation** (`8098d8ee`). The interviewer proposed logging the same-handle return as a pull with an outcome field; the owner split it onto its own counter. Pull-rate (pulls over externalizations, the threshold's calibration signal from `32809bf6`) counts only reads that admitted material; an echo adds to neither side, so a run of echoes reads as a prompting problem — the model not slicing — and never moves the override table. The echo's preview carries one mechanical line stating the handle is unchanged: a provable fact of the byte-count kind, no recommendation, so `0aa87641`'s defer-not-override holds. D09's residue — "a re-externalized pull would count against the tool's threshold twice" — dissolves: nothing is re-externalized, and the echo is not a pull.
- **Named.** *Pull* — a read whose source path is under the trace root. *Echo* — a whole-file pull answered with the handle it asked for. `bounce` was refused for the echo because the glossary already carries it on the store side as the rejected dedup bounce; `rebuff` implies declining, which `0aa87641` forbids.

