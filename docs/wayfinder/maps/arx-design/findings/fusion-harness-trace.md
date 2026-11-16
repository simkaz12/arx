---
labels: wayfinder:findings
source: hand-ex trace of github.com/disler/fusion-harness (extensions/fusion-harness/), 2026-08-25
provenance: ad-hoc — no research ticket; produced during the seats/thread rubber-duck session after a README-only read proved insufficient
grounds: D04, D06, D02 (as counter-evidence and donor material)
---

# fusion-harness — verified mechanics

A Pi extension orchestrating 2–5 models as child processes. Not a harness in arx's sense — it sits on a host loop it does not own. Every claim below is code-verified; the README held up on every checked claim.

## Child spawn

`runChild` launches `pi --mode json -p --session-dir <dir> --no-skills --no-extensions --no-context-files --thinking <level> --model <model> [--fork|--session|--session-id] [--system-prompt] [--append-system-prompt]... [--tools <allowlist>|--no-tools] <prompt>`. Env inherits the parent whole (API keys included) plus `PI_OFFLINE=1`. The child is its own process-group leader; kill is SIGTERM to the group, then SIGKILL after 5s grace unless the `close` event already fired — `ChildProcess.killed` is explicitly not trusted. The group sweep fires on abort and timeout only, **not on normal completion**.

## Context composition — two channels, stacked

The persistent-vs-fresh question is a false fork; both mechanisms run at once:

- **Own history via session resume.** One session per slot+model-hash, minted per app run. Round 1 spawns `--session-id <minted>`; later rounds resume `--session <ref>` using the id the child itself reported. "Clean-room" means no skills/extensions/context-files, not statelessness.
- **Peer material via fresh inline prompt text, every round.** Labeled opinion blocks injected into the round's user prompt. The rebuttal template states it: "Your own previous opinions are already in your resumed session. Below are the other agents' complete, clearly labeled opinions."

The one true fresh spawn is the FUSION merge agent — no session ref by design, partly for a TUI footer-state reason a code comment admits, not only the unbiased-merger principle.

## /fh-fusion pipeline

Fan-out: every slot answers read-only concurrently; answers land in the run dir; abort if fewer than 2 succeed. Writer lease acquired, then the sole-writer FUSION agent (full tools) merges. **Its input is the externalize shape verbatim**: a bounded excerpt per source (60k chars split across sources) plus each source's full artifact path, so it reads the complete file through the ordinary read tool when the excerpt is not enough. Sync back to Main is the opposite philosophy: fused text chunked at 80k bytes, chunk 0 visible, the rest as `display:false` messages — every byte stays in the host context, only the rendering truncates. Every slot then ACKs with `--no-tools`: exact-string `ACK FUSION <run-id>`, zero tool calls required, one retry, SHA-256 of the fused bytes recorded per slot.

## Enforcement — structural read axis, advisory process axis

Read-only is structural: `--tools read,grep,find,ls` on the child process; write tools are never registered, not merely forbidden. But nothing except a repeated prompt clause stops a full-tools writer child from backgrounding a process and returning "done" while it still runs — the harness has no mechanism for that class. This is the hole a container-per-hand (`21e8f9a9`) closes by construction: teardown kills everything the hand started.

## Writer lease

Atomic `O_EXCL` file create at `/tmp/fusion-harness-writer-locks/<sha256(realpath(cwd))[:24]>.lock`. Contention: if the recorded pid is alive, abort before anything spawns; if dead, unlink the stale lock and retry once. Release is compare-and-delete on the owner string. Worktree commands are observed and **fail the run** — worktrees would break the lease's soundness, so fusion forbids the isolation arx chose instead.

## /fh-collaborate

An in-command scheduler loop, not the DAG module (that only validates and renders). A task starts when its deps are done, its slot is free, and — for write tasks — an `activeWriters` counter is 0, incremented synchronously so at most one write-enabled child is ever mid-flight. A task's child receives the full report text of every completed dependency, concatenated and truncated to 60k, on the slot's persistent session. Final architect integration turn runs under the same counter.

**Correction (2026-08-25):** the worktree check above and the `activeWriters === 1` invariant do not abort the run — both are report-time assertions folded into the final `ok` flag. See `fusion-harness-commands.md` for the code-verified detail.

## What arx takes from this

- The single-writer token is what coordination looks like when you don't own the internals; arx makes each collision unrepresentable instead (role for the state object, worktree for the CWD, serialized admit for the store).
- The fuser's excerpt-plus-path input independently arrives at admission's head-plus-handle ladder.
- Peer-material-inline every round is the O(N²) shape the state-object bulletin board replaces.
- Sessions die with the app run; there is no memory story at all — no admission, no store, no traces.
