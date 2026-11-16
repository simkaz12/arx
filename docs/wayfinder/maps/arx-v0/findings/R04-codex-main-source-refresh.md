# R04 — Codex main source refresh

Date: 2026-09-11. Wave-2 flow investigation, following R03.

Installed binary: `codex-cli 0.153.4`. Official source tag `rust-v0.153.4` resolves to
[`3d2ee51ca2d5db578f328aa75e20aa22c0197c9a`](https://github.com/openai/codex/tree/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a).
The investigation compared that source with experimental schemas generated from the installed binary.
No live model prompt was run. R03 remains the runtime evidence; this file supplies its missing source refresh.

## Main-seat contract

The current arx loop needs fresh session start, streamed turns with arx-tool round trips, turn interrupt,
same-body model changes, and child replacement at reset. Admission, transcript, front, ledger, and running
hands remain owned by the Rust process. The existing paths are `claude_code.rs::ClaudeCodeMain`
and the turn/reset/respawn paths in `main.rs`.

Reset already means a fresh vendor session, never vendor resume. The new main orients from the checked
state object and transcript handle; exhaustion falls back to the latest checked object or transcript alone.
The configured `[main] window` remains authoritative.

## Protocol mapping

- Start a stdio child; send `initialize`, `initialized`, then `thread/start`. Enable
  `initialize.params.capabilities.experimentalApi` for dynamic tools.
- Put arx's standing prompt in `thread/start.baseInstructions`; `developerInstructions` is an additional
  developer fragment. A clean `CODEX_HOME` prevents ambient global instructions; `project_doc_max_bytes=0`
  suppresses project instruction discovery. Per-thread config overlays do not establish home isolation.
- Register arx tools in `thread/start.dynamicTools`. Requests arrive as
  `item/tool/call {threadId, turnId, callId, namespace, tool, arguments}`. Responses carry
  `{contentItems, success}`; text results use `inputText`.
- `turn/start.model` applies to that turn and subsequent turns. This supports the existing same-body
  `/model` behavior, subject to the compaction caveat below.
- `turn/interrupt {threadId, turnId}` stops the active turn while retaining the session, as R03 proved live.

Sources: [lifecycle](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/app-server/README.md#L76-L87),
[thread fields](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/app-server-protocol/src/protocol/v2/thread.rs#L62-L144),
[dynamic tools](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/app-server/README.md#L2066-L2116),
[turn fields](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/app-server-protocol/src/protocol/v2/turn.rs#L156-L238).

## Usage

`thread/tokenUsage/updated.params.tokenUsage.last` describes the latest model completion; `total` sums
completions across the vendor thread. Input tokens already include cached input tokens. Map the last
notification's `last.inputTokens` to arx's prompt-side occupancy once, without adding `cachedInputTokens`.
Retain cumulative accounting separately. Do not use `total` as occupancy or replace `[main] window` with
`modelContextWindow`.

Sources: [wire shape](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/app-server-protocol/src/protocol/v2/thread.rs#L1871-L1921),
[accumulation](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/protocol/src/protocol.rs#L2250-L2289),
[cache semantics](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/protocol/src/protocol.rs#L2393-L2409).

## Compaction conflicts with the earlier assumption

The checked source caps `model_auto_compact_token_limit` at 90% of the resolved model context window.
The context-window override is itself capped at the model maximum. Raising these values therefore does
not establish that arx's reset always precedes Codex compaction.

Codex can compact before a turn, during a turn that needs another model step, and when changing to a
model whose compaction compatibility or smaller window requires it. Arx's current turn-end reset check
cannot guarantee precedence over same-turn compaction. An earlier reset threshold provides margin but
does not prove the guarantee.

Sources: [compaction cap](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/protocol/src/openai_models.rs#L487-L510),
[context cap](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/models-manager/src/model_info.rs#L25-L37),
[in-turn triggers](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/core/src/session/turn.rs#L422-L503),
[model-switch compaction](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/core/src/session/turn.rs#L1141-L1210).

Automatic compaction has an observable lifecycle: `item/started` with `item.type = "contextCompaction"`
precedes the summary request; `item/completed` follows history replacement. Local, remote, remote-v2,
and model-switch compaction follow this path. Normal model work waits for compaction to return.

Arx can therefore discard the vendor child/thread on either event and use its existing checked-state-object
and transcript fallback. Event order does not guarantee that arx processes `started` before a fast compaction
finishes. Recovery must discard the whole vendor session rather than depend on interrupting the summarizer
in time. The owner agreed on 2026-09-11 to this extension of the exhaustion fallback. Ordinary checked
digestion/reset remains the normal path under Codex, as under Claude Code.

Sources: [compaction start](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/core/src/compact.rs#L245-L254),
[replacement and completion](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/core/src/compact.rs#L352-L399),
[notification mapping](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/app-server/src/bespoke_event_handling.rs#L1082-L1142),
[automatic-compaction test](https://github.com/openai/codex/blob/3d2ee51ca2d5db578f328aa75e20aa22c0197c9a/codex-rs/app-server/tests/suite/v2/compaction.rs#L52-L102).

## Agreement reached 2026-09-11

- A cross-body `/model` change runs the normal reset ritual immediately and starts the selected body from
  the checked state object and transcript orientation. Arx's process, front, ledger, transcript and running
  hands survive. This settles the behavior roster A deferred to wave 2.
- Codex uses the same ordinary checked digestion/reset path as Claude Code. Unexpected vendor compaction
  extends the existing exhaustion fallback: discard that vendor session, start fresh, and orient from the
  last checked state object plus transcript, or the transcript alone. Do not continue from the vendor summary.
- A fresh home per main incarnation is an implementation recommendation for instruction and vendor-history
  isolation, not a new permission to resume old vendor threads.
