---
id: L2-10c
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-10b
split-from: L2-10
ruled-by: D02 (`975f65cd` — one adapter per wire shape, OpenRouter is a base_url and not a third adapter; `72bfc6e9` — the stream item is a semantically complete partial-Message chunk assembled inside the adapter, wire deltas never cross the seam; `48af6ac7` — Anthropic's 400 "prompt is too long" is the error channel), D14 (`3a5aefb6`, `8b9f41ae` — the overshoot parsed from "N tokens > M maximum"), T01 (`../arx-design/findings/T01-findings.md`)
---

# L2-10c — the Anthropic adapter

Cut from L2-10 on 2026-09-04 by main under the owner's overnight clearance; the third session, on top of L2-10b's error classes, replacing the `AnthropicStub` L1-10 left.

## Deliverable

The Anthropic-native adapter beside the OpenAI-compatible one, behind the unchanged `Provider` trait: `POST /v1/messages` with `x-api-key` and `anthropic-version`, streaming, the SSE event sequence (`message_start`, `content_block_start`/`delta`/`stop`, `message_delta`, `message_stop`) assembled inside the adapter into the same `Chunk` stream — text and tool-use blocks as content, `stop_reason` mapped to the termination enum (`end_turn` → EndTurn, `tool_use` → ToolUse, `max_tokens` → MaxTokens, `refusal` → Refusal), usage in Anthropic's own field names, thinking blocks passed through as content the loop ignores. Errors through L2-10b's classes: 429 and 529 as `Transient` with `retry-after`, the 400 "prompt is too long: N tokens > M maximum" as `ContextWindowExceeded` with the overshoot parsed, the rest `Fatal`. A `[providers.<name>]` row with `kind = "anthropic"` and a definition naming it dispatch a native hand through it.

## Done when

Against an Anthropic-shaped fake server in the test harness: a text turn, a tool-use turn feeding L2-10a's loop into a second request whose messages carry the tool result in Anthropic's `tool_result` block shape, each `stop_reason` mapping, usage captured, a 429 retried before the first item, the 400 parsed into the overshoot; the OpenAI-compatible tests are untouched; `L1-10`'s trait is byte-identical. Live, when an `ANTHROPIC_API_KEY` line is present in `~/.config/arx/env`: one tool-using brief completes against Anthropic from one agent definition and its trace shows the request shape; when no key is present the live check is recorded as an owner item and the card closes on the hermetic proof, since the subscription login is not an API key.

## Ruled by

D02 (`975f65cd`, `72bfc6e9`, `48af6ac7`), D14 (`3a5aefb6`, `8b9f41ae`), T01 — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

`AnthropicStub` is gone. `anthropic.rs` now holds the real adapter: `Anthropic { base_url, api_key, model, cancel, client }`, `POST {base_url}/v1/messages` with `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`, `accept: text/event-stream`, `stream: true`, the system prompt on its own top-level `system` field, `max_tokens` fixed at 8192. `build_request_body` translates `Message`/`ContentBlock` into Anthropic's wire shape: a `Role::User` text block becomes one `{"type":"text"}` content block, a `Role::Assistant` message's text and tool-call blocks become `text`/`tool_use` content blocks in order, and each `Role::Tool` message becomes its own `{"role":"user","content":[{"type":"tool_result",...}]}` message — one per result, never merged on this side, since the Messages API folds consecutive same-role turns together on the wire. `ToolDef` becomes `{"name","description","input_schema"}`; `tools` is omitted when empty.

`AnthropicChunks<R: BufRead>` folds the SSE event sequence into the same `Chunk` stream every consumer already reads, using the identical cancel-poll-before-and-after-every-line loop L2-10a's `SseChunks` established: `message_start` remembers `usage.input_tokens` and the two cache fields; `content_block_start` opens a text, tool, or dropped block by index; `content_block_delta`'s `text_delta` yields `Content(Text)` immediately, `input_json_delta` appends to the open tool block's buffer, `thinking_delta`/`signature_delta` are no-ops; `content_block_stop` parses the tool block's accumulated JSON (empty string → `{}`) into one `Content(ToolCall)`; `message_delta` records `stop_reason` and `usage.output_tokens`; `message_stop` maps the stop reason (`end_turn`/`stop_sequence`/`pause_turn` → EndTurn, `tool_use` → ToolUse, `max_tokens` → MaxTokens, `refusal` → `Refusal{category:"refusal"}`, anything else → `Refusal{category:<raw>}`) and yields `Terminal` with the four Anthropic usage field names populated (missing → 0), so `transcript::occupancy`'s existing Anthropic-first sum reads it unchanged; `ping` is ignored; an `error` event or EOF before `message_stop` ends the stream `Fatal`.

Classification before the first item: a transport error and 429/529/5xx are `Transient` (429 and 529 carrying `retry-after`); a 400 whose body contains `prompt is too long` is `ContextWindowExceeded`, with `parse_overshoot` (in `openai_compat.rs`, shared) gaining a second form that reads `N tokens > M maximum` by splitting on `>` and taking the last integer on the left as `requested` and the first on the right as `maximum` — additive, the original `maximum context length is ... requested/resulted in` form untouched and still covered by its own tests. Every other non-2xx is `Fatal` with the redacted body tail. `RedactingStream` moved from private to `pub(crate)` with a `new()` constructor so both adapters share one redaction wrapper rather than each carrying its own; the OpenAI-compatible adapter's own construction site and tests are unaffected. `build_provider`'s `Anthropic` arm now builds the real adapter wrapped in `Retrying`, same as the OpenAI-compatible arm.

`tests/common/mod.rs` gained `fake_anthropic_server`, sharing one private `spawn_fake_server` with `fake_openai_server` — the listener never routed on the path, so there was nothing shape-specific to flag. `FakeState` gained an additive `headers: Mutex<Vec<Vec<(String, String)>>>`, one entry per request holding whichever of `x-api-key`/`anthropic-version` that request carried; the existing `requests` tuple every OpenAI test destructures is untouched.

`claude_main.rs`: `native_dispatch_through_the_anthropic_stub_surfaces_the_l2_10_fatal` is retired, replaced by `native_dispatch_through_anthropic_completes_a_text_turn` (asserts the request path, both captured headers, the request body's `max_tokens`/`system`/`messages`/`tools`, and the assembled verdict and usage). Four more land: `native_hand_on_anthropic_runs_a_tool_step_with_tool_result_blocks` (two requests; the second body's third message is the `tool_result` block naming the matching `tool_use_id`), `native_hand_on_anthropic_retries_a_429_before_the_first_item`, `native_hand_on_anthropic_reports_a_prompt_too_long_miss` (verdict text, a `miss` event sized 1808, the stdout notice), `native_hand_on_anthropic_http_error_body_echoing_the_key_is_redacted_everywhere_under_threads`.

One deviation from the brief's own count, found while implementing: deleting `AnthropicStub` also stranded a second, uncalled-out test — `provider.rs`'s `build_provider_anthropic_fails_with_the_l2_10_message`, which built a real `Anthropic` through `build_provider` and asserted a Fatal text only the stub produced; against the real adapter it would either dial the live `api.anthropic.com` host or fail its assertion on a transport-`Transient` error text instead. The brief's Decisions locked names only the one e2e stub test as retired; this second one is the same category of collateral, not named. It was deleted rather than adapted, since adapting it to exercise the real adapter without a fake server would mean a live network call inside a unit test. Net effect: unittests baseline 174 → 182 (`+8`, not the brief's stated `+10`) — two stub-specific tests gone (this one plus `anthropic.rs`'s own retired `complete_is_fatal_naming_l2_10`), ten new ones landed in `anthropic.rs`'s module as named.

Live check: `grep -c '^ANTHROPIC_API_KEY=' ~/.config/arx/env` reports `0`. `~/.config/arx/env` carries `DEEPSEEK_API_KEY` only; the card closes on the hermetic proof per its own "Done when," and the live run against a real Anthropic account is an owner item.

`cargo test --workspace`: 378 passed, 1 ignored (arx 334: unittests 182, check_config 8, checkpoint 6, claude_in_container 4, claude_main 57, container_runtime 9, events 3, gate 4, hands_in_containers 5, layer_close 12, native_in_container 6, state_object 7, store 3, transcript_log 28; arx-engine 44). `docker ps -a --filter label=arx.hand` empty before and after.

Codex gate: not run, `gate_run: no` — the provider's usage limit is in force until 2026-09-07 08:38, confirmed by one attempt (log: `~/.claude/codex-logs/2026-09-04-l2-10c-anthropic-adapter.md.codex-review.r1.log`), per the brief's own stated exemption for this window.
