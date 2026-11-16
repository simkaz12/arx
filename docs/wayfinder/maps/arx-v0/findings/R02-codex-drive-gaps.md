# R02 — Codex app-server and exec: gaps the trace left

> Superseded in part by `R03-codex-runtime-probe.md` (2026-09-10), which ran what this file read.
> R03 confirms section 1 and the approval/interrupt claims, corrects the token-usage field names — the
> wire nests them as `params.tokenUsage.{total, last, modelContextWindow}`, not flat `total_token_usage`
> and `last_token_usage` — and records that the protocol has grown substantially since the `b8c86376`
> pin below. This file stays as written: it is the record of what the source said at that commit, not a
> live description of the CLI on PATH today.

Date 2026-08-30. Sources: openai/codex @ b8c86376a258e55efc8e5ecfbabc21c16c07d814 (2026-08-29 20:42 UTC,
shallow clone, deleted after reading — same commit the D16 addendum used); codex-cli 0.147.0 on PATH,
`codex exec --help` read live. No secondary write-ups consulted.

## 1 AGENTS.md suppression

Full suppression, not truncation. `load_project_instructions` (core/src/agents_md.rs:55) sets
`remaining = config.project_doc_max_bytes` (agents_md.rs:65) and the per-environment loop checks
`if remaining == 0 { break; }` (agents_md.rs:67) *before* ever calling `read_agents_md` — with the cap
at 0 no filesystem read is attempted for any environment. `read_agents_md` itself has a second guard,
`if max_total == 0 { return Ok(None); }` (agents_md.rs:129), before it even calls `agents_md_paths` to
walk the directory tree — so path discovery (the upward walk for `.git` / project root) doesn't run
either. Net: `-c project_doc_max_bytes=0` is a true off-switch, empty-set of files read, not "read then
drop." Host-supplied `user_instructions` (a separate parameter) survive independently — this cap only
kills the file-discovery path.

No cleaner switch exists. Grepped `config_toml.rs` for a boolean (`skip_project_doc`,
`no_project_doc`, etc.) — the only knobs are `project_doc_max_bytes: Option<usize>` (config_toml.rs:303,
default 32 KiB, `AGENTS_MD_MAX_BYTES` at core/src/config/mod.rs:230) and
`project_doc_fallback_filenames` (config_toml.rs:307), neither a disable flag. `-c
project_doc_max_bytes=0` is the intended mechanism.

## 2 Usage events

app-server (`thread/tokenUsage/updated`): fired from `EventMsg::TokenCount` via
`handle_token_count_event` (app-server/src/bespoke_event_handling.rs:983-990, 1642-1662). Source:
`send_token_count_event` (core/src/session/mod.rs:4306-4312) is called once per model round-trip on
`ResponseEvent::Completed` (core/src/session/turn.rs:2785-2787), gated by `should_emit_token_count` —
comment at turn.rs:2782-2784: emission is deferred until any in-flight tool calls for that step resolve,
so it lands *after* the item events for that step, not interleaved mid-stream. A turn with N model
completions (tool-calling loop) emits N of these notifications, each carrying both a running
`total_token_usage` (accumulated via `TokenUsageInfo::append_last_usage`, protocol.rs:2249-2252:
`total_token_usage.add_assign(last)`) and `last_token_usage` (this round-trip's own usage, not a delta
of deltas) — cumulative-since-thread-start plus latest-call, together, every time.

`codex exec --json` (`turn.completed.usage`): `codex exec` is itself an in-process app-server client
(`InProcessAppServerClient::start`, exec/src/lib.rs:807) — it receives the same
`ServerNotification::ThreadTokenUsageUpdated` internally but does not forward it as a JSONL line
(exec/src/event_processor_with_jsonl_output.rs:501-504: cached into `self.last_total_token_usage`,
no event pushed). Only on `ServerNotification::TurnCompleted` with `TurnStatus::Completed` does it emit
`ThreadEvent::TurnCompleted{usage: self.usage_from_last_total()}` (event_processor_with_jsonl_output.rs:
524-528), which reads `usage.total` (the cumulative total, not the last-call delta) minus
`total_tokens` (exec_events.rs:59-70, `Usage` struct has no `total_tokens` field). `TurnStartedEvent`
is an empty struct (exec_events.rs:46) — no context-window field anywhere in exec's JSONL schema.
Because exec shuts down after one turn (`CodexStatus::InitiateShutdown`,
event_processor_with_jsonl_output.rs:527), "cumulative since thread start" and "this turn's total"
coincide for a normal invocation — but the field is architecturally cumulative, not turn-scoped, and
would diverge under `codex exec resume` continuing a thread with prior usage. Net: exec surfaces usage
exactly once per process, at the very end; app-server surfaces it potentially many times per turn,
each carrying the running total plus the latest increment.

## 3 --output-schema conformance

`--output-schema <FILE>` is a global CLI flag (exec/src/cli.rs:45-47) independent of `--json` — no code
ties the two together, confirmed against `codex exec --help` (both flags listed as unrelated, no
mutual-requirement note). It flows into every entry path (fresh prompt, `resume`, `fork`:
exec/src/lib.rs:739,761,790) as `TurnStartParams.output_schema`, which app-server forwards to
`Prompt.output_schema`/`output_schema_strict: true` (core/src/client_common.rs:33-49) and
`create_text_param_for_request` turns into `text.format = {type: "json_schema", strict: true, schema,
name: "codex_output_schema"}` (codex-api/src/common.rs:389-406) — OpenAI Responses API's native
Structured Outputs field. Conformance is not enforced by Codex at all: no `jsonschema` crate anywhere
in the workspace (grepped every `Cargo.toml`), no local parse-and-validate step. The final message is
written verbatim: `handle_last_message`/`write_last_message_file` (exec/src/event_processor.rs:31-48)
do a raw `std::fs::write`, no `serde_json::from_str` check; same for the `item.completed`/
`turn.completed` JSONL text. Codex's guarantee is exactly OpenAI's server-side strict-mode guarantee,
nothing added and nothing checked afterward.

One concrete non-conformance path Codex does not special-case: `response.refusal.delta`/`.done` SSE
events are treated identically to an *unrecognized* event type — a test fixture at
codex-api/src/sse/responses.rs:1436-1449 asserts `"refusal_delta"` produces the same
`expect_first: is_completed, expected_len: 1` outcome as the adjacent `"unknown"` case
(`response.new_tool_event`, line 1433-1439). A refusal is silently dropped rather than surfaced as a
distinct error or partial result — the turn completes with whatever assistant-message text was
otherwise captured (likely empty), not a flagged failure.

## Consequences

- B16 (codex hand): no change. `-c project_doc_max_bytes=0` already fully suppresses AGENTS.md
  discovery at the filesystem-walk level, and `codex exec --json`'s single end-of-run usage line
  is adequate for a one-shot hand turn (context-window tracking, if ever needed, cannot come from
  exec's JSONL schema at all — would require app-server).
- B20 (judge, ruled by da3f5418/1e6ad121 as `codex exec --output-schema`, one turn, read-only): the
  strict-schema path has zero Codex-side fallback, unlike `review/start`'s
  `parse_review_output_event` lenient parser (tasks/review.rs:193-207, already noted in the prior
  trace). If the judge's driver treats "output-schema was set" as sufficient guarantee and does a bare
  `serde_json::from_str` on the final message, a refusal (silently dropped per above) or an
  incomplete/truncated response (a documented OpenAI strict-mode caveat, not separately probed here)
  will surface as an empty or malformed payload rather than a caught error. B20's driver needs its own
  parse-with-explicit-failure wrapper around the final message — Codex will not provide one.

## Open

Not settled by primary sources this session: whether an unsupported-by-strict-mode schema (e.g.
optional fields, unsupported keywords) is rejected by the Responses API before generation (a 400) or
accepted with degraded enforcement — no request-time schema pre-validation found in codex-api, and no
live API call was made to observe the rejection shape. Whether `--output-schema` combined with
`codex exec resume` on a thread that already has schema-shaped history behaves any differently was not
exercised. Both would need a live API probe, not just source reading, to close.
