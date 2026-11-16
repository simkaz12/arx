---
id: D14
labels: wayfinder:grill-with-docs
status: closed
assignee: none
closed: 2026-08-29
blocked-by: none
grounding: findings/debate/2026-08-26-arx-plans/ledger.md
---

# D14 — Loose seam ends

New ticket. Two small live items the plans debate raised that have nowhere else to sit — neither is large enough to carry its own ticket alone, and neither belongs inside an existing one.

## Question

- **`ContextWindowExceeded` has no wire-case discriminator.** D02 put it in the termination enum (`03bc048f`: EndTurn / ToolUse / MaxTokens / Refusal{category} / ContextWindowExceeded) and also in the error enum, organized by continuation class (`d8295dc3`: Transient / Fatal / ContextWindowExceeded). Nothing assigns the two variants mutually exclusive wire cases or says which continuation each one drives. This is the one row the plans debate left contested rather than agreed: D02's own round-1 recommendation was to remove the value from the error channel entirely, and what shipped diverges from that recommendation without comment (`S-SOL-3`, plans, contested — rune confirmed the gap itself while contesting sol's "incompatible" framing, sol held). The debate's own proposed settling check: a ruling that assigns the two variants mutually exclusive wire cases and states each one's continuation. Stated here as the open question, not as which enum should carry it.
- **The reset ritual's cost, re-checked rather than assumed.** *Folded into D15 on 2026-08-28 (owner pick; `map.md` frontier note of that date) — grilled with the reset seam whole, not here. Text kept for the record.* *Resolved there the same day: a reset fires against a reserve below the limit, sized to the ritual (`d65f8fe1`), and a window already past the reserve rewinds to fit first, mechanically, then runs the ritual (`e2b0ceb2`). "One participant" did not by itself mean affordable — the reserve is what does.* For the model pool arx can actually afford, context exhaustion is close to the median session outcome rather than an edge case, and the plans debate found that if the reset ritual costs two model calls, an already-over-budget window cannot make them — with no artifact anywhere sequencing what happens before a reset when the window is already past its limit (`S-RUNE-9`, plans, agreed, blocker). The 2026-08-27 observer drop (`749c8765`) may bear on this: the reset apparatus is now one participant, not two, so whatever ritual runs on reset no longer needs the second model call the observer's cross-validation would have cost. Whether "one participant" also means "affordable" — whether main writing the state object itself, inside a window already at its limit, is a call that window can still make — is not stated anywhere and should not be assumed either way. This bullet is a re-check, not a claim that the observer drop resolved it.

## Resolution — 2026-08-29, one grill, three rounds, seven questions, sol silent throughout

The discriminator is the wire's own: a completed response on HTTP 200 terminates, a request rejected before the first stream item is an error. No vendor emits context exhaustion as a stop reason, so the termination variant never had a wire case.

- **Channel.** `ContextWindowExceeded` leaves the termination enum, which drops to EndTurn | ToolUse | MaxTokens | Refusal{category}; it lives on the provider error enum only (`48af6ac7`, supersedes `03bc048f`). D02 round-1's "termination only" and the debate's "both, split by case" were both rejected — the stream-side candidate would have been OpenAI-compatible `finish_reason: "length"`, which the adapter cannot separate from MaxTokens without usage arithmetic and which has no separate continuation.
- **`length`.** OpenAI-compatible `length` translates to MaxTokens unconditionally; if the window is truly at the ceiling the next assembly 400s and the error path takes over one step later (`7e2202a7`).
- **Continuation.** A 400 on the wire is a reserve miss, past the floor D15 ruled — the harness fires rewind-to-fit then the reset ritual itself, no operator wait, and emits a D09 miss-size event that calibrates the reserve (`3a5aefb6`, supersedes `d8295dc3`'s "surface-to-operator" wording).
- **Inside the ritual.** Same rule one level down: rewind by the overshoot the vendor's error names plus the reserve, resend; no retry counter, the window strictly shrinks; the floor is authoring from the archive handle alone, `e2b0ceb2`'s rejected alternative reached mechanically and logged (`8b9f41ae`).
- **The limit.** A per-model config constant the operator keeps from the vendor's published pages; a miss reports that the config may need updating (`aca8f7ef`, owner ruling).
- **Hands.** An arx-native hand's 400 takes the same continuation on its own window; the miss event lands in its trace and counts toward the "shortening hand scopes" fog item (`5f17e29e`). Model-CLI hands manage their own windows.
- **Deferred to build.** How the gauge reads occupancy — vendor `usage` anchor plus local estimate was the recommendation — is settled by using and checking, not on paper; the reserve absorbs the reading's error and the miss event is the calibration (`5fadfcc0`, owner ruling).

Debate row `S-SOL-3` (plans, contested) retires on the first bullet — its own settling check was exactly this ruling. Vocabulary: **Provider error**, **Window limit** added; **Termination enum** and **Reset** amended. Ticket impact: D02's record carries two superseded rulings (`03bc048f` → `48af6ac7`, `d8295dc3` → `3a5aefb6`); D09's event list gains the miss-size event and the ritual-floor event; D15's reset seam gains the in-ritual miss rule.
