---
id: D02
labels: wayfinder:grill-with-docs
status: closed
assignee: none
blocked-by: none
grounding: findings/T01-findings.md, findings/subscription-auth.md
---

# D02 — The provider↔assembler seam

## Question

T01 answered the seam's headline question from the donor: the harness owns request construction, the adapter is pure wire translation. What remains to rule for arx's own trait:

- **Chunk shape.** Goose's stream item is a semantically complete `(Message, Usage)` unit, assembled inside the adapter. Does arx's seam take that shape, or does raw-delta-plus-harness-side-assembly belong on arx's side of the line — and what does each choice cost the event-translation stage?
- **Termination.** No `stop_reason` crosses Goose's seam; termination is structural inspection of the assembled Message, and the seven-value wire richness is collapsed before the loop sees it. Does arx want the same collapse, or does the termination branch (a thin BUILD item in the sourcing map) justify carrying the wire signal across the seam as a typed enum?
- **Capability flags.** `manages_own_context()` leaks a provider property into harness control flow at two call sites — the trait does not donate cleanly. Does arx's trait carry any capability flags at all, and if the CLI-wrapped-provider case is out of scope, what else would demand one?
- **The error enum.** Goose's 13-variant `ProviderError` (no stop-reason variant) versus the error taxonomy `eeedd9fe` cited as a reason for Rust without enumerating — what is arx's, and which variants demand distinct continuations?

**Gained from D08 (2026-08-24, closed without a grill):**

- **The projection step.** Goose splits durable store from request-time projection: `agent_invisible` flags decide what a request sees, and nothing is deleted to make that happen (T01). D08 held this as evidence for a cheap handle at eviction; with eviction gone the split survives as a plain assembly question. Is projection part of the assembler's interface — the same module that owns request construction also decides what of the durable material is rendered — or a stage in front of it with its own seam? T01's closing question asks the same thing from the donor's side. Note what arx's rulings already fix: material that is externalized, refused, archived at a cut, or captured as a trace is out of the window and reachable by handle, so the projection arx needs may be narrower than Goose's flag, or may be the same mechanism under a different name.

**Sharpened by the seats/thread session (2026-08-25):** projection is not only "what does one window see" — under `e0564add`/`e7affb52` the assembler fires once per seat over the same durable material, so projection is the mechanism by which N deliberately different windows are composed. Whatever seam the grill places it at must carry that N-window case, not just the worker's.

**Gained from the D02 opening session (2026-08-25, grill deferred):** the session was claimed, its round-1 frontier laid out, and then spent itself on where Fable can run. Yield: main does not run Fable under current access terms (`ca15f9eb`, scoped with reopen triggers), grounded in `findings/subscription-auth.md` — subscription spend exists only inside Anthropic's own harness surfaces, and both sanctioned routes surrender the window. Consequences for this ticket's bullets: the capability-flags question now has a concrete answer on the table (no flags; model-CLIs enter as hands, never providers — Claude Code included, which retires the case `manages_own_context()` exists for), and chunk shape / termination must span two production adapters from day one — Anthropic-native and OpenAI-compatible, with OpenRouter a `base_url` on the latter, not a third adapter. Round-1 recommendations for all five bullets (adapter-assembled chunks; a typed termination enum, arx's own, with refusal and context-exceeded moved out of the error channel; error enum organized by continuation class; no flags; dissolve projection) were tabled and not yet answered.

**Corrected 2026-08-30 (`975f65cd`):** the no-capability-flags clause above stands, but "model-CLIs enter as hands, never providers" does not — a model behind a CLI enters as a body through the D16 seam, in any role, main included.

**Revised by D14 (2026-08-29):** two of this ticket's rulings have successors. `03bc048f` → `48af6ac7` — `ContextWindowExceeded` leaves the termination enum, which is four values (EndTurn | ToolUse | MaxTokens | Refusal{category}); the channel test is the wire's, a completed response on 200 terminates, a rejection before the first stream item is an error. `d8295dc3` → `3a5aefb6` — the error enum's three classes stand, and ContextWindowExceeded's continuation is now the harness firing rewind-to-fit then the reset ritual, with a D09 miss-size event, rather than "surface-to-operator". OpenAI-compatible `finish_reason: "length"` is MaxTokens unconditionally (`7e2202a7`). The close stands.
