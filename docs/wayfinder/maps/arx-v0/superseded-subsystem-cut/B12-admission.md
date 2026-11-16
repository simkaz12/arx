---
id: B12
labels: wayfinder:task
status: open
assignee: none
blocked-by: B02, B03
ruled-by: D03 (`bb4099ad`, `0bec7b13`, `3d559805`, `32809bf6`, `1fa2bae0`), D07 (`e65b2a35`), D11 (pull mints no artifact; echo counter), D16 (`70260cad` — admission covers mounted tools only)
---

# B12 — admission

## Deliverable

Admission per tool result on main's mounted surface, the only in-loop clock: inline below 1k tokens, externalize-plus-handle above it; a per-tool override table that ships empty; the body lands on disk under the session that produced it, write-once, frontmatter carrying provenance; the handle is a filesystem path the ordinary read tool pulls; a session-scoped result cache with sha invalidation; a pull mints no artifact and counts as an echo when re-externalized. No store call in either direction. No tool-pair splice.

## Done when

A mounted tool returning 12k tokens lands as a handle and the model reads it back with the read tool; the same call again hits the cache; changing the underlying file invalidates it; the pull and echo counters in the log match the scripted sequence; a result under 1k is inline.

## Ruled by

D03 (`bb4099ad`, `0bec7b13`, `3d559805`, `32809bf6`, `1fa2bae0`), D07 (`e65b2a35`), D11 (pull mints no artifact; echo counter), D16 (`70260cad` — admission covers mounted tools only) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
