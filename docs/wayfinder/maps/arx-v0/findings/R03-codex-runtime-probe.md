# R03 — Codex app-server, run rather than read

Date 2026-09-10. Sources: three live probes against codex-cli 0.153.4 on PATH, logged in via ChatGPT,
driven over stdio from a throwaway `CODEX_HOME` under `/tmp` with `auth.json` symlinked from the real
home. No source tree was consulted — the installed binary is a stripped static-pie ELF and no codex
checkout exists on this machine. Every claim below was observed on the wire or in `ps`.

R02 read the source at `b8c86376` and closed its own section 1 by inspection. `e652ce3a` made a findings
trace a precondition to closing D16; R02 satisfied the reading half, this is the running half. Where the
two disagree, this file is the later evidence and R02 is the older pin.

## 1 The source pin is two weeks stale, and it matters

R02 read `b8c86376` (2026-08-29) against codex-cli 0.147.0. The installed CLI is 0.153.4 and exposes a
protocol several times larger: `codex app-server daemon {start,stop,restart,bootstrap,enable-remote-control}`,
a `codex agents` browser over "all agent sessions on the shared local app-server daemon", `remoteControl/*`,
realtime voice under `thread/realtime/*`, projects, sections, plugins, a marketplace, and an
`approvalsReviewer` concept routing approvals to `user`, `auto_review` or a `guardian_subagent`. The surface
R02 called "the one a main needs" is still present and still behaves as described, but it is now a small
part of a much larger whole. Anything built against R02's field names without re-reading current source
will be built against a shape that has already moved once.

## 2 Token usage: R02's field names are wrong on the wire

R02's cited Rust struct (`protocol.rs:2213-2220`) implies flat `total_token_usage` and `last_token_usage`.
The wire sends them nested under a wrapper with short keys: `params.tokenUsage.{total, last, modelContextWindow}`.
Cadence matches the claim — one notification per model completion, so once for a plain turn and twice for a
turn with a single shell round-trip — and the arithmetic checks out, with `total.totalTokens` in the second
event equal to the first event's total plus the second's `last.totalTokens`. Both numbers really do arrive
together every time.

`modelContextWindow` arrives in the same payload and R02 does not mention it. arx currently infers main's
window from a constant; codex hands it over.

Consequence for `transcript::occupancy` (`transcript.rs:313-324`): it checks for `input_tokens`+`cache_*`,
then `prompt_tokens`, then returns 0. None of codex's three names match, so a codex main's occupancy would
read zero — wrong, not crashing, and silent.

## 3 Approval, interrupt, instructions: confirmed as claimed

`item/commandExecution/requestApproval` is a real server-initiated request the daemon blocks on. `accept`
ran the command and the turn completed with its output; `decline` completed the turn normally with the item
marked `"declined"` and the agent told the command "did not run: permission was rejected" — matching R02's
"the agent will continue the turn". One nuance R02 does not carry: the request ships an `availableDecisions`
list that omitted `decline` for both commands tried, yet the server accepted `decline` anyway. The response
schema's enum is the protocol surface; `availableDecisions` behaves as a UI hint, not a validation whitelist.

`turn/interrupt {threadId, turnId}` holds exactly as claimed and is the strongest result here. A 20-second
command interrupted at roughly 5 seconds returned `turn/completed{status:"interrupted", items:[]}`, the
process stayed alive, and the same thread accepted a further turn afterwards. This is a better primitive
than Claude Code's session-level kill, where the turn and the session die together.

`baseInstructions` on `thread/start` replaces the system prompt wholesale — a canary codename injected there
came back as the model's entire answer to an unrelated question.

## 4 Model switching mid-thread exists, and neither earlier findings file mentions it

`turn/start.model` changes the model for that turn and every later turn on the same thread, with no restart:
a thread started on `gpt-6-astra` and given one turn with `model: "gpt-5.6-sol"` reported `gpt-5.6-sol` on a
subsequent `thread/read`, and kept it for a third turn that passed no override. The other documented lever,
`turn/settings/update`, refused with `"requires experimentalApi capability"` — a named gate on a flag the
client declares at `initialize`, not tested further.

This bears directly on card A's decision to wire `/model` on main for the claude-code body alone. The reason
given was that live switching is an Agent SDK method; codex has its own, so the restriction is now a choice
rather than a constraint.

## 5 AGENTS.md suppression is real, and project-scoped only

R02's section 1 is confirmed behaviourally. A project `AGENTS.md` canary was listed verbatim by default and
absent under `-c project_doc_max_bytes=0`. Request-payload telemetry under `RUST_LOG=trace` shows the
`agents_md.instructions` content item present by default and entirely missing under the flag — absent rather
than empty, consistent with the early return R02 read at `agents_md.rs:65-67,129`.

New: the flag reaches only the per-project file. With both files present, the global `$CODEX_HOME/AGENTS.md`
canary survived the flag intact while the project canary vanished. There is no flag for the global file. The
only lever that reaches it is relocating `CODEX_HOME`.

This is a prompt-sovereignty hole for a main on the host. `68c366dc` has arx replacing the body's system
prompt, and `baseInstructions` does that — but `AGENTS.md` is a separate injection path `baseInstructions`
does not cover, so a codex main running under the owner's real home would silently inherit
`~/.codex/AGENTS.md`. Two suppression moves are needed, and the second is an env var.

## 6 A codex main is arx's own child

`codex app-server --stdio` is a direct child of its spawner: same `ppid`, same `pgid`, no fork, no
daemonize-away. `SIGKILL` on the spawner alone killed it in the same beat rather than reparenting it to init.
`e5ae9e17`'s spawn-answer-die lifetime and the CLI-body child model hold as designed, on one condition —
arx never issues `daemon start`, `enable-remote-control`, or a non-stdio listen.

The shared daemon is a separate mechanism, not the same one wearing another name. It refused to start at all
under a mise-installed CLI, requiring an installer-managed binary tree at
`$CODEX_HOME/packages/standalone/current/codex`. Forced up with one symlinked in, it self-detached, opened a
unix socket at `$CODEX_HOME/app-server-control/app-server-control.sock` and wrote a pidfile carrying
`{pid, processStartTime}` for liveness. A plain `--stdio` call under the same `CODEX_HOME` ignored it
entirely and spawned its own process, on a different binary version. Two concurrent stdio processes likewise
got distinct pids and distinct threads with no attach.

## 7 The isolation unit is `CODEX_HOME`, not the pid

Threads outlive the process that made them. After a session's process was killed by killing its parent, a
brand-new app-server process resumed that thread ID and got the full turn history back, both items verbatim.
The durable record is `$CODEX_HOME/sessions/.../rollout-*.jsonl` plus `state_*.sqlite`; killing the process
touches none of it.

Concurrent access is arbitrated on disk, not in memory: a second process attempting to resume a live thread
was refused with `thread <id> already has an active writer`, backed by
`$CODEX_HOME/thread-writer-locks/<id>.lock`. That lock is a safety net worth relying on rather than working
around — an inspection session against a running hand's home cannot tear its transcript.

Everything observed stayed inside its given `CODEX_HOME`: sqlite, session files, locks, the daemon's socket
and pidfile. A second home run concurrently saw nothing of the first.

Consequence for quarantine. arx owning "the transcript" is true of what crosses the pipe and false of the
durable record, which is a directory subtree any later spawn can list and resume once the writer lock clears.
If one `CODEX_HOME` is shared across a position's resets, a quarantined transcript is still sitting there,
resumable, unless arx avoids naming the old thread. Per-position homes close this, and the same move closes
section 5's global-`AGENTS.md` hole. One mechanism, both problems.

## 8 A precondition nobody wrote down

Codex refuses to run in an untrusted directory, and `trust_level = "trusted"` in `config.toml` was not
sufficient on its own — it kept refusing with "Not inside a trusted directory" until the directory was also
a git repository. A write hand's detached worktree already is one. A bare scratch mount would not be.

## Open

- `codex agents` could not be listed live: it is a TUI and refuses without a real tty. Its cross-home reach
  is inferred from every other boundary holding, not observed.
- What `experimentalApi` unlocks once declared at `initialize`. The gate was confirmed; its contents were not.
- `enable-remote-control` was read as help text and deliberately not run — it opens a listener.
- R02's own two open items on strict-mode JSON schemas, untouched here, and unrelated to a main-seat driver
  unless one leans on schemas the way the judge does.
- Nothing here re-verifies R02's source claims at current HEAD. Section 1 says why that now matters.
