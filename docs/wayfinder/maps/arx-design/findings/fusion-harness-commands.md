# fusion-harness command survey

Second pass over disler/fusion-harness (2026-08-25), at its command surface rather than its
architecture. Companion to [fusion-harness-trace.md](fusion-harness-trace.md). Read from source at a
fresh shallow clone (`/tmp/fusion-harness`), not from README or docs.

## Where the commands live

Two disjoint sets, easy to conflate:

- `.claude/commands/{install,prime}.md` — Claude Code commands for onboarding into the repo as a
  dev. Pure ceremony: numbered prose checklists, zero orchestration logic.
- `extensions/fusion-harness/{fusion-harness.ts,modules/cmd-*.ts}` — the real product. A Pi
  coding-agent extension; the `/fh-*` commands are Pi slash commands. All the workflow structure is
  here.

Command index from the factory file's own doc comment (`fusion-harness.ts:8-16`):

```
/fh-opinion       N independent read-only opinions            (modules/cmd-readonly.ts)
/fh-debate        N-way all-to-all debate, no judge           (modules/cmd-readonly.ts)
/fh-fusion        N sources → sole-writer FUSION → ACKs       (modules/cmd-fusion.ts)
/fh-collaborate   plans → architect DAG → readiness execution (modules/cmd-build.ts)
/fh-auto-validate architect + Main gate-first build loop      (modules/cmd-build.ts)
/fh-only          direct one slot or arm the next plain prompt
/fh-model         slot → model → thinking picker (session-only)
/fh-system-prompt · /fh-reset · /fh model-bar front door
```

## /fh-opinion — `modules/cmd-readonly.ts:35-77`

Every configured slot (2-5) answers the same prompt independently, concurrently, read-only
(`tools: READONLY_TOOLS = "read,grep,find,ls"`). `Promise.all` over per-slot `runChild` calls. No
merge, no vote — the panel is every slot's answer rendered side by side. Writes `prompt.md`,
`stack.json`, per-slot `answer.md`, `summary.json` under a fresh `mkdtemp` artifacts dir. No writer
lease (nothing writes to the checkout).

## /fh-debate — `modules/cmd-readonly.ts:80-165`

N-way, no judge, all-to-all. `rounds` clamped 1-10, default 3, minimum 2 (opening + closing). Each
round every surviving run gets a prompt that bundles every other still-alive agent's prior labeled
opinion inline — round n's prompt for agent X literally contains Y's and Z's last answers verbatim.
Read-only throughout. Halts below 2 survivors. This is the O(N²) peer-material-inline pattern the
trace flagged as what arx's state object replaces.

## /fh-fusion — `modules/cmd-fusion.ts`

Matches the trace, code-verified again:

1. Every slot researches concurrently, read-only.
2. Abort if fewer than 2 succeed (`cmd-fusion.ts:92-95`).
3. `acquireWriterLease`, then one fresh `FUSION` run (`newRun("FUSION", stack.architect.model)`, no
   session ref — never resumes anything) with `FULL_TOOLS` merges. Its input is every source's
   answer text (60k-char excerpt split) plus the artifact path, so it can `read` the full file when
   the excerpt truncates.
4. Result syncs to the host: `splitUtf8(fuser.text, 80_000)` — chunk 0 rendered visibly, chunks
   1..N sent as `display: false` messages so every byte lands in Main's context without flooding the
   transcript (`cmd-fusion.ts:118-123`).
5. Every slot then gets the fused text with `tools: "none"` and must reply the exact string
   `ACK FUSION <run-id>`. One retry on mismatch (`cmd-fusion.ts:145-150`). Acks recorded per slot
   with delivery route and the fused text's SHA-256.
6. `syncOk` requires every slot acknowledged.

## /fh-collaborate — `modules/cmd-build.ts:56-301`

Four phases:

1. **Propose** — every slot plans read-only. Needs ≥2 successful plans.
2. **Delegate** — the architect alone (persistent session, read-only) merges proposals into one JSON
   delegation DAG, up to 3 repair attempts on parse/validation failure (`parseStrictJsonObject` +
   `validateCollaborationPlan` in `collaboration-graph.ts`). Validation: task ids matching
   `^\d+\.[A-Za-z0-9_-]+$`, no duplicates, known assignees, non-self dependencies, every configured
   slot assigned at least one task, and no dependency cycle (a topological "waves" pass throws if a
   round produces an empty ready-set while tasks remain). Waves are display preview only —
   `collaboration-graph.ts` explicitly does not schedule.
3. **Execute** — an in-command readiness scheduler (`cmd-build.ts:236-259`). A task starts when its
   deps are done, its assignee slot is idle, and — for write tasks — `activeWriters === 0`. The
   increment is synchronous before any `await`, so at most one write-enabled child is ever
   mid-flight. Loop: scan blocked/queued, launch ready, render board, `Promise.race(inFlight)`,
   repeat. Each finished task's report gets its own panel.
4. **Final integration** — one more architect turn, `FULL_TOOLS`, under the same counter.

**Correction to the earlier trace:** the worktree check (`worktreeCommandsObserved`, grepping run
`toolEvents` for `git worktree` in bash calls) and the `maxConcurrentWriteEnabledChildren === 1`
invariant do not abort the run. They are post-hoc audits folded into the final ok flag
(`cmd-build.ts:287`) — a violation marks the summary failed, it does not stop execution. The
trace's "fail the run" phrasing is directionally right but the mechanism is a report-time
assertion, not a guard.

## /fh-auto-validate — `modules/cmd-build.ts:303-780`

Not covered in the earlier trace. Gate-first, red→green:

1. **VALIDATOR designs the gate** — a `uv` Python script written to a harness-dictated absolute
   path (`gate.py`) with the validator's own write tool. Filesystem is the transport specifically
   so a gate whose source contains a code fence survives; fence extraction is a legacy fallback.
2. **Baseline** — `uv run gate.py` must FAIL (exit ≠ 0) before any build. A first-round pass
   triggers a suspicion warning but does not block.
3. **Build → validate loop**, up to `--max-validations` rounds (default 5): builder runs with
   `FULL_TOOLS` on its persistent/forked session; gate re-runs (`GATE_TIMEOUT_MS = 120_000`); on
   FAIL, raw gate output feeds back verbatim into the next round's correction prompt on the same
   session.
4. **Escalation** — on the `escalateAt`-th failure (default 3) the VALIDATOR triages: reads the
   builder's work plus full gate history, writes a directed brief. Triage holds write tools once
   per run (`gateRepairUsed`). If it actually edits `gate.py` (content diff against the pre-triage
   snapshot), that is treated as "the gate was defective": the old gate is archived as
   `gate.py.r<round>` and the repaired gate gets a free re-run that consumes no correction round. A
   pass there ends green with `gateRepaired: true` — the build was right all along, the gate was
   the bug. After one repair, triage drops to read-only for the rest of the run: repair power is
   tool-gated, not trust-gated.
5. Exhausting rounds halts loudly with the last gate output.

Writer lease held for the whole command, released in `finally`.

## Shared mechanics

- **Fan-out**: `Promise.all` over per-slot `runChild`, read-only unless a command needs a writer.
- **Writer lease** (`modules/writer-lease.ts`): atomic `O_EXCL` create at
  `/tmp/fusion-harness-writer-locks/<sha256(realpath(cwd))[:24]>.lock`; contention checks
  `process.kill(pid, 0)` on the recorded owner — alive throws, dead unlinks the stale lock and
  retries once. Release is compare-and-delete. Used by fusion, collaborate, auto-validate,
  `/fh-only`; never by the read-only pair.
- **Escape-to-stop**: one `AbortController` per run; a raw-terminal tap on bare `\x1b` aborts,
  propagating SIGTERM → 5s grace → SIGKILL to the child's process group.
- **Session identity**: `slotSession()` mints a fresh in-memory UUID per (slot, model) pair per
  process, deliberately never persisted across restarts. Round 1 spawns with `--session-id`; later
  rounds resume via the ref the child's own session event reported.
- **Artifacts**: every command writes an artifacts dir; `summary.json` via `ensureSummary` in a
  `finally`, so a summary exists even on early return or abort.

## Ceremony vs structure

Pure ceremony: `.claude/commands/install.md`, `prime.md`. Real structure: all five `/fh-*`
orchestration commands. By mechanism density: collaborate (DAG validation + readiness scheduler +
writer counter) and auto-validate (gate-first loop + triage escalation + one-shot repair downgrade)
are the deepest; fusion is architecturally distinct rather than complex; opinion/debate are simple
fan-outs but real code.

## Disposition (session 2026-08-25)

Lifted: the auto-validate loop shape whole (decision 5b9570c6 — gate as fix-thread charter, minted
by the observer, red-first baseline as hard precondition, refute-the-charter exit tool-gated to one
repair); excerpt-plus-path fold input; bounded-repair validation on minted plans; summary-in-a-
`finally`. Observer tally as digest material is decision 7c849074. Rejected as coordination
plumbing arx answers structurally: writer lease (engine serializes admit), ACK/hash ceremony (state
object is the shared surface), O(N²) debate injection (state object), post-hoc invariant audits
(live observer). Collaborate parked for its own grill — it is a workflow, not a mechanism. Owner
closed the source: that's it from fusion.

## Confidence

High — every claim read directly from source. Files read in full: `fusion-harness.ts`,
`cmd-readonly.ts`, `cmd-fusion.ts`, `cmd-build.ts`, `collaboration-graph.ts`, `writer-lease.ts`,
`child-runner.ts`, both `.claude/commands` files. Not read: `prompt-library.ts`, `model-stack.ts`,
`agent-layout.ts`, `tui.ts`, the `prompts/*.md` bodies — call sites confirmed, exact prompt wording
not inspected.
