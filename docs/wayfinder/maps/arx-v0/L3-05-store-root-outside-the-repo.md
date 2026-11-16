---
id: L3-05
layer: 3
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-close
ruled-by: `57aa480a` (the live store is per user, outside every repository, named by `[store] root`; absent, the root stays `<config dir>/store`; the repo's `store/` is deleted and its two FileStore-era rows dropped), `f09b9ed4` (a scope is a directory under the store root and nothing else; the owner creates it, the engine never mints one), `f89798aa` (the store root is its own git repository; `Engine::open` runs `git init` when it finds none)
---

# L3-05 — the store root outside the repository

Added 2026-09-08 on the owner's ruling, the way L2-14 and L2-15 were. Not a want: the defect that keeps `arx run` from starting in this repository, and the first thing the L3-04 day meets.

## Deliverable

`[store] root` in `arx.toml` names the store root. The repository's file points at `~/.local/share/arx/store`, one git repository per user with a directory per scope, the shape CONTEXT.md's Store entry describes. Every consumer of the root reads the one resolved path: grounding and the quarantine listing at spawn, the drain's quarantine read, the hand spec on both dispatch paths, `arx run`'s engine open, `arx ember`. An unregistered scope is still refused, and the refusal prints the directory to create. The repository's own `store/` is gone, its two L1-06 rows with it, and CLAUDE.md's store hazard is rewritten.

## Done when

`cargo test -p arx --test store` carries two more tests: a session whose `[store] root` sits outside its workspace grounds from that root and quarantines into it while the workspace holds no `store/`; a session against a root with no scope directory exits non-zero naming `mkdir <root>/<scope>`. On the owner's login, one operator line piped into `arx` from `/tmp` is answered against `~/.local/share/arx/store`, and the repository has no `store/` afterwards.

## Ruled by

`57aa480a`, `f09b9ed4`, `f89798aa` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
