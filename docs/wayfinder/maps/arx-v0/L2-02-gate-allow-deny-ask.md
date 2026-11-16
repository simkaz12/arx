---
id: L2-02
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L1-close
ruled-by: D06 (`ce46b9ea`, `86875715`, `64c789d0`), D16 (`70260cad`, `96d1ad3a`), D04 (`5e8da52e`), `ebe94e38`
---

# L2-02 — the gate: allow, deny, ask

## Deliverable

The real adapter behind L1-05's seam: the rule contract evaluated per call, deny returned and logged, ask landing on the terminal as a prompt with allow / deny and a note, the operator's answer the gate's own return. Adapters narrow a body's richer answers, never widen them. The seam's signature does not move.

## Done when

A scripted session drives one call through each of allow, deny and ask; the ask reaches the terminal and the operator's answer reaches the body; a denied call never executes; every gate decision is a log line; the diff against L1-05 shows no change at the callback's type.

## Ruled by

D06 (`ce46b9ea`, `86875715`, `64c789d0`), D16 (`70260cad`, `96d1ad3a`), D04 (`5e8da52e`), `ebe94e38` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

`Table` in `gate.rs` reads a `[gate]` table in Claude Code's own permission grammar — `deny`, `ask`, `allow` arrays of `Tool` or `Tool(pattern)` rules plus a `hooks` array of command strings — from two tiers: the repo's `arx.toml` and a user file at `$XDG_CONFIG_HOME/arx/gate.toml` (or `~/.config/arx/gate.toml`, or `ARX_USER_GATE` under test). The repo tier is evaluated first and in full; the user tier only if nothing there matched; an empty table still decides allow. Inside a tier the order is deny, then hook denies, then exact allow lines, then hook asks, then the ask list, then the allow list — the exact-allow stage exists so an "always" answer settles a call the ask or hook that raised it would otherwise keep re-asking. Hook rules run the owner's own PreToolUse contract over `sh -c`, five-second timeout, and can only narrow: an allow or no-opinion from a hook is no opinion, never a widen. The owner's `dippy-hook` was exercised live against `rm -rf` and `ls` payloads and answered `ask` and `allow` respectively — see the runner's verdict for the exact transcripts.

An `Ask` verdict never crosses the sidecar wire. `run_turn_cycle`'s gate closure renders the prompt on the terminal seam — `[arx] ask: <tool> <subject>` and a line of four keys, `a`/`A` allow once/always, `d`/`D` deny once/always, a trailing note optional — and blocks on the next operator line; `A`/`D` also append the exact rule to the repo file through a format-preserving TOML edit. Only the resolved `Allow` or `Deny` ever reaches the body, which is the one call taken against this card's own wording: "the operator's answer is the gate's own return" is satisfied by main's gate closure resolving the ask itself, not by the body ever seeing an `Ask`. Every decision is one log line (`Event::Gate` with `by` and `reason`, both omitted on a default allow); an ask writes two — the ask itself, then the answer with `by: "operator"`. A denied call never executes.

`Gate`, `GateCall` and `GateVerdict` are byte-identical to L1-05; `run_turn_cycle` takes `&mut dyn Gate` in place of `&mut AllowAll`, and the `on_gate` callback's type is unchanged. `map.md` is untouched by this card; main writes the frontier entry.
