# arx — field survey and sourcing map

> **What this file is — written 2026-08-19/20 (fifth session).** A **survey of the outside world plus a sourcing map**: what a harness is made of according to people who have built them, which of those parts arx takes from a donor, and which it has to build. It is not a plan and not a set of positions. Positions live in `POSITION.md`; that file is a context ledger and this one is its missing breadth.
>
> **Epistemic class is different from `POSITION.md`.** Nothing here is a ruling. Claims are externally sourced and dated, and the sources are uneven — two are peer-review-shaped arXiv papers, one is official API documentation, the rest are a gist and vendor blogs. Confidence is marked where it matters.
>
> **Nothing from this session is in the store**, same as the fourth.
>
> **Updated 2026-08-24 (sixth session).** The T-series research under `docs/wayfinder/maps/arx-design/findings/` grounds and in places corrects this survey; where they conflict, the findings win. Known corrections: Goose is 13 crates now, not 4, and `goose-server` is gone (T01, T02); the seven-`stop_reason` table in §1 is real at the wire level but collapses to structural Message inspection before it reaches a Goose-shaped loop (T01); the steer/queue/abort triad in §1 is a field *proposal*, not shipped — the convention is a two-way steer/queue split with abort as a separate control action that preserves the queue (T04). The sourcing map (§4, §5) is now the ratified architecture (`6bbde714`), not a hypothesis to weigh.
>
> **Standing note, 2026-08-27, retired 2026-08-30.** The line above still held at the time — `6bbde714` ratified this file's sourcing map as the working architecture, not a hypothesis — and this note opened `S-RUNE-5` (plans, agreed, blocker) over five subsystems §4-§5 still ordered after the design had killed them, annotated in place rather than rewritten, following the D08 close's precedent. That precedent stops here: `af68d429` (2026-08-30, map-integrity debate rows S-RUNE-2/S-SOL-4) ruled the annotate-in-place approach itself failed — three of §5's nine items were dead or reopened, two of §6's six seams described machinery that no longer existed, and neither got a durable fix. §4-§5 below are now one module table for the post-D16 design, not an annotated sourcing map, and `S-RUNE-5` retires with it.

Motivating problem, stated by the owner: `POSITION.md` had **depth without breadth**. Five sessions went deep on one column because conversation raises questions in the order conversation raises them, not in dependency order. This file is the breadth pass, done by reading rather than by arguing.

---

## 1. The turn, structurally

Three nestings: **session ▸ turn ▸ step**. The step is what repeats; most subsystems live there.

```
SESSION  (persists across turns; resumable; locked against concurrent editors)
└─ TURN  (one user input → next prompt ready)
   └─ ┌─ STEP  (repeats until termination)
      ├─ drain steering queue        ← steer / queue / abort
      ├─ pressure check + window assembly
      ├─ (optional) thinking call    ← read-only
      ├─ (optional) self-critique
      ├─ issue request               ← streaming
      ├─ consume stream              ← chunk → semantic event → display state
      ├─ branch on termination       ← stop_reason ∪ harness terminators
      ├─ dispatch tools              ← permission check first; subagent = nested SESSION
      └─ admit results
```

> **Dead 2026-08-24, unflagged until this pass (`5efb4c94`, `e65b2a35`).** The `pressure check + window assembly` line has no referent. Nothing evicts, folds or compacts mid-session; admission is the only in-loop clock arx has, and it fires per tool result rather than at a pressure check. The rest of the diagram — the turn/step nesting itself — still holds; only that one line is stale.

### Four corrections to the naive skeleton

A first pass had input at turn level, one model call per step, and a single branch on `stop_reason`. All three are wrong.

**1. Input is per-step, not per-turn.** Harnesses drain a **steering queue** at the top of each step — before the next model call, never mid-tool-call — with three delivery semantics:

| Semantic | Behaviour |
|---|---|
| **steer** | inject now, between tool execution and the next model call; work continues |
| **queue** | hold until the agent finishes all pending work |
| **abort** | drop the current run, clear the queue, start fresh |

This is contested and unsettled in the field — open issues on `pydantic/pydantic-ai-harness`, `anomalyco/opencode`, `tiann/hapi` as of Aug 2026. Confidence: high that the pattern exists, low that anyone has it right.

> **Corrected 2026-08-24 (T04).** The three-way split is opencode issue #32157 — a proposal, open and unimplemented. Every shipping system checked lands on a two-way split: steer (inject at a boundary) and queue (hold to idle). "abort" is not a third delivery mode but an orthogonal control action, and by the stated intent of Claude Code, Goose, and opencode it *preserves* the pending queue rather than clearing it — the "clear the queue" row above is wrong. The genuinely unsettled axis is the drain boundary: per tool call (Claude Code, pydantic-ai) versus per full turn (Goose).

> **Ruled 2026-08-28 (D05).** Main adopts Claude Code's flow whole (`ba965dd0`) — step-boundary delivery, Esc as abort, no queue mode — so the drain axis is settled for main by adoption. The steering arx designs for itself is main→hand: steer (interrupt, append, continue), tap (read-only), kill; dispatch non-blocking.

**2. Termination is not one branch.** Two independent sources of exit, merged.

*API-level* — seven `stop_reason` values, several needing different continuations:

| `stop_reason` | Required client action |
|---|---|
| `end_turn` | done |
| `tool_use` | execute, return `tool_result` blocks only, continue |
| `max_tokens` | raise limit or continue the truncated response |
| `stop_sequence` | read `stop_sequence` to see which fired |
| `pause_turn` | server-tool loop hit its iteration cap — resend the assistant message as-is |
| `refusal` | read `stop_details` for policy category; retry on a fallback model |
| `model_context_window_exceeded` | treat as truncated |

Notes: these are HTTP 200, not errors; `stop_reason` arrives in `message_delta`, not `message_start`.

*Harness-level* — iteration cap reached, user abort, safety threshold violated. So the exit condition is a merge of model signal + harness policy + user signal, not a switch.

**3. A step may make more than one model call.** OpenDev's loop is six phases: pre-check, **thinking** (read-only call), **self-critique**, action, tool execution, post-processing. "One request per step" was an assumption, not a constraint.

**4. Tools can suspend the loop and call the human.** `askUser` / `submitPlan` register back-channel callbacks, emit an event, and block on a promise — invisible to the model, wired to the abort signal. Control inverts at dispatch. Absent from the naive skeleton entirely.

### One layer the skeleton lacked

**Event translation is its own stage**: provider chunk → semantic harness event → reduced display state (Mastra batches at 250ms with a 500ms ceiling; approvals/questions bypass the batcher). Harness-originated events enter here without ever having been model output.

### One cut orthogonal to the loop

**Scaffolding vs harness** (OpenDev). Everything assembled *before* the first message, versus everything that runs per-step.

| Scaffolding | Harness |
|---|---|
| Type foundation (base agent / interface) | Central ReAct loop (6 phases) |
| Concrete agent class | Input/output boundaries |
| Factory assembly: skills → subagents → agent | Prompt composition engine |
| Subagent compilation (spec → compiled, tool-filtered) | Tool registry + dispatch |
| Dependency injection | Safety system (5 layers) |
| | Context engineering |
| | Subagent orchestration |

Lessons attached to that split, all reported as things they changed *after* getting it wrong:

- **Eager, not lazy, prompt building** — lazy construction raced with MCP discovery and added first-call latency.
- **One parameterised agent class, not a hierarchy** — multi-class designs produced diamond dependencies; a single class parameterised by `allowed_tools` removed them.
- **Delegate to a read-only subagent instead of a state machine** — a plan-mode state machine got stuck mid-execution; a Planner subagent eliminated it.
- **Enforce tool access at the schema level** — tools absent from the LLM-visible schema is more reliable than a runtime permission check. *(D06, 2026-08-29: taken for the read/write axis only — a hand's surface is static per agent definition and never widens (`ebe94e38`); everything finer is a rule in a deny → ask → allow gate (`799b0967`), the parallel runtime layer T05 found OpenDev itself keeps.)*

---

## 2. Five decompositions, and what they agree on

| Source | The cut |
|---|---|
| Modern Agent Harness Blueprint 2026 | **5 layers** — Execution Runtime, Context System, Capability Surface, Governance, Surface/Protocol Adapters |
| arXiv, *Architectural Design Decisions in AI Agent Harnesses* | **5 dimensions** — Subagent Architecture, Context Management, Tool System, Safety Mechanisms, Orchestration |
| awesome-harness-engineering | **12 design primitives** — agent loop, planning, context delivery, tool design, skills/MCP, permissions, memory/state, orchestration, verification, observability, DX, human-in-the-loop |
| Goose | **4 crates** — `goose` (core: orchestration, providers, extensions, sessions, recipes, model registry), `goose-mcp`, `goose-cli`, `goose-server` |

**The finding: they disagree on grouping, not on membership.** The same ~30 components appear in all five. So the *inventory* is stable even where the *architecture* is not — it can be taken as given, and judgement spent only on the cut.

### What is actually open, per the field

The arXiv paper is the most useful single artifact because it reports a **distribution over ~70 harnesses** rather than one author's choice. Nothing below is forced.

| Dimension | Spread |
|---|---|
| Subagent architecture | **None 30%**, Orchestrator-Worker 19%, Tool-delegation 17%, Multi-level recursive 13%, Event-driven 7%, Basic spawn 7%, Swarm 6%, Pipeline 1% |
| Context management | Hybrid 27%, File persistence 23%, Hierarchical 17%, Enterprise 13%, Vector/RAG 10%, LLM summarisation 6%, Window-only 4% |
| Tool system | Registry 34%, MCP-first 14%, Minimalist 11%, Decorator 10%, Plugin ecosystem 10%, Declarative/DSL 9%, Enterprise 9%, Proxy 3% |
| Isolation | Process 45%, Container 31%, None 17%, WASM 7% |
| Audit | None 40%, Basic logging 35%, Structured 20%, Tamper-evident 5% |
| Workflow definition | Imperative 45%, Event-driven 30%, Declarative 25% |
| Planning | ReAct 50%, Plan-and-Execute 35%, Hierarchical 15% |

Two claims worth carrying, both third-party and unverified: harness-only changes reportedly moved LangChain from rank 30 to top 5 on Terminal Bench 2.0; the Blueprint's own thesis is *"the harness matters more than the loop."*

---

## 3. The three donors donate different kinds of thing

Conflating them is the trap.

| Donor | Licence | Donates | Does **not** donate |
|---|---|---|---|
| **Claude Code** | closed | **behaviour** — what the thing must do. Five sessions of `.jsonl` plus daily use. This is the requirements document. | any source |
| **Goose** | Apache-2.0 | **parts** — streaming, provider trait, MCP client lifecycle, tool dispatch, session store, subagent spawn | its **topology**: memory is an MCP extension at the leaf, which is the arrangement arx exists to reject |
| **Hermes** | MIT | **one interface shape** — `MemoryProvider`, initialize/prefetch split | its shell (wrong language) |

**The rule: lift Goose's modules, reject its topology.** Apache-2.0 requires preserving licence and NOTICE text for lifted code; it carries an explicit patent grant.

**Second rule, from this session: copy behind an interface you wrote.** A module pasted in whole is not modifiable-on-feel — it becomes a black box that gets routed around rather than fixed. Since "modify what feels clumsy, from use" is the strongest leg of the argument for building this at all, every donated part needs a seam you own. The donated code is an **adapter at your seam**, never a layer.

Corollary: the copy/build split is also the learn/don't-learn split. Deliberate is fine; accidental is not.

---

## 4-5. Module table (post-D16)

Replaces the old §4 sourcing map and §5 build list. One row per post-D16 module: what it lifts from, whether it is a lift, a lift+adapt, or a build, and the interface it sits behind. Donor is one of Goose, Hermes, Tabularium v1, or none.

| Module | Donor | Lift or build | Seam | Ruled by |
|---|---|---|---|---|
| Native loop and provider trait | Goose | lift+adapt | provider | `975f65cd` (successor of `0be93430`); `7dae481c` (successor of `5efb4c94`) |
| Memory engine — Tabularium v2, Rust, over a file store | Tabularium v1 | build | memory seam | `c775b5c2`, `30e83788` |
| Grounding push at spawn; recall as a model-called tool | none | build | memory seam | `e79bed85`, `a82c08c5` |
| Admission — externalize-plus-handle, no store call | none | build | admission | `bb4099ad`, `e65b2a35`, `0bec7b13` |
| Claude-code adapter and codex adapter, one per CLI kind, role-agnostic | none | build | body | `e652ce3a`, `0bd4df2f`, `705e4df2` |
| Container runtime and allowlist | none | build | dispatch | `068d4cbf`, `ebe94e38` |
| Transcript log, checkpoint and reset ritual, state object | none | build | reset | `eff1f01a`, `adec3734`, `18fec68a`, `d65f8fe1`, `31a20ff6` |
| Gate — allow / deny / ask | none | build | gate | `ce46b9ea`, `96d1ad3a`, `64c789d0` |
| Operator surface | none | build | operator surface | `705e4df2` |
| Instrumentation | none | build | transcript log | `340b3f2e`, `01647e3a` |

The six seams §6 lists below are folded into the seam column above; nothing there wants re-deriving.

This table replaces §5's nine-item build list (written 2026-08-19, annotated dead or reopened 2026-08-27) and stands in for §6's six-seam observation (written 2026-08-24, annotated stale the same pass) rather than re-deriving it. It follows the 2026-08-30 architecture ratification restatement, `af68d429` (map-integrity debate rows S-RUNE-2, S-SOL-4), which ruled the sourcing map replaced rather than annotated a third time. Plans row `S-RUNE-5`, carried as an agreed blocker since the 2026-08-27 standing note above, retires with this rewrite.

## 6. The seams — where donated meets built

This is the real remaining design work. A lifted module's interface is not obliged to fit.

| Seam | The question it poses |
|---|---|
| provider ↔ assembler | who owns request construction — the adapter or the plan |
| tool dispatch ↔ admission | results pass *through* memory before entering the window |
| subagent ↔ trace archival | discarded context goes to the store, not the void |
| store ↔ harness | the trait — and whether `assemble` comes out chatty (`POSITION.md` §2's discriminator) |
| steering queue ↔ step boundary | where the "safe boundary" actually is |
| pressure signal ↔ assembler | who arms, who fires |

> **Two of six seams describe machinery that no longer exists, unflagged until this pass.** `store ↔ harness | the trait — and whether assemble comes out chatty` — there is no `assemble` left to come out chatty or not; `POSITION.md` §2's discriminator fired for a different reason (T03: ordinary admission may make no store call at all) and stands answered elsewhere. `pressure signal ↔ assembler | who arms, who fires` — there is no pressure signal; nothing arms and nothing fires. The Observation below, that all nine build-items sit at these six seams, is weaker than it reads once three of the nine (§5 above) are themselves dead or reopened — worth re-deriving rather than assumed to still hold as stated. **Retired 2026-08-30.** §5 above is gone — its nine-item list is now §4-5's module table, whose seam column is what this whole six-seam passage folds into.

**Observation:** all nine build-items sit at the two window-mutation points `POSITION.md` already covers. The depth-without-breadth was less damning than it felt — the depth landed precisely on the non-donated set. What is missing is not design of the other twenty components; it is these six seams.

---

## 7. Where this touches POSITION.md

**Corroboration.** Blueprint 2026 independently derives a tiered context ladder — Tier 0 structured outputs by default, **Tier 1 immediate large-result eviction**, Tier 2 deferred input/result eviction, Tier 3 compaction/summarisation, Tier 4 fresh-window restart. That is the admission ladder plus eviction, arrived at separately, from the same motivating problem (bulky tool output). Independent derivation, same shape.

> **Diverged 2026-08-24 (`5efb4c94`, `e65b2a35`).** arx now keeps Tier 0 and Tier 1, drops Tier 2 and Tier 3 outright, and hands Tier 4 to the operator as digest-and-clear rather than firing it from the harness. The independent derivation still corroborates the admission half. It no longer corroborates the eviction half, which arx dropped on measurement rather than on argument: T06 found zero eviction events across 270 sessions.

**Tension.** OpenDev fires compaction at the **top of every step**, on pressure. §6 rules it fires **at a seam in the work**, argued from prefix-cache mechanics. Not a refutation — the cache argument is stronger than theirs — but it is the one place the design visibly departs from the field, so it is the one that owes a written reason.

> **Widened 2026-08-24 (`5efb4c94`).** The gap is larger than this paragraph admits, and it did not close in either direction. arx fires compaction at no clock at all, so the departure is no longer about which seam to fire on. The written reason it owes is now the measurement rather than the cache argument.

**Coverage, precisely.** Against the arXiv five dimensions: **Context Management** deep; **Subagent Architecture** and **Tool System** grazed only on their memory-facing edge; **Safety** empty; **Orchestration** empty but for `2d15b921`, which is an ownership claim rather than mechanism. And it is not even all of context management — the field splits that into storage backend, compression strategy, persistence scope, token awareness, while `POSITION.md` covers *the window inside one session*. Session persistence, checkpointing, prompt layout and cache discipline are context engineering too and are absent.

---

## 8. Standing conclusions from this session

Argued and left standing; **not ruled, not stored.** Weakest class in this repo.

- **Executing the sourcing map competently yields an ok-ish harness** — a Goose-clone with a Rust shell and a better compaction pass. Worth stating plainly, because the opposite belief (every component is differentiated) is how harness projects die hand-building all thirty. Of the nine build-items, most are craft or degree; only the never-assertable evidence class and the transcript eval corpus are unusual, and neither is a feature. **Sharpened 2026-08-24 (`5efb4c94`):** there is no better compaction pass, because there is no compaction pass. The comparison to a Goose-clone survives and gets simpler — arx differs from the donor by an append-only window, an admission threshold, an evidence class the donor does not have, and an eval corpus.
- **The bet is singular.** In every donor, memory is a leaf — Goose's is an MCP extension, Claude Code's is six passive layers, the survey's whole context-management dimension is storage strategy. Nowhere is memory in the control path. arx's one structural claim — mandatory party to every window mutation, sole owner of eviction content — has **no instance found in the field**. If it pays, the behaviour is unavailable to any donor. If it does not, arx is a fork with nicer compaction. **Halved 2026-08-24 (`5efb4c94`):** the eviction clause is gone, and it was the half with no instance in the field. What remains of the claim is that memory is a mandatory party to every window mutation, which after `e65b2a35` means a size threshold with two outcomes on the way in. Whether that is still a bet or has become an assembler detail is D03's to answer. If it is the latter, the second branch of this bullet is the live one, minus the compaction. **Corrected 2026-08-27 (D10 grill):** the surviving half has an instance in the incumbent after all — Claude Code externalizes over-threshold tool output to a file and admits a ~2KB preview plus the path (`e65b2a35` already recorded this; this bullet did not). What arx owns is the threshold and who sets it, not the mechanism. And `3117ac26` (2026-08-19) had already rejected the bet-pays-off frame as the success criterion — parity-or-better is the bar, so "fork with nicer compaction" is not the failure case this bullet made it.
- **For the two stated drivers, ok-ish is the whole spec.** Learning and daily-driver are satisfied by *ok-ish but yours*. Differentiation anxiety is imported from a product frame `c8f20ac3` does not have.
- **The lobotomy risk is structural, not social.** Memory can withhold, per tool result, on the hot path, under a policy the model did not author — so the system's ceiling is set by the admission policy rather than by the model. No outside opinion catches this; it is in the design, in daylight.
- **A handle is what makes a removal falsifiable.** This recasts §6 question 7 from a symmetry question into the anti-lobotomy property: drop with no handle and the model cannot disagree, so the mistake is silent and permanent. Currently present at three seams (admission, refusal, rewind) and absent at eviction — the one under pressure. **Closed 2026-08-24 (POSITION §6 question 7, map ticket D08):** the missing seam was deleted rather than fixed, so the property holds everywhere by construction. Admission, refusal, cut-to-archive and traces all demote to a handle, and nothing else removes anything.
- **The control arm is free only while the incumbent is still used.** **Moot 2026-08-27 (D10, `a194176a`):** there is no control arm — arx runs no evaluation against Claude Code, and the falsifier is whether the owner goes back. The bullet stays as history. The day arx becomes the daily driver, Claude Code transcripts stop being generated and the baseline goes stale. §5 *(now §4-5's module table, 2026-08-30)* treats the corpus as a standing asset; it is a **depleting** one. Preserving it costs deliberate work on the old host, which is the first thing dropped when busy.
- **Pull-rate is a rate, and personal-tool volume is low.** Separating "the policy is wrong" from noise may take months of use rather than weeks.
- **The risk that matters is time-to-daily-driver.** Both strong legs of the case for building this — modify-from-use, and learn-regardless-of-outcome — cash out only once it runs daily. Reached independently from "the list is big enough to eat all the effort."

---

## Sources

| Source | Kind | Confidence |
|---|---|---|
| [Architectural Design Decisions in AI Agent Harnesses](https://arxiv.org/html/2604.18071v1) | arXiv, ~70-harness survey | high for the distributions |
| [Building AI Coding Agents for the Terminal (OpenDev)](https://arxiv.org/html/2603.05344v1) | arXiv, first-hand build report | high for the lessons |
| [Handling stop reasons](https://platform.claude.com/docs/en/api/handling-stop-reasons) | official API docs | authoritative |
| [Modern Agent Harness Blueprint 2026](https://gist.github.com/amazingvince/52158d00fb8b3ba1b8476bc62bb562e3) | gist, opinionated | medium |
| [awesome-harness-engineering](https://github.com/ai-boost/awesome-harness-engineering) | curated index | index only |
| [Anatomy of a harness (Mastra)](https://mastra.ai/blog/anatomy-of-a-coding-agent) | vendor blog, first-hand | medium |
| [Building a Coding Agent From Scratch](https://www.decodingai.com/p/building-a-coding-agent-from-scratch-system-design) | blog, first-hand | medium |
| [block/goose (DeepWiki)](https://deepwiki.com/block/goose) | generated docs over real source | medium; verify against source before lifting |
