---
id: L2-03b
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-03a
split-from: L2-03
ruled-by: `bb81d2c6` (layout decided at build against the real prompt and tool surface), D16 (`68c366dc` — arx's own system prompt replaces the body's preset; `ceb0390f` — arx includes the repo instruction file itself), `952d3b69` (reset is kill-and-respawn under a CLI body), `a82c08c5` (recall appends at the tail), D02 (`975f65cd`), owner rulings 2026-09-05 (`de2622ef`, `f102a51e`, `eabaa612`)
---

# L2-03b — prompt layout and cache discipline, the build

The prototype half of L2-03 closed 2026-09-05: the draft at `docs/specs/2026-09-04-l2-03-prompt-layout-draft.md` carries the four-band layout, the standing prompt text and the owner's reactions folded in. This card builds it.

## Deliverable

The four bands in the adapter: the standing prompt from `prompts/main.md` as the static block, the per-thread section (workspace line, repo instructions, grounding and quarantine listing "as of this window", the hand roster) as the dynamic block, passed to the sidecar as the Agent SDK's array form around `SYSTEM_PROMPT_DYNAMIC_BOUNDARY`; the ten tool descriptions rewritten as the one source of per-tool semantics; the system row recording both blocks; the hand roster rendered from the definitions with each file's first body line as its role line; the size rule and the escalation rule in the standing prompt, true because L2-03a's tools exist.

## Done when

Two spawns of one config produce byte-identical static blocks and a changed store or repo file changes only the dynamic block (hermetic, stub sidecar); on the owner's login, within the cache TTL, the second turn's `cache_read_input_tokens` covers the first turn's `cache_creation_input_tokens`, and a second `arx run` with a changed per-thread section reads bands one and two from cache; the layout is documented in one file the adapter reads.

## Ruled by

`bb81d2c6`, D16 (`68c366dc`, `ceb0390f`), `952d3b69`, `a82c08c5`, D02 (`975f65cd`), the three owner rulings of 2026-09-05 — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

`prompts/main.md` carries the standing prompt verbatim from the draft's band two; `arx.toml` gained `[main] instructions = "prompts/main.md"`. `config.rs`'s `RoleRow` gained `role_line: Option<String>`, read by a second pass over each definition file past its closing fence (main's own row stays `None` — `arx.toml` is not a fenced markdown file). `main.rs`'s `compose_system_prompt` now returns `claude_code::SystemPrompt { static_block, dynamic }`; the old `compose_store_grounding` folded into a new `compose_dynamic_block`, which renders the five parts (workspace line, repo instructions, store grounding, quarantine, hand roster) in the locked order, joins them with a blank line, and omits any part with nothing to say. `render_hand_roster` walks `resolved.hands` in config order, writing `tools none` when a definition lists none and dropping the trailing `. <role line>` when a definition has no body text. `claude_code.rs` gained the `SystemPrompt` type; the `init` op now carries `systemPrompt: {static, dynamic}`. `transcript.rs`'s `Line` gained `boundary: Option<usize>`, and `append_system` now takes both blocks and computes `text` and `boundary` itself. `gate.rs` gained the `mcp__arx__read_file => path` subject line. `sidecar/claude-main.mjs` imports `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` from the SDK, passes `[static, SYSTEM_PROMPT_DYNAMIC_BOUNDARY, dynamic]` to `query`, and carries the ten rewritten descriptions.

One deviation the type change forced: `SpawnOpts.system_prompt` moving from `String` to `SystemPrompt` reaches into `tests/claude_main.rs`'s `spawn_stub()` helper, the one place outside this brief's own test additions that builds a `SpawnOpts` literal. The literal was updated to the new type with no assertion or behavior change — the stub sidecar ignores `op.systemPrompt` outright, so the fix is mechanical, not a rewrite of what the test checks.

The hermetic half: `stub-init.json`, the whole `init` op written by the stub sidecar next to the fixture, lets a test see exactly what crossed the wire. Four `claude_main.rs` tests: the static block matches `prompts/main.md` byte for byte and the dynamic block carries the workspace line, the store scope, and the hand roster with `hand-cc`'s role line; two `arx run` invocations over the same store share a static block while a planted hot claim moves only the dynamic one; `repo_instructions` lands in the dynamic block and nowhere in the static one; a `[gate] deny` rule on `mcp__arx__read_file` denies the call before it reaches the tool, the same shape as L2-03a's `edit_file` test. One `check_config.rs` test admits a `mcp__arx__read_file(store/*)` rule. `tests/common/mod.rs` gained `fresh_run_fixture_with_store_scope`, used by three of the four. The stub sidecar's new `writeFileSync('stub-init.json', …)` writes to whatever directory the `arx` process itself was started in; since most existing tests don't pin that directory, `.gitignore` gained a `stub-init.json` line so an ordinary test run stops leaving it behind as untracked cruft at the crate root.

Live, on the owner's login: a scratch workspace with the real sidecar, `prompts/main.md`, and the repo's `agents/`, no store scope configured. Run A, two turns inside the cache TTL: turn 1's `cache_creation_input_tokens` was 3535, covering the tool schemas, the static block, and run A's dynamic block; turn 2 read 3535 back — exactly turn 1's creation — and created only 53 more for the turn itself. Run B, a fresh `arx run` within five minutes of run A's last turn with `repo_instructions` pointed at a fifty-line `REPO.md`: its one turn read 3039 tokens from cache (most of run A's total — the shared tool-schema-and-static-block prefix) and created 1082 new (the larger dynamic block, `REPO.md` included). Both system rows carry `boundary: 2619`, unmoved between runs since the static block never changed; the second row's `text` is 1350 bytes longer, all of it in the dynamic half. The draft's three predictions hold: turn 2's read covers turn 1's creation, run B's read is well above zero and of the same order as bands one and two, and run B's creation sits at the size of a dynamic block that now carries a repo file. Scratch workspace removed after; container and hand-root sweep empty before and after.

`cargo test -p arx` after Scope 1: unittests 197 (195 + 2), every other binary at baseline. After Scope 2: `claude_main` 67 (63 + 4), `check_config` 10 (9 + 1), every other binary unchanged. `cargo test --workspace`: 406 passed, 1 ignored (arx 362, arx-engine 44) — the brief's own numbers, exactly. `check-config`'s stdout is byte-identical before and after, since neither `instructions` nor `role_line` prints in the table.

Codex gate: `gate_run: no` — the provider's usage limit is in force until 2026-09-07 08:38, confirmed by one attempt.
