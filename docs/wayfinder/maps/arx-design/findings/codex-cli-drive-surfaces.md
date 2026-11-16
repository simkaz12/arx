---
labels: wayfinder:findings
source: hand-ex trace of github.com/openai/codex at f5636bb733c4653a6b91413fed1aaf8842374f2e (2026-08-29, shallow clone, deleted after reading); codex-cli 0.147.0 installed on PATH, `codex --help` / `codex exec --help` / `codex app-server --help` read live
provenance: dispatched mid-D16-grill to close the precondition named in decision e652ce3a — Codex's drive-surface shape was "unverified at ruling time"
grounds: D16 (e652ce3a: one adapter per CLI kind, role-agnostic; 705e4df2: arx hosts main under every body), D06 (96d1ad3a: where a hand's gate runs), subscription-auth.md (Claude-side policy for comparison)
---

# Codex CLI — driving it as an arx body (main / hand / judge)

**Answer, in short:** Codex has two real drive surfaces. `codex app-server` is a persistent JSON-RPC 2.0 daemon (stdio/unix-socket/websocket) with a live approval callback, per-turn cancel, and mid-turn steer — the one a main needs. `codex exec --json` is a one-shot subprocess with a simpler JSONL event stream, no approval callback at all (policy hardcoded to `never`), and interrupt-by-SIGINT — the one a hand needs. Below, each numbered question with path:line evidence.

## 1. Drive surfaces

`codex exec` (alias `e`): one-shot, `--json` prints `ThreadEvent` JSONL — `thread.started`/`turn.started`/`turn.completed`/`turn.failed`/`item.started`/`item.updated`/`item.completed`/`error` (codex-rs/exec/src/exec_events.rs:9-37); prompt via stdin or positional arg; multi-turn continuation is a *new process*, `codex exec resume <id>` or `--last`, reading the on-disk rollout (exec/src/cli.rs:150-267). `codex app-server`: JSON-RPC 2.0 with the `"jsonrpc":"2.0"` header omitted on the wire, transports stdio (default)/unix-socket/websocket-experimental (app-server/README.md:20-33); core RPCs `thread/start|resume|fork`, `turn/start|steer|interrupt` (README:76-84, codex-rs/docs/codex_mcp_interface.md:15-19); it's what powers OpenAI's own VS Code extension and ships TS/JSON-Schema generation (README:1,71-75). A third binary, `codex mcp-server`, wraps the same v1/v2 protocol types behind MCP framing as two coarse tools (`codex`, `codex-reply`) rather than exposing the methods directly, and its own doc states "Status: experimental and subject to change without notice" (codex-rs/docs/codex_mcp_interface.md:1-11,104-118). **Stable pick: app-server** — still CLI-tagged `[experimental]`, but it's the most-documented, most-consumed protocol Codex ships.

## 2. Approval callback

Exists only under app-server: server-initiated `item/commandExecution/requestApproval` / `item/fileChange/requestApproval` (README:192,210; legacy names `execCommandApproval`/`applyPatchApproval`, mcp_interface.md:31,124-125). Client answers `{decision: "accept"|"acceptForSession"|"acceptWithExecpolicyAmendment"|"applyNetworkPolicyAmendment"|"decline"|"cancel"}` (README:185,193) — richer than the SDK's binary allow/deny. `codex exec` has **no** callback: headless runs hardcode `approval_policy: Some(AskForApproval::Never)` (exec/src/lib.rs:411-413), and grepping exec/src for approval-event handling returns nothing — a blocked command just fails back to the model. `approval_policy`/`AskForApproval` (protocol/src/protocol.rs:965-987): `untrusted`, `on-request` (default; CLI help's "on-failure" is a serde *alias* for on-request, line 973, not a fifth mode), `granular` (per-category booleans, protocol.rs:988-1004), `never`. `sandbox_mode`/`SandboxPolicy` (protocol.rs:1051-1071; config_types.rs:104-114): `read-only` (optional network), `workspace-write`, `danger-full-access` — governs what's technically permitted, orthogonal to whether it's asked about. Net: exec can only pre-set a policy; app-server answers live.

## 3. Usage reporting

app-server streams `thread/tokenUsage/updated` separately from `turn/completed` (README:61,67); `TokenUsage` = input_tokens, cached_input_tokens, cache_write_input_tokens, output_tokens, reasoning_output_tokens, total_tokens (protocol.rs:2192-2211), wrapped by `TokenUsageInfo{total_token_usage, last_token_usage, model_context_window}` (protocol.rs:2213-2220) — cumulative, not per-turn deltas. `codex exec --json`'s `turn.completed.usage` carries the same five token fields minus `total_tokens` and with **no context-window field at all** (exec_events.rs:50-73); context window only appears via `TurnStartedEvent.model_context_window` (protocol.rs:2154), an app-server-side thing exec's JSONL schema doesn't expose.

## 4. Compaction

On by default, keyed by `model_auto_compact_token_limit: Option<i64>` (config/src/config_toml.rs:167-168) plus `model_auto_compact_token_limit_scope` (Total vs BodyAfterPrefix, config_toml.rs:170-172); leaving it `None` doesn't disable compaction, it falls back to a per-model built-in limit (core/src/session/context_window.rs:64,73) — no boolean off-switch found, the lever is `-c model_auto_compact_token_limit=<big number>`. Manual trigger over the protocol: `thread/compact/start` (README:914-923), progress via `item/started`/`item/completed` on a `contextCompaction` item. Core event: `EventMsg::ContextCompacted` (protocol.rs:1379).

## 5. Interrupt

app-server: `turn/interrupt {threadId, turnId}` (README:1289-1300) — cancels the active turn, emits `turn/completed{status:"interrupted"}`; the thread and process both survive. `codex exec` is single-shot, so there's no live RPC to send: Ctrl-C is caught (`tokio::signal::ctrl_c()`) and translated into the same internal interrupt op against exec's own in-process client (exec/src/lib.rs:946-952) rather than a raw kill — but the process still exits once that turn finishes.

## 6. MCP

`[mcp_servers.<name>]` in config.toml, `McpServerConfig` (config/src/mcp_types.rs:196-268): transport is `Stdio{command,args,env,env_vars,cwd}` or `StreamableHttp{url,bearer_token_env_var,http_headers,env_http_headers,http_headers_helper}` (mcp_types.rs:533-565) — stdio or streamable-HTTP only. `enabled_tools`/`disabled_tools` per server (mcp_types.rs:245-251) restrict to an allowlist, deny wins on conflict.

## 7. Session persistence

`$CODEX_HOME/sessions/YYYY/MM/DD/rollout-YYYY-MM-DDThh-mm-ss-<uuid>.jsonl` (rollout/src/list.rs:427; filename builder rollout_file_name.rs:67-70; `SESSIONS_SUBDIR` rollout/src/lib.rs:68); `codex exec --ephemeral` skips writing it. `codex exec resume <id>|--last` (exec/src/cli.rs:150-267) hands the id to app-server's `thread/resume` — a host talking app-server directly never needs to parse the rollout file itself, same non-parsing stance as Claude Code's session UUID resume.

## 8. Auth

`chatgpt` (recommended, OAuth, Codex owns refresh) vs `apiKey` vs Amazon Bedrock, selected via `account/login/start` (README:2454-2463). A host-spawned child inherits the operator's stored login exactly like a local `codex` invocation would, scoped by `$CODEX_HOME` (not the OS keychain, unlike Claude's macOS path). Policy: unlike Anthropic's explicit ban (subscription-auth.md:12, citing code.claude.com/docs/en/legal-and-compliance), nothing in this repo forbids third-party hosting — the README instead invites it: "Applications building on top of `codex app-server` should identify themselves via `clientInfo`… if you are developing a new Codex integration intended for enterprise use, please contact us to get it added to a known clients list" (README:129-133). That's a compliance-logging ask, not a ToS page, and I did not fetch OpenAI's actual usage-policy site this session — treat the apparent permissiveness as unconfirmed.

## What arx takes from this

- **Main** needs dispatch/tap/steer/kill plus a live gate (D16, D06). Only app-server has what it needs — `turn/steer` for steer, `turn/interrupt` for kill-without-death, the `requestApproval` RPCs for the gate; tap is arx's own transcript-log read and needs nothing from the body. A Codex-bodied main has to be driven over app-server JSON-RPC, the same shape as the ruling that a CC-bodied main needs the Agent SDK, not bare `claude -p`. *(Dispatcher's correction: the trace as returned mapped `turn/steer` to tap; tap is defined in CONTEXT.md as a read of the hand's transcript log.)*
- **Hand** (96d1ad3a: clamped at spawn, own internal gate, no arx-side interception) maps cleanly onto `codex exec`: spawn with `--sandbox`/`-a never`/`-c mcp_servers...`, read the `--json` stream, done — there's structurally nothing to intercept because exec never asks. Better fit for hand than main.
- **Judge** wants one scored turn plus usage, no back-and-forth: `codex exec --json --output-schema`/`--output-last-message` fits directly; app-server's approval/steer machinery is overhead a judge doesn't need. `review/start` (README:1399-) is a Codex-native reviewer mode worth a follow-up trace, not covered here.
- **Versus the Claude Agent SDK route**: the SDK is in-process (t3code's `query()`); Codex has no equivalent SDK found in-repo — every drive path is an external subprocess arx must frame and parse itself over stdio/socket. Traded for that cost: richer approvals (execpolicy/network-policy amendments) and real per-turn cancel that Claude Code's session-level kill lacks.

## Unknowns and confidence

Not verified: OpenAI's actual usage-policy site for third-party subscription hosting (Q8, repo text only); whether `codex mcp-server` clients receive the same approval RPCs as app-server (doc-only, source not read); a config boolean to fully disable auto-compaction beyond the token-limit lever; none of this was exercised against a live `codex app-server` session — everything is source-cited, not runtime-probed. Confidence: high on drive-surface shape, approvals, token fields, session paths (source-cited, cross-checked against `--help`); medium on the auth-policy comparison and mcp-server/app-server behavioral parity.


## Addendum (2026-08-30) — `review/start` and the instructions override

Second hand-ex trace, openai/codex at `b8c86376a258e55efc8e5ecfbabc21c16c07d814` (shallow clone, deleted after reading), codex-cli 0.147.0 `--help` read live. Dispatched for D16 round 4 (Q11: what `review/start` is; Q12: whether a host can replace Codex's base instructions).

### `review/start`

Params: `threadId` + `target` (`uncommittedChanges` / `baseBranch{branch}` / `commit{sha,title}` / `custom{instructions}`) + `delivery` (`inline`|`detached`, default inline) — `codex-rs/protocol/src/protocol.rs:3313-3346` (`ReviewTarget`, `ReviewRequest`). No diff or schema param; the host names a target, not a payload.

It runs a dedicated review agent, not the parent thread. `codex-rs/core/src/tasks/review.rs:120` sets `sub_agent_config.base_instructions = Some(REVIEW_PROMPT)`; REVIEW_PROMPT is `include_str!("../templates/review/rubric.md")` (`codex-rs/prompts/src/review_request.rs:9`), a static file replacing the model's normal instructions wholesale for that sub-turn. `run_codex_thread_one_shot` gets `initial_history: None` (review.rs:136): fresh context, no parent conversation carried in. Model: `config.review_model`, falling back to the parent model only if unset (`session/review.rs:14-16`). Web search, view-image and collab tools are hard-disabled for the sub-agent (review.rs:104-105). So `review/start` is structurally unlike `turn/start` with a review-flavoured prompt: that would keep the normal system prompt and full thread history; this swaps both out.

Result delivery: `item/started`→`enteredReviewMode`, then `item/completed`→`exitedReviewMode` carrying `review_output: Option<ReviewOutputEvent>` — `findings: [{title, body, confidence_score, priority, code_location:{absolute_file_path, line_range:{start,end}}}], overall_correctness, overall_explanation, overall_confidence_score` (protocol.rs:3348-3388). The README's plain-text `review` field is `render_review_output_text` flattening that struct for display (`codex-rs/protocol/src/review_format.rs:64`); the wire payload is structured.

Schema enforcement: review does **not** use the Responses-API structured-output path. `final_output_json_schema: None` is hardcoded at `tasks/review.rs:137` and `session/review.rs:172`. The model is prompted (via REVIEW_PROMPT) to emit JSON, and `parse_review_output_event` (`tasks/review.rs:193-207`) parses it leniently: whole text, then the first `{...}` substring, then a fallback `ReviewOutputEvent{overall_explanation: raw_text, ..Default}`. A host cannot inject its own findings schema into `review/start`; `custom.instructions` only adds free text to the prompt.

`codex exec --output-schema <FILE>` **is** the strict mechanism review opts out of: it threads a caller-supplied JSON Schema into the same `final_output_json_schema` field (`exec/src/lib.rs:739`, `turn_context.rs:1026`) that becomes `Prompt.output_schema` sent to the Responses API with `output_schema_strict: true` (`client_common.rs:33-49`, `client.rs:1009-1010`). Two mechanisms sharing one plumbing point; review deliberately does not take the strict branch.

CLI: `codex review [PROMPT] [--uncommitted|--base <BRANCH>|--commit <SHA>] [--title]` exists and maps onto the same `ReviewTarget` variants.

### Base instructions override

`thread/start` accepts `baseInstructions` and `developerInstructions`, both threaded into `ConfigOverrides{base_instructions, developer_instructions}` (`app-server/src/request_processors/thread_processor.rs:1616,1634`). `baseInstructions` **replaces wholesale**: it becomes `session_configuration.base_instructions`, which `get_base_instructions()` returns verbatim in place of the model's built-in prompt (`core/src/session/mod.rs:1308-1313`; the config-file comment reads "override the built-in instructions for the selected model", `config_toml.rs:242-245`). `developerInstructions` is **append-only**, "inserted as a `developer` role message" (`config_toml.rs:226-228`). The same two knobs exist as config.toml keys (`instructions`, `model_instructions_file` — a path, "users are STRONGLY DISCOURAGED" — and `developer_instructions`), resolved as `base_instructions.or(file_base_instructions).or(cfg.instructions)` (`core/src/config/mod.rs:3878-3888`); no dedicated CLI flag, only the generic `-c key=value` path.

Both overrides are new-thread-only: on a running thread they are rejected as a no-op mismatch — `"baseInstructions override was provided and ignored while running"` and the same for `developerInstructions` (`thread_processor.rs:200-210`). `codex exec` reaches the identical config keys via `-c`, so the override surface is the same under both drive surfaces; it is a config field, not an app-server-only feature.

The default base prompt is per-model: `model_info.get_model_instructions(personality)` renders `ModelMessages.instructions_template` off the model catalog (`protocol/src/openai_models.rs:518,545-552`). AGENTS.md is separate from both: auto-discovered and concatenated as project-doc content (`core/src/agents_md.rs`), not part of `base_instructions`; no boolean disable found, only `-c project_doc_max_bytes=0` as the size-based off-switch (same shape as the compaction-limit lever above).

### Unknowns and confidence (addendum)

Not verified: whether `-c project_doc_max_bytes=0` fully suppresses AGENTS.md discovery or leaves an empty block; whether `thread/resume` accepts the instruction overrides on a not-yet-running thread; `review/start` was not exercised against a live app-server session. Confidence: high on `review/start` params, delivery, schema mechanism, and the replace/append split (struct definitions and exact rejection strings cited); medium on AGENTS.md disable behaviour and resume-time overrides.
