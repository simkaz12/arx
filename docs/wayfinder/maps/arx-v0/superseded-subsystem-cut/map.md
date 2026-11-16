---
labels: wayfinder:map
effort: arx-v0
charted: 2026-08-30
tracker: local-markdown
---

# arx-v0 — wayfinder map

> Charted 2026-08-30, the same day the design map closed and its integrity debate ran. This is the build map the design map deferred: `7e74d751` rejected a harness-exists destination *for that effort* and said it "returns only if the destination is redrawn, as a fresh effort". This is that effort. Tickets here are build cards, not decide tickets — see Notes for the execution override.

## Destination

V0 running: arx drives the owner's daily work. A Claude-Code-bodied main under arx, with arx's memory engine, checkpoint and reset apparatus, hands (Claude Code, Codex, native) in containers, and a Codex judge — and the owner has done a full day's work in it. The map ends when the use claim `58b34830` can be tested, not when it is won. Decision `bbfaeafb`.

## The board

Phases encode blocking edges only; order inside a phase is free. Frontier = open, unblocked, unassigned.

**Phase 0 — spine** (all takeable now)
- [B01 — workspace and config shape](B01-workspace-and-config.md)
- [B02 — transcript log](B02-transcript-log.md)
- [B03 — instrumentation events](B03-instrumentation-events.md) — blocked by B02
- ~~[R01 — driving Claude Code from Rust with a CanUseTool callback](R01-claude-code-drive-from-rust.md)~~ — **resolved 2026-08-30**
- ~~[R02 — Codex app-server and exec gaps the trace left](R02-codex-drive-gaps.md)~~ — **resolved 2026-08-30**

**Phase 1 — a main with no hands** (opens as Phase 0 lands)
- [B04 — claude-code main adapter](B04-claude-code-main-adapter.md) — blocked by B01, B02, R01
- [B05 — operator surface v0](B05-operator-surface-v0.md) — blocked by B02
- [B06 — system prompt layout and cache discipline](B06-prompt-layout-and-cache.md) — prototype, HITL; blocked by B04
- [B07 — the gate on main](B07-gate-on-main.md) — blocked by B04, B05
- [B08 — state object and checkpoint](B08-state-object-and-checkpoint.md) — blocked by B04, B05, B03
- [B09 — reset and the CLI-main floor](B09-reset-and-floor.md) — blocked by B08

**Phase 2 — memory** (parallel with Phase 1)
- [B10 — the engine](B10-engine.md) — blocked by B01
- [B11 — grounding push, recall tool, save and drain on main](B11-grounding-recall-save.md) — blocked by B10, B04
- [B12 — admission](B12-admission.md) — blocked by B02, B03

**Phase 3 — hands** (opens as B04 and B07 land)
- [B13 — dispatch primitive and spawn ledger](B13-dispatch-and-spawn-ledger.md) — blocked by B04, B07
- [B14 — container runtime and allowlist](B14-container-runtime.md) — blocked by B13
- [B15 — claude-code hand](B15-claude-code-hand.md) — blocked by B13, B14
- [B16 — codex hand](B16-codex-hand.md) — blocked by B13, B14, R02
- [B17 — native loop and provider trait](B17-native-loop-and-provider.md) — blocked by B01, B12
- [B18 — native hand](B18-native-hand.md) — blocked by B17, B14
- [B19 — capture sweep and drain](B19-capture-sweep-and-drain.md) — blocked by B13, B10

**Phase 4 — judge**
- [B20 — the judge role](B20-judge-role.md) — blocked by B16

**Phase 5 — cutover**
- [B21 — daily-driver cutover](B21-daily-driver-cutover.md) — HITL; blocked by B09, B11, B12, B15, B19, B20

## Notes

- **Execution override.** Wayfinder plans by default; this map does. A card is a build ticket sized to one implementer session. It is resolved by `to-spec` at card grain — the card is the brief's whole world — then hand-do or spec-runner, then spec-reviewer, with the codex gate on the behaviour-change axis as hand-do runs it. One card per session. Cards never merge into a whole-build brief; that is the shape the owner ruled out.
- **Done is a running check.** Every card carries `## Done when` as something that runs — a command, a driven session, a forced failure path — never prose. A card whose done-check cannot be written yet is fog, not a card.
- **Landing against a ruling.** Every card carries `## Ruled by`. If building it shows a ruling wrong, the card stops: the ruling is reopened in the store first, as a fresh superseding record per `ef3b93f2`, and only then does the card continue. The build map never edits the design map; it cites it.
- **Store.** Scope `arx`. Card outcomes are code, not memory; what gets saved is a corrected ruling or a fact that cost something to learn (an SDK trap, a container gotcha).
- **Vocabulary and prose.** `CONTEXT.md` is the glossary; `~/.claude/LANGUAGE.md` the architecture vocabulary; every brief and report passes unslop.
- **Bodies in V0.** Main is Claude-Code-bodied (the daily shape, `96ce60c0`, `705e4df2`). Hands are Claude Code over `claude -p`, Codex over `codex exec`, and native over the provider trait. Judge is Codex. A native main is fog, not a card.
- **Tracker mechanics.** Sibling files; `status:` open/closed, `assignee:` is the claim, `blocked-by:` holds card ids, `ruled-by:` holds the design tickets and decision ids the card answers to.

## Decisions so far

- Board charted with a harness-exists destination and card-grain briefs; one whole-build brief rejected, resuming the design map rejected. Decision `bbfaeafb`. (Charting decision, no ticket.)
- [R02 — Codex app-server and exec gaps the trace left](R02-codex-drive-gaps.md) — `project_doc_max_bytes=0` is a true off-switch; app-server emits usage per round-trip, exec once at `turn.completed`; `--output-schema` is unchecked by Codex, so the judge driver parses with its own failure path (carried to B20).
- [R01 — driving Claude Code from Rust with a CanUseTool callback](R01-claude-code-drive-from-rust.md) — three routes verified (MCP permission tool, SDK sidecar, arx-served `PreToolUse` http hook); B04 takes the ruled SDK sidecar; no `autoCompactEnabled` exists — the lever is the `--autocompact` window ceiling (`c474a7f4` supersedes `e4510547`); the hook route is a cheaper alternative surfaced for the owner, not substituted.

## Not yet specified

- **Native main.** The provider-backed loop in the main seat: rewind as an operator command, rewind-to-fit and the overshoot ladder (`e2b0ceb2`, `8b9f41ae`), checkpoint turns rewound out. B17 builds the loop for hands; the main seat waits until the owner picks a native main, and its cards graduate then.
- **Codex main.** The app-server adapter in the main seat (`70260cad`'s degraded case). Same trigger.
- **Operator surface beyond a terminal.** B05 is a terminal. Whether anything richer exists is not a V0 question.
- **Hot-tier curation.** The engine starts empty and the hot tier is re-typed by hand (`c775b5c2`). What the pass looks like and when it runs is owner work, not a card, until B10 lands.
- **Shortening hand scopes.** Carried from the design map: repeated nudge crossings mean briefs are too big; the counter accumulates in the transcript log until a question sharpens.
- **Card splits.** B17 (the Goose lift) and B08/B09 may each turn out to be two sessions; split at claim time, not now.

## Out of scope

- Design. The design map is closed; nothing here decides. A card that lands against a ruling reopens the ruling in the store (Notes) — it does not reopen a design ticket.
- Phone dispatch for unattended runs — a named reopen outside design-complete (`bb81d2c6`), so outside V0.
- The judge as a third participant, Codex `review/start`, multi-provider critic seats — design reopens, not build cards.
- Tabularium v1 work of any kind; Claude Code stays on v1 (`c775b5c2`).
- Evals against Claude Code — none exist by ruling (`58b34830`); B21 is a use, not a comparison.

## Frontier note

**Charted 2026-08-30.** Twenty-one cards, two research tickets. Frontier: B01, B02, R01, R02. Both research tickets are dispatched at charting per the protocol; their findings land under `findings/`.

**Updated 2026-08-30, R02 resolved.** Frontier: B01, B02; R01 dispatched, pending. B16's blocking edge on R02 is released.

**Updated 2026-08-30, R01 resolved.** Both research tickets closed; frontier: B01, B02. B04's edge on R01 is released and its route is the SDK sidecar. One question for the owner before B04 is briefed: R01 found the `PreToolUse` http-hook route delivers the same per-call verdict with no sidecar and no second runtime, and `96d1ad3a`'s hook rejection was scoped to a hand's foreign body — reopening `e652ce3a`'s main-drive clause is a design call, taken or declined in the store, never inside a card.
