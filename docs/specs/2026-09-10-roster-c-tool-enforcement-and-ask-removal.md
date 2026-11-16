# Roster C — the tools list means something, and nobody gets asked

## Problem / Solution

Card B shipped a notice saying a position's `tools:` list is inert on the codex body, on the grounds that announcing a gap beats pretending it isn't there. This card closes as much of that gap as codex allows, and settles what the gate is for.

The owner ruled 2026-09-10 that gating is tool possession, not per-call approval: a seat holds the tools it should have and is never asked about them. Main runs ungated. That ruling makes two changes true at once. Codex hands must actually lose the tools their position does not list, which today they keep — `build_argv` (`codex_hand.rs:38-54`) never reads `spec.tools`. And `GateVerdict::Ask` must go, along with the `a / A / d / D` operator surface, because no path should produce it.

Enforcement on codex is partial and cannot be made complete. Codex has no allowlist: tools are negated one at a time across two config namespaces, `features.*` booleans and separate top-level fields for tools that outgrew a boolean. `codex features list` reports over 150 flags and the set grows per release, so arx cannot enumerate what to negate. This card negates what it can name and makes the notice say so. A `tools:` list on codex means "these, plus whatever codex ships that arx does not yet know to switch off" — stated, not implied.

## Inherited agreement

### Observable behavior and examples

A codex hand whose position lists no shell tool cannot run one. Today:

```
$ codex exec --json "run the shell command 'echo hello'"
{"type":"item.completed","item":{"type":"command_execution","command":"/usr/bin/bash -lc 'echo hello'","exit_code":0}}
```

After, for a position without shell in its `tools:`, arx adds `--disable shell_tool` and the stream carries zero `command_execution` items; the model reports no shell execution tool is available.

`check-config` gains a second line per codex position, naming what arx does not control:

```
[arx] tools: bolt lists tools, but gpt-5.6-luna runs on the codex body, which ignores them
```

becomes a statement of partial coverage rather than total inertness. Exact wording is scope 3's to settle.

Nothing asks the operator anything. The `[arx] ask: {tool} {subject}` line and its `a allow once · A allow always · d deny once · D deny always` prompt are gone.

### Evidence

- Codex negation knobs are real and work under `exec` — `--disable shell_tool`, `--disable view_image`, `-c web_search="disabled"`, each proven live against codex-cli 0.153.4 on 2026-09-10 [ran]. `view_image` disabled collapsed `cached_input_tokens` from 25600 to 0, so the tool schema genuinely shrank rather than the model refusing.
- `web_search` is **not** a `features.*` flag. It is a top-level field, enum `disabled | cached | indexed | live`. The names `web_search_cached` and `web_search_request` exist in the feature registry but are deprecated and off, and do not govern the live tool [ran].
- No allowlist exists anywhere in the protocol schema — `enabled_tools`, `allowed_tools`, `tool_allowlist` return zero hits across 740 definitions. `mcp_servers.<name>.enabled_tools` is real but filters external MCP servers, a different axis [ran].
- `codex_hand.rs` never reads `spec.tools`; `build_argv` already emits two `-c key=value` pairs and generalizes [read].
- claude-code hands already enforce — `effective_tools()` (`claude_hand.rs:66-82`) filters `spec.tools`, strips write tools when `!spec.write`, builds `--allowedTools` and `--disallowedTools` [read].
- Main is already ungated — `claude-main.mjs:364` sets `allowedTools` to every arx tool unconditionally and the init op never passes `spec.tools` [read].
- `GateVerdict::Ask` has 13 references across `gate.rs`, `main.rs`, `claude_code.rs`, `native_hand.rs`. It is live on the native body (`native_hand.rs:671-697` stops the hand, polls for an answer, then executes or denies) and converted to deny on claude-code (`claude_code.rs:455`) [read].
- `R03-codex-runtime-probe.md`, committed at `7c70ccd`, for the codex runtime claims this card builds on.

### Uncertainty

- ~~Which flag governs the `collaboration.*` family~~ — **settled 2026-09-10, negatively.** `features.multi_agent`, `multi_agent_v2` and `collaboration_modes` were each probed live; all three left `input_tokens` byte-identical to baseline at 14641, while `--disable shell_tool` shifted it to 14001, calibrating the method. No flag was found. The family stays enabled and the table carries no entry rather than a guessed one.
- ~~Whether `features.unified_exec` needs negating alongside `shell_tool`~~ — **settled 2026-09-10.** It does not. `--disable shell_tool` alone removed the shell including a direct `exec_command` attempt; `--disable unified_exec` alone left a real command running; the two together produced the identical 14001 as `shell_tool` alone.
- **Whether `gate::append_rule` has callers besides the deleted prompt.** If not, it and its rule-writing path go with the surface; if so, they stay.

## Decisions locked

- **D1: Gating is tool possession, not per-call approval.** Owner ruling 2026-09-10, on the standing practice of running Claude Code under bypass permissions without incident. Rejected: per-call human approval.
- **D2: Main runs ungated with every tool.** Destructive work stays main-exclusive by the existing shape — hands write into a detached worktree and main promotes the diff (`4f90f38f`). No code change; this records why none is needed.
- **D3: `GateVerdict::Ask` is deleted, with its operator surface.** Under D1 no path produces it. This removes working behaviour on the native body, not dead code.
- **D4: A gate rule or hook naming `"ask"` becomes a load error, not a silent downgrade.** A verdict arx cannot honour must not be accepted quietly. Rejected: treating it as deny, which would leave configs asserting something untrue.
- **D5: Codex enforcement is partial and labelled.** arx negates the tools it can name; the notice states that coverage is incomplete. Rejected: silent partial enforcement, which is card B's "active lie" in smaller form; also rejected: no enforcement, which leaves the list decorative.
- **D6: The codex mapping is a hand-maintained table, keyed on arx's existing tool vocabulary.** No allowlist exists to derive it from. The keys are the names already used in `agents/*.md` frontmatter and enforced by `claude_hand.rs` — `Read`, `Glob`, `Grep`, `Bash`, `Write`, `Edit`, `MultiEdit` — not new words. `Bash` maps to `features.shell_tool`. Amended mid-run 2026-09-10 after the first pass keyed the table on invented names (`shell`, `image`, `web`), which would have stripped `Bash` from `bolt` and `hand-do` the moment the card landed, since both declare `tools: [Read, Glob, Grep, Bash]` and neither is roster-pinned off codex. Rejected: deriving negations from `codex features list` at runtime — the registry has over 150 entries with no marker for "is a model-facing tool". Rejected: inventing a second vocabulary for codex, which would make one `tools:` list mean different things per body, the exact fault card B's notice exists to announce.
- **D7: arx has no tool name for image viewing or web search, so both are negated for every codex position.** This is the intended reading of D1, not an oversight: a seat holds what it is given, and nothing gives it those. Adding `web_search` or an image tool to arx's vocabulary is a later decision, not this card's.

## Constraints

- `crates/arx` has no `lib.rs`. Integration tests `#[path]`-include `src/*.rs`, so an inline `#[cfg(test)]` module in an included file registers twice and breaks locked counts. New unit tests go in `main.rs`'s test module.
- `cargo test -p arx` needs the Docker daemon. After editing any script under `crates/arx/tests/fixtures/*-stub/`, run `docker rmi` on that tag first or the tests keep running the old stub.
- **This card deletes tests.** The count will fall, not rise. The guard is that every removed test is one asserting `Ask` behaviour and nothing else is deleted — name each removed test in the verdict.
- No branch switch, no commit, no staging.

## Execution plan

### Scope 1 — delete `Ask` and its operator surface

- **Files:** Modify `crates/arx/src/gate.rs`, `crates/arx/src/main.rs`, `crates/arx/src/claude_code.rs`, `crates/arx/src/native_hand.rs`, `crates/arx/src/terminal.rs`
- **Build:** a gate with two verdicts, and no prompt anywhere.
- **Steps:**
  1. Remove the `Ask` variant from `GateVerdict` (`gate.rs:13-17`) and its `Serialize` arm (`gate.rs:37-41`).
  2. Make deserialization of `"ask"` an error rather than a variant (`gate.rs:389`), per D4. Give it a message naming the file and the rule so an operator can find it.
  3. Remove `hook_ask_decision()` (`gate.rs:418`) and any hook path that can only produce `Ask`.
  4. Remove the conversion at `claude_code.rs:455` and the early-return check at `main.rs:1666`, which exists only to route `Ask`.
  5. Remove the native ask loop (`native_hand.rs:671-697`) and the verdict-to-label arms at `main.rs:1640`, `main.rs:1783`, `native_hand.rs:652`.
  6. Remove `TerminalMsg::Ask` (`terminal.rs:168-174`), the `a / A / d / D` prompt, and the key reader at `main.rs:1717-1777`.
  7. Check whether `gate::append_rule` still has callers. If the deleted prompt was its only one, remove it and its path; if not, leave it. Report which.
  8. Remove the tests that assert `Ask` — `gate.rs:593`, `gate.rs:610`, `main.rs:5220` — and no others.
- **Verify:** `cargo test -p arx` green. `cargo test --workspace` green. `grep -rn "GateVerdict::Ask\|TerminalMsg::Ask\|ask not yet supported" crates/ sidecar/` returns nothing. A test asserting a gate config naming `"ask"` fails to load with the D4 error. Report the before and after test totals and name every removed test.

### Scope 2 — codex hands lose the tools their position does not list

- **Files:** Modify `crates/arx/src/codex_hand.rs`
- **Build:** `build_argv` negates codex built-ins absent from `spec.tools`.
- **Steps:**
  1. Add the mapping table from D6, keyed on arx's existing tool vocabulary. `Bash` → `features.shell_tool`. Image viewing → `features.view_image` and web search → `web_search="disabled"`, both unconditional per D7 since no arx tool name maps to them. `features.unified_exec` is NOT needed — the live probe showed `--disable shell_tool` alone removes the shell entirely, including a direct `exec_command` attempt, while `--disable unified_exec` alone left a command running. No entry for the `collaboration.*` sub-agent family: three candidate flags were probed and all three left the token count byte-identical to baseline, so no lever was found. See Out of scope.
  2. In `build_argv` (`codex_hand.rs:38-54`), for each table entry whose arx tool is absent from `spec.tools`, emit the negation as an additional `-c` pair beside the existing `instructions=` and `project_doc_max_bytes=0`. Route the string-valued `web_search` through `escape_toml_basic_string` (`codex_hand.rs:23`); the booleans need no quoting.
  3. An empty `spec.tools` means the position named nothing, which under D5 negates every tool in the table. Do not treat empty as "allow all" — `claude_hand.rs:66-82` treats empty as a default set, and the two bodies must not disagree about what an empty list means. If matching claude's default-set behaviour is the better read, say so in the verdict rather than choosing silently.
  4. **Before writing the table, settle the two open flags live**, per the Uncertainty section: whether `--disable multi_agent` removes the `collaboration.*` family, and whether `unified_exec` needs negating alongside `shell_tool`. Run these as real `codex exec` calls in a throwaway `CODEX_HOME` under `/tmp` with `auth.json` symlinked, never copied, and a git-initialised project dir — codex refuses an untrusted directory. Never point codex at this repo. Report the raw before and after.
- **Verify:** `cargo test -p arx` green. Unit tests asserting the emitted argv for a position listing shell and one listing nothing. Live: dispatch a codex hand whose position omits shell, and confirm from its trace that no `command_execution` item appears and the model reports no shell tool. Live: dispatch one that lists shell and confirm a command runs.

### Scope 3 — the notice tells the truth

- **Files:** Modify `crates/arx/src/config.rs`, `crates/arx/src/main.rs`
- **Build:** the codex notice states partial coverage instead of total inertness.
- **Steps:**
  1. `inert_tools_notices` (`config.rs:416-436`) currently tells a codex position its tools are ignored. That is now false — some are enforced. Rewrite the codex arm to name what arx controls and to say the rest of codex's built-ins remain, per D5.
  2. Leave the native arm alone; `resolve_native_tools` (`main.rs:650`) still enforces exactly `read_file` and `recall`, and its notice is accurate.
  3. Leave `effective_rows_for_notices` (`main.rs:518-530`) alone. Card B's pin-aware behaviour is correct and tested.
- **Verify:** `cargo run -p arx -- check-config ./arx.toml` exits zero and prints the new wording for `bolt` and `hand-do`. Update the card B tests that assert the old wording; do not delete them.

## Out of scope

Done means: no path can produce `Ask`, a codex hand loses the named tools its position omits, and the notice says what arx does and does not control.

Not in this card:
- **Container network, capabilities and resource limits.** `container.rs:148-182` bounds the filesystem (`--user`, explicit mounts, tmpfs masks) but passes no `--network`, `--cap-drop`, `--read-only`, `--memory`, `--cpus` or `--pids-limit`. The owner ruled 2026-09-10 that this stays deferred. It is the named hole in tools-as-the-gate: egress cannot simply be closed, since hands need the model API, so bounding it needs a proxy or a custom network rather than flags.
- **Growing native's tools** — `grep` and `bash` as native tool defs. Capability, not enforcement.
- **Codex's tools under `app-server`** — the main seat's surface, and a later card's problem. This card touches `exec` only.
- **Converging codex hands onto `app-server`**, worth doing later for the approval callback hands would gain.
- **Anything in the main seat.** D2 records that main needs no change; the main-seat interface extraction is the next card.
- **Auditing the rest of codex's 150-plus feature flags.** D6 accepts a hand-maintained table.
- **Codex's `collaboration.*` sub-agent family** — `spawn_agent`, `send_message`, `list_agents`, `interrupt_agent`, `followup_task`, `wait_agent`, shipped enabled with no flag found to switch them off. A codex hand can therefore dispatch outside arx's ledger, gate and trace. This is a live hazard, not a tidy-up: it contradicts arx owning dispatch and the process tree (`705e4df2`). Nothing in this card can close it, and the container is not currently a boundary against it either, since container hardening is deferred. Recorded in the map as a hazard class to recheck on every new body.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-10-roster-c-tool-enforcement-and-ask-removal.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
