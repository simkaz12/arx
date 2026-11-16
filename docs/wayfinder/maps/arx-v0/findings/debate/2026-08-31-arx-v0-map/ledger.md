# Ledger — arx-v0 build map debate, 2026-08-31

Subject: file — `docs/wayfinder/maps/arx-v0/map.md`, the depth-layer build board charted and re-cut 2026-08-30, with its 29 cards, the resolved R01/R02 research tickets and the closed design map underneath it. Seats: rune (Claude seat, hand-ex on fable), sol (Codex seat, companion `task`, read-only). Three rounds, nonce `61405e`, run dir `~/.claude/discuss/2026-08-31-arx-v0-map/`. Store scope `arx` + `global`, live and read-only for both seats; 23 records resolved into `rulings.md` (29.0k of the 30k cap) over three walks — 16 cited by the subject, 6 added after round 1, 1 (`4f90f38f`) after round 2. Not walked in, for cap room: `952d3b69` and `31a20ff6`, which rune cited only as "these resolve" with no claim on their content, and the six ids appearing solely inside rune's verbatim quotes of the design map and card frontmatter (`bb4099ad`, `0bec7b13`, `3d559805`, `32809bf6`, `1fa2bae0`, `40ebd8d3`) — a seat needing one could pull it live. No courier failures, no delimiter breaches, no vendor naming. Complete.

## Table

| id | claim | severity | rune | sol | status | fix | settling check |
|----|-------|----------|------|-----|--------|-----|----------------|
| S-SOL-1 | L2-04 revives digest records and omits the ruled four-field claim schema (`7be68c4a`) | blocker | confirm | hold | agreed | rework L2-04's record model to fact/decision claims with question/answer/why/handle and the ember tier; digest deleted | — |
| S-SOL-2 | Cards make save an immediate store write, against `da0b0d8b`/`27e7cea9`'s landscape → quarantine → two-pen drain | blocker | confirm | hold | agreed | respec the save path across L2-04/L2-05/L2-close as intent → landscape return → quarantine file; drain proposes cross-family, main writes promotions; commit-log assertion moves to the drain's promotions | — |
| S-SOL-3 | The native hand's exhaustion continuation (`5f17e29e`) has no card; L2-10's check stops at surfacing ContextWindowExceeded | blocker | confirm | hold | agreed | a native-hand exhaustion card (new layer-2 card or L2-11 extension) whose done-check forces a hand over its window and requires the same brief to finish after respawn | — |
| S-SOL-4 | Capture sweep never proves worktree-diff survival; teardown can erase the only copy of write-enabled work (`4f90f38f`) | blocker | confirm | hold | agreed | add diff survival to the sweep's contract — done-check tears down a write-enabled hand and applies the work from the trace handle alone — and pull the capture into L2-07's teardown check | — |
| S-RUNE-1 | Four ruled-by citations point at findings paths that do not exist under arx-v0 | major | hold | confirm | agreed | repoint all citations to the `../arx-design/findings/` form L2-04 already uses, including T01 and the codex drive-surface finding | — |
| S-RUNE-2 | L1-close demands "a stop from a hand answered by main" that no real layer-1 body can raise | major | hold | confirm | agreed | rune: script the stop through L1-07's fake hand by name in L1-close, and add the real-native-hand stop assertion to L2-close where L2-11 gives it a producer / sol: keep the fake hand in the layer-check explicitly as the stop producer, alongside the real-body checks | — |
| S-RUNE-3 | End-condition contradiction: L1-close claims first testability of `58b34830` while L3-04 is the map's end | major | hold | confirm | agreed | end the map at the full-depth cutover day as the claim's one test; strike L1-close's "first point it can be tested" gloss and recast its script as a layer-depth rehearsal/probe | — |
| S-RUNE-4 | The owner's hook-route call gates L1-03 only in frontier prose; the tracker releases L1-03 without it | major | hold | confirm | agreed | rune: give owner questions a tracker form — an owner-call entry in blocked-by that clears only when the store records the `e652ce3a` hook clause taken or declined — and wire L1-03 to it / sol: record the hook-versus-sidecar ruling before L1-03 can become frontier, then cite the ruling and chosen route on L1-03 | — |
| S-RUNE-8 | The landing-against-a-ruling stop cannot fire on the D13 lifecycle rulings — no card carries `da0b0d8b`/`7be68c4a` | major | hold (narrowed) | confirm | agreed | rune: add `da0b0d8b` and `7be68c4a` to L2-04's and L2-05's ruled-by so the stop fires and forces the lifecycle rework / sol: add `7be68c4a`, `da0b0d8b` and `27e7cea9` to the relevant memory cards and reflect each obligation in their deliverables and checks | — |
| S-SOL-5 | No layer-2 card owns `068d4cbf`'s mandatory per-body container auth | major | confirm | hold | agreed | name auth injection as a deliverable and done-check on the real container cards (L2-08/L2-09/L2-11), citing `068d4cbf`: credential present off the designated paths, each real body starts with no host env or home | — |
| S-SOL-7 | L2-10 admits two sessions against one-card-per-session; the split rule is unenforced prose | major | confirm (narrowed) | hold | agreed | rune: a needs-split tracker status on L2-10 and L2-06 that keeps them off the frontier until split / sol: split L2-10 into independently runnable cards before it reaches frontier | — |
| S-SOL-8 | L3-04 can close the map while minting new open cards on it | major | confirm | hold | agreed | rune: route the day's findings to store records and a named successor effort's inbox; rule that a closed map accepts no new cards / sol: keep L3-04 blocked by V0 correctness findings and route non-blocking cutover learnings to a named successor effort | — |
| S-RUNE-5 | `a82c08c5` is misattributed to D01 and D03; it is an unticketed owner position | minor | hold | confirm | agreed | cite it in all three frontmatters as the unticketed owner ruling with its own id, outside any D-ticket parenthesis | — |
| S-RUNE-6 | map.md quotes design-map prose as `7e74d751`'s words | minor | hold | confirm | agreed | reattribute map.md:11 — the ruling rejected destination B; the "returns only if the destination is redrawn" clause is the design map's own prose, cited as such | — |
| S-RUNE-7 | L1-close's flat-file store assertion breaks "script unchanged" after the engine swap | minor | hold | confirm | agreed | make the accumulated script adapter-neutral (seam crossings via transcript-log lines only); flat-file and commit-log assertions live in per-layer checks outside it, written into both close cards | — |
| S-SOL-6 | L1-01 fixes a configuration interface too narrow for later layers | major | contest | withdraw | withdrawn | — | — |

## Verdict

**Bottom line.** The seats converged on 15 of 16 items — every blocker and major except S-SOL-6, which sol withdrew after rune showed the agent-definition block extends additively and the signature rule binds adapter seams, not config. Nothing is contested: the depth-layer cut itself survived both sweeps; what failed is card text — the memory cards contradict the ruled save lifecycle, four ruled obligations own no card or check, and the board's bookkeeping (findings paths, stop producer, end condition, owner-gate, misattributions) is wrong in place. Both seats call **fix-first**: rune on S-SOL-1/2/3/4 and S-RUNE-1/2/3/8 (first briefs would build against in-force rulings or cite files that do not exist; no board redraw needed), sol on S-SOL-1/2/3/4 and S-RUNE-8 (core ruled behaviour absent or contradicted; the rest is map repair, not a new destination).

**Fix.**
- S-SOL-1 — rework L2-04's record model: fact/decision claims with question/answer/why/handle, ember tier, digest deleted (blocker)
- S-SOL-2 — respec the save path across L2-04/L2-05/L2-close as intent → landscape → quarantine, drain proposes cross-family and main writes promotions; commit-log assertion moves to the drain's promotions (blocker)
- S-SOL-3 — a native-hand exhaustion card whose done-check forces a hand over its window and requires the same brief to finish after respawn (blocker)
- S-SOL-4 — diff survival in the sweep's contract: tear down a write-enabled hand, apply the work from the trace handle alone; pull the capture into L2-07's teardown check (blocker)
- S-RUNE-1 — repoint the four findings citations to the `../arx-design/findings/` form; give L2-10's T01 a path (major)
- S-RUNE-2 — rune: name the fake hand as L1-close's stop producer and add the real-native stop to L2-close / sol: keep the fake hand in the layer-check explicitly as the stop producer alongside the real-body checks (major)
- S-RUNE-3 — end the map at the cutover day as the claim's one test; strike L1-close's testability gloss, recast as rehearsal (major)
- S-RUNE-4 — rune: an owner-call entry in blocked-by cleared only by the store recording the `e652ce3a` hook call / sol: record the hook-versus-sidecar ruling before L1-03 can become frontier, then cite it on L1-03 (major)
- S-RUNE-8 — rune: add `da0b0d8b`/`7be68c4a` to L2-04/L2-05 ruled-by / sol: add those plus `27e7cea9` to the memory cards and reflect each in deliverables and checks (major)
- S-SOL-5 — auth injection as deliverable and done-check on L2-08/L2-09/L2-11, citing `068d4cbf` (major)
- S-SOL-7 — rune: a needs-split tracker status keeping L2-10/L2-06 off the frontier until split / sol: split L2-10 before it reaches frontier (major)
- S-SOL-8 — rune: findings to store records and a successor effort's inbox, closed map accepts no new cards / sol: L3-04 blocked by correctness findings, non-blocking learnings to a successor effort (major)
- S-RUNE-5 — cite `a82c08c5` as the unticketed owner ruling in all three frontmatters (minor)
- S-RUNE-6 — reattribute map.md:11's quote to the design map's prose (minor)
- S-RUNE-7 — adapter-neutral accumulated script; adapter artifacts in per-layer checks outside it (minor)

**Settle first.** None — no contested rows.

**Handoff.** A plan subject: the Fix rows go to `grill-me`.
