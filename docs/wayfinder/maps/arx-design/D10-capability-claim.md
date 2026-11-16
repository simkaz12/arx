---
id: D10
labels: wayfinder:grill-with-docs
status: closed
assignee: none
blocked-by: none
grounding: findings/debate/2026-08-26-arx-premises/ledger.md, findings/debate/2026-08-27-arx-next-steps/ledger.md, docs/research/memory-2026.md
---

# D10 — The capability claim

New ticket, opened from the two 2026-08-26/27 debate runs rather than from a research ticket — no T-series findings ground it, only the debate ledgers themselves. **Gates D09**: the eval apparatus cannot decide what to label until this ticket says what it is certifying against.

## Question

No record since the 2026-08-26 trim states, as a falsifiable claim, what arx does that Claude Code structurally cannot. Both seats of the premises debate agreed on this at blocker severity, from opposite directions: the "singular bet" framing was halved at some point and referred to D03, and D03 closed on admission mechanics without ever returning to answer the capability question it was handed (`S-RUNE-1`, premises, agreed, blocker); and from the deliverable side, nothing currently separates arx's output from a well-configured Claude Code session running the owner's own skillbook (`S-SOL-1`, premises, agreed, blocker). Neither seat proposed what the claim should say — both only established that none exists.

Three further findings sharpen why this is hard rather than merely undone:

- **The destination excludes the only way to test it.** The map's own ruled destination (`7e74d751`) is design-complete — tickets are decision questions, never build slices, and the map ends exactly where `to-spec` picks up. The daily-driver motive (POSITION §1) needs something built and run to be judged against Claude Code; nothing in the destination as drawn produces that (`S-SOL-2`, premises, agreed, blocker). Does the capability claim get written and then wait for a redrawn destination to pay off, or is there a form of the claim design work alone can certify?
- **The pricing constraint may make the claim untestable as stated.** Main cannot run the top-tier model under current subscription terms (`ca15f9eb`); any comparison therefore runs arx on a mid-tier API model against Claude Code on a top-tier subscription incumbent, confounding harness against model rather than isolating either (`S-RUNE-2`, next-steps, agreed, blocker). The same research this map now holds elsewhere records that swapping only the backbone model moves accuracy by double-digit points and inverts rankings across harnesses (`memory-2026.md` §4, MemDelta figures: Gemini +14pp from full context, Sonnet +31pp from RAG). If "the harness dominates" is the claim, what design would let arx test it without the model itself being the actual variable?
- **The daily-driver driver has not been re-priced since the model-access ruling.** The affordable model pool arx is left running on is a weaker decider than whatever was originally imagined carrying the loop, and nobody has gone back to ask whether the original driver still holds against it (`S-RUNE-5`, premises, agreed, major) — the same confound `S-RUNE-2` raises from the testability side, raised here from the motivation side instead.

One shape was floated in the next-steps debate without being adopted by either seat as a ruling, named here as material for the grill rather than as an answer: a narrow, authoritative claim carrying a same-model, same-task, same-budget eval contract, paired with a destination redraw that permits the one discriminating slice such a contract would need (`S-SOL-1`, next-steps, agreed, blocker — agreed as a *next step*, not as this ticket's ruling). Naming it here is not adopting it.

## Closed 2026-08-27 — after one grill, two rounds

**Ruling (`a194176a`, superseding the round-1 draft `4b756d4b`):** arx makes no claim against Claude Code and runs no evaluation against it. The falsifiable statement is a use claim — arx replaces Claude Code as the owner's daily driver and the owner does not go back. Falsified by going back; nothing else. *(Superseded 2026-08-30 by `58b34830`: the falsifier given an observable — under the ruled daily shape main is Claude-Code-bodied, so the owner never structurally leaves Claude Code; falsified when the owner runs bare `claude` for the day's work instead of arx driving it.)* The reason it is arx is control: working on Claude Code, the owner's approach kept evolving, and a harness the owner owns changes with the workflow instead of being worked around. Owner, verbatim: "I don't really need to evaluate arx vs cc. I don't really want to prove anything… if it works it works and I got more than one win out of this. If not I am going back to cc. Simple as that."

**What the debates could not see.** `3117ac26` (2026-08-19, in force) had already rejected differentiation-as-goal — parity-or-better against the donors is the bar, and "does the structural bet pay off" was refused as a pass/fail gate because it imports a product criterion the drivers do not have. The ledgers cite ids they cannot dereference, so both seats read the absence of a capability claim as an omission. It was a ruling. `3117ac26` stands, with parity now meaning the owner's own felt verdict in daily use, not a measured one.

**The three sharpening findings, dispositioned.**
- *Destination excludes the test* (S-SOL-2) — moot; there is no test. No redraw of `7e74d751`.
- *Pricing confound* (S-RUNE-2, fact `906de3a7`) — describes a test nobody is running. The fact stays true and idle.
- *Daily-driver re-pricing* (S-RUNE-5) — answered by the owner rather than priced: the model landscape moves faster than the build (GLM 5.3 Flash is already far cheaper and Opus-4.8-level on some things), so whatever is cheaper or better when arx is done is what it runs. A comparison declared now against today's models would be stale by the time it could run.

**Rejected:** the round-1 instrument claim ("quality comes mostly from the harness" as the statement arx exists to test); a three-cell same-model eval (arx(M) vs CC(M), arx(M_top) vs arx(M_affordable)); the floated narrow-claim-plus-eval-contract shape from the next-steps debate; a control arm kept alive on Claude Code (SURVEY §8's depleting-baseline bullet is moot with it).

**Also landed in passing:** SURVEY §8's "no instance found in the field" for the surviving half of the bet was wrong — Claude Code externalizes over-threshold tool output to a file and admits a ~2KB preview plus the path, observed live this session; `e65b2a35` had already recorded it, SURVEY had not. Annotated in place.

**Consequence for D09:** unblocked. Its apparatus certifies against nothing external; whatever survives of it is arx's own instrumentation for tuning arx's own policies, and its control-arm bullet is moot. Whether the rest of D09 still needs a grill is D09's own question.
