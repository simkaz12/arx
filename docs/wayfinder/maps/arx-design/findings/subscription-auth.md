# Subscription-auth findings — spending a Claude Pro/Max subscription from a third-party harness

Produced 2026-08-25 by a web-research agent during the D02 opening session, prompted by the owner's want: Fable on main, billed to the Claude Code subscription rather than per-token. Grounds decision `ca15f9eb` (main does not run Fable under current access terms; supersedes `ed109bc5`). Web sources cited inline; Goose source read from `aaif-goose/goose` at `main` as of 2026-08-25 (the `block/goose` repo was transferred; GitHub redirects).

## 1. OAuth replay against the raw Messages API — banned, enforced

`claude setup-token` issues a long-lived subscription token (`sk-ant-oat01-...`, ~1 year), also consumable via `CLAUDE_CODE_OAUTH_TOKEN`. A third-party client could historically replay it against `api.anthropic.com/v1/messages` by fingerprinting itself as Claude Code: `Authorization: Bearer` (not `x-api-key`), `anthropic-beta: claude-code-20250219,oauth-2025-04-20`, Claude Code user-agent and `x-app: cli`, plus a `system` field that must *begin* with the exact block "You are Claude Code, Anthropic's official CLI for Claude." — server-validated for all models except Haiku (claude-code issue #40515, 2026-03-29). Without the full fingerprint: "OAuth authentication is currently not supported." (issue #28091).

This is the only route that ever combined subscription billing with full host ownership of request construction, and it is closed:

- **2026-01-09**: server-side rejection deployed — "This credential is only authorized for use with Claude Code." OpenCode, Cline, RooCode broke overnight; account bans reported, not just token rejections (opencode issue #6930).
- **2026-02-19**: formalized in the Claude Code legal-and-compliance page: Anthropic "does not permit third-party developers to offer Claude.ai login into their own applications, or to route requests through Free, Pro, or Max plan credentials on behalf of their users."
- **2026-03-19**: OpenCode removed its max-plan plugin — "they sent lawyers" (thdxr).

The ban is categorical over intermediaries, which also answers the OpenRouter question: an aggregator bridging subscription credentials would be this route at company scale. OpenRouter is per-token resale (pass-through rates plus a purchase fee) — same cost class as direct API, never a subscription door.

## 2. Agent SDK — sanctioned for personal use, owns the loop

The SDK is Claude Code as a library; subscription billing works via same-machine `claude` login or `CLAUDE_CODE_OAUTH_TOKEN` (with `ANTHROPIC_API_KEY` unset — API-key auth silently wins). The docs' own comparison table draws the line: the Agent SDK "runs the agent loop in your own process"; a host that wants to build Messages requests itself is pointed at the Client SDK, which is API-key-only. The host gets extension points — MCP tools, hooks, `canUseTool` veto, system-prompt modification, resume/fork — never request construction, context assembly, or compaction. Personal use on your own plan is explicitly supported; shipping a product that logs users into *their* subscriptions is prohibited without prior approval.

## 3. CLI wrapper / ACP — sanctioned, surrenders the window wholesale

Goose's `claude_code.rs` (read in source): spawns the `claude` CLI as a persistent child with `--input-format stream-json --output-format stream-json`; the CLI holds conversation state across turns. `manages_own_context()` returns `true`, and Goose sends **only the last user message** — history, compaction, and request construction all live inside the child process. Goose keeps tool injection via MCP and a permission veto, nothing more. The provider is deprecated in favor of a `claude-acp` provider — the same surrender formalized over the Agent Client Protocol (Zed's route). Policy: sanctioned under the unmodified-binary carve-out.

## 4. Bottom line and watch item

Full host ownership of the window plus subscription billing coexist on zero sanctioned routes. The two sanctioned routes both trade window ownership for legitimacy. For arx this means subscription-Fable can only enter as a **hand** (dispatch whose body is `claude -p` / Agent SDK / ACP — spawn, answer, die, trace; arx controls the container, never the wrapped loop).

Watch item: the "Agent SDK credits" split (separate monthly credits for SDK / `claude -p` / third-party-app usage, announced for 2026-06-15) was paused on its due date — "For now, nothing has changed" — so subscription coverage of hand-shaped usage is provisional, not guaranteed.

## Unknowns and confidence

Not verified: post-2026-03 behaviour of the fingerprint spoof (moot — ToS-prohibited regardless); whether Anthropic ever contacted Goose/Block (no evidence either way). High confidence on the policy timeline (multiple independent sources with dates) and on the Goose mechanics (read in source). Sources: code.claude.com/docs/en/legal-and-compliance · code.claude.com/docs/en/agent-sdk/overview · support.claude.com article 15036540 · anthropics/claude-code#40515, #28091 · anomalyco/opencode#6930 · paddo.dev/blog/anthropic-walled-garden-crackdown · zed.dev/blog/anthropic-subscription-changes · x.com/thdxr/status/2034730036759339100 · github.com/aaif-goose/goose `crates/goose/src/providers/claude_code.rs`.
