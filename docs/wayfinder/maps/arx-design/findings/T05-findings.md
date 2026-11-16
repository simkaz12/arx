# T05 findings — permission models as actually built

Ticket: [T05](../T05-permission-models.md) · resolved 2026-08-24 · produced by a hand-ex research agent from Goose source (commit `8d844eecbdfd65626a881c9e8784ae8dc6093f1d`, shallow clone, deleted after reading), official Claude Code docs (`code.claude.com/docs/en/*.md`, v2.1.2xx as fetched), local `.claude` configuration, and the OpenDev paper (`arxiv.org/html/2603.05344v1`).

## 1. Goose — where each layer runs

Goose's tool-visible schema and its per-tool policy sit at two different seams, and they don't talk to each other.

**Schema construction** — `crates/goose/src/agents/extension_manager.rs:1835-1874` (`get_prefixed_tools_excluding` → `filter_tools`). What the model sees is built from which MCP *extensions* are enabled for the session. `ToolExecutionOperation::inference_tools` (`crates/goose/src/agents/state_machine/ops_toolcalling.rs:722-729`) calls this to produce the list handed to the completion call. Granularity is extension-level, not tool-level: enabling or disabling an extension adds/removes tools from the schema; nothing narrower does.

**Runtime gate (per-tool policy)** — `crates/goose/src/config/permission.rs` (`PermissionManager`, YAML-persisted `always_allow`/`ask_before`/`never_allow` lists keyed by tool name) feeds `PermissionInspector::inspect` (`crates/goose/src/permission/permission_inspector.rs:144-268`). Mode-dependent: `Chat` skips tool calls entirely, `Auto` allows everything, `Approve`/`SmartApprove` consult the stored list, and `SmartApprove` can defer to an LLM classifier call (`detect_read_only_requests`) for uncached tools. Default mode is `Auto` (`crates/goose-provider-types/src/goose_mode.rs:22-32`) — all tool calls auto-approved out of the box. This whole apparatus runs *after* the model has already seen and requested the tool: a `never_allow` tool is still in the schema, still requestable, and gets declined post-hoc (`DECLINED_RESPONSE`, `ops_toolcalling.rs`) rather than never offered.

A second, independent runtime gate sits closer to dispatch: `run_pre_tool_hooks` (`ops_toolcalling.rs:116-160`) runs the `PreToolUse` hook chain immediately before `extension_manager.dispatch_tool_call`, after approval has resolved, and can deny outright via `HookDecision::Deny`.

Security/prompt-injection scanning (`SecurityInspector` → `SecurityManager`, `crates/goose/src/security/`) is pattern-matching plus an optional ML classifier, feeding the same `InspectionResult`/`PermissionCheckResult` pipeline (`crates/goose/src/tool_inspection.rs`). Off by default (`SECURITY_PROMPT_ENABLED` → `unwrap_or(false)`, `security/mod.rs:58-73`).

**Sandbox — none.** A full-repo search for `docker|container|seccomp|chroot|namespace|bubblewrap|firejail|landlock` in non-test Rust source turns up nothing that isolates Goose's own tool execution. The only hits are inside the security pattern-matcher's regexes (`security/patterns.rs:246-254`, `docker_privileged_exec`, `container_escape`) and an egress-URL extractor for `docker push`/`docker login` — used to *detect* a model attempting privileged-container escape, not to provide isolation. Tools run as direct host subprocesses.

**Approval reaches the human** through an `ActionRequired` message (`ops_tool_approval.rs:124-133`) held on a `ToolConfirmationRouter` — a `HashMap<String, oneshot::Sender<PermissionConfirmation>>` keyed by request id (`crates/goose/src/agents/tool_confirmation_router.rs`) — until `Agent::handle_confirmation` (`agents/agent.rs:1629-1652`) delivers a decision. The CLI renders this as a blocking `cliclack::select` menu (Allow/Deny/Cancel) (`crates/goose-cli/src/session/mod.rs:2340-2363`); the ACP/desktop path routes through `provider.permission_routing() == ActionRequired`.

## 2. Claude Code — documented surface + observed local configuration

**Enforcement point, stated directly.** `/docs/en/permissions`: "Permission rules are enforced by Claude Code, not by the model. Instructions in your prompt or `CLAUDE.md` shape what Claude tries to do, but they don't change what Claude Code allows." Rules evaluate deny → ask → allow, first match wins regardless of specificity; a broad deny can't carry allowlist exceptions.

**Two enforcement points inside one rule syntax.** A bare tool name (`"Bash"`, or an unanchored glob deny like `"mcp__*"`) "removes the tool from Claude's context entirely, so Claude never sees it" — schema-level. A scoped rule (`Bash(rm *)`) "leaves the tool available and blocks matching calls when Claude attempts them" — runtime gate. Same config array, two different mechanisms, chosen by whether the pattern names a whole tool or a specifier within one.

**Hooks as a gate, with a carve-out.** `PreToolUse` hooks run before the permission prompt and can deny, force a prompt, or skip it — but "Hook decisions don't bypass permission rules": a matching deny or ask rule still fires even after a hook returns `"allow"`. Only a hard block (exit code 2) preempts rule evaluation entirely, including allow rules.

**`auto` mode's classifier is a separate model call**, not the acting model reasoning about its own request. Fixed decision order: rule matches resolve first (protected-path and critical-path writes are forced to the classifier even under a matching allow rule); otherwise a large, versioned, hardcoded allow/block list applies (`curl | bash`, force push, secret-manager writes, IAM grants, the bypass-permissions flag itself, etc.); conversational boundaries ("don't push") are re-read from the transcript each check, not persisted as rules, and are lost on compaction; 3 consecutive or 20 total blocks fall back to manual prompting.

**Protected paths** (`.git`, `.claude`, `.vscode`, `.husky`, etc.) are checked *before* allow-rule evaluation, so an `Edit(.claude/**)` allow rule doesn't pre-approve them — a structural precedence override independent of the deny/ask/allow table.

**Sandbox — OS-level, Bash-only.** `/docs/en/sandboxing`: macOS uses Seatbelt; Linux/WSL2 use `bubblewrap` (unprivileged user-namespace filesystem isolation) plus `socat` (network relay through the sandbox proxy), with an optional `seccomp` filter for Unix-domain-socket blocking. Scope is explicit and narrow: "It applies only to Bash commands and their child processes." Two sandbox modes control only whether sandboxed commands still prompt — the isolation boundary is identical in both. An escape hatch (`dangerouslyDisableSandbox`) lets a command that fails sandboxed retry unsandboxed, itself routed back through the ordinary permission flow. Docs state permissions and sandboxing are complementary: "Sandbox restrictions prevent Bash commands from reaching resources outside defined boundaries, even if a prompt injection bypasses Claude's decision-making."

**Approval reaches the human** as an interactive terminal/IDE prompt (Yes / Yes-and-don't-ask-again / No), with `Ctrl+E` triggering an on-demand risk explainer (a separate model call, Low/Med/High) and an optional comment field delivered back to Claude as post-hoc context (on Yes) or denial reasoning (on No). `/permissions` manages the rule set live, applied from the next tool call in the same turn.

**Observed local configuration.** `~/.claude/settings.json` — `permissions.deny` lists eleven bare tool names: `EnterPlanMode`, `ExitPlanMode`, `DesignSync`, `NotebookEdit`, `PushNotification`, `RemoteTrigger`, `ScheduleWakeup`, `AskUserQuestion`, `CronCreate`, `CronDelete`, `CronList`. Per the docs, every one is schema-removed on this machine, not merely runtime-blocked — a live instance of the pattern the ticket asked about. The same file sets `skipDangerousModePermissionPrompt: true`, `disableClaudeAiConnectors`, `disableBundledSkills`, `disableWorkflows`, `disableArtifact` (surface-reduction toggles, a different mechanism from permission rules), and a `PreToolUse` hook matched on `Bash` piping every Bash call through `~/.local/share/bashfilter/bin/bashfilter-chain` before dispatch — a running instance of hook-as-gate. No `permissions.allow`/`ask` block and no `defaultMode` override at user scope.

Project-scoped files under `~/Projects/*/.claude/`: `client-a-be/.claude/settings.local.json` — a narrow allow list (four Playwright MCP tools, one exact `Bash(mkdir -p ...)`); `pi-assistant/.claude/settings.json` — wildcard Bash prefix allows (`Bash(curl:*)`, `Bash(python3:*)`, `Bash(php -l:*)`, `Bash(docker exec:*)`); `Tabularium/.claude/settings.local.json` — no permission block (`outputStyle` only); `~/Projects/.claude/settings.local.json` — hooks only. None of the eight settings files declares a bare-tool-name deny; the schema-removal instance is confined to the user-level file.

## 3. OpenDev's lesson, as the paper states it

`arxiv.org/html/2603.05344v1`, §2.2, §2.4.1, §3, §5. Five-layer safety stack: Layer 1 prompt-level guardrails, Layer 2 schema-level tool restrictions, Layer 3 runtime approval system, Layer 4 tool-level validation, Layer 5 lifecycle hooks.

Concrete instance (§2.2, on retiring a plan-mode state machine): "Planning is delegated to a Planner subagent with a schema that contains only read-only tools, enforcing the separation at the schema level rather than through runtime permission checks. The Planner cannot write because write tools do not exist in its schema, not because a runtime check blocks the attempt."

The general claim (§3): "Runtime permission checks are the wrong primary abstraction for agent safety. A model that sees a dangerous tool in its schema can reason about invoking it, argue for why it should be allowed, and probe for edge cases in the permission logic. The more robust approach is to make violations structurally impossible: if write tools are absent from the agent's schema, the agent cannot attempt writes because it never sees a way to invoke them. […] This is the difference between a guard rail and a missing road: the model cannot reason about capabilities it does not know exist."

And immediately after: "Defense-in-depth with independently designed layers ensures that no single bypass compromises safety: schema gating prevents unauthorized attempts, the approval system intercepts operations requiring human review, file freshness validation prevents overwriting concurrent edits, and shadow git snapshots […] make all filesystem changes reversible. These layers are deliberately independent; a bug in one does not weaken another."

Layer 3 itself (§2.4.1), for comparison: three autonomy levels (Manual / Semi-Auto / Auto), an `ApprovalRulesManager` with four rule types (Pattern, Command, Prefix, Danger) evaluated in priority order, hardcoded `Danger` rules (`rm -rf /`, `chmod 777`, …) that configuration cannot override, two-tier persisted JSON stores (user-global and project-scoped, project wins), and a frontend-adaptive approval channel — a blocking TUI menu, or a Web UI `approval_required` WebSocket event polled with a 300-second timeout.

## 4. The enforcement-point taxonomy

**Schema-level** — the tool never enters the completion call's tool list; the model cannot reason about it.
- OpenDev: per-subagent schema construction (write tools absent from the Planner's schema entirely).
- Claude Code: bare tool-name deny rules and unanchored deny globs.
- Goose: only at extension-enable granularity; no per-tool mechanism reaches this seam.

**Runtime gate** — tool visible, model requests it, a check between request and dispatch can allow/ask/deny.
- Goose: `PermissionInspector` (rule table, optional read-only heuristic/LLM classifier) → human round-trip via `ToolConfirmationRouter`; plus a `PreToolUse` hook chain right before dispatch.
- Claude Code: scoped deny/ask rules; `PreToolUse` hooks (deny-only override of an allow, no rule bypass); the `auto`-mode classifier (a distinct model call against a fixed, versioned rule list).
- OpenDev: `ApprovalRulesManager`, priority-ordered pattern/command/prefix/danger rules, persisted two-tier.
- All three converge independently on the same shape: a rule table keyed by name/pattern, plus a channel that suspends the step and resumes on a keyed human decision (Goose's `oneshot` per request id, Claude Code's interactive prompt, OpenDev's blocking TUI menu or polled WebSocket).

**OS-level** — enforced by the kernel/process boundary; holds even if the model's reasoning is compromised.
- Claude Code: Seatbelt / bubblewrap+socat+seccomp, scoped to the Bash tool and its children only.
- Goose: absent.
- OpenDev: not a layer in the five-layer stack; reversibility (shadow git snapshots) substitutes for isolation rather than duplicating it.

**Who uses what.** OpenDev and Claude Code run all three levels at once, on different axes of the same tool call (which tool exists; whether this call is allowed; whether the process can physically reach the resource). Goose runs only the middle one for per-tool policy — the reference implementation the design docs cite for the *shell* has, on this question, the shallowest enforcement of the three.

## 5. Mismatches

**Against SURVEY's paraphrase.** SURVEY §1 renders OpenDev's lesson as a flat claim that schema absence beats runtime checks. The paper supports that specific comparison, but the one-liner drops two things:

1. Schema gating is one of five *independently designed, deliberately overlapping* layers, not a replacement for runtime approval — OpenDev keeps a full priority-ranked, persisted runtime approval system running in parallel, in a shape close to what Goose and Claude Code both build independently.
2. The concrete case is narrower than "tool access" in general: a read/write split realized through *dual-agent* schema separation (a Planner subagent whose schema omits write tools), not a general prescription that every tool needs schema-level allow/deny.

A sharpening, not a contradiction: schema gating is what OpenDev trusts for *structural* bypass-proofing of one axis (read vs write), while the *approval* axis (which specific action needs a human) stays a runtime concern in all three systems studied.

**Inside the source itself.** Goose — the repo POSITION §4 names as the shell donor — has its per-tool policy entirely downstream of schema construction: a `never_allow` tool is still shown to the model and only declined after the model requests it. That is the exact arrangement OpenDev argues against, sitting inside the codebase already flagged as "a good donor for the loop and a bad donor for the shape." This extends that caveat onto the permission surface: lifting Goose's tool-dispatch code imports the runtime-only pattern by default.

## 6. Decision questions this exposes

- Where does tool-schema construction sit relative to the harness/memory split already ruled for window content ("harness owns validity, memory owns value")? Is a tool's presence in the completion call's schema a harness-owned protocol fact, or an admission-owned worth judgment?
- Does arx want the OpenDev shape — a structurally distinct schema per capability tier, so a read-only mode has no write tools compiled into its interface at all — or the Goose/Claude-Code shape, one tool registry with per-call runtime classification? Per-mode static schema, or dynamic per turn?
- Does the human-approval suspend/resume mechanism (present in all three systems, already logged in SURVEY as the `askUser`/`submitPlan` back-channel) belong to the tool-dispatch seam, or is it a second, separate control-inversion point?
- Rust bought exhaustive compile-time checking for window plans. Does tool dispatch invite the analogous move — a closed enum of permitted actions a call must satisfy at the type level — or does that over-fit a language feature to a tool list MCP builds at runtime, where Goose's extension system shows the schema is dynamic by construction?
- Goose caches a `smart_approve` decision per tool name across the session; Claude Code saves an approved Bash command or WebFetch domain permanently per repository. Does arx want a persisted, cross-session permission store as its own artifact, or does that belong behind the memory trait as a specialized recall?
- Claude Code resolves conflicting rules with fixed deny→ask→allow precedence; Goose lets non-permission inspectors override the permission inspector's baseline. Does arx need a fixed precedence for its dispatch checks, and is that precedence harness-side (protocol-shaped) or policy-side (Python behind MCP)?
- Does arx want an OS-level sandbox at all, and at what seam — wrapping the tool-dispatch adapter generally (as Claude Code does for Bash specifically), or wrapping subagent execution (a subagent is already a nested session)?
- Both Claude Code's classifier and Goose's SmartApprove insert a *second* model call into the permission decision itself. Does arx's permission model ever call a model to judge a tool call — and if so, does that sit under §5's "no LLM on the write/eviction path" guardrail, or is dispatch a different path than that guardrail was scoped to?

## Confidence and unknowns

High on what's documented/coded as of the fetched commit/doc versions (all claims cite file:line or a doc URL/quote, read directly). Medium on completeness of the local-config survey — eight `.claude/settings*.json` files found under `~/Projects`; managed/enterprise settings (which outrank everything) aren't configured on this machine and weren't probed.

Unknowns: Claude Code's `auto`-mode classifier's own prompt/model is not public; Goose's ACP/desktop approval-delivery path wasn't traced past its call site; whether OpenDev describes OS-level isolation outside the five-layer figure wasn't exhaustively checked.
