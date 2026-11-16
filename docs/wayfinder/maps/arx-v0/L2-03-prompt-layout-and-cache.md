---
id: L2-03
layer: 2
labels: wayfinder:task
status: split
assignee: none
blocked-by: L1-close
ruled-by: `bb81d2c6` (first section of the V0 brief), D16 (`68c366dc`), `a82c08c5` (unticketed owner ruling — recall appends at the tail, cache breaks only before the divergence point), D02 (`975f65cd`)
---

# L2-03 — system prompt layout and cache discipline

Prototype, HITL — gate exempt (`gate_run: no`, class prototype).

## Deliverable

A draft of arx's system prompt for a claude-code main, laid out for the prefix cache: the fixed prefix (identity, tool surface, the standing rules), the per-thread section (grounding push, the state object at respawn, the repo instruction file), and the rule for what may never move once a thread starts. The owner reacts to the draft; the resolution is the layout and the discipline, written into L1-03's adapter.

## Done when

The prompt runs in L1-03's adapter; two consecutive turns show a cache hit on the prefix in the body's usage stream; a grounding push or a respawn changes only the per-thread section; the layout is documented in one file the adapter reads.

## Ruled by

`bb81d2c6`, D16 (`68c366dc`), `a82c08c5` (unticketed owner ruling), D02 (`975f65cd`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Split

Cut on 2026-09-05 by main after the owner's reaction to the draft. The reaction added a bounded read and a minimal edit on main's surface, and those tools must exist before the standing prompt may mention them, since a prompt line ahead of the code is a lie the model acts on. [L2-03a — main's bounded read and edit](L2-03a-main-bounded-read-and-edit.md) (the two tools, the wire, the gate subject, the log row) lands first; [L2-03b — prompt layout and cache discipline, the build](L2-03b-prompt-layout-and-cache.md) (the four bands, `prompts/main.md`, the descriptions, the roster, the system row, the measurement) follows. The prototype half of this card is done: the draft and the owner's reactions live at `docs/specs/2026-09-04-l2-03-prompt-layout-draft.md`. This card is not claimed on its own.
