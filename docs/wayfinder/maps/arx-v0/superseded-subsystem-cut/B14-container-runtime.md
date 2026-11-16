---
id: B14
labels: wayfinder:task
status: open
assignee: none
blocked-by: B13
ruled-by: D06 (`068d4cbf`, `ebe94e38`, `96d1ad3a`), D01 (`21e8f9a9` — arx on the host with its hands in containers), D12 (`e79bed85` — the store is never mounted)
---

# B14 — container runtime and allowlist

## Deliverable

Every hand runs in a container arx controls, on the host's process tree: an allowlist that is empty by default — no host env, no home — a worktree per write-enabled hand, the store never mounted, and a native hand's gate running in arx on the host before the call executes in the container. The runtime is what B15 to B18 spawn into.

## Done when

A stub hand in a container sees no host environment variables and no home directory; a write-enabled hand gets its own worktree and its writes are visible there and nowhere else; a hand's attempt to read the store path fails; the container is gone after kill.

## Ruled by

D06 (`068d4cbf`, `ebe94e38`, `96d1ad3a`), D01 (`21e8f9a9` — arx on the host with its hands in containers), D12 (`e79bed85` — the store is never mounted) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).
