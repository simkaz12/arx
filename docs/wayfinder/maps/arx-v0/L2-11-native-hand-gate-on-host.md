---
id: L2-11
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-10, L2-07, L2-02, L2-05
ruled-by: D12 (`e79bed85` — recall as a tool on a native hand's surface when its definition lists it), D06 (`96d1ad3a` — gate on the host; `068d4cbf` — per-body auth off the designated paths), D04 (`5e8da52e` — a native child holds at the wall), `69946e37`
---

# L2-11 — native hand in the container, gate on the host

## Deliverable

L2-10's loop in the hand role inside L2-07's container: grounding pushed at spawn, recall on its surface when the agent definition lists it, its credential injected off the container's designated paths with no host env and no home (`068d4cbf`), the gate running in arx on the host before each call executes, a permission wall becoming a child-initiated stop that lands in main's window, the brief as its whole orientation.

## Done when

A native hand on a DeepSeek-class model completes a read-only brief; a call outside its allowlist holds at the wall, the stop reaches main, main's answer releases or kills it; a recall from the hand is served by L2-04's engine on the host and never touches the container's filesystem; the hand's provider credential is present off the designated paths and nowhere in the environment.

## Ruled by

D12 (`e79bed85`), D06 (`96d1ad3a`, `068d4cbf`), D04 (`5e8da52e`), `69946e37` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

The last host body moved its tool calls into a container it keeps for the whole brief. `ToolHost` is the seam: `LocalHost` for the unit tests, `ContainerHost` running one `docker exec` per call — `realpath` then `cat`, both bound to `/hand/repo` — over `Container::exec`/`workdir`, new on `Container`. Every call crosses L2-02's gate on the host before it runs: allow executes; deny returns the reason as the tool result and the trace's `gate` row carries it; ask sends `Delivery::Stop` (`Event::Wall` on the walled stop) and holds the hand on a channel the watcher feeds until main's steer lands — a text that trims to `allow` runs the call, anything else denies with that text, and a kill while holding ends the brief with no delivery. `recall` joins `read_file` as a `ToolDef` only when the definition lists it, served by a per-hand read-only engine handle on the host; the store is never mounted, proven by a masked-read probe in the same container. The native trace became the directory shape the other bodies carry, arx's own step rows swept in as `transcript.jsonl`; the flat `threads/<thread>.<hand>.jsonl` is gone. The provider key stays in the hand's own thread on the host the whole time — no `docker` argv, no `-e`, no mounted file ever carries it, checked live against `docker inspect` on a running hand.

Three deviations from the brief's own wording, each with evidence: the `read_file` exec script drops `realpath`'s `--` — BusyBox's `realpath` has no option parser and reads `--` as a literal path, verified live (`docker exec ... realpath -- /tmp` prints the `--` error and still resolves `/tmp` on a second argument) — dropping it is the only form that both denies an escape and finds a real file. The container is polled into `State.Running` before the loop starts, and teardown retries `kill` against `try_wait` rather than a single `kill`-then-`wait`: `docker kill`/`docker exec` racing the moment `docker run` returns both surfaced as "No such container" under concurrent dispatch, reproduced live with two bare `docker run`+immediate-`kill` pairs before the fix and clean after. Two tests needed retargeting beyond a path swap: `native_hand_kill_aborts_the_stream` and the new gate.rs subject-matching test poll the ledger's `spawn` row rather than the swept trace file, since the trace now exists only after teardown; `format_delivery`'s `Stop` arm skips its own "— answer with the steer tool" suffix when `wall` is set, since the native hold's question already carries it and the fake's does not.

The live check ran three real DeepSeek-bodied briefs from a real Claude Sonnet 5 main against a scratch repository: a plain `read_file` landed `allow`; a `secret.txt` read held at the wall, main answered `allow` through the steer tool, and the hand's contents came back; a `recall` hand named the planted claim. `docker inspect` on the running second hand showed no key in `Config.Env` or `Args`; the scratch tree was removed after.
