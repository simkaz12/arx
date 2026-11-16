# arx

An agent harness: a Rust shell around the model loop, with memory and context policy owned by the harness rather than bolted on as a tool. This glossary covers the terms arx uses for its own machinery — the ones that mean something specific here and that get muddled when left undefined.

Reconciled to decisions `749c8765` and `da0ccf24` (2026-08-27); D15, D09, D11 and D05 rulings of 2026-08-28 folded in; D14, D06 and D12 rulings of 2026-08-29 folded in.

## The loop

**Turn**:
One operator input through to main's next EndTurn — the unit the operator waits on. Idle is between turns.
_Avoid_: request, exchange, prompt

**Step**:
One model call plus the dispatch and admission of the tool calls it returned; repeats within a turn until the termination enum ends it. The boundary a stopped child holds at.
_Avoid_: iteration, cycle, round

**Abort**:
The operator cancelling main's in-flight step — `q` in the panes front (owner ruling 2026-09-09, reopening the 2026-09-08 clause that kept abort out for want of a path; the sidecar now calls the SDK's `interrupt()`). What arrived stays in the window under a `stopped by operator` notice; anything the operator typed meanwhile waits in the front's visible queue and goes as the next turn, one turn per line. No mid-turn delivery and no per-message priority; steering main for now is stop plus a queued line. Main-side only: a hand is steered or killed, never aborted.
_Avoid_: interrupt (the mechanism inside a steer), cancel

## The window

**Window**:
The material sent to the model on a given step. Append-only during a session; nothing is removed by harness policy.

**Append**:
The one automatic window mutation. Material enters at the tail and stays.

**Rewind**:
Truncating the window back to an earlier point, discarding the tail to the trace archive. Fires two ways: an operator command for the poison case — a stale read, a wrong verdict, a leaked secret — and as a mechanical step of the checkpoint and reset rituals: checkpoint turns rewound out with a breadcrumb, a window past the reserve rewound to fit before the ritual, a rejection inside the ritual rewound by the vendor's reported overshoot. Never fired on a pressure clock (2026-08-30, map-integrity debate).
_Avoid_: compaction, eviction, pruning

**Splice**:
Removing or rewriting material mid-window while keeping the tail. Deliberately not built; named only to be excluded.

**Nudge**:
The harness's one-time notice that main's occupancy has crossed the soft limit (per-model config, default 120k) — said once on the operator surface and once as a line in the window, and nothing else follows from it. Main only: a hand that gets there was given too much for one dispatch, which is logged, not acted on. Distinct from the reserve, which arms a reset.
_Avoid_: warning, smart zone, soft limit (the number, not the notice)

**Reserve**:
The room below main's window ceiling sized to the ritual — `[main] reserve`, default 16k under a default ceiling of 800k. Arming happens at ceiling minus reserve; the harness fires alone at the ceiling. Distinct from the nudge, which only informs.

**Window limit**:
The model's context size, a per-model config constant the operator maintains from the vendor's published model pages. The nudge line and the reserve are both measured against it; it is never discovered at runtime. A ContextWindowExceeded is the signal that it may be stale.
_Avoid_: context length, max tokens (that is the output cap)

## The provider seam

**Provider**:
The stateless seam to a model vendor — (system, messages, tools) in, a stream of semantically complete partial-Message chunks out, the terminal chunk carrying the termination enum. All wire knowledge lives inside the adapter: delta assembly, stop-signal translation, retry-before-first-item. No capability flags cross; nothing about a provider changes the loop's control flow. A body is not a provider, and the provider trait is untouched by bodies; a model behind a CLI enters as a body through the D16 seam, in any role — main, hand or reviewer (2026-08-30, map-integrity debate).
_Avoid_: backend, LLM client

**Termination enum**:
The typed signal on a stream's terminal chunk — EndTurn, ToolUse, MaxTokens, Refusal with its category. arx's own values, translated from each vendor's wire signal by the adapter; the loop branches on it in one place. The channel test is the wire's: a completed response (HTTP 200, Refusal included) terminates; a request rejected before the first stream item is a Provider error, never a termination. OpenAI-compatible `length` is MaxTokens, unconditionally.
_Avoid_: stop_reason, finish_reason

**Provider error**:
What the seam returns when no Message was produced, classed by what the caller does next — Transient (retried by the stream wrapper before the first item), Fatal (halt, surface to the operator), ContextWindowExceeded (a reserve miss: the harness fires rewind-to-fit then the reset ritual itself, logs the miss size, and tells the operator the configured window limit may need updating). Refusal is not one; the wire delivers it as a completed response.
_Avoid_: ProviderError, exception, ContextLengthExceeded

**Assembler**:
The request-construction step, and it is identity: serialize the window as-is, prepend the system prompt and the tool surface. No projection, no filtering — what the window holds is what the model sees. Difference between windows is made where material enters (admission, a child's brief at spawn), never here.
_Avoid_: projection, view, renderer

## The operator surface

**Front**:
The trait the operator surface implements — `render`, `event`, `read` — so the loop drives any front the same way. Two adapters: `LineFront`, today's blocking `> ` prompt loop, and `PanesFront`, a full-screen ratatui dashboard with main's transcript, a session panel and party panels for hands and reviewers, chosen with `[terminal] front = "panes"` in arx.toml (default `line`); piped stdin always keeps the line front, and so does an interactive session whose stdout is redirected. `[terminal] keys` names a `keys.toml` overriding the panes front's embedded key bindings by name; a missing or unreadable file falls back to the defaults it would have overridden. In raw mode Ctrl-C is a key the panes front handles, not a signal — the drain and the adapter's shutdown run afterward on the restored screen, which piped Ctrl-C today does not; `q` no longer quits, it is the stop key. `q` while main's turn is open sends a control-lane stop, the SDK's own `interrupt()`, ending the turn early with what streamed so far kept and a `stopped by operator` notice under it; `q` on a running or stopped hand or reviewer opens the same kill confirm `x` does; `q` with nothing running says so. `/reset` and `/kill <id>` are operator lines the harness intercepts before either becomes a turn: `/reset` fires the reset ritual (see Reset); `/kill <id>` calls the same spawn-ledger kill the `kill` tool calls, outside any turn, and reports an unknown or already-settled id back to the front the way a bad `/model` argument does. A line submitted while a turn is in flight waits in a visible queue and is dispatched, oldest first, as its own turn once the turn ends; `Ctrl-V` pastes text or an image from the system clipboard into the input.
_Avoid_: terminal (the crate, not the concept), UI

## Cutting

**Reset**:
Ending the main's window mid-thread and starting it fresh. Fires against a reserve, never the limit: the harness holds a reserve below the hard window limit sized to the ritual, arms at limit-minus-reserve and tells the operator, the operator fires. The operator's line is /reset; before arming it runs the same ritual. The harness fires alone only at the reserve's floor, so an unattended thread never dies without a state object. A window already past the reserve rewinds to fit first (mechanical, cut-to-archive), then runs the ritual. The ritual, the same one a checkpoint runs: the outgoing main authors the state object in one turn and truth-checks it in a second, harness-prompted turn; the harness runs a mechanical check on every write (file exists, decision id resolves, sha still matches) and blocks on failure; an attending operator sees the object before teardown and can amend it. Then teardown, respawn, re-orient from the state object plus grounding — and the respawned main's first turn is the probe. Hands in flight run on across it; their verdicts land at the respawned main's first step boundary after that turn. Main still holds both model pens; the mechanical check, the operator surface and the probe are the checks of the other kinds. A rejection inside the ritual itself rewinds by the overshoot the vendor reported and resends; the floor is authoring from the archive handle alone, reached mechanically and logged. An arx-native hand resets the same way on its own window, under the same brief. Multiple resets per thread are normal.
Binds main under every body: on a CLI body it is kill-and-respawn, a fresh session with the state object as orientation and the body's own compaction switched off where it can be, with no rewind-to-fit first, since rewind is the one native-only mechanic (D16 round 2). The exhaustion floor for a CLI-bodied main: on the child's ContextWindowExceeded during the ritual, arx kills and respawns from the last checkpoint's state object plus the archived transcript handle, and the respawned main's orientation turn reports what that checkpoint cannot recover, logged as its own miss event; with no checkpoint in the thread the floor is the transcript handle alone (2026-08-30, map-integrity debate).
_Avoid_: cut (its pre-2026-08-25 name — stored records use it), digestion, compaction

**Cut**:
Retired 2026-08-27. Was ending the thread at doneness, cheap by design, with facts demoting out of the store's embedding space into the dead thread's archive while the why-layer (decisions, digests, handles) stayed — a per-fact triage keyed to the event. Rejected along with the observer and the digest: done now means the thread's record lands in a plain file, never the store — no archive, no triage, no why-layer kept separate from the thread. Whether cut is still the working name for that ending is not settled by either ruling. See Thread.
_Avoid_: as a live term — see Thread; older stored records also call this expiry (pre-2026-08-25).

**Doneness**:
Whether the work in flight is finished, supplied by the operator when ending a window. The discriminator between the two endings: without it, a reset, and the thread continues; with it, the thread's record lands in a plain file, never the store, and the thread ends. The signal the session ▸ turn ▸ step nesting cannot produce on its own.

**Readiness**:
Whether a fresh context could resume the work from the state object alone. Distinct from occupancy — a full window can be ready, an empty one can be unready.

## The thread

**Thread**:
Redefined 2026-08-27: the WIP itself, not a continuity unit that outlives it. Doneness still discriminates the ending: not done means a reset, on a token counter, and the WIP continues; done means its record lands in a plain file, never the store. Rejected along with the old shape: a continuity unit above the session carrying a charter, a cut discriminated by doneness with a per-fact triage, and two rails — the artifact chain within the thread, the store's claims across threads.
_Avoid_: spine (session shorthand, not the term)

**Main**:
The deciding loop — the agent the operator converses with, and the only party holding ground truth about the session. A role, filled by a configured body (see Body); what follows describes the arx-native body, and a CLI-bodied main carries its own equivalents. Everything else works on its behalf: hands execute. The only claim-minter, and the only party that can check the state object for truth rather than sufficiency. Its final acts before a reset are authoring the state object and truth-checking it, holding both pens; the mechanical check, the operator surface and the respawn probe are the checks of the other kinds. A post-reset main rejoins the thread by spawning, orienting from the state object plus grounding, and proceeding. Its tool surface is startup configuration — arx has no per-thread loop config and no workflow phases.
_Avoid_: worker (its pre-2026-08-25 name — stored records use it), architect

**Observer**:
Retired 2026-08-27. Was the off-loop party that maintained the state object as a fold over the main's output — spawned per fold, wrote, died; never a standing process. Dropped: the main now writes the state object itself at reset, and the reset apparatus is one participant, not two — no cross-validation pair replaced it. See Main, State object.
_Avoid_: as a live term — see Main, State object.

**Dispatch**:
The one spawn primitive. Task-bounded — spawn, answer, die, trace-captured — and there is no second lifetime class, so add/remove has no domain. Runs under a named agent definition (model, store grounding, tool surface) in a worktree when write-enabled; "hand" is the working name for one. Non-blocking: the spawn call returns at once and the verdict, or the stub, lands at main's next step boundary as an injected message — the same delivery point the operator's messages use — so main runs while its hands run and can tap, steer or kill them. A hand orients from its brief and nothing else: not the state object, not the main's transcript. A hand runs a smaller model, and full thread position is dilution — admission's own logic applied at a spawn. Its container holds an allowlist, empty by default — no host env, no home: the worktree (read-only when not write-enabled), the output dir, what the agent definition names, the body's own auth as the one mandatory entry, and two pushes at spawn, the store's grounding for the task and the brief with its handles. The auth sits off every designated path, so the surface never carries it. Teardown removes the worktree with the container; for a write-enabled hand the diff in its trace is the deliverable, applied from the handle as plain working-tree modifications, never a rebase, never a commit on the hand's behalf (D12, 2026-08-29).

**Steer**:
Main pushing a message into a running hand: the hand's in-flight step is cancelled, the message appends to its window under a one-line harness prefix naming the step it cut, and the hand continues from there. No hold — main always has the text when it reaches for a hand, so the step-boundary case is an interrupt that found nothing in flight. Addressed to one hand, never cascading. Binds arx-native hands only; a foreign body is killed or left alone. The other verb on a hand is kill: teardown, sweep, stub. Triggered by main's own judgment, usually after a tap, or on the operator's say-so.
_Avoid_: stop (the retired parent-side hold), interrupt on its own, SendMessage

**Tap**:
Main reading a running hand's work so far, read-only — the hand's live transcript log through the ordinary read tool, admitted like any result. Main's window holds nothing of a hand until main taps or the verdict lands. What a steer or kill decision is made from.
_Avoid_: monitor, watch, stream, peek

**Stop**:
Narrowed 2026-08-28: only the child-initiated hold survives — an arx-native hand halting itself at a permission wall, the escalation crossing as a stub-shaped event and the parent's answer as the correction, after which it continues or is killed. The parent-side hold at the next step boundary is retired in favour of steer. A clamped foreign body cannot initiate: needing something outside its allowlist is a loud failure returned as its verdict, never a negotiation.
_Avoid_: abort (main-side, the operator's `q`), pause, graceful shutdown, steer (parent-side)

**Capture surface**:
The paths a child is told to leave material in, designated at spawn — worktree, output dir, whatever its body writes there, and always the child's own transcript in whatever format its body keeps it: the transcript log for a native hand, the session `.jsonl` for a CC-bodied one, lifted out of the container before teardown. The designation is the only bound (D12, 2026-08-29): no per-child cap, nothing excluded by kind, nothing undesignated ever swept; a worktree contributes its diff against the spawn base, never a tree copy. The index records byte counts per path, so growth is an instrumentation line, not a rule. At teardown the surface is swept verbatim into the child's trace under a frontmatter index arx writes (producer, paths, shas, exit status); the contents are never parsed or normalized, so no uniform trace shape exists across bodies. One obligation for every child — clean return, crash and cancel alike. The sweep is synchronous on the return path: the verdict carries the trace handle, and a child that dies without a verdict yields a mechanical stub — exit status plus handle — in its place. No child ends in silence.
_Avoid_: transcript capture (implies a format arx owns)

**Spawn ledger**:
An append-only file under the thread dir recording every child arx starts, written before the child runs. The durable half of the one registry: a restarted arx reads it to find orphaned containers and worktrees, kill them, and sweep. Together with the trace directory layout it is where the spawn tree survives — v1 keeps no session DB.

**Probe**:
A fresh context asked whether it can resume from the state object alone — and that context is the respawned main itself, whose first turn after a reset is a mandated orientation report (what it can resume, what it cannot determine), surfaced to the operator. Tests sufficiency, catches gaps, cannot catch falsehoods — the complement to the main's truth-check and the harness's mechanical check. Not a separate dispatch and not periodic: it fires at every respawn, because a checkpoint nobody reads needs no sufficiency check and the one that is read gets it from its reader. On a gap it pulls turns from the transcript log by handle.
_Avoid_: critic, reviewer, validator

**State object**:
The small live artifact describing where the work stands — position (what's in flight, what's decided, what's open, the next concrete move, which files are hot), the target, and whether the target drifted; the charter folds in here rather than staying its own record. Written for someone who was there five minutes ago and lost their notes. Single-writer: the main holds the pen, authoring it fresh from the window at every checkpoint and at reset — the prior object is orientation input at spawn, never the template for the next. Header carries checkpoint ordinal, timestamp and a (thread, turn) handle into the transcript log; entries carry no anchors beyond what the mechanical check needs (path plus sha, decision id). One reader, the main that comes up after the reset — a handoff artifact, not an orientation surface for other agents.
_Avoid_: summary, digest, handoff doc, context file

**Checkpoint**:
A state object authored mid-thread — on a token-delta clock between turns, hands in flight running on, or by operator command — with the full ritual: authoring turn, truth-check turn, mechanical check. The harness then rewinds the checkpoint turns out of the window and leaves a breadcrumb, so a checkpoint costs no window room and the prior object is on disk rather than in context. The recovery artifact for unplanned death — crash, kill, exhaustion before the reserve. Cadence off means reset-only.
_Avoid_: fold, snapshot, save-point, autosave

**Transcript log**:
The per-thread append-only turn log the harness writes as it goes and never rewinds — rewind archives from the live window, the log is untouched. The address space for every handle into a thread, at turn grain. Also where the harness writes its own event lines — admission outcomes, later pulls and echoes, rewinds with tokens cut, checkpoint and reset events with their mechanical-check result, the respawned main's orientation verdict, steer, tap and kill events, stops at a permission wall — so instrumentation is a read over thread dirs, never a fourth file. Third persisted file beside the state object and the spawn ledger; machine-local in v1.
Under a CLI-bodied main, arx writes it from the child's stream at the same turn grain; the body's own session file is never read (D16 round 2).
_Avoid_: session log, history, session DB

**Charter**:
Retired 2026-08-27, folded into the state object. Was the thread's why, minted at birth by the operator and amended on target change as an append-only decision chain. The state object now carries the target and whether it drifted in its place, rather than keeping a standalone record. See State object.
_Avoid_: as a live term — see State object.

**Triage**:
Retired 2026-08-27. Was the per-fact judgment run at a thread's end — demote to the archive, graduate to permanent, or already-global. Its trigger, thread death at a cut with facts demoting to an archive, is rejected along with that whole shape. See Thread, Store.
_Avoid_: as a live term — see Thread, Store.

## Permissions

**Tool surface**:
The list of tools in a model call. Harness-owned configuration, never a per-call judgment: startup config for main, the agent definition for a hand. Built once at spawn as a set of names and never widened afterwards — a hand that needs more is killed and re-dispatched with a wider one. The read/write axis lives here. A body that can strip built-ins exposes only the declared tools; a body that cannot enforces the subset it can control and reports the remaining coverage gap (roster C, 2026-09-10).
_Avoid_: schema (the wire encoding of it), registry (the runtime pool of everything available), mode

**Gate**:
Tool possession is the governing rule: a seat holds the tools it should have without per-call approval, and main runs ungated with every tool. Hands write into detached worktrees; main promotes their diffs. The retained deterministic rule machinery returns allow or deny, with no model call inside the decision. `Ask` and its operator approval UI were removed in roster C; rules or hooks naming `ask` are load errors. Where a body cannot strip its built-ins, arx reports partial enforcement rather than promising a complete clamp (ruled 2026-09-10, superseding D16's approval behavior).
_Avoid_: permission check, inspector, classifier

**Rule**:
One line in the gate's table: a tool name or a pattern within one, and a decision — deny or allow. Order is fixed, not-in-surface then deny then allow, first match wins, nothing overrides. Lives in arx's own config, user tier and repo tier with repo winning, never in the store — a rule has no question, answer or why, so it is not a claim. arx ships none; every rule is the operator's. The table speaks Claude Code's PreToolUse hook contract; a rule or hook naming the retired `ask` verdict fails at load with its file and offending rule named.
_Avoid_: policy, danger list, protected path

**Body**:
What runs a role — an arx-native loop over a provider, or a CLI (Claude Code, Codex) driven as a process. A position's model id selects its body through the model registry. arx hosts under every body: it owns the operator surface and the process tree, and a CLI body is a child it drives, never a host that reaches arx over MCP (ruled 2026-08-29, D16 round 1). A CLI body keeps its own window, so rewind is native-only; the reset apparatus, state object and quarantine bind main under every body, since arx reads the child's stream (D16 round 2).

Adapters are cut by role: a main-seat interface and a hand-seat interface, with a body adapter at each. Shared body knowledge lives in a body module used by both adapters. This supersedes the role-agnostic, one-adapter-per-CLI ruling (2026-09-10); both role seams now have body adapters, including Codex in the main seat. Main's tool surface is arx-owned wherever the body permits — Claude Code strips its built-ins and runs on mounted tools alone; Codex retains some built-ins, and admission covers mounted tools only, logged as a degradation. Main runs without per-call approval under the roster-C ruling.

The reserve arms below the body's compaction threshold, which arx pushes above its own hard limit where the body has a switch or lever, to favor arx's reset (D16 round 3). Codex can still compact inside a turn; its normal digestion/reset follows the same checked ritual as Claude Code, but an unexpected vendor-compaction event discards the vendor session and takes arx's exhaustion fallback from the latest checked state object plus transcript, or the transcript alone. A cross-body `/model` change runs the normal reset ritual immediately, with arx's process and running hands surviving both cases (wave-2 implementation, 2026-09-11). arx's own system prompt replaces the body's built-in instructions, and arx pushes the repo's instruction file itself rather than letting the body discover it (D16 round 4). Main supports Claude Code and Codex; the effective registry binding selects the body.
_Avoid_: provider (the seam under a native body, never a body itself), backend

**Runner**:
The writing half of the execution pair. It takes an agreed brief, implements it, runs the brief's checks and reports PASS or FAIL without committing. A position with a model binding in the roster and a hand at runtime: a dispatch under the reserved agent definition `runner`, with the write tools declared by that definition and no third lifetime class. The registry supplies the bound model's body.

**Reviewer**:
The read-only half of the execution pair. It receives the same agreed brief and the runner's result, checks the work independently and returns PASS or FAIL with `must_fix`. A position with a model binding in the roster and a hand at runtime: a dispatch under the reserved agent definition `reviewer`, read-only clamp, no third lifetime class. The registry supplies the bound model's body. Same family as main is logged as a warning at dispatch, never refused. Codex CLI uses `codex exec --json --output-schema`; Claude Code and native bodies use the same verdict contract.
_Avoid_: judge, critic, sol (a session nickname, not the term)


## Memory

**Digest**:
Retired 2026-08-27. Was Tabularium's narrative record of a session, authored by the outgoing main and validated by the observer against its fold. Dropped along with the observer: the state object is the sole handoff artifact now, and the store keeps facts and decisions only — no narrative layer. See State object, Store.
_Avoid_: as a live term — see State object, Store.

**Handle**:
A filesystem path to material held outside the window, or a (thread, turn) address into a transcript log. The harness mints thread handles mechanically — on every store save from main, on every checkpoint, and in the state object's header — so a claim carries a look-back into the context that minted it. The ordinary read tool pulls it; arx ships no dedicated retrieval verb. Never assertable content in itself, and one that does not resolve (another machine's thread) leaves the claim standing.

**Trace**:
Material archived as evidence rather than knowledge, reachable by handle, never merged back into a window. One class, three producers: an externalized tool result, a child's swept capture surface, a rewound tail. The result cache mints none — it hands back the first run's trace. Lands on disk under the session that produced it, as directories and `.md` files meant to be grepped by hand. Write-once — an artifact is sealed when its handle is issued, so the sha under a handle never moves. Every trace opens with frontmatter (source path, sha at capture or `none: always recheck`, producer, timestamp), so material read out of one carries its provenance without the harness attaching anything at read time. No redaction — a trace's trust class is Claude Code's own session file: on the owner's disk, outside git and the store, entering a window only by a sliced pull. No expiry in v1; a future pruner never removes a trace whose handle a claim or ember holds, and pruning is an operator-invoked ritual, never the harness's (D12, 2026-08-29).

**Store**:
Tabularium v2, holding claims — facts and decisions — and embers, no narrative layer; the digest operation that once supplied one is dropped, and no finished thread ever lands in the store at all — done work's record goes to a plain file instead. arx's own engine, separate from the v1 Claude Code keeps using, and it starts empty. Files as truth: one markdown file per claim with frontmatter, a directory per scope, in a git repository arx owns — git is the sync across machines. Every index (embeddings sidecar, BM25, nearest-neighbour) is derived from the files and rebuildable; no server holds the truth. Raw material is arx's and lives on disk beside it. No harness policy calls it on the per-step path, in either direction; the model reaches it mid-loop through ordinary tools, and grounding pushes from it at session start. A hand gets the same two ways when native — grounding pushed at spawn, recall as an ordinary tool on its surface when the agent definition lists it, served by arx on the host, never a mount — and the push alone when CC-bodied, in v1 (D12, 2026-08-29). Claims are minted only by the main; a hand's findings arrive as verdicts through the main's window first, and a thread's candidates wait in quarantine until the drain. Embers are the one exception on both counts: owner-written, stored at once. Its root is per user and outside any repository, `[store] root` in arx.toml (`~/.local/share/arx/store` on the owner's machine); a project's config names a scope inside it, never a store of its own.
_Avoid_: Qdrant as the store (deferred to a measured trigger, as an index adapter only), LanceDB

**Claim**:
One record in the store, four required fields and nothing else: the question a future session would ask (naming a future condition, not the current task), the answer, the why (for a decision: the rejected alternative, itself a claim — "none" refused), and a handle to the sealed trace of the turns that minted it. No question, nothing to save. Derivability from the repo is the drain's check, not the schema's.

**Quarantine**:
Where a candidate claim waits between the main's save and the store. One file per candidate, carrying its trace handle, under the thread that produced it; survives resets and is read at spawn, so every main in a thread sees what earlier mains staged. Nothing in quarantine is recallable until promoted, so the store lags the work by one drain.
_Avoid_: staging, pending, inbox

**Drain**:
The one pass that empties a thread's quarantine into the store, at doneness — dedup, trimming, supersession edges. Two pens: a dispatched hand of a different model family from the thread's main reads the quarantine with fresh context and the trace handles and returns suggestions — promote, promote superseding an in-force claim, drop, each with a reason — and the thread's main decides per candidate over the proposal, asking the operator when unsure, and applies them. The hand holds no save verb; the judgment is cross-model, the write is main's. Merge-into is a save superseding the target plus a drop of the original. Nothing is destructive: a dropped candidate stays in the drain hand's swept input under the thread, a supersession is lineage.
_Avoid_: fold, triage, cleaner (the fold is retired; a cleaner works the store, a drain works one thread's candidates)

**Landscape**:
What the main sees at save intent: the N nearest in-force claims in the same scope, with their content, returned by the engine before the candidate exists. Not a gate — the store refuses and bounces nothing. Its job is contradiction surfacing, so a new ruling that overturns an in-force one is caught while composing rather than at the drain.
_Avoid_: bounce, dedup gate (the synchronous bounce is rejected for arx)

**Ember**:
A correction or standing guideline the owner writes at the moment of correcting main, stored straight into the store with the (thread, turn) handle of that turn — no quarantine, no drain, in effect from birth. Grounded at session start alongside hot; small in text so it stays one. Two verbs, one owner: only the owner appoints an ember and only the owner removes it — no decay, no recurrence ladder, no codify suggestion. Not a claim; if a rule ever deserves one, that is a separate ask to main. Also the intervention record — embers per thread is the friction count. A correction given to a hand through stop is a stop event, not an ember.
_Avoid_: correction (the act, not the record), intervention mark, feedback

**Engine**:
The judging half of the store — the landscape lookup, lexical recall (BM25 over the claim files, no vector leg, no fusion), lifecycle, the calibrated constants. A Rust crate linked into arx, so reaching it costs no transport. It reads the claim files and owns the derived indexes behind one adapter seam (brute-force cosine in-process today; a server client only if the corpus earns one); the embedder is Ollama on localhost. No dedup gate: the store refuses and bounces nothing — duplicates are the drain's job. Its write path is serialized internally, so callers never coordinate.
_Avoid_: dedup gate, bounce

**Admission**:
Judging what a tool result contributes to the window before it enters. The one policy that runs per tool result, inside the loop. Two outcomes only: inline or externalize. Results crossing the memory seam are exempt and always inline — the exemption is keyed on the seam, not on the content.

**Memory seam**:
Where arx meets the store. Everything crossing it is a claim under the store's own budget, so admission does not apply — grounding at session start, and the model's mid-loop recalls and saves.

**Inline**:
Admitting a tool result whole, because it sits under the threshold.

**Externalize**:
Storing an over-threshold tool result entire and admitting a preview plus a handle. The preview is the head plus mechanical metadata — path, byte and line count. Nothing decides which part matters; pulling the rest is the model's next move, through the ordinary tool cycle. A whole-file read of a sealed trace is an echo — that trace's own handle and preview again, nothing minted — so the model's only way forward is a slice or a grep; a slice over the threshold externalizes ordinarily, with its own sha.
_Avoid_: filter, extract, reduce, summarize

**Pull**:
A read whose source path is under the trace root — the model fetching externalized material back. The unit instrumentation counts against externalizations: a slice admits under the ordinary threshold (inlined, or externalized again with its own sha) and is a pull; a whole-file read is an echo, not a pull; a whole read under the threshold inlines and is a pull too.
_Avoid_: retrieve, fetch, dereference, re-read

**Echo**:
A whole-file pull answered with the handle it asked for — the read's source path is under the trace root and the result's sha is the trace's own. Nothing minted, nothing new in the window; the preview is the same head plus counts with one line stating the handle is unchanged. Counted on its own counter, apart from pulls, so pull-rate stays the threshold's signal and a run of echoes reads as the model not slicing. Only over the threshold — a small trace read whole simply inlines.
_Avoid_: bounce (the store's word), re-externalization, loop

**Result cache**:
A session-scoped map from an exact tool call to the trace it produced. A repeat call whose first run was externalized and whose source sha is unchanged gets the first run's handle back instead of re-running; a sub-threshold repeat re-runs, and anything with no sha — Bash, browser, network — always re-runs. Not a memory judgement. The harness returns an answer it can prove is identical.
_Avoid_: refusal, residue, applicability

**Grounding**:
Store material loaded at session start, above the turn and step loop — the hot tier and every ember in scope.
