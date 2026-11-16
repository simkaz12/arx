# SUBJECT
Kind: file
Path: docs/wayfinder/maps/arx-v0/map.md
Context: The subject is the build map for arx v0 (repo `~/Projects/arx`, an agent harness: Rust shell, memory/context policy behind MCP; Tabularium is the memory layer). It sits on the closed design map at `docs/wayfinder/maps/arx-design/` (map.md plus decide tickets D01–D16, whose rulings live in the Tabularium store, scope `arx` plus `global`). Its own cards are sibling files in `docs/wayfinder/maps/arx-v0/` (L1-01…L1-close, L2-01…L2-close, L3-01…L3-04), the resolved research tickets R01/R02 with findings under `findings/`, and the rejected first cut under `superseded-subsystem-cut/`. Repo-root context docs: POSITION.md, CONTEXT.md (glossary), SURVEY.md. The execution machinery the Notes invoke (to-spec, hand-do, spec-runner, spec-reviewer, the codex gate) is the owner's standing fleet, defined outside this repo.

---
labels: wayfinder:map
effort: arx-v0
charted: 2026-08-30
recut: 2026-08-30
tracker: local-markdown
---

# arx-v0 — wayfinder map

> Charted 2026-08-30, the same day the design map closed and its integrity debate ran, and re-cut the same day from subsystem phases to depth layers (decision `4c1ee2ce`, superseding `bbfaeafb`; the subsystem cut is kept under `superseded-subsystem-cut/`). This is the build map the design map deferred: `7e74d751` rejected a harness-exists destination *for that effort* and said it "returns only if the destination is redrawn, as a fresh effort". This is that effort. Tickets here are build cards, not decide tickets — see Notes for the execution override.

## Destination

V0 running: arx drives the owner's daily work. A Claude-Code-bodied main under arx, with arx's memory engine, checkpoint and reset apparatus, hands (Claude Code, Codex, native) in containers, and a Codex judge — and the owner has done a full day's work in it. The map ends when the use claim `58b34830` can be tested, not when it is won. Decision `bbfaeafb`, cut redrawn by `4c1ee2ce`.

## The board

Three layers. A layer is a depth: every seam gets its adapter at that depth, and the layer ends in a check card that runs one scripted session across all of them. The next layer's cards are blocked-by that check. Inside a layer, order is free except where a card names another. Frontier = open, unblocked, unassigned.

**Layer 1 — the loop closes on the host.** Every seam present at its thinnest adapter; nothing in a container; the owner could run a session on it, badly.
- [L1-01 — workspace and config shape](L1-01-workspace-and-config.md)
- [L1-02 — transcript log](L1-02-transcript-log.md)
- [L1-03 — claude-code main adapter](L1-03-claude-code-main-adapter.md) — blocked by L1-01, L1-02
- [L1-04 — the terminal, thin](L1-04-terminal-thin.md) — blocked by L1-03
- [L1-05 — the gate seam, allow-and-log](L1-05-gate-seam-allow-and-log.md) — blocked by L1-03
- [L1-06 — the memory seam with a file adapter](L1-06-memory-seam-file-adapter.md) — blocked by L1-03
- [L1-07 — dispatch and the spawn ledger, with a fake hand](L1-07-dispatch-and-spawn-ledger.md) — blocked by L1-03, L1-05
- [L1-08 — claude-code hand on the host](L1-08-claude-code-hand-on-host.md) — blocked by L1-07
- [L1-09 — codex hand on the host](L1-09-codex-hand-on-host.md) — blocked by L1-07
- [L1-10 — native: the provider trait, one adapter, one turn](L1-10-native-one-provider-one-turn.md) — blocked by L1-01, L1-07
- [L1-11 — the judge as a dispatch under `judge`](L1-11-judge-as-dispatch.md) — blocked by L1-09
- [L1-close — the loop closes on the host](L1-close-loop-on-host.md) — blocked by every L1 card
- ~~[R01 — driving Claude Code from Rust with a CanUseTool callback](R01-claude-code-drive-from-rust.md)~~ — **resolved 2026-08-30**
- ~~[R02 — Codex app-server and exec gaps the trace left](R02-codex-drive-gaps.md)~~ — **resolved 2026-08-30**

**Layer 2 — the seams get real; hands into containers.** The real adapter behind each layer-1 seam, with the seam's signature unchanged.
- [L2-01 — instrumentation events](L2-01-instrumentation-events.md) — blocked by L1-close
- [L2-02 — the gate: allow, deny, ask](L2-02-gate-allow-deny-ask.md) — blocked by L1-close
- [L2-03 — system prompt layout and cache discipline](L2-03-prompt-layout-and-cache.md) — prototype, HITL; blocked by L1-close
- [L2-04 — the engine](L2-04-engine.md) — blocked by L1-close
- [L2-05 — grounding, recall and save over the engine](L2-05-grounding-recall-save-over-engine.md) — blocked by L2-04
- [L2-06 — state object and checkpoint, hands in flight](L2-06-state-object-and-checkpoint.md) — blocked by L2-01
- [L2-07 — container runtime and allowlist](L2-07-container-runtime.md) — blocked by L1-close
- [L2-08 — claude-code hand in the container, clamped](L2-08-claude-code-hand-clamped.md) — blocked by L2-07
- [L2-09 — codex hand in the container, clamped](L2-09-codex-hand-clamped.md) — blocked by L2-07
- [L2-10 — the native loop](L2-10-native-loop.md) — blocked by L1-close; likely two sessions
- [L2-11 — native hand in the container, gate on the host](L2-11-native-hand-gate-on-host.md) — blocked by L2-10, L2-07, L2-02, L2-05
- [L2-12 — the judge: schema, clamp, family warning](L2-12-judge-schema-and-clamp.md) — blocked by L2-09
- [L2-close — the loop closes in containers](L2-close-loop-in-containers.md) — blocked by every L2 card

**Layer 3 — what only traffic can test.**
- [L3-01 — admission](L3-01-admission.md) — blocked by L2-close
- [L3-02 — reset and the CLI-main floor](L3-02-reset-and-floor.md) — blocked by L2-close
- [L3-03 — capture sweep and drain](L3-03-capture-sweep-and-drain.md) — blocked by L2-close
- [L3-04 — daily-driver cutover](L3-04-daily-driver-cutover.md) — HITL, the layer-3 check and the map's end; blocked by L3-01, L3-02, L3-03

## Notes

- **Execution override.** Wayfinder plans by default; this map does. A card is a build ticket sized to one implementer session. It is resolved by `to-spec` at card grain — the card is the brief's whole world — then hand-do or spec-runner, then spec-reviewer, with the codex gate on the behaviour-change axis as hand-do runs it. One card per session. Cards never merge into a whole-build brief; that is the shape the owner ruled out.
- **Horizontal, not vertical.** Cards are cut seam × layer, never seam to full depth. A layer-1 card ships the interface and the thinnest adapter that closes the loop; a layer-2 card ships the real adapter behind the same interface. If a layer-2 card has to move the interface, the layer-1 card was wrong and that is a finding, not a refactor — it goes in the frontier note.
- **Layer gate.** Each layer ends in a check card (`L1-close`, `L2-close`; `L3-04` for layer 3) whose deliverable is one scripted end-to-end session across every seam at that depth. The next layer's cards are `blocked-by` the check. The scripts accumulate: L2-close re-runs L1-close's script unchanged before its own.
- **Done is a running check.** Every card carries `## Done when` as something that runs — a command, a driven session, a forced failure path — never prose. For a thin card the check is on the seam contract: the next layer's adapter compiles against it, the crossing is a transcript-log line. A stub that passes because it does nothing is not a done-check. A card whose done-check cannot be written yet is fog, not a card.
- **Landing against a ruling.** Every card carries `## Ruled by`. If building it shows a ruling wrong, the card stops: the ruling is reopened in the store first, as a fresh superseding record per `ef3b93f2`, and only then does the card continue. The build map never edits the design map; it cites it.
- **Store.** Scope `arx`. Card outcomes are code, not memory; what gets saved is a corrected ruling or a fact that cost something to learn (an SDK trap, a container gotcha).
- **Vocabulary and prose.** `CONTEXT.md` is the glossary; `~/.claude/LANGUAGE.md` the architecture vocabulary; every brief and report passes unslop.
- **Bodies in V0.** Main is Claude-Code-bodied (the daily shape, `96ce60c0`, `705e4df2`). Hands are Claude Code over `claude -p`, Codex over `codex exec`, and native over the provider trait. Judge is Codex. A native main is fog, not a card.
- **Tracker mechanics.** Sibling files; `layer:` 1–3; `status:` open/closed, `assignee:` is the claim, `blocked-by:` holds card ids, `ruled-by:` holds the design tickets and decision ids the card answers to.

## Decisions so far

- Board charted with a harness-exists destination and card-grain briefs; one whole-build brief rejected, resuming the design map rejected. Decision `bbfaeafb`. (Charting decision, no ticket.)
- Board re-cut from subsystem phases to depth layers the same day; layer-1 native width is the provider trait with one adapter and one turn, sized for one gauntlet session. Decision `4c1ee2ce` supersedes `bbfaeafb`. (Owner ruling, no ticket.)
- [R02 — Codex app-server and exec gaps the trace left](R02-codex-drive-gaps.md) — `project_doc_max_bytes=0` is a true off-switch; app-server emits usage per round-trip, exec once at `turn.completed`; `--output-schema` is unchecked by Codex, so the judge driver parses with its own failure path (carried to L2-12).
- [R01 — driving Claude Code from Rust with a CanUseTool callback](R01-claude-code-drive-from-rust.md) — three routes verified (MCP permission tool, SDK sidecar, arx-served `PreToolUse` http hook); L1-03 takes the ruled SDK sidecar; no `autoCompactEnabled` exists — the lever is the `--autocompact` window ceiling (`c474a7f4` supersedes `e4510547`); the hook route is a cheaper alternative surfaced for the owner, not substituted.

## Not yet specified

- **Native main.** The provider-backed loop in the main seat: rewind as an operator command, rewind-to-fit and the overshoot ladder (`e2b0ceb2`, `8b9f41ae`), checkpoint turns rewound out. L2-10 builds the loop for hands; the main seat waits until the owner picks a native main, and its cards graduate then.
- **Codex main.** The app-server adapter in the main seat (`70260cad`'s degraded case). Same trigger.
- **Operator surface beyond a terminal.** L1-04 is a terminal and grows only by message kind. Whether anything richer exists is not a V0 question.
- **Hot-tier curation.** The engine starts empty and the hot tier is re-typed by hand (`c775b5c2`). What the pass looks like and when it runs is owner work, not a card, until L2-04 lands.
- **Shortening hand scopes.** Carried from the design map: repeated nudge crossings mean briefs are too big; the counter accumulates in the transcript log until a question sharpens.
- **Card splits.** L2-10 (the Goose lift) and L2-06 may each turn out to be two sessions; split at claim time, not now.

## Out of scope

- Design. The design map is closed; nothing here decides. A card that lands against a ruling reopens the ruling in the store (Notes) — it does not reopen a design ticket.
- Phone dispatch for unattended runs — a named reopen outside design-complete (`bb81d2c6`), so outside V0.
- The judge as a third participant, Codex `review/start`, multi-provider critic seats — design reopens, not build cards.
- Tabularium v1 work of any kind; Claude Code stays on v1 (`c775b5c2`).
- Evals against Claude Code — none exist by ruling (`58b34830`); L3-04 is a use, not a comparison.

## Frontier note

**Charted 2026-08-30.** Twenty-one cards in five subsystem phases, two research tickets. Frontier: B01, B02, R01, R02. Both research tickets dispatched at charting; findings under `findings/`.

**Updated 2026-08-30, R02 resolved.** Frontier: B01, B02; R01 dispatched, pending.

**Updated 2026-08-30, R01 resolved.** Both research tickets closed; frontier: B01, B02. One question for the owner before the main adapter is briefed: R01 found the `PreToolUse` http-hook route delivers the same per-call verdict with no sidecar and no second runtime, and `96d1ad3a`'s hook rejection was scoped to a hand's foreign body — reopening `e652ce3a`'s main-drive clause is a design call, taken or declined in the store, never inside a card.

**Re-cut 2026-08-30.** The owner read the board and rejected its shape: subsystem phases meant each seam was built to full depth before the next existed, the hands chain was eight serial sessions, and nothing ran end to end until the last card. Redrawn as three depth layers — 29 cards (11 + check, 12 + check, 4) from the same 21 deliverables and the same ruling ids, with the subsystem cut parked under `superseded-subsystem-cut/`. Two rulings taken in the re-cut: layer 1's native width is the provider trait with one adapter and one turn ("for gauntlet loop it should be ok to be done"); the layer gate is a check card so the tracker's `blocked-by` semantic stays as it was. Decision `4c1ee2ce`. Frontier: **L1-01, L1-02** — the same two cards as before, because the workspace and the log have no thinner form. The R01 hook-route question stands and is still the owner's, not L1-03's.
