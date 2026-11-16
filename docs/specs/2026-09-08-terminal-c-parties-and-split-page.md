# Terminal, phase C: parties, the split page, scroll memory

Class: polishing brief under the owner's working-mode ruling (store decision `b51158b9`): one bounded hand-do brief, the diff left uncommitted for the owner to commit and use. Not a card. `gate_run: yes`.

Phase A named the front seam; phase B (`2026-09-08-terminal-b-panes-front.md`) put the panes front into `arx run` with main's page, the session panel, the hands and judges rows fed by `spawn` and `delivered` events, and the round-two party page. This phase gives each hand and judge the split page the owner ruled on 2026-09-08 (store decisions `5be77638`, `ea96b3ac`): top the brief it was dispatched with, bottom what the harness has of it. It also lands the per-party scroll memory. The prototype's round three (`prototypes/ui/briefs/panes-v3.md`, its README section, its `src/`) is the rendering reference; phase B lifted that code, so most of this phase is feeding it real data.

The rulings that bound this phase: the front shows nothing the harness does not already hold; nothing is editable; no steer, no re-dispatch, no live steps from a running hand. A running hand shows `running` and its timer, nothing else, because nothing else arrives.

## What the harness holds, and how it reaches the front

The brief, the stop question and the verdict text are not on any `Event`. The brief sits on the ledger entry and in the `spawn` row of `threads/main.ledger.jsonl`; the stop question reaches the transcript only inside the `[hand h1 stop] …` turn text; the verdict text sits on the ledger's `verdict` row and the same kind of turn. The front cannot call the ledger from its thread, so the harness sends copies at the moment it has them. Three additions to `TerminalMsg`, all ignored by `LineFront` so the line output does not move:

- `HandSpawned { hand, agent, role, body, image, model, brief }`, sent by `dispatch_and_record` next to its `spawn` event. `body` is the agent's `kind`, `image` the container image from the agent row, `model` the resolved model.
- `HandStopped { hand, question }`, sent where `pump_deliveries!` and the drain loop handle `Delivery::Stop`.
- `HandDelivered { hand, outcome, text, trace, bytes, elapsed_ms, lag_ms, schema }`, sent where they handle `Delivery::Verdict` (and the killed path, with `outcome: "killed"` and no text), after the `delivered` and any `schema` event, so `schema` is `Some("ok" | "malformed")` for a judge and `None` for a hand. `trace` is the handle string `threads/<thread>.<hand>`.

The front's spawn clock is its own `Instant` when `HandSpawned` arrives; the settled figure is `elapsed_ms` from the delivery. No wall-clock spawn time exists in the harness and none is added.

## The trace, read from disk

The trace handle is a directory relative to the run's cwd: `threads/<thread>.<hand>/` with `out/stdout.log`, `out/stderr.log`, `worktree.diff`, `trace.md`, and one session file by body: `session.jsonl` for claude-code (the Claude Code SDK's own transcript: rows with `type` `user` or `assistant`, `message.role`, `message.content[]` blocks of `text`, `tool_use {name, input}`, `tool_result {content}`) and for codex (the rollout file: `type` `session_meta` or `response_item`, `payload.type` `message` with `role` and `content[]`, `function_call {name, arguments}`, `function_call_output {output}`); `transcript.jsonl` for native (rows `request`, `assistant` with `content` blocks `Text` / `ToolCall {name, arguments}` / `ToolResult {content}`, `tool_result`, `gate`, `answer`, `termination`); nothing but `out/stdout.log` for the fake body. The exact shapes are in `crates/arx/src/container_hand.rs`, `claude_hand.rs`, `codex_hand.rs`, `native_hand.rs` and `container.rs`; read them rather than this paragraph when they disagree.

The front reads the directory once, the first time the delivered party's page is opened, on the UI thread, and keeps the parsed turns. Missing or unparseable file: the bottom pane says `trace unreadable: <path>: <error>` above the verdict block and nothing else. Parse into three turn kinds and render them the way the prototype's round three does: assistant text through markdown-lite; a tool call as the dim one-liner `· Name <short args>` where short args is the command, the path, or the first line of the arguments, cut at the pane width; a tool result dim and cut to four lines. Developer, system and `session_meta` rows are skipped. For the fake body, each stdout line is one assistant-text turn. Then the verdict block as the prototype shows it: outcome, the verdict text in full, the trace handle, bytes, `elapsed_ms`, `lag_ms`, the schema line for a judge.

## The split page

Exactly the prototype's round three, fed as above:

- Top, `brief`, about a third of the height: the header line (id, agent, role, body, image, model, `t+4.2s` from the front's session clock), then the brief through markdown-lite, scrolling on its own.
- Bottom, `turns`: `running · 4.2s` ticking; `stopped · 4.2s` with the question in full under it; after delivery the turns and the verdict block; `killed` shows the block with the outcome and the figures it has.
- Enter from the hands or judges panel opens the page with the bottom pane focused; `Tab` swaps panes (the `[split]` key context); `j`/`k`, `gg`/`G`, `Ctrl-d`/`Ctrl-u` and the wheel act on the focused pane; Esc returns to the panel. The bottom line reads as the prototype's split line. Main's page and the session page do not split.
- The hands and judges panels keep round two's rows: id, agent, a status cell ticking while running and settling to `✓ 5.9s`, `stop`, `killed`, or `✗` for a malformed judge schema.

## Scroll memory

The prototype's round-three rule, if phase B did not already carry it: one scroll state per pane, `Following` or `Anchored { row }`, kept for the session, restored on selection, never reset. A first-seen live main follows; ended mains and delivered parties open at the top; a delivered party's `turns` pane opens at the top of its turns. Anchored panes with rows appended below show the dim right-aligned `n new lines · G`; `G` and scrolling to the bottom return to following; `gg` anchors at the top. Wrapped-row counts cached per pane, invalidated on width or content change.

## Bounds

- Edits under `crates/arx/src/panes/`, plus `terminal.rs` (the three variants and the line front's ignore arms) and the sending sites in `main.rs`. Nothing under `crates/arx/tests/`, `sidecar/`, `prompts/`, `docs/`, or the prototypes. No config change, no new dependency unless a JSON shape needs `serde_json` where it is not already in scope.
- The working tree carries uncommitted phases A and B and earlier briefs. Build on them. Never `git stash`, never `git checkout --` a file, never revert a hunk you did not write.
- No inline `#[cfg(test)]` module in a file an integration test `#[path]`-includes.

## Verify

1. Before any edit: `cargo test -p arx` green; record totals per binary (phase B's verdict has them).
2. After: the same totals plus your new unit tests, listed; `cargo test --workspace`; `check-config` unchanged.
3. Unit tests through `TestBackend` at 110x36: a spawn, a stop, and a delivery message sequence renders the split page in each state against fixed frames; the three trace parsers each turn a fixture of five or six rows of the real shape (write the fixtures from the shapes in the source files, one per body, plus a fake stdout) into the expected turn list; an unreadable trace path renders the `trace unreadable` line; per-pane scroll survives leaving and returning; the anchored hint counts appended rows.
4. Live under tmux against the stub sidecar and the fake hand body, as phase B did: dispatch something through the stub so a hand row appears, open its page while running, watch it deliver, read its turns, `G`, `Tab`, Esc, scroll main up during a stream and see the hint, leave and return. Paste the captures. Kill the tmux session when done.
5. Piped mode against the same config with `front = "panes"` prints exactly what it printed before this phase.

## Verdict

Return as text: what was built, the file list with line counts, the `TerminalMsg` additions as landed, the test totals before and after per binary, the last 10 lines of `cargo test -p arx`, the tmux captures fenced, the piped-mode diff (empty), `git status --short`, three things you would change next, and the codex gate outcome with dispositions and log path. Fence field: `gate_run: yes`.
