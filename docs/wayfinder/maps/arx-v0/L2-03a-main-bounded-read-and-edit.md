---
id: L2-03a
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L1-close
split-from: L2-03
ruled-by: owner rulings 2026-09-05 (`de2622ef` — escalation is main's judgement, never the definition's; `f102a51e` — minimal edits on the same footing as bounded reads; `eabaa612` — bounded is the tool's property), `f5b5caf4` (delegate discipline is prompt taste where main has edit tools; children hold no store-write surface), `70260cad` (main's surface under a Claude Code body is arx-owned), `3a873e9a` (no store-write surface for children)
---

# L2-03a — main's bounded read and edit

## Deliverable

Two tools on main's surface, `read_file` and `edit_file`, served by arx on the host over the same wire and gate as the ten existing tools: reads bounded to the workspace and cut at a fixed cap with a trailer that says so; edits as one exact-once replacement in one existing file, refusing arx's own state; one `file` row in main's transcript log per call. Born from the owner's reaction to the L2-03 draft: a hand's verdict is a claim main could not check, and a one-line fix cost a hand's spawn. The prompt lines that tell main when to use them are L2-03b's, so the standing prompt never names a tool that does not exist.

## Done when

On the stub sidecar: a read returns a workspace file and a `file` row records it; a read past the cap ends with the truncation line; a read outside the workspace and an edit under `store/` are refused with the locked texts; an edit changes exactly one occurrence and the row carries old and new; a `[gate]` rule patterned on `edit_file`'s path denies it before it runs. On the owner's login: a real Claude-bodied main reads `arx.toml` and edits a planted file on request, both rows in the log.

## Ruled by

The three owner rulings of 2026-09-05 above, `f5b5caf4`, `70260cad`, `3a873e9a` — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

Both tools land on `sidecar/claude-main.mjs`'s `arx` server as `read_file { path }` and `edit_file { path, old, new }`, wired through `callFile`/`fileToolResult` the way the ten go through `callStore`/`storeToolResult` — except `fileToolResult` unwraps `text`/`error` to the plain string, not a JSON-encoded object, per the locked result shape. `claude_code.rs` gained `FileCall { tool, args }` and a sixth `run_turn` parameter, `on_file`; the `"file"` arm mirrors `"store"` line for line. `main.rs` holds `READ_CAP_BYTES`, `canonicalize_in_workspace` (root and candidate both canonicalised; an absolute path replaces the join; an io error names whichever side failed), `is_arx_state`, and `read_file_call`/`edit_file_call`, each returning the tool result and the transcript row together from one read or one write — a truncated read's numbers and an edit's `old`/`new` never get re-derived from the filesystem or re-parsed out of the result text afterward. `run_turn_cycle` and `run_checkpoint_ritual` each gained a fifth `ManuallyDrop<ClaudeCodeMain>` clone, `adapter_for_file_kill`, since `on_file` needed its own exclusive borrow the same way `on_hand` already had one. `transcript.rs` gained `append_file`, the `append_system` shape with `kind: "file"`.

One correction against the brief's Ground truth: `subject_field` lives in `gate.rs`, not `config.rs` — a stale file reference in the brief, not a moved line. The one gate line the brief allowed for landed there: `"mcp__arx__edit_file" => Some("path")`. Main's tools cross the gate under the SDK's own `mcp__arx__<name>` form, the same as the ten — confirmed by `tests/gate.rs`'s `mcp__arx__save`/`mcp__arx__recall` rows and by the stub's own `crossGate('mcp__arx__' + verb, ...)` — so both the gate-deny test and the check-config admission test write the rule as `mcp__arx__edit_file(...)`, not the bare `edit_file(...)` the brief's illustrative text used; the bare form parses but would never match a real call, `Rule::matches` being exact-string on the tool name.

The stub's `file:` trigger mirrors `store:`'s shape with one addition its own scope 2 test needed: `crossGate` throws on a denial, as `store:`/`hand:` already do but no test had exercised; a denied edit would otherwise have crashed the stub process mid-session. The trigger wraps the cross-gate call in a try/catch and folds a denial into `{error: err.message}`, replied the same `stub-file:<json>` way as success — no `file` row is written on that path, since the throw fires before the `file` event does.

Six unit tests on a tempfile workspace exercise `file_call` directly: bounding, a whole small read, the cap-and-trailer cut, an exact-once edit, zero/many-match refusal, and the arx-state refusal. Six `claude_main.rs` tests run real sessions over the stub sidecar: a read and its row; a 40 KiB file cut at 16384 with the trailer; `/etc/hostname` and `../` both refused; an edit followed by a second, denied edit on the same file; a `store/` edit refused; a `[gate] deny = ["mcp__arx__edit_file(docs/*)"]` rule denying the call before it ever reaches the tool. One `check_config.rs` test admits both the patterned and bare-tool forms of an `edit_file` rule.

`cargo test -p arx` after Scope 1: unittests 195 (189 + 6), every other binary at baseline. After Scope 2: `claude_main` 63 (57 + 6), `check_config` 9 (8 + 1), every other binary unchanged. `cargo test --workspace`: 399 passed, 1 ignored (arx 355, arx-engine 44) — the brief's own numbers, exactly. `check-config`'s stdout is byte-identical before and after. A `194 passed, 1 failed` blip surfaced once under full-workspace parallelism, on the same `unittests src/main.rs` binary the cold baseline had already shown flaking on before any line of this card landed; three clean reruns after, unrelated to the diff.

Live: a scratch workspace (`sidecar` pointed at the real `sidecar/` dir, `agents/judge.md` and `agents/hand-ex.md` for config validity, a fresh git init) ran a real Claude Code main over one operator turn asking it to read `arx.toml`, quote its first line, then edit `NOTE.md`'s `sonnet` to `SONNET`. It read `arx.toml` (98 bytes), then — its own choice, not scripted — read `NOTE.md` before editing it, then edited it: three `file` rows landed, not the two the done-when named, all three correctly gated and logged, `NOTE.md` reading `model: SONNET` afterward, the reply quoting the first line and closing with `done`. Scratch workspace removed after; container and hand-root sweep empty before and after.

Codex gate: `gate_run: no` — the provider's usage limit is in force until 2026-09-07 08:38, confirmed by one attempt.
