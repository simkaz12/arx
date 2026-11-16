---
id: L2-05
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-04
ruled-by: D12 (`e79bed85`), D01 (`40ebd8d3`), `a82c08c5` (unticketed owner ruling — mid-session recall stays, results tail-appended), D16 (`e652ce3a`), D13 (`7be68c4a`, `da0b0d8b`, `27e7cea9` — the save lifecycle and the drain's two pens)
---

# L2-05 — grounding, recall and save over the engine

## Deliverable

L1-06's file adapter swapped for L2-04's engine: grounding at spawn is hot plus ember for the thread's scope from the engine; recall, decisions_query and decision go to it; a save from main is intent → landscape return → quarantine, never a direct commit — the drain's promotion is the commit (`da0b0d8b`, `27e7cea9`). Drain on main's mounted surface as two pens per `27e7cea9`, the proposing hand side stubbed until L3-03 and main's minting live. If L2-03 has landed, the cache check runs after a recall.

## Done when

L1-06's done-check passes unchanged over the engine; a save intent from main returns a landscape and lands in the quarantine with the thread handle, with no store commit; a drain promotion minted by main is one commit carrying the thread handle; grounding in main's first turn shows only the scope's hot and ember records; L2-03's cache check, where landed, still passes after a recall.

## Ruled by

D12 (`e79bed85`), D01 (`40ebd8d3`), `a82c08c5`, D16 (`e652ce3a`), D13 (`7be68c4a`, `da0b0d8b`, `27e7cea9`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

`cmd_run` opens `Engine::open` once against an absolute `<config dir>/store` and holds it as `&mut dyn Store` for every call site, the way it already held `gate: &mut dyn Gate`; `FileStore` is untouched, its tests still run, and the trait's six shipped signatures and three `_with` methods took no edit. `[store]` gained one optional key, `embedder`, the Ollama base URL — present, main wires `Engine::with_embedder(OllamaEmbedder::new(url, "nomic-embed-text"))`; absent, the landscape stays BM25-only; empty, `check-config` refuses with `store.embedder must not be empty`. `on_store_save` calls `store.save_with`, returns `landscape.claims` byte-identical to `serve`'s save shape, and writes `landscape.degraded` as one `[arx] store: landscape degraded — <reason>` line on the terminal seam when the embedder is down; `Landscape.legs` never reaches the model. Grounding at spawn is exactly `Store::grounding(scope)` — hot claims plus embers, nothing else — and `read_quarantine_listing` keeps orienting on the raw quarantine JSON unchanged.

The drain grew its two-pen shape on main's surface: `drain_with(suggestions, handle)`, where `None` promotes every candidate (the stub proposer L3-03 replaces) and `Some(list)` must cover every quarantined id exactly once with `promote`, `promote` carrying `supersedes`, or `drop` — one uncovered, unknown or duplicate id and nothing is written. A `Suggestion`'s `supersedes` validates through the same `UnknownClaim`/`NotSupersedable` path a `SaveIntent.supersedes` takes; a candidate's own persisted `supersedes` (from `None`) keeps the engine's original `MalformedClaimFile` path, since that failure is about a corrupt quarantine row, not a caller's bad target. One commit per drain, subject `drain <thread>@<turn>: promoted N, dropped M`; `Engine::drain()` and its own `drain <thread>: promoted N` subject are untouched, both routed through a shared plan-resolve/validate/write path so the file-writing and the commit are not duplicated. `DrainReport.dropped` is additive with `skip_serializing_if` at zero, so the L1-06 null-store JSON assertion needed no edit. `arx ember [--path] <scope> <text>` opens the thread's transcript, refuses with `ember: thread main has no committed turn yet — run a session first` when nothing is committed, otherwise mints through `Engine::ember` at the last committed turn's handle and prints `ember <id> <root>/<scope>/embers/<id>.md`.

L1-06's done-check kept every test name and every assertion's meaning at the locked count of 49; only the file-shape probes moved from `store/testscope.jsonl` rows to `store/testscope/*.md` files and the store repo's own commit count — the one deviation from the card's literal "passes unchanged" wording, which the brief itself sanctioned. Fixtures seed through the engine's own API alone (`plant_engine_scope`, replacing `planted_claims`): `save`+`drain` two facts, the hand-edited `label: hot` `hot_label_edited_by_hand_grounds_after_reopen` already performs, then one `ember`. A latent bug surfaced wiring `Engine::open` under `cmd_run`'s existing relative config-path convention — `git -C <relative-root> add -A -- <relative-root>/.gitignore` resolves the pathspec against the wrong directory and fails `git add`; fixed by resolving the store root to an absolute path in `cmd_run` and `cmd_ember` before opening the engine, one call site each, no engine-side change. The live check ran end to end on the owner's login against the real sidecar, Claude Code body and host Ollama: a fact saved, drained into one commit, hand-labelled hot, embered by the new command, and a paraphrase save on session three surfaced the hot claim through the cosine leg with a live `nomic-embed-text` vector on disk — the repository's own tracked `store/` was never touched. `map.md` is untouched by this card; main writes the frontier entry.
