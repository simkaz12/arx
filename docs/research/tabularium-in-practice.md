---
subject: what a month of running Tabularium measured, as input to arx's memory layer
gathered: 2026-08-27
method: two read-only investigations against the live store (user@memory.example.internal:/home/user/tabularium-store, LanceDB 0.34.0) and current source at ~/Projects/Tabularium
status: evidence only, nothing ruled
companion: memory-2026.md holds the external evidence; this holds the owner's own
---

# Tabularium in practice — the internal evidence

`memory-2026.md` gathers what the field measured. This gathers what the owner's own store measured,
over 29 days and 8,552 valid records. Where the two agree the agreement is worth something, because
one is a literature sweep and the other is a single operator's production data.

Numbers here carry their provenance. Where a number could not be produced it says so rather than
estimating, and where confidence is medium it says that too.

## 1. The dedup gate

**Bounces are not persisted.** `probe_candidates` in `engine/gate.py` is a pure function with no
adapter write anywhere in the module. A bounce is built by `_make_bounce` in `engine/facts.py` into
a `SaveResult(outcome="bounced")` and returned to the caller. `ReinforcementEvent.source` is a
closed literal — `dedup, feedback, revival, compact, shelved, unsupersede, edit` — with no member
for a bounce, and the live store has no audit table. So the count of bounces presented, including
every one the agent walked away from, is permanently unrecoverable. Everything below is
reconstructed from resolution byproducts.

**Of collisions that left a trace, the large majority were judged not-duplicates.**

| Resolution | Count | Share (strict) |
|---|---|---|
| `confirm_distinct` (`pairs` table, all `source="save"`) | 591 | 89.4% |
| `reinforce` (`events`, `source="dedup"`) | 70 | 10.6% |

Counting `supersede` (98) and `promoted_from` (7) as folded-in gives 591/766 = 77.2%
`confirm_distinct`. Those 105 are weaker evidence: `_save_decision`'s own docstring says a call
naming `supersedes=` or `promoted_from=` *skips* the bounce, so some of them were proactive
corrections that never collided at all.

**Every one of the 70 reinforces was agent-judged, not auto-applied.** `paranoid=True` is the config
default and is unoverridden in the deployed systemd unit, and under paranoid
`effective_auto_ceiling == gray_floor`, which makes both auto-reinforce branches
(`facts.py:662`, `:751`, each gated `and not cfg.paranoid`) structurally unreachable.

**The false-alarm share is flat-to-rising, not falling.** By ISO week, excluding the bulk-import
week: 81.7%, 90.8%, 86.2%, 93.4%. If the store were getting cleaner through use, this should bend
down.

**Lexical near-duplicates are rare.** 12 of 8,552 valid records (0.14%) have a lexical near-twin at
Jaccard ≥ 0.5 over 6-word shingles. Read as a floor, not a ceiling — a looser overlap scan in the
second investigation surfaced pairs this measure missed, so verbatim copies are not accumulating
but restatement is.

**Thresholds were never tuned against real data.** `gray_floor=0.80`, `decision_gray_floor=0.70`,
`auto_ceiling=0.97`, `paranoid=True` are `Field(default=...)` literals, each added once and never
modified. The only calibration on record is `calibration/nomic-embed-text.json`, generated
2026-07-23 — before the store had content — on 40 hand-curated pairs, recommending `gray_floor=0.50`
and `auto_ceiling=0.80`. Its own script says applying the recommendation is a manual step gated on
an M3 that never landed. Note the live data arguably refutes that recommendation rather than the
deployment: agents call things distinct 89% of the time in exactly the band it wants auto-merged.

**A documented field that was never populated.** `ReinforcementEvent.matched_from_id` is documented
as recording the record that triggered reinforcement, "useful for lineage tracing and the offline
reconciliation judge." Verified exhaustively: all 9 `_reinforce(...)` call sites in `engine/` pass
`None` literally, as do all 5 direct `ReinforcementEvent(...)` constructions outside it
(`feedback.py`, `edit.py`, `dispositions.py` twice, and `_reinforce` itself). The field has never
been populated in production.

The reason is the interesting part, and it is not an oversight. `_reinforce` accepts the parameter
and forwards it, so the plumbing exists structurally — but `matched_from_id` needs a *second* record
id, and at every call site the only id in scope is the target's own. The incoming content that
triggered the dedup match never gets an id at all: it is either rejected or it becomes the same
record. **A write-time gate cannot record what it matched against, because the thing doing the
matching does not exist as a record yet.** Any design that wants collision lineage has to mint an id
for the rejected candidate, or capture the pair at judgment time rather than at write time.

**Embedder stability is not a problem here.** 8,595 of 8,595 rows carry `ollama/nomic-embed-text`;
`embedding.py` has two commits ever and no other model was configured. Local inference, no per-save
API cost, no re-embedding event. The published case against write-time judgment was built on
systems paying a cloud embedding and an LLM extraction call per write — Mem0 at 1000+ calls and
$0.50 per instance to reach parity with doing nothing. **None of that cost structure transfers.**
The gate's real price is one extra model round trip per collision, roughly 20 a day at the observed
rate, catching two or three real restatements.

## 2. The cleaner

It runs — 136 passes over 15 days, and its writes land — but it is inert for four stacked reasons.

**Reason one: the revisit axis has never been eligible to run.** `revisit_min_age_days = 30`
(`rubric.toml:64`); the oldest record in the store is 29.24 days old, because the store was seeded
2026-07-29. `list_due_for_review(created_before = now - 30d)` therefore returns the empty set on
every scope on every pass, which is why revisit reports 0 shelved / 0 unclear / 0 contested on all
136 rows ever recorded. Arithmetic, not judgment. It self-resolves as the seed cohort crosses the
floor — at which point roughly 7,581 records become eligible at once against a `revisit_pass_cap`
of 48, about 158 passes of backlog.

**Reason two, and the structural one: the birth axes are frontiers, by design.** Relevance selects
`list_born_since(scope, since=watermark)`, excluding Decisions, the `global` scope, and anything not
`active`/`archived`. Falsity only ever proposes an edge from a newer record to an older one
(decision `3933e548`). Decision `a301382f` states it outright: a first pass must never sweep the
store. So two pre-existing records can never be compared to each other — permanently, by
construction. And 89% of the store is one seed cohort born the same day.

**Reason three: coverage is about 3.8%.** 327 relevance-axis judged-record-instances against 8,552
valid records, and that is an upper bound since it includes re-judged retries. Five of eleven scopes
holding valid records have zero engagement: `pi-assistant` (2,282 valid) and `side-project-a` (759) each ran
the cleaner exactly once, to set a watermark, and never again; `scope-x` (403), `ai-assistant` (295) and
`mobile` (19) have no `cleaner_state` row at all. That is 3,758 records, 44% of the store, never
looked at. Mitigating detail: those last three have no active facts left, having been bulk-shelved
at seed time by `scripts/shelve_seed_records.py` — which also accounts for 4,175 of the store's
4,188 shelved records, against 13 the live cleaner ever shelved itself.

**Reason four: a live schema bug aborts whole passes.** Since 2026-08-20, judge seat A returns a
`RelevanceVerdict` missing the required `unclear` field; pydantic rejects it, and the entire pass
blocks with no per-record partial application. 45 of 136 passes are `blocked` — the Aug 16-19
tranche from the other seat hitting a usage limit, the Aug 20-onward tranche from this.
`JudgeUnavailable`'s docstring promises a fallback seat that `clean()` never builds.

**And when the panel did write, it was mostly wrong.** 13 relevance shelves ever; 7 of those 13
(54%) have since been reverted to `active` by the owner. A split verdict routes to `contested` and
writes nothing, which is deliberate (decision `444821c6`) — but a completed pass advances the
watermark regardless, so a contested record is never reconsidered.

**The counterfactual closes it.** A cheap overlap scan over client-a's 1,880 pre-existing active facts
surfaced six top pairs; reading their content, all six are unambiguous duplicates — two byte-identical
records on the same Meilisearch `scout:sync-index-settings` gotcha, two on the same skill-path 404,
two on the same computed-column repro, two restating the same route contract. Real work, sitting in
the exact population no axis can reach. *Confidence medium on the reviewer judgment: one
investigator reading 15 records, not a calibrated panel.*

## 3. The finding that matters most for arx

**Nothing in the system ever compares two pre-existing records.**

- The gate compares a newcomer against existing records.
- The cleaner's relevance and falsity axes compare a newcomer against existing records.
- Revisit judges each record alone against its own birth session, with no visibility into a twin.

A store seeded in bulk therefore has a blind spot exactly the size of the seed, and it is closed by
no mechanism at any point in the record's life. This is not a bug anyone introduced; it falls out of
two individually reasonable decisions — never sweep on first pass, and only ever point supersession
from newer to older — meeting a corpus that arrived all at once.

## 4. Read against arx's memory layer

| Piece | What the internal evidence says |
|---|---|
| Grounding pushed at session start | Untouched by any of this, and vindicated externally. Keep. |
| Supersession and validity | The write path works: all 8 falsity supersessions verified live as `validity=invalid` with a `superseded_by`. Keep. |
| The dedup gate as a **candidate surfacer** | Defensible on this cost structure. One round trip per collision, free local embedder, ~9 interruptions per catch — against the published comparator's ~420. The agent supplies the judgment; the embedder only has to put a plausible neighbour in front of it, which is the easy job. |
| The dedup gate as a **precision instrument** | Not supported. 89% of adjudicated collisions were judged distinct, and the rate is rising. |
| Vector recall as the retrieval path | Not supported at this scale; see `memory-2026.md`. Nothing internal contradicts that. |
| A cleaner built as a frontier over newcomers | Fails on a bulk-seeded store, structurally. Whatever arx does, it needs one mechanism that can compare two records neither of which is new. |
| In-place cleaner mutation | 7 of 13 shelves reverted by hand. That is the base rate to beat, and it argues for the reviewable-artifact shape — emit a proposed change set, let a human diff it — over automatic application. |
| Whole-pass abort on one bad judge output | A single record's malformed verdict should never cost the pass. Per-record isolation, not batch atomicity. |

## 5. What is still unmeasured, and the one column that would answer it

The unresolved question is whether the gate's 89% distinct rate reflects **a mistuned threshold** or
**an embedder that cannot separate related-from-identical at all**. It is unresolvable as built,
because `DistinctPair` stores only `lower_id, higher_id, source, judged_at` and
`ReinforcementEvent` stores no score either. There is no way to tell whether the 591 distinct
judgments cluster at 0.81, where a single vector honestly cannot discriminate, or at 0.95, where it
should have been able to.

Persisting the similarity score on both resolution paths produces that distribution. **This shipped
on 2026-08-27** — `similarity` is now a nullable float on both `DistinctPair` and `ReinforcementEvent`,
threaded from the cosine the gate already computes, with existing rows backfilled to null on open.
Section 6 records its first data. Everything judged before that date stays scoreless, so the 591
historical distinct-judgments can never be placed in a band.

Two other things stay unrecoverable and are worth naming rather than working around: the count of
bounces presented but abandoned without a resolve call, and any lineage from a reinforcement back to
the record that triggered it.


## 6. First instrumented run — 2026-08-27 digest

The similarity column captured its first production data within hours of deploying, during a digest
of the session that built it. Twelve save attempts, eight bounces, six resolved to `confirm_distinct`
and scored:

```
0.8280   0.7400   0.7943   0.7709   0.7335   0.7434
```

`events` carried no scores, because the session produced no reinforce resolutions.

**Zero of the eight bounces was a genuine duplicate.** One further atom was dropped, but on admission
grounds — it was session stance rather than a standing fact — not because it duplicated its candidate.

**The band split is the useful part, and it points somewhere new.** Every false alarm landed between
0.733 and 0.828, five of six below the fact arm's own `gray_floor` of 0.80. They fired because the
nearest candidate happened to be a Decision, gated at `decision_gray_floor` 0.70. The collisions that
were real — a cluster of in-force decisions contradicted by a new ruling — sat at 0.799 to 0.858. On
this sample nothing in the 0.73-0.79 band was a genuine relationship.

That implicates **the decision arm's 0.70 floor as the false-alarm generator, not the embedder** —
a different culprit than either the threshold-vs-embedder framing in section 5 anticipated, and one
that is cheap to test. Read it as directional only: six scored pairs, one author, one session, and a
design session's content is unusually self-similar. The sample also sees only collisions that were
resolved, never ones abandoned mid-judgment.

### The finding that matters most for arx's design

**The gate's most valuable act in that run was not deduplication.** It went 0 for 8 on duplicates.
What it did instead was surface, one at a time, three in-force decisions that a new ruling
contradicted — the reset cross-validation pair, the participant-set ruling, and the observer's fold
mechanism. Two of those would not have been retired otherwise, because nobody was looking for them.

**"Does this contradict something in force" is a different question from "have I said this already,"
and this store answers the first one well while charging the agent's turn for the second.** An arx
memory layer should treat them as two mechanisms rather than one, because they want different
thresholds, different exits, and plausibly different retrieval — contradiction detection wants high
recall over in-force claims on the same subject, while duplicate detection wants precision at the top
of the similarity range.

### Two lifecycle mechanics worth carrying

**Superseding one member of a contradicted cluster promotes the next member to best match.** Naming
either neighbour as the superseded record made the other bounce, and vice versa — a ping-pong that
reads as a defect but is actually the gate enumerating the cluster. The constraint that makes it
awkward is that the Decision arm accepts a single `supersedes` id where the Fact arm accepts a list,
so one ruling that retires N decisions needs one supersede and N-1 separate retirements.

**Revoke and supersede are not interchangeable, and the discriminator is whether the question
survives.** A record whose question now has a different answer is superseded. A record whose *subject
has been removed* — the reset cross-validation pair, once neither counterparty exists — is revoked,
because there is no replacement pairing to point lineage at. Getting this backwards writes a lineage
edge that claims a successor answers a question the successor never addresses.
