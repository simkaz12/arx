---
subject: agent memory systems vs filesystem alternatives
gathered: 2026-08-27
axes: three parallel web sweeps — the case against vector memory, filesystem-as-memory designs, the state of purpose-built systems
status: evidence only, nothing ruled
---

# Memory systems and their alternatives — the 2026 evidence

## In short

The question was whether save/embed/retrieve earns its keep at personal scope. The evidence says
no, but not for the reason you would expect, and it convicts a different half of the design than
the one under suspicion.

**Small corpora are where embeddings break, not where they are merely unnecessary.** The intuition
is that a few hundred notes are too small to bother indexing. The measured result is worse than
that: single-vector retrieval has a dimensional ceiling that a small corpus hits *more* easily,
because a small corpus still admits combinatorially many top-k subsets. On a 46-document set built
to expose this, the best embedding models cannot solve retrieval at recall@20 while BM25 is nearly
perfect. On a natural-corpus sweep across 28 size tiers, dense retrieval finished last at every
single tier, and at the smallest tier a grep-style file agent beat BM25 too. Lexical search does
not overtake agentic file reading until roughly ten million tokens of corpus. A personal note
store sits an order of magnitude below the floor where embeddings had already lost.

**The expensive half is the write path, and it is the half that does not pay.** Extraction,
similarity dedup, consolidation — the machinery that runs before anything is stored. Mem0 shipped
an LLM arbiter deciding add, update, delete, and then deleted it in April 2026, reporting a
twenty-point gain for its removal; conflicting facts now both persist and read-time ranking sorts
them out. One operator audited ten thousand of their own stored entries and found two hundred
worth keeping. His cosine dedup had flagged nearly three thousand near-duplicates; on manual
review, seven of those flags were right. Upgrading the extraction model made the junk worse rather
than better, because a stronger model follows an extraction prompt more faithfully and therefore
extracts more indiscriminately.

Consolidation carries its own damage. An agent running on consolidated memory failed 54% of
problems it had previously solved unaided — and it degraded even when consolidating from correct,
verified solutions. Agents that kept raw episodes scored twice as well as ones forced to compact.

**Everyone who shipped a memory layer this year landed on files.** Anthropic's memory tool is a
directory with six file operations and no similarity search of any kind. Letta replaced its vector
archive with git-tracked markdown compiled into the prompt — and published a benchmark where a
plain file agent with grep beat their competitor's product. Karpathy's wiki keeps write-time
judgment and throws away the index entirely, which is the exact opposite trade from a vector store
with a dedup gate. In the one benchmark run by a group with nothing to sell, plain BM25 led on
accuracy at 47.0%. The energy spread across that table is wide and worth stating precisely rather
than at its extreme: against GraphRAG, the nearest system on accuracy at 46.0%, BM25 costs about
3.6× less; against Letta, the worst of the eight, 26.5× less.

**The honest counter-case, which the summary should carry rather than bury.** The only controlled
A/B anyone ran with both an offline eval and an online holdout is a vendor's, and it came out
positive: semantic search over grep-only gained 12.5% average accuracy. Its own numbers say the
benefit concentrates in large codebases — +2.6% code retention at 1,000+ files against +0.3%
overall — and they trained a custom embedding model on agent session traces to get it, which is
an investment personal scope cannot make. It does not overturn the scale argument. It does mean
nobody has shown the benefit at small scale is zero, only that it is small.

One more caveat belongs up here rather than in the footnotes: several of the headline numbers in
this doc, the +20 gain for removing write-time arbitration among them, are measured on LoCoMo — a
benchmark whose answer key is 6.4% wrong and whose judge accepts 62.81% of deliberately wrong
answers. Read every LoCoMo-derived figure as directional, never as a magnitude.

**Memory systems do win, but only past the context window.** The one paper that measured the gap
between memory-assisted and full-context performance found it opens only above a million tokens.
Below the window, a memory system is solving a problem the window already solved. That makes this
a verdict about scale, not about the idea — and it puts your ~1,000 notes on the wrong side of the
line for a vector store and the right side for files, grep, and a model that rewrites them.

## What it means for Tabularium

The split runs against intuition. **The piece that runs invisibly — grounding pushed into context
at session start — is vindicated by everything here.** Injecting a small stable core beats fetching
it; OpenAI and Letta both moved that way. **Supersession with a validity window is the strongest
surviving idea in the entire field**, because production failures turn out to be forgetting
failures rather than recall failures, and almost nothing else implements forgetting at all.

**The pieces built with the most care are the contested ones.** The embedding dedup gate has one
practitioner endorsement, and he kept an embedder for dedup *instead of* retrieval rather than
alongside it. The typed taxonomy has no measured retrieval benefit anywhere — the one system that
tried thirteen types found its entire gain came from widening the search, not from the types.
Vector recall is the losing path at this scale. The offline cleaner survives in one shape only:
read the old store, emit a *new* one, never mutate in place.

## What it means for arx

One result lands on the harness rather than the store, and it is uncomfortable. When retrieval
results were handed to an agent inline, grep beat vector search on every model and harness tested.
When the same results were handed over **as a file the agent had to open**, the ordering inverted
on half the pairs and the strongest configuration lost nearly forty points.

Externalize-plus-handle is file-based delivery. That is the closest anyone has measured to arx's
admission policy, and it points against it. Two things blunt it: the paper never says what preview
accompanied the handle, and it tested retrieval results rather than arbitrary tool output. It is a
warning with a gap in it, not a verdict.

The same paper cuts the other way on the larger question. Moving one model between harnesses
shifted accuracy as much as swapping the retrieval method did. "The harness dominates" is the
falsifiable form of the claim that quality comes from the shell rather than the model.

## What nobody has measured

Nobody has run this exact comparison — embed-and-retrieve against grep against load-it-all — over
a few thousand short, self-authored notes across months of real use. Nobody has priced a wrong
recall against a missing one, so "bad memory is worse than none" stays a good intuition without a
number. Nobody has tracked a self-maintained store over six months to measure how fast it rots or
how much attention it costs to keep. And nobody has shown that write-time dedup reduces downstream
contradictions; the audit above measures what a gate *catches*, never what it *prevents*.

---

*Everything below is the evidence, with sources, for the summary above.*

## 1. Scale — where embeddings lose

**Small is not safe for single-vector retrieval; it is the regime where the ceiling bites.**
Weller, Boratko, Naim, Lee (DeepMind + JHU), *On the Theoretical Limitations of Embedding-Based
Retrieval*, arXiv:2508.21038, ICLR 2026. For embedding dimension d there exist document
combinations no query can return. On LIMIT-small — **46 documents** — SOTA embedders cannot solve
the task at recall@20 while BM25 reaches 85.7% recall@2. On the 50k-doc LIMIT set: GritLM-7B 12.9%,
Promptriever 18.9%, Gemini Embed 10.0%, Qwen3 Embed 4.8% recall@100.
*Caveat: LIMIT is constructed to expose the dimensional ceiling, not a natural corpus.*

**The natural-corpus sweep agrees.** *BM25 Wins at Scale: A Scaling Study of RAG Paradigms*,
arXiv:2607.26497, 30 Jul 2026. 28 nested tiers, 1,144 docs (1.7M tokens) to 511,959 docs (600.8M
tokens), questions and evidence held fixed, seven pipelines.

- Smallest tier: **File-System Agent (grep-style) 77.4, BM25 74.7** — agentic file reading leads.
- BM25 overtakes the file agent at roughly **10M corpus tokens**.
- **Dense retrieval is last at every one of the 28 tiers** — 29.9 at full scale.
- Verbatim: "lexical retrieval is the strongest scalable default, while agentic reasoning works
  best after ranked discovery rather than in place of it."

A few hundred to a few thousand short notes sits an order of magnitude *below* the smallest tier
where dense retrieval already lost.

**The counter-case, and it is the best-measured one.** Cursor, *Improving agent with semantic
search*, 6 Nov 2025. Semantic search over grep-only: **+12.5% average accuracy** (6.5–23.5% by
model), code retention +0.3% overall but **+2.6% on codebases of 1,000+ files**, +2.2% dissatisfied
requests when it is removed. Vendor-published, but it is a real offline eval plus an online
holdout — more than anyone on the other side ran. Note the benefit concentrates in large
codebases, and they trained a custom embedding model on agent session traces to get it.

**The "just put it in context" line** sits around 100K–200K tokens in multiple 2026 write-ups.
1,000 notes at ~300 tokens each is ~300K — above the dump-it-all line, comfortably inside
grep-and-read.

## 2. The write path — extraction, dedup, consolidation

This is where the evidence is harshest, and it lands on the machinery, not the storage.

**The vendor that invented write-time judgment deleted it.** Mem0 v3 (Python SDK 2.0.0,
2026-04-16) removed the LLM ADD/UPDATE/DELETE arbiter for a single-pass ADD-only write; dedup is
now exact MD5 hash only; conflicting facts both persist by design and read-time ranking surfaces
the current one. Self-reported gain: +20 LoCoMo (71.4→91.6), +26 LongMemEval (67.8→93.4), ~50%
lower extraction latency. Maintainer, github.com/mem0ai/mem0/discussions/4787: "only exact MD5
hash dedupe runs" and "both are kept by design… retrieval is meant to surface the current truth,
not overwrite/delete the old one." Also dropped external graph stores from OSS.
Source: docs.mem0.ai/migration/oss-v2-to-v3

**The similarity dedup gate finds real duplicates and is right about almost none of them.**
github.com/mem0ai/mem0/issues/4573, opened 2026-03-27 by `jamebobob`: *"What we found after
auditing 10,134 mem0 entries: 97.8% were junk."* One user, one agent, Qdrant, 32 days.
- 10,134 entries → **224 survived**; 38 clean enough to keep as-is, 186 deleted and rewritten.
- **Cosine dedup flagged 2,943 near-duplicates; manual review kept 7.**
- 808 entries asserting "User prefers Vim," **191 of them exact copies of one sentence** — memory
  re-extracting its own output.
- Upgrading gemma2:2b → Sonnet 4.6 on day 21 **did not reduce the junk rate**. Author's conclusion:
  "a better model follows the extraction prompt more faithfully, which means it extracts more
  indiscriminately. The extraction prompt is the bottleneck, not the model."

**Extraction beneath verbatim retrieval, at 250× the write cost.** MemDelta (Kuan Wang,
arXiv:2606.29914, 29 Jun 2026), GPT-4o-mini, n=500: no memory 2.2%, random 3.2%, **agent
self-memory 42.0% at 250 LLM calls**, **verbatim RAG 47.2% at zero write-path calls**, full context
49.8%. Write-path comparison (n=88): Mem0 v2 **72.7% at 1000+ LLM calls, ~120 min, $0.50+** vs
verbatim RAG **73.9% at 0 LLM calls, ~60s, $0.01 — p=1.0**. Also: swapping only the embedding
model moves accuracy **+6.2pp (p=0.004)**, comparable to the architectural claims papers make.

**Consolidation can drop you below no-memory.** Zhang, Lin, Wu, Sun, Li, Li, Peng, arXiv:2605.12978,
13 May 2026: **GPT-5.4 fails 54% of ARC-AGI problems it had previously solved**, when running on
consolidated memory — degrading even when consolidating from correct, ground-truth solutions.
Memory utility follows an inverted U and can fall below the no-memory baseline. Agents preserving
raw episodes **double the accuracy** of forced-consolidation counterparts. Conclusion: "robust agent
memory should treat raw episodes as first-class evidence and gate consolidation explicitly."

**But consolidation wins under a starved budget.** Kang et al., arXiv:2607.17545v2, 21 Jul 2026,
LongMemEval: at a **256-token budget retention beats every consolidation operator** (Abstract
−0.080, Merge −0.107, Rewrite −0.084); at **32 tokens** Abstract hits 52.0% vs retention 4.0%.
Consolidation is compression for a starved context, not a quality technique.

**Type taxonomies show no measured retrieval benefit.** Memanto's 13-type schema contributed
nothing in its own ablation — **+20.4pp of its total came from raising k from 10 to 40 and dropping
the similarity threshold from 0.15 to 0.10**. Its stated conclusion: "the precision-recall tradeoff
in agentic memory skews decisively toward recall." Chen & Cheng (arXiv:2606.12945, 11 Jun 2026),
LongMemEval, 479 cases: learned multi-factor 0.770 ± 0.011, uniform weights 0.657, best single
factor 0.518, **recency-only 0.368**; goal-relevance weight driven to **0.00** in the blind
regime — query-agnostic consolidation cannot exploit future-query similarity by construction.

**Retrieval can be worse than no retrieval.** Astute RAG (Google, arXiv:2410.07176, ACL 2025):
~70% of retrieved passages do not contain the answer; with Claude, **RAG scored below No-RAG —
NQ 44.4% vs 47.1%, TriviaQA 76.7% vs 82.0%**. Knowledge conflicts in 19.2% of data; when internal
and external knowledge disagreed, internal was right 47.4% of the time.

**And the retrieved distractor is the mechanism.** Chroma, *Context Rot*, 14 Jul 2025, 18 models:
all degrade at every input-length increment, degradation is *faster* when needle and question are
semantically similar, and **a single distractor lowers accuracy**. Combined with a 37.6%
near-duplicate rate, the memory system becomes the distractor source.

## 3. Where memory genuinely beats no-memory

The honest refutation, and it is narrow.

**Only above the context window.** Jiang et al., *Anatomy of Agentic Memory* (UT Dallas,
arXiv:2602.19320, 22 Feb 2026) propose Δ = Score_memory − Score_fullcontext and find **only
LongMemEval-M (>1M tokens) structurally requires external recall**. Below the window, memory
systems solve a problem that does not exist.

**BEAM** (ICLR 2026, arXiv:2510.27246): 100 conversations to **10M tokens**, 2,000 validated
questions; their LIGHT architecture beats the strongest baseline by 3.5–12.69%, measured where
full-context is physically impossible.

**Focused beats dumped.** Chroma's LongMemEval finding: focused ~300-token prompts substantially
outperform ~113k-token full prompts across every model family. The honest framing of the
anti-vector case is not "don't retrieve" — it is "retrieve with grep or BM25 instead of cosine."

## 4. What the benchmarks are worth

**LoCoMo, which made this category look necessary, is broken.** Penfield audited all 1,540
questions (2026-04-08): **6.4% of the answer key is wrong**; the LLM judge **accepts 62.81% of
deliberately wrong, topically-adjacent answers**; true ceiling ≈ **93.6%**, so anything reported
above that is measuring judge leniency. Companion post: **56% of per-category system comparisons
are statistically indistinguishable from noise**. 446 questions have refusal as the correct answer
and the standard harness drops them while instructing the model never to abstain. Conversations
average ~9,000 tokens — they fit in every frontier context window, which is why full-context is
competitive.

**Full-context beats the memory products on it.** Memanto's table (arXiv:2604.22085, 23 Apr 2026),
LoCoMo: **Full Context 72.9%**, Mem0 66.9%, Mem0-graph 68.4%, Zep 75.1%. Zep conceded the point
publicly while attacking Mem0: "If simply providing all the text yields better results than the
specialized memory system, the benchmark isn't adequately stressing memory capabilities."

**The vendor numbers are not reproducible.** Mem0's CTO and Zep each published corrections of the
other's methodology in 2025 (getzep/zep-papers#5); neither retracted, and a third party cannot
adjudicate because the contested settings are not prescribed by the benchmark.

**The one study with no product in the race.** Omri et al. (Harvard/MIT/Minnesota),
arXiv:2606.06448, 4 Jun 2026, Table 3 — LongMemEval, Qwen3-32B, 300 queries:

| System | Accuracy | Wall time | LLM calls | Energy (kJ) | J / correct |
|---|---|---|---|---|---|
| **BM25** | **47.0%** | **16.3 min** | **300** | **582** | **4,128** |
| EmbedRAG | 39.8% | 14.4 min | 610 | 495 | 4,144 |
| GraphRAG | 46.0% | 1.83 h | 3,215 | 2,082 | 15,084 |
| HippoRAG v2 | 44.3% | 44.2 min | 2,743 | 1,339 | 10,079 |
| A-Mem | 42.7% | 11.76 h | 19,230 | 14,864 | 116,116 |
| Mem0 | 32.0% | 4.02 h | 4,538 | 4,878 | 50,813 |
| Letta | 27.7% | 14.36 h | 18,394 | 15,429 | 185,873 |
| MIRIX | 20.0% | 6.03 h | 7,655 | 8,678 | 144,629 |

Plain BM25 — zero LLM writes, sub-second index — is the accuracy leader and 26.5× cheaper in
energy than Letta. The cost is in construction, not serving: median decode share of construction
tokens is 4.6%, so it is prefill and embedding, not generation. Per-write cost **rises with store
size** because agentic systems query the growing store before merging.

*Caveat that must travel with this table:* MemDelta shows rankings **invert across backbones** —
Gemini gains +14pp from full context, Sonnet gains +31pp from RAG, and Sonnet refuses 63% of
full-context queries.

**Break-even against just sending the transcript** (Pollertlam & Kornsuwannawit, arXiv:2608.11879,
12 Aug 2026), in turns: Mem0 82 / **Never** / 10 / 0 across four configs; Hindsight **Never** in
all four. Their measured LoCoMo accuracy for Mem0 was **32.2 / 21.4 / 49.8 / 51.6%** against
66–92.5% quoted by the vendor. Cost-prediction error on held-out data: 18–22% (Mem0), 41–69%
(Mastra OM), vs **2.9–6.5%** for full-history and rolling-window baselines.

## 5. Everyone converged on files

- **Anthropic's memory tool** (`memory_20250818`) is a `/memories` directory with six ops —
  view, create, str_replace, insert, delete, rename. **No embeddings, no vector DB, no similarity
  search.** Retrieval is directory listing plus file read.
- **Letta v2** replaced vector archival passages with **MemFS — git-tracked files** under
  `~/.letta/agents/<id>/memory`, compiled into the system prompt by `renderMemfsProjection`.
- **Letta's own benchmark** (2025-08-12): a GPT-4o-mini agent given the LoCoMo transcript as a
  **file** plus grep/search_files/open/close scored **74.0%** against Mem0's best graph variant at
  **68.5%**. A memory vendor published files beating a memory product.
- **Karpathy's `llm-wiki`** (2026-04-04, 46,712 stars): immutable `raw/` sources, an LLM-owned
  `wiki/` of interlinked markdown, a CLAUDE.md schema. Working scale ~100 articles / ~400,000 words.
  "the wiki is a persistent, compounding artifact… the contradictions have already been flagged."
  Note the trade: he **kept write-time judgment** (integrate, revise, flag, lint) and **dropped the
  index entirely** — the opposite of a vector store with a dedup gate.
- **Manus** (Yichao Ji, 2025-07-18): glob and grep over the filesystem, no vectorstore, after four
  framework rebuilds. Names KV-cache hit rate as the top production metric — itself an argument
  against dynamically-retrieved memory, which busts the cache every turn.
- **Cline** publicly refuses embeddings: ripgrep + fzf + tree-sitter, "no RAG, no embeddings, no
  vector databases. This isn't a limitation — it's a deliberate design choice."
- **Claude Code**: CLAUDE.md hierarchy concatenated (not overridden), imports to depth 4,
  auto memory at `~/.claude/projects/<p>/memory/` where **the first 200 lines or 25KB of MEMORY.md
  load every session and topic files never load at startup**. Over-limit writes succeed but return
  an error telling Claude to rewrite the index. Retrieval is Glob/Grep/Read.
- **Cherny on dropping RAG from Claude Code** (Latent Space, 7 May 2025): "it outperformed
  everything. By a lot." Asked for the benchmark: **"This was just vibes, so internal vibes… mostly
  vibes. It just felt better."** Treat as a preference, not a result. His stated reasons — index
  drift, "this index has to live somewhere," security — are real regardless.
- **The one dissent from an embeddings vendor is about dedup, not retrieval.** Han Xiao (Jina,
  25 Feb 2025): "Embedding model is useful but in a completely unexpected way. We thought it would
  be used for agent memory retrieval or context compression alongside a vector database (which, as
  it turns out, isn't needed), but we actually used it for **deduplication**."

## 6. The one result that lands on arx, not on the store

Sen, Kasturi, Lumer, Gulati, Subbiah (PwC), *Is Grep All You Need? How Agent Harnesses Reshape
Agentic Search*, arXiv:2605.15184, 14 May 2026. 116 LongMemEval questions, four harnesses,
five models.

With results delivered **inline**, grep wins uniformly:

| Harness + model | grep | vector |
|---|---|---|
| Chronos + Claude Opus 4.6 | **93.1%** | 83.6% |
| Codex + GPT-5.4 | **93.1%** | 75.9% |
| Chronos + Gemini Flash-Lite | **86.2%** | 62.9% |

With results delivered **as a file the agent must open**, the ordering **inverts on 5 of 10
harness/model pairs**, and Codex + GPT-5.4 collapses to **55.2% grep vs 67.2% vector**.

The paper's actual conclusion is that the harness matters as much as the retrieval method — moving
one model between harnesses shifted accuracy by margins comparable to swapping grep for vectors.

**Read against arx:** externalize-plus-handle *is* file-based delivery. This is the closest measured
result to arx's admission policy and it points against it. What the paper does not say is what
preview accompanied the handle, and it tested retrieval results rather than arbitrary tool output —
so the finding is a warning with a gap in it, not a verdict. It also cuts the other way on the
larger question: "the harness dominates" is the falsifiable version of the claim that quality comes
from the shell rather than the model.

## 7. Read against Tabularium, component by component

| Component | Verdict from the evidence |
|---|---|
| Grounding pushed at session start (hot/ember tiers) | **Survives.** Injecting a small stable core beats fetching it — OpenAI's Dreaming injects a synthesized summary every message; Letta compiles MemFS into the system prompt. Anthropic's per-turn tool call is the outlier and its named costs are tokens and latency. |
| Supersession, validity windows, shelving | **Strongest surviving piece.** Zep/Graphiti's bi-temporal invalidation is the only design in the field that survives scrutiny — supersession as a first-class queryable event rather than an overwrite or delete. ForgetEval: "production failures are predominantly forgetting failures rather than recall failures." |
| Embedding dedup gate at admission | **Contested hard.** Mem0 deleted theirs and gained 20 self-reported points; the 10,134-entry audit flagged 2,943 near-dupes and kept 7. Han Xiao is the one endorsement, and he kept it *instead of* retrieval, not alongside it. |
| Vector recall as the retrieval path | **Loses at this scale.** Dense is last at all 28 tiers of the scaling sweep; BM25 leads the independent benchmark on accuracy *and* energy. |
| fact / decision / digest types | **No measured retrieval contribution.** Memanto's 13 types added nothing; the gain came from raising k. Useful as a writing discipline; not as a retrieval mechanism. |
| The cleaner (offline consolidation) | **Survives in one shape only** — the Dreams shape: read the old store plus transcripts, emit a *new* store, never mutate the input, let a human diff it. In-place lossy consolidation is what cost 54% of previously-solved ARC-AGI problems. |
| Digests as narrative records | Already superseded by the state-object ruling in this repo's own design. |

## 8. What nobody has measured

- **This exact case.** No controlled comparison of embed-and-retrieve vs grep vs load-it-all over a
  few hundred to a few thousand *self-authored, short, personal* notes across months of real use.
- **The regret asymmetry.** No study prices a false recall against a missing one. "Wrong recall is
  worse than no recall" remains well-motivated intuition, not a measured finding.
- **Longitudinal drift of a self-maintained store.** Every benchmark is a fixed corpus with scripted
  updates. Nobody has run a personal store for six months and measured contradiction rate,
  staleness rate, or curation attention consumed.
- **Whether write-time dedup reduces downstream contradictions.** No source found for a measured
  effect. The 10,134-entry audit measures what the gate *catches*, not what it *prevents*.
- **Threshold re-derivation cost on an embedding upgrade.** The principle is documented; no team has
  published a before/after price.

## Sources

Papers — arXiv:2508.21038 (embedding limitations, ICLR 2026) · arXiv:2607.26497 (BM25 Wins at
Scale) · arXiv:2606.29914 (MemDelta) · arXiv:2605.12978 (consolidation harm) · arXiv:2607.17545v2
(consolidation under budget) · arXiv:2606.12945 (factor weights) · arXiv:2410.07176 (Astute RAG) ·
arXiv:2606.06448 (independent cost/accuracy benchmark) · arXiv:2608.11879 (serving cost break-even) ·
arXiv:2605.15184 (Is Grep All You Need) · arXiv:2607.17598 (progressive disclosure) ·
arXiv:2604.24594 (skill retrieval distractors) · arXiv:2602.11988 (Evaluating AGENTS.md) ·
arXiv:2605.10039 (instruction adherence factorial) · arXiv:2607.14611 (Bad Memory / injection) ·
arXiv:2602.19320 (Anatomy of Agentic Memory) · arXiv:2510.27246 (BEAM) · arXiv:2602.16313
(MemoryArena) · arXiv:2605.12493 (LongMemEval-V2) · arXiv:2606.15903 (ForgetEval) · arXiv:2606.30306
(Always-On Agents survey) · arXiv:2604.22085 (Memanto) · arXiv:2606.24775 (agent-native memory)

Audits and operator reports — penfieldlabs.substack.com/p/we-audited-locomo-64-of-the-answer ·
github.com/mem0ai/mem0/issues/4573 · github.com/mem0ai/mem0/discussions/4787 ·
getzep/zep-papers#5 · blog.getzep.com/lies-damn-lies-statistics-is-mem0-really-sota-in-agent-memory

Vendor and practitioner — trychroma.com/research/context-rot · cursor.com/blog/semsearch ·
anthropic.com/engineering/effective-context-engineering-for-ai-agents ·
platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool · code.claude.com/docs/en/memory ·
letta.com/blog/benchmarking-ai-agent-memory · docs.mem0.ai/migration/oss-v2-to-v3 ·
manus.im/blog/Context-Engineering-for-AI-Agents-Lessons-from-Building-Manus ·
gist Karpathy llm-wiki 442a6bf555914893e9891c11519de94f · jina.ai DeepSearch guide ·
latent.space/p/claude-code · jxnl.co/writing/2025/09/11/why-grep-beat-embeddings-in-our-swe-bench-agent-lessons-from-augment
