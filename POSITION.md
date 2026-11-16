# arx — standing position

**Read this first.** The banner stack below has grown past the point where "read the top one" is self-explanatory. For the current state: `CONTEXT.md` is authoritative on vocabulary, the design map (`docs/wayfinder/maps/arx-design/map.md`) is closed, and the build board (`docs/wayfinder/maps/arx-v0/map.md`) is authoritative on what is currently open. Treat everything below as the history that produced that state, newest first.

> **Roster wave 2, 2026-09-11 — Codex main.** Roster A/B/C are landed. Adapters are now cut by role: main-seat and hand-seat interfaces, with body adapters at each. Tool possession governs access; `Ask` and per-call operator approval are retired, while deterministic allow/deny rules remain. These rulings supersede the older adapter and gate clauses below.
>
> A cross-body `/model` change runs the ordinary checked handoff immediately. Codex uses the same normal digestion/reset ritual as Claude Code, with arx's process, transcript, ledger, front and running hands surviving. Current Codex can compact inside a turn before arx's reset check; on its compaction event arx discards the vendor session and recovers from its last checked state object plus transcript, or transcript alone. This extends the existing exhaustion fallback rather than continuing from the vendor summary. The source evidence is [R04](docs/wayfinder/maps/arx-v0/findings/R04-codex-main-source-refresh.md); implementation follows the [wave-2 brief](docs/specs/2026-09-11-roster-wave-2-codex-main.md).

> **Twenty-first session, 2026-08-30 — the map-integrity debate is ruled: seven records land, S-SOL-6 settles, SURVEY's rewrite begins.** A `/debate` over the closed map (ledger `docs/wayfinder/maps/arx-design/findings/debate/2026-08-30-arx-map-integrity/ledger.md`, seats rune/sol, three rounds) found three in-force store contradictions the grills had left standing plus doc lag; both seats called fix-first. The owner ruled the same session, in scope `arx`.
>
> - **Lineage repair, seven records.** `890ebff2` supersedes `568f864d` — the hands-service and CC-as-loop clauses drop, the configurable-roles clause survives as the D16 seam, `705e4df2` holds. `7dae481c` supersedes `5efb4c94` — rewind still never fires on a pressure clock, but also fires as a mechanical step of the checkpoint and reset rituals, not only by operator command. `975f65cd` supersedes `0be93430` — the no-capability-flags clause stands, the hand-only clause falls: a model behind a CLI enters as a body through the D16 seam, in any role. `af68d429` supersedes `6bbde714` — the memory clause restated around arx's own engine, SURVEY §4–5's nine-item list to be replaced by a module table. `58b34830` supersedes `a194176a` — the use claim's falsifier is now observable: the owner running bare `claude` for the day's work instead of arx driving it. `bb81d2c6` places the two items the close banner left pending: phone dispatch is a named reopen outside design-complete, prompt layout and cache discipline are the V0 brief's first section. `31a20ff6` rules the CLI-main exhaustion floor: kill and respawn from the last checkpoint's state object plus the archived transcript handle on the child's `ContextWindowExceeded` inside the reset ritual.
> - **S-SOL-6 settled, not fixed.** The debate's one contested row — a clamped foreign body's stop contract against the SDK hand's callback — resolves by refutation: `0bd4df2f` already says the drive surface follows from the role, so CONTEXT.md's Stop and Body entries stand as written.
> - **The build board is charted.** Same session, after the owner ruled that a single `to-spec` or gauntlet brief would go sideways: `docs/wayfinder/maps/arx-v0/` — harness-exists destination, twenty-one build cards in five phases plus two research tickets (R01 driving Claude Code from Rust, R02 Codex drive gaps), each card with a running done-check and the rulings it answers to; decision `bbfaeafb`. Research dispatched at charting.
> - **The build board is re-cut.** Same day, the owner read the charted board and rejected its shape — "the work should be horizontal not vertical ... layer by layer"; the subsystem phases meant nothing ran end to end until the last card. Redrawn as three depth layers, 29 cards, each layer closed by a check card that runs one scripted session across every seam; layer 1 closes the loop on the host with every seam at its thinnest adapter (native as one provider, one turn, by owner ruling). Decision `4c1ee2ce` supersedes `bbfaeafb`; the subsystem cut is parked under `docs/wayfinder/maps/arx-v0/superseded-subsystem-cut/`.
> - **SURVEY.md's rewrite is in progress**, same session, in a parallel pass — §4–§5 becoming a donor/lift/build module table per `af68d429`.

> **Twentieth session, 2026-08-29/30 — D16 closes: arx hosts main under every body, and the map's frontier is empty.** One grill across two days, four rounds, fourteen decisions in scope `arx`, sol silent on rounds 1 and 2 and skipped on 3 and 4 (prompt over the 40k cap); vocabulary in `CONTEXT.md` (Body, Judge, Gate, Reset, Transcript log amended). Ticket record in `docs/wayfinder/maps/arx-design/D16-bodies-per-role.md`, frontier note in the map, two Codex source traces in `findings/codex-cli-drive-surfaces.md`.
>
> - **The charter's premise fell in round 1.** The interviewer read the D06 phrase "think/plan with cc cli" as Claude Code hosting with arx an MCP server. Owner: "arx should be the main, not cc. we are making harness for a reason and i want the main, hands and judges to be swapable, may it be cli or provider" (`705e4df2`). arx owns the operator surface and the process tree under every body; a CLI body is a child arx drives over the Agent SDK or Codex's app-server, one adapter per CLI kind, the role deciding what arx mounts in (`e652ce3a`). Bare `claude -p` and `codex exec` have no approval callback and are hand-only.
> - **The reset apparatus binds every body; rewind is native-only.** Usage read from the child's stream, reset as kill-and-respawn, the reserve armed below the body's compaction threshold which arx pushes above its own hard limit — Claude Code by switch, Codex by lever, "the codex should not even get to compact" (`952d3b69`, `e4510547`). Main's transcript log is arx-written from the stream, so a claim's handle is body-independent (`7b607c7e`). The price named by the owner: "what i cant change is the controll on cc and codex... until there is some model from a provider that is usefull and with reasonable price."
> - **Two Fable-placement rulings superseded, two stand.** A CC-bodied Fable may fill main, "never as main's provider" staying literally true (`96ce60c0`); the native-main Fable ruling narrows rather than reopens, its triggers unfired (`c8ef1d51`). The participant set and the no-capability-flags provider trait are untouched.
> - **Judge, config, gate, instructions.** Judge is a role in config and a hand at runtime under a reserved `judge` definition, different family logged never refused, returning an arx-defined schema and not Codex's `review/start` (`1e6ad121`, `da3f5418`); the owner leans toward a third participant, deferred. Config per role is `body` / `model` / `provider`, drive surface following from the role (`0bd4df2f`). The gate's three values hold across bodies (`64c789d0`); main's tool surface is arx-owned where the body permits, Codex the logged degradation (`70260cad`); arx's system prompt replaces the body's and arx pushes CLAUDE.md / AGENTS.md itself (`68c366dc`, `ceb0390f`).
> - **Frontier:** empty. Sixteen charted, sixteen closed. **The map is closed** (`map.md` header, `closed: 2026-08-30`). Owner's call: no `to-spec` yet; a `/debate` over the settled design in a fresh session comes first.

> **Nineteenth session, 2026-08-29 — D12 closes: the designation at spawn is the sweep's only bound, and the secret question moved to the container.** One grill, three rounds, seven decisions in scope `arx`, sol silent on every round; vocabulary in `CONTEXT.md` (Capture surface, Trace, Dispatch, Store amended). Ticket record in `docs/wayfinder/maps/arx-design/D12-capture-sweep-rules.md`, frontier note in the map.
>
> - **Two charter premises fell first.** The surface is what a child leaves, not what it read; admission is a size threshold that never inspected content, so the sweep loses nothing by skipping it.
> - **One rule closed four bullets.** No per-child cap, byte counts in the trace index as the growth gauge, a worktree contributing its diff never a tree copy (`4e10940c`); a child's own transcript always on the surface, the CC session `.jsonl` by rule not accident, nothing excluded by kind (`e2e92711`); no expiry in v1, a future pruner never removing a trace a claim or ember handle holds (`72f65636`); no redaction, a trace's trust class equal to Claude Code's own session file (`cd27a1f6`).
> - **Secrets are the container's question.** Allowlist, empty by default — no host env, no home; worktree, output dir, what the agent definition names, the body's own auth as the one mandatory entry, plus two pushes at spawn: store grounding for the task and the brief with its handles. Owner: "it should get the context of the store + context for the task" (`068d4cbf`). The worktree's gitignored `.env` is the accepted residual.
> - **Two rulings no bullet asked for.** The trace's diff is the deliverable — worktree dies with the container, main or a dispatched hand applies the diff from the handle as working-tree modifications, never a rebase, never a hand commit (`4f90f38f`). A native hand reaches the store by push plus `recall` on its surface, served by arx on the host, never a mount; a CC-bodied hand by push alone in v1 (`e79bed85`) — the MCP surface that would change that is D16's.
> - **Also this session:** the cite-substance rule left the store for global CLAUDE.md (`skillbook/GLOBAL-CORE.md`), the ember buried; an ember has no demotion edge but bury, a tabularium note.
> - **Frontier:** D16.

> **Eighteenth session, 2026-08-29 — D06 closes: the gate is deny-only by posture, ask by option, and the last question opened D16.** One grill, two rounds, nine decisions in scope `arx`, sol silent on both rounds; vocabulary in `CONTEXT.md` (Tool surface, Gate, Rule, Body, Judge added; Assembler corrected; Main amended). Ticket record in `docs/wayfinder/maps/arx-design/D06-permission-model.md`, frontier in the map's note of the same date. The D14 close earlier the same day has its record in the map note and the D14 ticket, no banner of its own.
>
> - **The owner's own Claude Code posture was the ground.** Eleven bare-name schema denies, dippy as a deterministic allow-or-deny gate on every Bash call, the interactive prompt off. Main gets schema-level plus one pre-dispatch gate returning allow / deny / ask, the ask branch built but shipping with no rules — owner: "its not needed but option is good" (`ce46b9ea`); no sandbox on main (`cb4b1d07`). Hands: a container for every hand regardless of the write axis, worktree only when write-enabled, a static schema-level surface per agent definition that nothing widens (`ebe94e38`, supersedes `f4e08417`). The tool surface is harness-owned configuration, no type-level enum — "no need to overcomplicate here" (`86875715`).
> - **One approval shape, two receivers.** The gate's ask suspends the step for the party above — the operator for main, main for a hand. A native hand's gate runs in arx on the host; a Claude-Code-bodied hand is clamped at spawn and its own gate runs inside, `CanUseTool` becoming the stop-shaped request under the SDK, a shortfall failing loud under `claude -p` (`96d1ad3a`). The D05 edge dissolves: a stop's reply is the gate's return, never a steer. Rules live in arx's config, user and repo tiers, never the store (`91d75ffe`); precedence is not-in-surface → deny → ask → allow, first match, no override, no built-ins, no model call, and the gate speaks Claude Code's PreToolUse hook contract so dippy and bashfilter-chain plug in unchanged (`799b0967`).
> - **The shape moved.** Owner: "the shape will be quite diff. i will want to think/plan with cc cli and have arx as hands and cross model 'judge' to be codex cli" — and each role configurable by model and/or CLI (`568f864d`). It lands against `7d1f7da4`, `41bf6e4f`, `0be93430` and `ca15f9eb`; by the map's own rule it is chartered as **D16** rather than absorbed. Every D06 ruling binds arx's hand-runner under either shape.
> - **Frontier:** D12, D16. *(D12 closed later the same day.)*

> **Seventeenth session, 2026-08-28 — D05 closes: the friction was main→hand access, and main keeps Claude Code's flow.** One grill, four rounds, six decisions in scope `arx`, sol silent on every round; vocabulary in `CONTEXT.md` (Turn, Step, Abort, Steer, Tap added; Stop narrowed; Dispatch, Checkpoint, Reset, Capture surface, Transcript log amended). Ticket record in `docs/wayfinder/maps/arx-design/D05-steering-semantics.md`, frontier in the map's note of the same date.
>
> - **The charter was wrong about where the friction sat.** Owner: "not much friction in cc, i have adapted" and "on main i would like to keep what cc is doing, i do like the main flow of it." Main adopts Claude Code's shipped input handling whole (`ba965dd0`) — step-boundary delivery, Esc as abort preserving the queue, no end-of-turn queue mode, no per-message priority; the two-priority split, Goose's three states and HAPI's queue-default are rejected for main. The friction is main's access to its dispatched hands.
> - **Three verbs on an arx-native hand.** Steer (`001823de`, superseding `426670ee`): interrupt the in-flight step, append the message under a harness prefix naming the cut step, continue — no parent-side hold, since main always has the text when it reaches for a hand; addressed, not cascading. Tap (`7213cb42`, owner ruling over the recommendation): a read-only look at the hand's live transcript log through the ordinary read tool. Kill unchanged. A foreign body has kill only; `426670ee`'s hold-and-continue mechanic closes as not built.
> - **Dispatch is non-blocking** (`23642b44`): the verdict or stub lands at main's next step boundary as an injected message, `6aab1d24`'s sweep-before-verdict invariant untouched. Consequence for D15: `2c198c9f`'s "never mid-dispatch" clause is dropped (`adec3734`) — hands run on across checkpoints and resets, the state object lists them as in flight. That closes the steer-meets-reset bullet with nothing to carry. A steer is content plus events, not a trace or a claim (`04870014`); D09 gains steer/tap/kill counts.
> - **Frontier:** D06, D12, D14. D06 inherits one edge — the child→parent approval path for a CC-bodied hand has no steer to answer with.

> **Sixteenth session, 2026-08-28 — D11 closes: the handle-read recursion is an echo, not a regress.** One grill, three rounds, three decisions in scope `arx`, sol silent on every round; vocabulary in `CONTEXT.md` (Pull, Echo added; Trace, Result cache, Externalize, Transcript log amended). Ticket record in `docs/wayfinder/maps/arx-design/D11-handle-read-admission-gap.md`, frontier in the map's note of the same date.
>
> - **`0bec7b13` was considered, not blind.** Its own clause — pulling a large body back "returns a preview and a handle to the file just asked for, so the model slices or greps" — names the recursion and its exit. The plans debate's blocker (`S-SOL-1`) was the one thing the ruling left unsaid: whether the pull writes a new artifact.
> - **It does not — a whole-body pull of a sealed trace is an echo** (`21f24656`): source path under the trace root, result sha equal to the sealed sha, so externalize hands back the same handle and preview and writes nothing. A prefix test plus one sha compare, no tracked set. A slice over the threshold externalizes ordinarily with its own sha. The exemption and the `Read` override were both rejected again, on the same ground as before.
> - **The result cache is downstream and holds externalized results only** (`2248d3ac`). It keys on the exact call, so it cannot catch the first pull; a sub-threshold repeat re-runs; "a cached prior result" was never a trace producer — three producers, not four. `S-RUNE-10` closes. The `(thread, turn)` answer to a sub-threshold repeat is deferred on D09's sub-1k occupancy count.
> - **The echo is its own counter, owner ruling over the recommendation** (`8098d8ee`). Pull-rate counts only reads that admitted material; echoes are logged apart, so a model that keeps whole-pulling shows up as a prompting problem and never moves the override table. The echo's preview carries one mechanical line — handle unchanged — and nothing that recommends. D09's "counts twice" residue is gone.

> **Fifteenth session, 2026-08-28 — D09 closes: not an eval apparatus, instrumentation.** One grill, three rounds, nine decisions in scope `arx`, one fact superseded; vocabulary in `CONTEXT.md` (Ember, Nudge added; Store, Grounding, Transcript log amended). Ticket record in `docs/wayfinder/maps/arx-design/D09-eval-apparatus.md`, frontier in the map's note of the same date.
>
> - **The apparatus certifies nothing and tunes four constants** (`01647e3a`): admission default and override table, checkpoint delta, reset reserve, occupancy nudge. No scorer, no judge, no labels — the owner reads counts. No offline replay over the CC corpus (`e50e2fcf`); `8c427f93`'s "evals are the best-done part" is superseded by `22899fd8`, and §5 below carries the narrowing.
> - **The ~120k soft limit survives as a nudge, owner ruling over the recommendation** (`f11a9548`): said once on the operator surface and once in the window when main crosses it, nothing follows. Main only — a hand should die well short of it, and one that crosses is a too-big brief, logged not killed. Repeated crossings want some future shortening of scopes; fog item on the map.
> - **Instrumentation is event lines in the transcript log** (`340b3f2e`), grepped by hand, no fourth file. The owner moves every knob; the harness surfaces per-tool pull-rate (`d5e60f47`). The checkpoint delta's default is set at build (`25664aa5`).
> - **Intervention capture is the ember** (`8915c49b`, `c5c0dce8`, `2acce830`): owner-written at the moment of correction, straight to the store with a `(thread, turn)` handle, grounded alongside hot, small. Two verbs, one owner — appoint, remove — no decay, no recurrence ladder, no codify suggestion; not a claim. A stop correction to a hand is a stop event, not an ember. §5's intervention problem closes on this: the explicit mark is the ember save, embers per thread is the friction count.

> **Fourteenth session, 2026-08-28 — D15 closes: the reset seam whole, and the state object gets its checks.** One grill, three rounds, eight decisions in scope `arx`; vocabulary in `CONTEXT.md` (Reset, Probe, State object, Handle rewritten; Checkpoint, Transcript log added). Ticket record in `docs/wayfinder/maps/arx-design/D15-state-object-check.md`, frontier in the map's note of the same date.
>
> - **A reset fires against a reserve, not the limit** (`d65f8fe1`). "Who fires" and "can an exhausted window afford the ritual" (`S-RUNE-9`) were one knob: the harness arms at limit-minus-reserve and tells the operator, the operator fires, the harness fires alone only at the reserve's floor. A window already past the reserve rewinds to fit first, mechanically, then runs the ritual (`e2b0ceb2`). The §6 "who fires a reset" fog item closes on this — the harness-never-initiates line is narrowed, not reversed.
> - **A `ContextWindowExceeded` on the wire is a reserve miss, and the harness fires on it** (`3a5aefb6`, D14). It is an error, never a termination — the termination enum is four values (`48af6ac7`) — and its continuation is rewind-to-fit then the ritual, harness-fired, with a D09 miss-size event; the same rule holds one level down when the ritual's own authoring turn is rejected, rewinding by the vendor-reported overshoot until the floor of authoring from the archive handle alone (`8b9f41ae`). The window limit is per-model config from vendor pages (`aca8f7ef`); how the gauge reads occupancy is deferred to build, settled by using and checking.
> - **The missing check is three checks of different kinds, not a second pen.** The ritual is authoring turn, separate truth-check turn, mechanical check on every write, operator surface before teardown (`18fec68a`); the probe is the respawned main's own first turn, a mandated orientation report, fired at every respawn — periodic is dead (`253334ad`). What none of them catch — a complete, consistent, wrong judgment claim — is accepted as the residual and becomes a D09 signal (`b03f77fc`). The second model pen was rejected on `S-RUNE-3`'s own ground.
> - **Checkpoints, owner ruling over the recommendation** (`2c198c9f`, superseding `4833a3a3` on lifecycle). The main authors the state object on a token-delta clock between turns, full ritual each time, and the harness rewinds the checkpoint turns out of the window afterwards so the object does not spend the budget it protects. Reset-only had no recovery for exhaustion, the common death — the transcript that killed a window cannot fit the fresh one. This is a named override of §6 question 4's manual-first readiness test; cadence-off is the manual path.
> - **Look-back handles, owner addition** (`eff1f01a`, superseding `ba6fd355`). The harness mints `(thread, turn)` handles mechanically on every store save, checkpoint and state object header — D13's claim handle field now points into the context that minted the claim. This needs a per-thread append-only transcript log independent of rewind, the third persisted file beside the state object and the spawn ledger. Machine-local in v1. Provenance on the state object is header-level and the object is authored fresh from the window each time (`18b4928b`).
>
> **Thirteenth session, 2026-08-27/28 — the D13 grill runs across two days; seven rulings restate the store around files rather than Qdrant, and the ticket closes on a measurement.** A `/debate` run over the owner's two positions (a vector-then-BM25 funnel; the bounce kept as the landscape) produced a ledger (`findings/debate/2026-08-27-arx-d13-recall-gate/ledger.md`, seats `rune`/`sol`, 3 rounds, 14 agreed rows, 2 contested, 1 withdrawn — both seats call rethink). `c775b5c2` (superseding `dbbe749c`) keeps arx's own engine but drops Qdrant and the snapshot-based three-machine deployment; `095a930e` (superseding `d1ff8175`) makes the store files as truth and index as adapter — one markdown claim per file, git as the sync, brute-force cosine in-process, Qdrant deferred to a measured trigger as an index adapter only; `3ed57643` re-grounds the empty start on the v1 audit's junk rate rather than migration pain. `da0b0d8b` (superseding `3f7d9b5c`) splits save judgment by job — an inline landscape surfacing contradiction on save intent, duplicate judgment waiting in a per-thread quarantine drained once at doneness; `27e7cea9` (superseding `dc06e97b`, a same-session correction — owner: "the suggestions should come from diff -> main writes") makes that drain two pens, a cross-model hand suggesting and main writing; `7be68c4a` fixes a claim to four fields, question/answer/why/handle, with derivability checked at the drain rather than the schema; `6523f41f` ships v1 with no cleaner, only a constraint on a future one. Vocabulary in `CONTEXT.md`: Claim, Quarantine, Drain, Landscape. The last bullet — the recall shape — neither debate seat would rule unmeasured, so **T08** measured it against a read-only copy of the v1 store the same day: text-only recall@1 0.97, vector 0.95, v1's RRF hybrid 0.85 (below both legs), and the funnel's clutter premise absent in 29 of 30 real queries. `896b62ad`: recall is lexical only, BM25 over the claim files, the embedder kept for the landscape's cosine leg alone. **D13 is closed.** Frontier: D05, D06, D09, D11, D12, D14, D15.

> **Twelfth session, 2026-08-27 — D10 closes: no claim against Claude Code, no evaluation, the falsifier is going back.** One grill, two rounds, one ruling (`a194176a`, superseding the round-1 draft `4b756d4b`). arx makes no claim against Claude Code and runs no evaluation against it; the falsifiable statement is a use claim — arx becomes the owner's daily driver and the owner does not go back. The reason it is arx is control over a loop that changes with the owner's workflow instead of being worked around; the model landscape moves faster than the build, so a comparison declared now is stale before it could run. The debates that opened D10 could not dereference `3117ac26` (2026-08-19), which had already rejected differentiation-as-goal — it stands, parity now meaning the owner's felt verdict in daily use. No destination redraw (`7e74d751` stands), no same-model eval, no control arm on Claude Code (SURVEY §8's depleting-baseline bullet is moot). Also corrected: SURVEY §8's "no instance in the field" for admission — Claude Code externalizes over-threshold tool output with a preview plus path (`e65b2a35` already had it). **D09 is released** and certifies against nothing external. Frontier: D05, D06, D09, D11, D12, D13, D14, D15.

> **Eleventh session, 2026-08-27 — the next-steps debate, then two owner rulings retire the observer, the digest, and the thread as a continuity unit.** Session `a57cb42b`, 2026-08-26 20:53 UTC → 2026-08-27 08:51 UTC: the third `/debate` run (`findings/debate/2026-08-27-arx-next-steps`), then the two rulings, in one continuous session.
>
> - **The two rulings.** `749c8765` drops the observer: the main writes the state object itself at reset, one participant rather than two, no cross-validation pair — and the ruling's own text names the resulting gap as live rather than closed (see map ticket D15). `da0ccf24` redefines the thread as the WIP itself and drops the digest: not done means reset on a token counter, done means the thread's record lands in a plain file, never the store; the charter folds into the state object rather than staying its own record; the store keeps facts and decisions only. Between them the two rulings retire five things this file and `CONTEXT.md` used to carry as live: the observer, the digest, the charter as a standalone record, triage, and the thread as a continuity unit living on two rails above the session. `da0ccf24` was recorded with `supersedes: null` — see the store-lineage bullet below.
> - **The next-steps run, 18 rows, all agreed.** The first of the three `/debate` runs with nothing contested and nothing withdrawn — asked what to do about the tenth session's two runs, sharpened by two research artifacts that landed the same window (`docs/research/memory-2026.md`, `docs/research/tabularium-in-practice.md`). Four rows landed at blocker severity, including that dropping the observer and the digest together removes both counterparties from the reset check, leaving main to author and truth-check its own sole continuity artifact with no replacement written anywhere. The skill that produced all three `/debate` runs is read-only by contract (`~/.claude/skills/debate/SKILL.md:46`), so none of this reached the repo's own record until the reconciliation pass that closed out this session — see the map's provenance note.
> - **D10-D15 now exist.** Five tickets opened directly from the debate ledgers: D10 (the capability claim), D11 (handle-read re-externalization), D12 (the capture sweep's rules), D13 (the store's shape after the 2026 research), D14 (loose seam ends). D15 (the state object's missing check) opened this pass, from `749c8765`'s own live-consequence clause, folding in the probe's now-subjectless trigger question and the state object's now-subjectless per-entry provenance question. `CONTEXT.md` is reconciled to both rulings; D05, D06 and D09 were revised against the new participant set and D09's new blocking edge. The frontier is D05, D06, D10, D11, D12, D13, D14, D15, with D09 blocked on D10 — see the map's frontier note.
> - **Store-lineage repair, 2026-08-27.** The gap below is closed, the same day it opened. Owner ruling `ef3b93f2`: a stale `in_force` decision is retired by writing a fresh decision per record that supersedes it — never by wiring `da0ccf24`'s own `supersedes` field after the fact. Thirteen fresh decisions were written, each superseding one of the contradicted records: `6cdbb779`→`a8cca0e0` (thread is the WIP, no continuity unit above the session), `2260999e`→`1b640192` (charter folds into the state object), `ea59cfb2`→`8909fdca` (done lands in a plain file, never the store), `bec6bebb`→`3f7d9b5c` (no triage or fold at thread end), `1b32d02e`→`7d1f7da4` (participants are main plus dispatched hands, no observer), `e65225be`→`1e27032c` (reset is main authoring and truth-checking the state object, teardown, respawn — the doneness-ending's name stays open), `9d3b57dd`→`1a92f383` (state object single-writer: main), `f478d4bc`→`1154bf53` (fold gone, its mitigations have no event, per-entry provenance left to D15), `b27d9b6c`→`4833a3a3` (contents clause restated unchanged; lifecycle now authored by the outgoing main at reset, read by the respawned main), `23742d71`→`bd73693e` (D02's identity-assembler ruling restated with seat vocabulary dropped), `c6684d2f`→`ba6fd355` (D04's files-only persistence restated: state object plus spawn ledger, no charter file), `993ef454`→`1686f374` (fix thread stays out of v1, not revivable as designed), `94441f97`→`b318ea86` (observer exhaust dropped rather than parked). One record stays unrepaired: `eeedd9fe` (Rust shell / Python memory layer) lives in scope `tabularium`, cross-scope supersession is refused by the engine, and arx sessions never receive scope `tabularium` in grounding — closing it needs a session opened in the Tabularium repo (see the decision index below). `30e83788` needed no repair, its content still holds. `989ac3bc`→`f5b5caf4` was already wired correctly before this pass.

> **Tenth session, 2026-08-26 — a corpus audit, then a premises attack, over the trimmed design.** Session `d2661a89`, 2026-08-26 07:58 UTC → 20:53 UTC: two `/debate` runs, back to back.
>
> - **34 rows, two attacks.** The plans run (`findings/debate/2026-08-26-arx-plans`, 18 rows) audited the design for internal consistency; the premises run (`findings/debate/2026-08-26-arx-premises`, 16 rows), with the plans run's agreed findings ruled off-limits, attacked whether the design should exist at all.
> - **Load-bearing finding: no falsifiable capability claim.** Both premises seats agreed, from opposite directions, that no record anywhere states as a falsifiable claim what arx does that Claude Code structurally cannot — the "singular bet" framing was halved at some point and referred to D03, and D03 closed on admission mechanics without ever returning to answer the question it was handed. Neither run reached this repo's own record at the time — both sat in `~/.claude/discuss/` until the eleventh session's reconciliation pass copied the ledgers in and opened the tickets their agreed rows justified.

> **Ninth session, 2026-08-26 — the trim. The bench collapses to two participants.** The owner's own overcomplication check ran over everything the fusion apparatus produced and cut most of it. Twenty-one decisions in scope `arx`, twenty of them superseding; vocabulary in `CONTEXT.md` (seat, teamlead, flex, mode all deleted; observer, dispatch, probe, state object rewritten). **The two blocks below this one are largely superseded — read this first, and treat any seat, teamlead, flex, mode or fix-thread machinery underneath as history.**
>
> - **Two participants, main and the observer.** Seats as a lifetime class are dropped and the whole bench with them (`1b32d02e`, superseded 2026-08-27 by `7d1f7da4`); one spawn primitive, one lifetime class (`e5ae9e17`); no teamlead, main dispatches directly and its spawn width is never zero (`4d250a07`); no channel (`5999f7f2`); integration is main's own move (`d70f022c`). The reason, in the owner's words: a seat copies main's shape, so the design was most of a harness per participant, and none of it is needed to do day-to-day work.
> - **Mode is dropped** (`696b882c`), taking the mode set (`0a37cb02`), the automated switch (`27bc963e`) and its park (`aae8db20`) with it. Two of a mode's three bundle items died with seats; what was left was one config value.
> - **The observer is a per-fold invocation, not a standing process** (`2b8cae45`) — spawned with the artifact's path plus the delta, writes, dies; a small model behind a CLI. Folds fire on the main minting a fact or decision, which keeps the delta small and syncs both rails at one event; hand spawns and resets are backstops. Folds queue rather than race.
> - **A hand orients from a precise brief and nothing else** (`69946e37`) — not the state object, which now has exactly one reader, the post-reset main. Full position handed to a smaller model is dilution.
> - **The fix thread is out of v1** (`993ef454`, superseded 2026-08-27 by `1686f374` — no longer revivable as designed), with observer exhaust (`94441f97`, superseded 2026-08-27 by `b318ea86` — dropped, not parked). Parked on depth, not autonomy — the owner was explicit about that. The design is kept intact for revival, and the cost of parking the recording half is priced in the record.
> - **No workflow phases, and v1 is a durable day-to-day harness first** (`7324f934`). The recon → plan → verify → to-spec → dispatch → review loop lives in main's own procedure over the dispatch primitive; bounded-attempt counters defer with the phases.
> - **Two things got worse and are recorded as such.** Delegate discipline loses its structural enforcement and goes back to the prompt (`f5b5caf4`) — the alternative `989ac3bc` explicitly rejected, accepted here as a known cost. And two of the four mitigations bounding fold drift are gone (`f478d4bc`, superseded 2026-08-27 by `1154bf53`), which strengthens the case for a periodic probe.
> - **What survived untouched:** the thread and its two rails (`6cdbb779`, superseded 2026-08-27 by `a8cca0e0` — the thread is now the WIP itself, not a unit above the session), the state object's contents and lifecycle, cut/reset and the truth-check, the charter, triage, dispatch with its capture surface and spawn ledger, stop/kill, the window ops, admission, the store, the provider seam.
>
> **One thing is open and was deliberately not ruled:** what triggers a probe. It used to ride free on seat spawns. See `e5bc73d5`.

> **Eighth session, 2026-08-25 — the fix thread got a shape, and observer exhaust rides the store rail.** *(Superseded by the ninth session: the fix thread is parked and observer exhaust with it.)* A second pass over disler/fusion-harness (code-verified: `docs/wayfinder/maps/arx-design/findings/fusion-harness-commands.md`), this time at its command surface rather than its architecture, closed the source — owner: "that's it from fusion." Two decisions in scope `arx`; vocabulary amended in `CONTEXT.md` (charter, observer, expiry).
>
> - **Fusion's command surface is fully mined.** Five `/fh-*` workflow commands read to code; `.claude/commands` is onboarding ceremony, not product. Nothing further is owed to this source.
> - **The fix thread's gate is a charter, not a claim** (`5b9570c6`): the observer accumulates inconsistencies hook-like across turns and, on threshold, births a fix thread by minting an executable gate — the gate is that thread's charter, and green is its expiry. Red-first baseline is a hard precondition: the gate must fail against current state before the thread is cut, or the observer misread and no thread spawns. Triage's refute-the-charter repair is kept, tool-gated to once per thread.
> - **Observer exhaust rides the store rail** (`7c849074`): unfired inconsistency tallies flush to the store at cut as flagged digest material instead of dying with the session, so the firing threshold accumulates across sessions — repeated digests carrying the same flagged near-miss are themselves the firing pattern, readable by an observer at the next thread's birth.
> - **Collaborate is parked for its own grill.** It is a workflow (plans → architect DAG → readiness execution), not a mechanism; its liftable fragment is strict validation with bounded repair attempts on anything minted before execution.
>
> Ticket impact is recorded in the map's frontier note (2026-08-25 update); D09 gains a second auto-labelling source and the observer-exhaust kind as open instrumentation work.

> **Seventh session, 2026-08-25 — the thread, seats, and the two-rail spine.** *(Half superseded by the ninth session: the thread and the two rails stand, everything about seats does not.)* A rubber-duck session over disler/fusion-harness (code-verified: `docs/wayfinder/maps/arx-design/findings/fusion-harness-trace.md`) ruled a layer no ticket owned: what stands above the session. Twelve decisions in scope `arx`; vocabulary in `CONTEXT.md` (thread, seat, dispatch, charter, expiry, triage). Where anything below speaks of the session as the top continuity unit, this block wins.
>
> - **The thread** (`751e6f5d`) is the continuity unit above the session, living on two rails — the artifact chain (intra-thread) and the store (inter-thread) — never in a window. A cut is the seat-cycle applied to the worker; the outgoing worker's truth-check of the state object is its non-skippable final act.
> - **Seats** (`e0564add`, promoted from the parked critic-seats roadmap `9def5cea`): operator-composed loops on the main thread; gauntlet philosophy moves into the core. Seat vs dispatch is a lifetime split over one spawn primitive (`14aed109`). The observer is thread anatomy, unremovable, supervised (`5686b742`).
> - **Shared context** is the state object plus the store, opposite concurrency disciplines (`9d3b57dd` — the engine serializes its admit path internally; the state-object half superseded 2026-08-27 by `1a92f383`, main is now the sole writer); seats orient from four inputs and never read the worker's transcript (`e7affb52`); durable claims are minted only by the worker (`3a873e9a`).
> - **Thread lifecycle**: charter minted at birth as an append-only chain (`2260999e`, superseded 2026-08-27 by `1b640192` — the charter folds into the state object, no separate record); expiry demotes facts to the dead thread's archive while the store keeps the why-layer (`ea59cfb2`, superseded 2026-08-27 by `8909fdca` — done goes to a plain file, never the store); triage is a cleaner-shaped offline fold with a small closed rubric (`bec6bebb`, superseded 2026-08-27 by `3f7d9b5c` — no triage or fold at thread end).
> - **Accepted residual**: the fold's unwatched window, bounded by four mitigations, no verifier seat (`77420619`). **Ruled in passing for D06**: isolation coupled to the write axis (`f4e08417`).
>
> Ticket impact is recorded in the map's frontier note (2026-08-25 update); D04 shrank, D02/D06/D09 gained sharpened input.

> **Sixth session, 2026-08-24 — the ledger stopped being only a ledger.** Five things were ruled and written to the store, and they overturn parts of what is below. Read this block and the store before trusting any section beneath it; where a section conflicts with this block, the store wins and the section is history.
>
> - **Architecture ratified** (`6bbde714`): Goose base, a Hermes-shaped memory trait, Tabularium behind it, in Rust. The sourcing map in `SURVEY.md` is now the working architecture, not a defeasible given. Planning has a live artifact — the wayfinder map at `docs/wayfinder/maps/arx-design/`, whose nine grill tickets refine the seams rather than reopen the sourcing.
> - **The residue subsystem is stripped** (`a8895a1e`, superseding `79da6421`). Rewind is cut-to-archive plus a mechanical, harness-authored breadcrumb — no model call at the cut. The write→cut→append order (§6 q2) is retired, and the fact that recorded it (`f88c7521`) is invalid. Distillation happens at offline folds or stays a hand ritual; the automated in-context digest is rejected.
> - **Refusal is a bare handle** (`ee932ea8`, superseding `80bffad0`) — a pointer to the archived prior attempt, no residue registry, no applicability embedding. §6 q5 describes the dead design.
> - **The in-process contingency** (`30e83788`) is a Rust copy of Tabularium inside arx's root, never a Python shell; the Python service keeps serving Claude Code and the Pi hosts. The flip-to-Python limit in `eeedd9fe` is retired, and §2's "third branch" is superseded.
> - **Design mode** (`54018869`): feel-first — positions come from daily-use taste, grounded by the T-series findings under `docs/wayfinder/maps/arx-design/findings/`, then tuned or dropped by evals. Contradictions with earlier rulings are expected and are dive input, not defects to clear first.
>
> Tabularium needs nothing new for the current design (owner, this session, not stored as a decision): grounding push, save of facts/decisions/digests, and lookup on demand cover it. Admission stays harness-local and does not add store surface.
>
> **Stale 2026-08-27 (`da0ccf24`), unflagged until this pass.** "Save of ... digests" no longer describes what arx does. The digest operation is dropped along with the observer; the store keeps facts and decisions only, with no narrative layer, and no finished thread ever lands in it — done work's record goes to a plain file instead. Grounding push and lookup on demand still hold.

> **What this file is — ruled 2026-08-19 (fourth session).** A **context ledger**. It is not a plan, not a spec, and not a design converging on an architecture. arx is not being planned yet; these sessions accumulate the context needed to plan it well *later*, because the driver is learning how to build a harness rather than arriving at an architecture quickly.
>
> **Nothing here is set in stone** — including the entries marked *Stored*. Those carry decision ids because a record id is the store's only durable shape, not because they bind anything. Read every ruling below as *the best-supported position so far*, and expect to overturn some.
>
> Two drifts this corrects, both visible in the sections beneath:
> 1. the document had begun to **read** like a plan — numbered questions marked ANSWERED, typed invariants, a settled op-set;
> 2. and its positions had begun to **act** settled, which is how memory-layer rulings came to make loop commitments nobody priced as loop commitments. See §8.

Assembled from Tabularium scopes `arx` / `tabularium` on 2026-08-19, **plus owner rulings made in conversation that day which the store does not yet carry.** Revised later the same day after the second design session, again after the third, and again after the fourth — which closed question 3 and opened two more. **Nothing from the fourth session is in the store.**

Three kinds of claim live here and they are not equally reliable:

- **Stored** — carries a record id. The store is source of truth; if the record is superseded, this file is wrong and the store is right.
- **Ruled 2026-08-19** — an owner ruling made in conversation that overrides the stored record cited beside it. These are **now written to the store** and carry their own ids. One exception remains open, recorded in §7.
- **Proposed / raised 2026-08-19 (fourth session)** — argued in session and left standing, but **not ruled by the owner and not written to the store**. Weakest class in this file. Anything marked so is the state of an argument, not a position.

That ordering is **how well attested a claim is, not how binding it is.** All three classes are revisable; the distinction only says how much work overturning one has to account for. A *Stored* record still being in force means nobody has revisited it, never that it has been ratified.

```bash
# the framing decisions
mcp__tabularium-mcp__decision c8f20ac3-f727-45e4-866c-eb7296b1ae92   # full harness is the deliverable  (IN FORCE)
mcp__tabularium-mcp__decision 9e5c6e47-ea72-4789-a2ba-c4f92b3e35a0   # Tabularium is the product     (SUPERSEDED by the above)
mcp__tabularium-mcp__decision eeedd9fe-15eb-45ee-b124-90f8af6ca925   # Rust shell / Python policy    (IN FORCE in scope tabularium only — text superseded in substance by 30e83788 + dbbe749c; formal supersession pending a Tabularium-repo session)
mcp__tabularium-mcp__decision dc433ba5-1b83-42dc-ab99-6fd4ef2487b2   # store stays out-of-process    (IN FORCE)

# the architecture decisions (2026-08-19, second session)
mcp__tabularium-mcp__decision 2d15b921-d9ed-4292-bc86-e1c4647e6bf8   # harness owns the merge
mcp__tabularium-mcp__decision 9a764559-e12f-438c-9bb2-6e87d8fa8564   # window ops: append + rewind, no splice
mcp__tabularium-mcp__decision 061d7942-b798-45c7-8e62-ae76eb280656   # traces are evidence, not knowledge
mcp__tabularium-mcp__decision 3e2d9a86-fee9-474e-8900-426abd5ece2c   # admission ladder: inline / filter / delegate

# the admission + store decisions (2026-08-19, third session)
mcp__tabularium-mcp__decision 0aa87641-0989-41ae-925e-b4f6dca497fe   # memory defers, never overrides   (IN FORCE)
mcp__tabularium-mcp__decision 80bffad0-377a-4f06-869a-f4979329a575   # refusal = deferral + residue      (SUPERSEDED by ee932ea8)
mcp__tabularium-mcp__decision 79da6421-b864-455c-b71b-fdcc1de675b7   # residues are shared, key holds set (SUPERSEDED by a8895a1e)
mcp__tabularium-mcp__decision df14a2bc-10a4-48da-9571-5d81824e5ce8   # dedup adjudication folds at a seam (IN FORCE)

# the sixth-session rulings (2026-08-24) — the map's charter and the strip
mcp__tabularium-mcp__decision 6bbde714-e3cf-47e9-8027-c515a6c4dc7b   # architecture ratified: Goose + Hermes trait + Tabularium, Rust
mcp__tabularium-mcp__decision a8895a1e-a3b8-42cf-b61c-21668388ad53   # residue registry stripped; rewind = cut-to-archive
mcp__tabularium-mcp__decision ee932ea8-88a9-48ab-b386-677bb96d0d09   # refusal = bare handle to the archived attempt
mcp__tabularium-mcp__decision 30e83788-1627-47c2-a48e-2f75aafee301   # in-process contingency = Rust copy of Tabularium in arx root
mcp__tabularium-mcp__decision 54018869-3af3-4f4e-ae0f-5fbebd3a154c   # design mode: feel-first, evals correct
mcp__tabularium-mcp__decision 7e74d751-63d1-4da4-8403-02d0de7d7213   # wayfinder destination: design-complete
mcp__tabularium-mcp__decision f9f0a0f5-a260-4eaf-a434-6fe25760fae5   # grills fire only grounded

# the sessions it came out of
mcp__tabularium-mcp__sessions --scope tabularium --query "rubber duck"
mcp__tabularium-mcp__sessions --scope arx
```

Working-mode atom: `a63e96c8` (scope `tabularium`, supersedes `e967bbed`) — "rubber duck v2".
Delivered to sessions here by its arx-scoped twin `6e9c89f7`, **hot** for `decider` + `navigator`. Demote it when the design phase ends.

---

## 1. Posture — what this project is for

**Ruled 2026-08-19**, stored as decision `c8f20ac3` (scope `tabularium`, superseding `9e5c6e47`). arx is a **full agent harness**, with Tabularium as its core. Two drivers, both stated by the owner: **learning**, and a **daily driver**. The exploration-only posture is retired.

That overrides atom `e967bbed` (`fact`, purpose `preference`, scope `tabularium`) — **superseded 2026-08-19 by `a63e96c8`**, which carries the corrected version below. The original read:

> The owner explores the idea of building an own agent harness, with Tabularium as its memory layer, by thinking out loud — "my rubber duck on the table" — not as a run-up to implementation. The driver is control and legibility plus moving into AI-engineering work, explicitly **NOT** pain with Claude Code, which works well and whose shortcomings are already known and worked around; the stated analogy is MacBook Pro to Arch Linux, "never going back". Tabularium comes first and is not done. Engage substantively on design, but do not push toward next steps, plans, scoping or code unless asked.

**It overrides less of it than an earlier draft of this file claimed.** What survives:

- the driver is control and legibility plus moving into AI-engineering work, **not** dissatisfaction with Claude Code;
- and the **working mode**: engage substantively on design, do not push toward next steps, plans, scoping or code unless asked.

What does not survive: "not as a run-up to implementation", and "Tabularium comes first".

**Ruled 2026-08-19 (second session): the mode is "rubber duck v2".** The deliverable changed; how sessions are run did not. Something is being built, and sessions are still design conversation rather than planning. An earlier draft of this file listed the no-planning instruction among the clauses that die — that was wrong and is corrected here.

Operationally, for any agent reading this: **design is the live subject. Do not produce plans, scopes, task breakdowns or code unless asked.** Wanting the thing built is not permission to start scoping it.

## 2. The stack ruling

Decision `eeedd9fe` — **in force**, recorded 2026-08-13 22:03, fourteen minutes after the rubber duck session closed.

> If the personal agent harness is ever built, it is written in **Rust**, with the memory and context-policy layer staying in **Python behind MCP**.

**Ruled 2026-08-19:** the conditional is discharged. It is being built, so this reads as unconditional.

**Why.** The harness shell is systems code: startup latency on a CLI invoked constantly, a single static binary, tokio's multi-threaded work-stealing concurrency for a streaming completion plus several simultaneous MCP connections plus parallel subagents, and compile-time exhaustiveness over an agent loop's many error paths.

The archetype of what that buys is Tabularium's own hazard — a handler that is not async silently escapes the write lock on a worker thread, which Rust turns from a runtime bug into a compile error.

The Hermes `MemoryProvider` interface ports as a trait and *ports better*: a refusing gate's bounce envelope becomes a `Result` with a must-use error enum, rather than something smuggled through an exception because the turn-shaped hook has nowhere to return it.

**Rejected.** Python — whose genuine advantage is iteration speed on the policy layer, which churns constantly. But under this split that layer stays out-of-process, so the compile-time penalty mostly does not land.

**Binding limit as recorded.** The choice flips to Python if Tabularium is ever wanted **in-process as a library rather than a service**, because Rust makes that boundary permanent.

> **Superseded 2026-08-24 (`30e83788`).** The flip-to-Python limit is retired. The shell never leaves Rust; the in-process wish, if it ever lands, is met by a Rust copy of Tabularium inside arx's own root, while the Python service keeps serving other hosts. The "third branch" below is now the designated path, not a deferred option — though its hazard stands: the calibrated constants do not port, so a Rust copy re-derives them rather than translating them.

**Ruled 2026-08-19 — the third branch**, stored as decision `dc433ba5` (scope `arx`). The decision framed this as binary. There is a third option it never enumerated: satisfy the in-process wish by **rewriting Tabularium into Rust** rather than flipping the shell to Python. Considered and **deferred, not rejected.** Tabularium stays a service behind a Rust trait for now:

- Transport is not the cost. A recall is dominated by the embedding step; JSON-RPC over stdio is a rounding error against it. Startup is the better argument and a long-running daemon answers it without a rewrite.
- The real in-process case rests almost entirely on **per-turn context assembly being chatty** — session-start grounding is one call and the boundary is free, but an `assemble(window, budget)` that must see the window every turn is not. That is unproven until the trait exists.
- The code would port; the **calibrated constants would not**. Cosine thresholds, the dedup gray band and the importance calibration are empirical, tuned against a specific embedder's geometry. Same hazard class as the fitted-constant lesson already in the store, with a much larger blast radius.
- During bring-up, in-process **deletes the fallback**. A daily driver has to keep working on the days the harness does not; a process boundary means a harness crash is not a memory outage, and Claude Code stays usable as host throughout.

**Discriminator to watch:** the shape of `assemble` on the trait. Write the trait and one adapter over the wire; if the signature comes out chatty, the second adapter has earned itself.

**Strengthened 2026-08-19 by `2d15b921`.** Freshness checks stay harness-side, so the store never needs to read the working tree. One fewer reason to go in-process; the ruling above got stronger rather than weaker. See §6.

> Note: the rubber duck session's own digest resolved Rust as "viable-but-a-competing-learning-curve, not an upgrade." That line is stale — `eeedd9fe`, recorded minutes later, reverses it.

## 3. Framing lineage — and its reversal

Two decisions from 2026-08-07, one superseding the other the same day. Both are now behind the 2026-08-19 ruling, but the reasoning is worth reading, because one of them is right about a boundary and wrong about a conclusion.

**`d3619071` — superseded** (16:41): "The deliverable is a harness with sophisticated memory, not a standalone LTM system." Settled on three findings:

1. Every unit of work since the 2026-08-02 grounding cutover came from *operating* the system, not from the plan — bounce envelope, tool-surface token diet, shelved lifecycle, per-role grounding, the curation block. None were predicted by any milestone.
2. The component carrying the most policy is curation, and it already lives in the hook layer, not the engine. The engine owns only mechanism (`select_default` + `pack`); cwd-to-scope mapping, role assignment and budget live in `load-tabularium-context.sh` / `load-tabularium-role-context.sh`, unversioned and invisible to every document in the repo.
3. The project was specced around three memory primitives and only two exist.

**`9e5c6e47` — in force in the store, OVERTURNED by the owner** (20:22, same day):

> Tabularium is the product, and its thesis is that **memory is a harness concern rather than a database**: it owns the whole path — the read side (grounding at session start, per-role, budget-packed), the write side (the dedup gate), and the curation policy between them. The hooks under `~/.claude/` are Tabularium's delivery layer, not a second product. Claude Code is the host and portability is a non-goal. Experimental by intent.

Its stated reason for overturning `d3619071` was that **most harness slots are Claude Code chores the owner has no appetite to build**, recorded alongside the owner's own words on reading the slot list back: *it will never be a full harness*.

**Ruled 2026-08-19** (decision `c8f20ac3`). The appetite changed, so that rationale has evaporated. The thesis half of `9e5c6e47` survives and is in fact the reason arx exists — memory is a harness concern, and it owns the read path, the write path and the curation between them. What dies with it is **"Claude Code is the host and portability is a non-goal"**, and that clause is load-bearing across the current design.

**What is host-shaped today, and therefore suspect:**

- Grounding fires at session start because that is the only hook point the host offers.
- The hot tier exists because the injection budget is fixed by someone else.
- The bounce envelope returns mid-turn to the calling agent because an MCP tool has nowhere else to return — and **ruled 2026-08-19 (third session), the dedup gate's *synchrony* is the host artifact, not merely the envelope's return path.** The store bounces blind because it cannot see the client: it cannot know what the agent already holds in context, what it recalled this session, or what it is mid-way through establishing. In arx the writer and the store are the same program, so the check can be informed and pre-emptive instead of reactive. Adjudication moves to a fold at a seam — decision `df14a2bc`, which also records what that costs and how it is mitigated. [`df14a2bc` still stands; the concrete "fold" changed hands twice since — `3f7d9b5c`'s thread-death fold fell to the D13 grill, and the mechanism is now the landscape-plus-quarantine drain, `da0b0d8b` and `27e7cea9`.]
- `d3619071`'s finding #2 — the most policy-dense component sits in bash under `~/.claude/`, unversioned, invisible to every document in the repo. It lives there because bash is where the host let it stand.

The harness's first real yield is therefore not a feature but **reclaiming the policy layer from the adapter it is trapped in**. That argument is stronger than any slot list and would have survived 2026-08-07.

The concrete instance, from the second 2026-08-19 session: the owner had already written a **bash filter** to keep bulky tool output from polluting the window. It worked, and it was crippled by having no access to the host's internals. Owning the seam is what makes that policy stop being a hack — see §6, admission ladder.

## 4. Reference implementations

Atom `55120b04` (pre-cutover pi-memory id, not resolvable in the Tabularium store). Two objects, read for different things, both cleanly licensed for reuse.

| | Goose | Hermes Agent |
|---|---|---|
| Licence | Apache-2.0 | MIT |
| Language | Rust | Python |
| Read it for | the **shell**: loop, streaming, tool execution, concurrency, extension system | the **memory-interface shape** only: the `MemoryProvider` ABC and the initialize/prefetch split |

Goose is MCP-native and extension-first (70+ MCP extensions, 15+ providers incl. local Ollama), donated by Block to the Linux Foundation's Agentic AI Foundation alongside MCP itself and AGENTS.md — so `block/goose` now resolves to the AAIF org. Apache-2.0 permits commercial use, modification and embedding with no source-disclosure obligation; what does bite is preserving licence and NOTICE text for any lifted code, and it carries an explicit patent grant.

Two objects rather than one because Goose's memory is just another MCP extension, so it cannot teach the memory-provider seam; and Hermes' shell is in the wrong language.

**Caveat from the 2026-08-18 recovery session:** Goose is a good donor for the **loop** and a bad donor for the **shape** — the first thing that would have to change in it is precisely the thing arx exists for, since memory-as-just-another-extension is the arrangement being rejected.

**Framing that settled it:** use these as *reference implementations, not dependencies* — the same move that produced Tabularium from the pi ledger.

## 5. Evidence base — the corpora, and evals

**Ruled 2026-08-19**, recorded as fact `8c427f93`. Evals are to be "almost the best-done part", on the grounds that they are the only real metric besides *feel* and *friction*. Two things make that achievable rather than aspirational.

**Three corpora, three evidence classes, produced by three different processes:**

| Corpus | What it is | Bias |
|---|---|---|
| Claude Code `.jsonl` transcripts | what *happened* — unfiltered, high volume, no interpretation | complete; unlabelled |
| Tabularium session digests | what *mattered* — already prose, already carrying `Corrected:` / `Preferred:` lines | interpreted at session boundaries only |
| Tabularium memories | what *survived* — deduped, curated, tiered | heavily filtered |

Agreement across the tiers is weak signal. **Disagreement is the finding**: something constant in the transcripts and absent from every digest is either working invisibly or costing invisibly, and those want opposite responses.

**Why evals are cheap here:**

- The transcripts are a **retrospective eval set that already exists**. At any point in a past session you know what came next, so "would grounding have surfaced the atom that turned out to be needed at turn 47" is answerable offline, with no judge in the loop.
- Every dedup-gate bounce adjudicated by hand is a **labelled example** — candidate, similarity, the exit chosen, and the store's later state as the label.
- Every `Corrected:` line in every digest is a **recorded failure with known ground truth**.
- **Friction is countable**, not a vibe: owner interventions per unit of work, by kind. And the transcripts give a **baseline on the incumbent for free**. This harness has a measurable predecessor, which is unusual — most do not.

**Corrected 2026-08-19 (third session): friction is a sensor, not only a cost, so this metric cannot simply be minimised.** An intervention is where your judgement gets recorded — driving interventions-per-unit-of-work to zero also drives the label supply to zero. Two amendments follow:

1. **The count has to split** into friction that bought a judgement and friction that only cost one. A bounce at 0.97 costs a turn and yields a label no threshold depends on. Which half an intervention landed in **cannot be captured at the time** — it is derived later, when an eval shows which labels actually moved a boundary. So capture liberally and unlabelled; classify at a seam.
2. **The count is a proxy that decays.** Fewer corrections can mean the system improved, or that you stopped bothering — and your engagement swings on sleep, time of day and interruptions, so the noise term is large and not system-shaped. Reply latency is *not* the fix: being away and being tired stretch the same gap, and fitting a time proxy repeats the mistake §6 already rejected when freshness became a diff rather than an age. The design's own answer is self-report at a seam, per the owner's stated preference for explicit friction over inferred state.

The signals that need **no** owner in the loop, and so cross-check the above: **handle pull-rate** — memory defers, the model reaches for it anyway, and memory's admission judgement is falsified by behaviour — and **wasted tokens per unit of completed work**, since a rewind is a costed event whoever fires it. Neither replaces intervention counts; they measure whether the system was self-consistent, not whether it did what you wanted. They are specifically the detector for the failure where those two diverge because you disengaged.

Also note what the harness buys here that no host can: **it is the only thing that can capture an intervention at all.** Under Claude Code your correction arrives as an ordinary user turn, indistinguishable from any other message; the harness sees proposal, disposition, and what came next.

> **Narrowed 2026-08-28 (D09; `22899fd8` superseding `8c427f93`).** Three of the four "why evals are cheap" bullets have lost their subject. The transcripts are not a retrospective eval set — the one thing they can replay, admission volume, T06 already measured, and the thing that matters, handle pull-rate, they cannot contain (`e50e2fcf`). Bounces are not labels: arx's store has no bounce. Digests are gone with `da0ccf24`, so there are no `Corrected:` lines. The intervention bullet survives and got its mechanism: the **ember** (`8915c49b`) — the owner writes the correction, the harness stores it with the turn's handle, and embers per thread is the friction count. The "capture liberally, classify at a seam" line above is retired with it: capture is explicit, the owner's own act, and the bought-vs-cost split is read later off which embers stay. Handle pull-rate and wasted tokens per rewind stand as the no-owner signals, now as event lines in the transcript log (`340b3f2e`).

Constraint inherited from the store: guardrail `5cb398aa` — **no LLM on the write path or the eviction/hot path**. The dedup gray band is pure arithmetic; hot/eviction ranking is the calibrated importance number; the Cleaner's two-seat panel is the only place a model adjudicates, and it runs offline. Note that this was scoped to a *store*, not to a system, so whether it extends to window-level eviction is an open decision rather than an inheritance. **Moot 2026-08-24 (`5efb4c94`):** there is no window-level eviction to extend it to. The guardrail still binds the store, and the two in-loop paths that survive — admission's size threshold and the harness-authored breadcrumb at a cut — are arithmetic and mechanical, so no LLM sits on this write path by construction rather than by rule.

**Sharpened 2026-08-19 (fourth session):** the rewind residue is where the two collide concretely. It is model-generated — the write→cut→append order in §6 exists precisely because only the model holds the branch — *and* it is written to the store scoped like a fact (`79da6421`). That is an LLM artifact on the write path under any reading. The open decision is therefore not abstract; it already has a first instance inside the design.

> **Superseded 2026-08-24 (`a8895a1e`).** This collision was designed away rather than resolved. Rewind no longer produces a model-generated residue: it cuts to the archive and leaves a mechanical breadcrumb, so there is no LLM artifact on the write path here at all. Any distilled lesson is derived later, off the hot path, at an offline fold over the archived branch — which keeps the no-LLM-on-the-write-path guardrail intact by construction.

**Three numbers this design owes the eval set** (all measurable from the transcripts rather than guessed): the ~120k smart-zone boundary, the admission-ladder crossover, and how often mid-task overflow with a mixed tail actually happens. See §6. A fourth, added by the third session: **handle pull-rate**, which prices how often deferral guesses wrong — and, with no splice op, a wrong guess is paid twice.

## 6. Open questions — and what closed them

### Question 1 — what "core-ish" actually means. **ANSWERED 2026-08-19.**

Three architectures hid behind the hedge: Tabularium as (i) an in-process library, (ii) a service behind a trait, (iii) the thing the loop is *organised around* — context assembly being memory's job rather than a slot memory plugs into. (i) and (ii) are a transport axis; (iii) is orthogonal to both.

**Transport: (ii)**, per `dc433ba5` — see §2.

**Organisation: not (iii).** Decision `2d15b921`. The loop is organised around a **merge** with three inputs — agent intent, world state, and the store — and the **harness owns the merge**, because the harness is the seam between agent, world and memory.

> **Narrowed 2026-08-24 (`5efb4c94`).** Memory's authority is stated three times below as worth plus sole ownership of what survives eviction: in the next paragraph, in the arx row of the table, and in the reading of `c8f20ac3` under it. The eviction half has lost its referent. Nothing evicts, folds or compacts mid-session, so there is no surviving content for memory to own. What is left of memory inside the loop is admission, and after `e65b2a35` admission is a size threshold with two outcomes. The *worth* clause holds; the sole-ownership clause does not, and it was the half SURVEY §8 calls arx's one structural bet. Whether the bet survives in some other form is D03's question, not this one's.

Memory's authority is bounded to *worth*. It decides what is worth having from the store, and it is the sole owner of what content survives eviction. It does not merge, does not arbitrate the model's own explicit reads against its own retrievals, and does not answer whether evidence is still valid.

The landing is a **fourth position the original list did not enumerate**:

| | Memory is… |
|---|---|
| **Slot** (Claude Code) | a tool the model may call. Optional, model-initiated, no seat at the moment anything is dropped. |
| **arx** | a mandatory party to every window mutation, and sole owner of what content survives eviction — without owning the merge. |
| **Sense (iii)** | the owner of context assembly itself. **Rejected.** |

So "Tabularium at its core" in `c8f20ac3` reads as *non-optional input to the loop's central operation, plus sole owner of eviction content* — narrower than the phrase implies.

**Shape of the seam.** The harness is the type system, memory is the program, the assembler is the checker:

- memory emits a **plan** — a small typed instruction set, not prose (`Keep`, `Fold`, `Drop`, `Admit`);
- the harness declares the invariants — undroppable set, budget, `tool_use`/`tool_result` pairing, validity, and `Keep`-contiguity (below);
- the assembler evaluates the plan against them, exhaustively, returning a `Result`. Judgement-free but not trivial. **An illegal plan does not compile.**

**`Keep` must form a contiguous prefix from zero.** Proposed 2026-08-19 (fourth session), resolving an inconsistency this file carried: the instruction set was more expressive than the ops that execute it. `Fold` at item 12 while `Keep`ing item 13 *is* a splice, and splice is not built (question 2). Constraining `Keep` to a prefix makes every legal plan compile down to exactly rewind-to-first-non-`Keep` plus an append of the folded material — which is what compaction already is. The gain: **"no splice" stops being a missing capability in the harness and becomes a type-level property of the plan**, enforced by the same exhaustive check, and the assembler gets a second real invariant beyond protocol pairing.

That property — memory has full authority over content and *cannot* produce an invalid window — is exactly what `eeedd9fe` bought by choosing Rust.

> **Emptied 2026-08-24 (`5efb4c94`, `e65b2a35`).** The instruction set above has lost three of its four instructions. `Fold` and `Drop` have no caller once nothing evicts or compacts, and `Keep` was only meaningful as their complement — its contiguity rule existed to make a legal plan compile down to a rewind, and rewind is now an operator command that takes a cut point rather than a plan. `Admit` is the only instruction that still fires, and it resolves to a size comparison with two outcomes. The assembler therefore keeps its protocol-pairing invariant and loses the plan it was checking, which leaves an open question rather than a settled shape: what the assembler is for when the only in-loop policy is a threshold. That belongs to D02 and D03.

**Freshness is a diff, not a timestamp.** Trace evidence records `(path, sha at capture)`. Unchanged sha means the item is provably as true as when written — no re-verification — and only changed paths are dirty. Computed **harness-side**: whether a file changed is a fact about the world, not a question about worth. Time would be a fitted constant, and a bad proxy besides.

### Question 2 — compaction: memory's policy, or the harness's mechanism with memory ranking? **ANSWERED 2026-08-19. Firing and ladder SUPERSEDED 2026-08-24.**

The three sub-decisions the previous session separated all landed.

> **Superseded 2026-08-24 (`5efb4c94`, `e65b2a35`).** Two of those three are dead. **When to fire** is gone, because nothing fires: compaction lost its automatic trigger, so "armed on pressure, fired at a seam" describes a mechanism that will not be built. T06 is what killed it — zero eviction events across 270 sessions, meaning the automatic path would mechanize something that has never once happened. **What survives among the droppable** goes with it, since nothing is dropped. **What is structurally undroppable** stands, and its general line, harness owns validity and memory owns value, is the durable result of this question. The cache argument survives too, in a stronger form: it now explains why the window is append-only rather than why eviction batches. The admission ladder at the end of this section falls separately to `e65b2a35`. Everything below is kept as the reasoning that produced the op-set.

**When to fire** — harness, armed on pressure, fired at a seam in the work. Independently corroborated by cache mechanics, which is why it is closed: the window is a **cache-shaped object**, prefix caching matches from token zero and stops at the first divergence, so eviction wants to be rare and batched rather than continuous. Two unrelated lines of reasoning, same answer, same shape already ruled for the digest on 2026-08-04.

**What is structurally undroppable** — harness. It is protocol correctness, not judgement: a `tool_use` without its `tool_result` is a malformed request. General line: **harness owns validity, memory owns value.**

**What survives among the droppable** — memory, returning a plan rather than a score vector.

**The op-set that falls out** (`9a764559`): **append** and **rewind**. Splice is not built.

| Op | Effect | Cache cost |
|---|---|---|
| **append** | add at the end | free — hit on everything prior |
| **rewind** | cut the tail, keep the head | free — the survivor is still a prefix |
| **splice** | drop mid-window, keep the tail | full recompute from the cut. **Not built.** |

Proposed 2026-08-19 (fourth session): enforce this upstream as `Keep`-contiguity on the plan rather than downstream as a missing op — see question 1.

Rewind keeps the *oldest* content and discards the *newest*, which is backwards for capacity pressure and exactly right for **repair** — the agent went astray, so the tail is the garbage. Compaction is then rewind-to-seam plus an appended digest, paying only the digest's tokens.

Splice is only needed if the window goes **multi-task**. Keeping it mono-task — head plus one unit of work — means the stale region is always the tail. This is the owner's hand-run ritual mechanized, and strictly cheaper than auto-compact, which rewrites the whole window and resets the cache to zero.

> **Superseded 2026-08-24 (`a8895a1e`).** The write→cut→append order below is retired, and the fact recording it (`f88c7521`) is invalid. Rewind is now **cut-to-archive**: the discarded branch goes to the trace archive whole (mechanical, no model call), and the window keeps only a harness-authored breadcrumb — branch cut here, size, handle. The argument for writing before cutting was that the cut destroys the evidence; archival removes that argument, since the branch survives and its lesson can be derived later at an offline fold or contributed by the owner in one line. The paragraph below is kept as the reasoning that produced the rule it replaced.

**A rewind with nothing written down is a loop.** Cut the bad branch, and the model returns to the state that produced it with no record that it failed. So rewind is: **write → cut → append** — ask the model what the branch established *while it still holds the branch*, then cut, then append that back as a short note. **Corrected 2026-08-19 (third session); an earlier draft had this as cut → write → append.** The order is load-bearing: the residue's only producer is the model whose window is being cut, and after the cut that evidence is gone. It also means rewind is not free after all — the window *ops* are cache-free, but the residue costs one model turn, and that turn only exists before the cut. A dead end becomes one line instead of 40k tokens of thrashing, which is *better* than the branch never happening — and only available if the store is in the loop.

**Admission ladder** (`3e2d9a86`) — inline / filter / delegate, discriminated by whether material must be *interacted with* or only *read*. Pollution compounds (full price once, then a cached fraction every turn forever, plus dilution); a sub-loop's window is discarded, so delegation costs once. Admission control is what buys the right to never build splice: the case that begs for mid-window eviction is bulky output admitted verbatim.

> **Superseded 2026-08-24 (`e65b2a35`): two tiers, not three.** The filter tier's extract was priced against an artifact size T06 shows does not occur, and once it went, delegate came apart into filter-with-a-richer-return, which died with it, and the model dispatching a subagent, which is an ordinary tool call and D04's seam. What is left is inline below the threshold and externalize-plus-handle above it. The last sentence still holds and is now the whole argument: admission control is what buys the right to never build splice.

**Traces are evidence, not knowledge** (`061d7942`). A subagent's discarded context goes to the store rather than the void, but in a different durability class: the **verdict** is a claim and enters as memory; the **trace** is material — archived, handle-addressable, **never retrievable as an assertion**. Grounding never surfaces it, recall never surfaces it. Beyond token savings this closes the epistemic gap delegation opens: main can pull the handle and audit the material when a report smells wrong, which is not possible today and which the store already holds two records of needing.

**Two constants this owes the eval set:** the ~120k smart zone is calibrated to one model's degradation curve and must be per-model config, never a literal. The admission crossover is roughly low single-digit thousands of tokens and is measurable rather than guessable.

**The case that would earn splice back:** mid-task overflow with a mixed tail — half failed rabbit hole, half live progress. Currently treated as a scoping failure (park, digest, restart carrying the lesson). If the transcripts show it is common, revisit.

**Measured 2026-08-24 (T06):** 1 of 16 sampled high-pressure windows, plausibly *because* delegation already demotes failed material to a verdict before it reaches the parent. Not common. Splice stays unbuilt, and the scoping-failure treatment stands.

**A second case considered and refused 2026-09-16: stale grounding.** A record the session loaded at start is superseded while the session runs, by another session, the cleaner, or an owner edit. Swapping it in place is a splice at the very head, so it recomputes the whole cache, compaction's price for one line. An appended "superseded, successor says" line was refused too, since it leaves two authorities in the always-on path, and regenerating the grounding block at rewind-to-seam was refused as a store read on every ritual for a once-a-night case. Ruling: the session carries the grounding it started with until it ends, and learns of a change only when it acts on the stale record and the store bounces or a recall returns the successor. Raised off the TencentDB Agent Memory trace, whose skill bridge pins first-seen version per session and refuses stale writes but never refreshes reads. Store decision recorded this date, extending `9a764559` from the tail to the head.

### Question 3 — does the "third primitive" framing hold? **ANSWERED 2026-08-19 (fourth session): no.**

It is a continuity claim, not a mechanism question — nothing in the design moves either way. What is at stake is whether the harness is the *completion* of the Tabularium work or a departure from it. Three things had to hold; none do.

**1. Different object.** The parked primitive was *importance-decay eviction over records in a corpus*. Window assembly evicts *turns from a transcript*.

| | Corpus eviction | Window eviction |
|---|---|---|
| Unit | a record — self-contained claim | a turn — order-dependent fragment |
| Independence | a record stands alone | a `tool_result` without its `tool_use` is malformed |
| Horizon | thousands, long-lived | hundreds, one session |
| Reversibility | permanent | source usually still on disk |
| Failure visibility | silent — a fact is simply gone | immediate — loop or task failure within turns |
| Clock | offline, batched, no latency budget | in-loop, at a seam |

Independence is the row that carries it. Decay works on a corpus *because* records are independent: rank, drop the tail, and the survivor is still a valid corpus. Turns are not independent — which is exactly why question 2 had to invent "harness owns validity, memory owns value." That split has no counterpart in the parked primitive, because a set of independent records has no validity constraint to violate.

**2. The mechanism is not merely unnecessary — it is disallowed.** A corpus is a set and drops from anywhere; a window is ordered and cache-shaped, and its only legal removal is a rewind. An importance score over turns would rank turn 12 below turn 30 and instruct a drop at 12 — precisely the mid-window drop `9a764559` refuses. Applying the parked primitive would **generate exactly the illegal plans the assembler exists to reject.**

**3. It was superseded, not parked.** Per the store, the Cleaner superseded it *in function*, so there is nothing waiting to be unblocked. The phrase is precise though: in function, not in mechanism. The Cleaner is panel adjudication over contradiction and relevance; decay is arithmetic over scores. Same outcome, different means — and a panel has an outage mode arithmetic does not, demonstrated by this session's own grounding (Cleaner blocked, batch failed on a model credit limit). Whether decay still earns a place as a cheap floor under a panel that can fall over is a **Tabularium question in the other scope**, which is itself the point.

**What is genuinely inherited, and where it was misfiled.** The calibrated importance number is a real asset and does carry into the harness — but on the **admission** side, deciding what is worth pulling from the store into the window, not the eviction side. The continuity claim was aimed at the wrong seam. Corrected, it is true and much smaller: *the harness inherits a calibration, not a primitive.*

**Strengthened 2026-08-24 (`5efb4c94`).** The comparison table's right-hand column has since emptied: there is no window eviction to compare a corpus against. This question's answer was already no, and the ruling makes it no for a blunter reason than any of the three arguments below — the object the continuity claim wanted to inherit into has no counterpart in the harness at all. The inherited calibration lands on admission, which is the one seam that survived.

### Question 4 — learning vs daily driver. **Open.**

They conflict, and only on the shell: learning wants the loop written by hand, daily-drive wants it boring and finished. They agree everywhere else — the context layer is simultaneously the novel part, the part with no reference implementation worth copying, and the part daily-drive most needs to be owned.

**Widened 2026-08-19 (third session), and this is the correction that matters.** The conflict is not confined to the shell. A hand-run ritual is not only a cost awaiting automation — performing it by hand is how the friction that produced it stays visible and how the better path gets found. So automating *any* ritual, context layer included, trades the same understanding away.

The readiness test that falls out is **repetition without variation**, not repetition and not annoyance: a ritual teaches while it is still being adjusted, and once performed identically many times the learning is extracted and only the labour remains. Its sibling signal is **repetition without mention** — normalised friction is constant in the transcripts and absent from every digest, because you only write down what you noticed. Both are readable off one corpus pass. Digest-then-clear qualifies on both counts.

Partial mitigation, and its limit: ember preserves a cooled correction's *conclusion* but not its *trigger*. Reignition is demand-driven, and the hand-run ritual was generating the demand — so mechanising it leaves the ember correctly stored and never reignited. Ember needs a veto surface beside it: the harness fires the step and shows what it decided.

**Noted 2026-08-19 (fourth session):** this and §5's "friction is a sensor" are one principle in two instances, not two findings. Both say a cost about to be automated away is also a *supply* — of labels in §5, of understanding here — and both land on the same mitigation, a veto or self-report surface at the seam where the step now fires by itself. Stating it once would extend this question's readiness test to the dedup gate, which §5 currently treats as a separate problem.

### Question 5 — raised 2026-08-19, second session. **ANSWERED 2026-08-19, third session. Residue design SUPERSEDED 2026-08-24.**

> **Superseded 2026-08-24 (`ee932ea8`, superseding `80bffad0`; `a8895a1e`, superseding `79da6421`).** The deferral answer stands — memory withholds a body and admits a description plus a handle. What is stripped is everything below about the *residue*: the two-gate refusal, the shared fact-scoped residue key, the growing per-artifact set, and the semantic applicability match. Refusal is now a bare handle to the archived prior attempt; if the model wants to know whether the old finding still applies, it pulls the handle and judges with full context, rather than memory guessing it from a cosine threshold. The caveat this section closes on — that the residue lookup inherits the dedup gate's fitted-constant hazard — is what killed the design: with no residue set to match against, the hazard is gone. Read the rest of this section as the dead design, kept for its reasoning.

As posed — does memory **override** the model's choice to inline, or only compress what it already pulled? — the question has a false premise. **There is no disagreement.** The two choices are made at different times against different information: the model emitted a read of a *path*, it never asked for 40k tokens, and the size is unknowable when the request is formed. Admission decides something nobody had decided.

**Memory defers; it does not override** (`0aa87641`). It may withhold a body but must admit a description plus a handle — the same mechanism `061d7942` built for traces. The discriminator I had worried about ("must this be interacted with?") then stops being a prediction memory cannot make and becomes an observation: interaction reveals itself when the model pulls the handle. Cost asymmetry says default to deferring — filtering wrongly costs one round-trip; admitting wrongly costs full price once and a cached fraction every turn forever.

**Where it bites:** no splice. A deferral the model then pulls leaves the summary *and* the body in the window, permanently. Guessing wrong costs double. Handle pull-rate is therefore also the direct eval signal on whether filtering was right — and unusually, one with no owner in the loop.

**The carve-out** (`80bffad0`): memory *may* refuse a read outright, but only by handing back what the previous attempt established. This is the rewind rule one layer down — removal is legal only if it leaves a residue. Two gates, different owners: `(path, sha)` is **validity**, harness-side; the question the earlier read was made under is **applicability**, memory-side. The why is mandatory, because a dead end is dead only relative to a question.

**Residue shape** (`79da6421`): scoped like a fact, not local to the task, so a dead end is paid once rather than by every session. The key holds a **growing set**, not a value — one artifact accumulates an index of what has been searched for in it and found absent. Validity is global; applicability is per-question and cannot be cached. Growth is not the hazard it looks: the store is key-addressed rather than searched, so volume costs storage not attention, and a file changing kills its whole key at once.

**Caveat raised 2026-08-19 (fourth session), unresolved.** Applicability is stated here as per-question and uncacheable, and the residue set then caches exactly that, keyed by artifact. It resolves — what is cached is a *specific* question's answer, not a general verdict — but matching a new question against the stored set is semantic matching, which is embeddings, which is a cosine threshold. So the residue lookup **inherits the dedup gate's fitted-constant hazard**, the same one §2 used to argue against porting Tabularium to Rust. Key-addressing does not save it: the key is exact for validity, not for applicability.

### Question 6 — do admission and eviction belong in one plan? **DISSOLVED 2026-08-24 — there is no second clock to share a plan with.**

> **Dissolved 2026-08-24 (`5efb4c94`).** The question needs two clocks and arx has one. `Keep` / `Fold` / `Drop` never fire, so `Admit` shares an instruction set with nothing. The observation that raised the question was right and the ruling took a blunter form of it: the two seams did not merely deserve separate plans, one of them turned out not to exist. What the paragraph below still describes correctly is admission's own profile, which is now the only in-loop clock arx has.

`Admit` fires per tool result, continuously, on the hot path, with a live model intent outstanding. `Keep` / `Fold` / `Drop` fire rarely and batched at a seam — argued from cache mechanics in question 2. Different frequency, different latency budget, different information available at decision time. One typed instruction set spanning both may be conflating two seams that merely happen to share an object. From the type side this is the same observation as `Keep`-contiguity: the instruction set is doing two jobs and the invariants constrain only one of them.

### Question 7 — is removal always demotion to a handle? **CLOSED 2026-08-24: yes at every seam that exists, and the fourth seam stopped existing.**

> **Advanced 2026-08-24 (`a8895a1e`, `ee932ea8`).** The session answered this for three of the four seams the question names. Rewind now demotes to the archive (cut-to-archive), and refusal now demotes to a handle (the archived prior attempt); admission already deferred to a handle. Only **eviction proper** remains open — whether a dropped turn also leaves a pointer, and whether "nothing is removed, only demoted" gets ruled as a single removal law. That last question is D08 on the map. The tax this section worries about — a description occupying window space forever — is softened by the donor precedent (Goose keeps everything in the store and only flips a visibility flag): the handle can live in the archive, with only a bare pointer surviving in-window.

> **Closed 2026-08-24 (map ticket D08, no new ruling stored).** The fourth seam went away before anyone settled it. With eviction deleted by `5efb4c94`, the asymmetry described below has no remaining term: admission externalizes to a handle (`e65b2a35`), refusal is a bare handle (`ee932ea8`), a cut archives the branch whole and leaves a breadcrumb (`a8895a1e`), and a trace is handle-addressable and never assertable (`061d7942`). Every removal arx performs is already a demotion. Writing that down as a single law would restate four decisions, which is why it was not stored as a fifth — the only place the law could still do work is a seam nobody has designed yet, and that is D04's discarded sub-loop window. The cost objection in the last paragraph below is void with it: the tax was priced against the one seam under capacity pressure, and that is precisely the seam that no longer exists.

The design is currently asymmetric. At admission memory **defers** — withhold a body, admit a description plus a handle (`0aa87641`). At refusal it must leave a residue (`80bffad0`). At rewind it must write before cutting (`9a764559`). But at eviction memory **drops**: full authority, no surviving pointer.

The machinery that would close the gap already exists — `061d7942` makes traces handle-addressable and never retrievable as an assertion. If evicted window content also left a handle, eviction becomes deferral too, and four rules collapse into one: **nothing is ever removed, only demoted to a handle.** Admission's deferral, refusal's residue, rewind's residue and eviction become the same move at four seams.

Why this is a question and not a ruling: the handle is not free. A description occupies window space forever — the same "guessing wrong costs double" tax question 5 already flags under no-splice — and eviction is the one seam under capacity pressure, so the tax lands hardest exactly where it can least be afforded. The counter is that a dropped turn's material is usually still reconstructible from the world, so what eviction needs may be a bare pointer rather than a summary, and far cheaper than the admission case.

## 7. Operational notes for a session starting here

- **The fourth session (2026-08-19) is not in the store.** Question 3's close, and every item marked *proposed* or *raised* that day, exist only in this file and the session transcript. Everything from the second and third sessions carries a decision id; nothing from the fourth does.
- **`e967bbed` is closed.** Superseded 2026-08-19 by `a63e96c8` in scope `tabularium`, which keeps the working-mode clause — engage on design, do not push toward next steps, plans, scoping or code — and drops "not as a run-up to implementation" / "Tabularium comes first". An earlier draft of this file wrongly listed the no-planning instruction as dead; see §1.
- **`c8f20ac3`'s own text overstates on one clause.** It closes with "Consequence: the still-active atom e967bbed, which instructs sessions to engage on design but not push toward next steps, plans, scoping or code, is overtaken by this" — which reads as wholesale, and is the source of the error this file made. It is not: only the framing clauses were overtaken, and `a63e96c8` is authoritative on that. A Decision's text can be corrected in place from the console but not through the MCP surface, so this needs an owner correction there or it will keep misleading.
- **`9e5c6e47` is closed.** Superseded 2026-08-19 by `c8f20ac3`, written into scope `tabularium` because **cross-scope supersession is refused by the engine** — a correction produced by work in one scope must be written into the scope where the stale record lives, not where the work happened.
- **The store cannot mark a position provisional, so the banner at the top is not self-enforcing.** Grounding hands a session stored facts and decisions with no durability class separating a working position from a ratified one, and every repair mechanism the store has is retrospective — supersession marks a record overturned, never never-ratified; shelved and archived mark it out of play rather than in play but unratified. So the next session's grounding will deliver `2d15b921`, `dc433ba5` and the rest reading exactly as settled, with `bfcc23c8` beside them as a note rather than as a property of them. **Read the banner first and apply it to everything grounding hands you.** Logged as a Tabularium gap 2026-08-19 (fourth session), stored in scope `tabularium` as `48606976` because the fix lives there, not here.
- **Grounding is scope-partitioned, and this file is not.** A session opened in this directory receives `arx` + `global` hot/ember only; nothing scoped `tabularium` arrives automatically. So a stale record in a neighbouring scope will *not* leak in via grounding — but a deliberate cross-scope `recall` or `sessions --scope tabularium`, which is the documented way to reconstruct position here, walks straight into it. Scope isolation is not a defence against a stale ruling; only superseding it is.
- **Scope registration is a two-step.** The CLI reporting a scope registered against the live store is not sufficient — the grounding hook needs a branch for the new directory or it exits silently, and a scope with no hook branch is indistinguishable from a scope with nothing curated. Wiring it up meant four places beyond the store itself. *(Both are done for `arx`: `.mcp.json` is declared here and the hook fires.)*
- **When a new effort starts from an old conversation, read the session rows first** and reconstruct the position from the store rather than asking the owner to restate it. That is how this document was assembled.
- **Watch the `-ish` suffixes — and the words that never carried one.** "base-ish" and "core-ish" each hid three distinct projects behind one word; "core-ish" is now unpacked — §6 question 1 — and the answer was none of the three. **Widened 2026-08-19 (fourth session):** "eviction" did the same with no `-ish` to warn anyone, hiding decay-over-a-corpus, the Cleaner's actual behaviour, and turn-level window pruning (§6 question 3). The generalisation the narrower note could not catch: **the collapsing words are the ones reused across the store/harness boundary**, because the same noun names a different object on each side. `memory`, `context`, `importance` and `freshness` are all in that category and all load-bearing in the current design.
- **Keep answers short.** The owner's questions land in the first paragraphs; long expositions bury them. More rounds, less per round.
- **This file is a ledger, so absence is data.** A section that does not exist marks context not yet gathered, not a decision to omit. §8 is the current largest instance.

## 8. What is not designed yet — the loop

> **In progress 2026-08-24.** Still largely true — the loop is not designed — but it now has a live workface. The wayfinder map's D07 (the step and the clocks) is exactly this section turned into a grill, grounded by the T07 loop-commitments inventory, which enumerated twelve step-shape requirements the memory rulings silently assume. The silent loop commitments this section warned about are now written down as findings; the map is where they get owned.

**Raised 2026-08-19 (fourth session).** Everything above is the **context layer**. The loop is not designed, and the file's own vocabulary hides that: "loop" appears mostly as a reason Rust was chosen (§2), a thing Goose donates (§4), or the idiom *in the loop*. There is exactly one substantive loop ruling — `2d15b921`, the loop is organised around a merge and the harness owns it.

Not addressed anywhere: the turn cycle itself, provider and streaming abstraction, tool dispatch and parallelism, MCP client lifecycle and failure isolation, subagent spawn and supervision, the permission model, interrupts mid-stream, session persistence and resume, and the error taxonomy §2 cited as a *reason* for choosing Rust without ever enumerating.

**Defensible sequencing.** §6 question 4 argues it directly: the context layer is the novel part with no donor worth copying, and the loop has an Apache-2.0 donor sitting right there. Effort went where copying was unavailable.

**But it is not free, and this is the finding.** Several memory rulings have already made loop commitments that were never priced as loop commitments. "Memory is a mandatory party to every window mutation" plus admission firing per tool result puts memory on the hot path of every turn. §2 said the in-process case "rests almost entirely on per-turn context assembly being chatty" and called that *unproven until the trait exists* — but §6 then ruled the chattiness in. **The discriminator §2 named has arguably already fired**, four sections from where it was defined, so it read as a memory decision rather than a transport one.

Same shape for both questions opened that session: question 6's "different clocks" is meaningless without a turn cycle to locate them in, and question 7's cost argument depends on when eviction runs relative to the completion call.

> **Borne out 2026-08-24.** Both questions closed the moment the loop got designed, and neither closed on its own terms. D07's grills ruled the step shape first; question 6 then dissolved because one of its two clocks does not exist, and question 7 closed because its cost argument was priced against a seam that was deleted. The diagnosis in this paragraph was right, and the correction it implies is stronger than "answer these later" — a question posed about an undesigned mechanism can turn out to have no subject rather than no answer. Note the same hazard now standing over the paragraph above: T03 measured that ordinary admission may make no store call at all, so "the discriminator §2 named has already fired" is once again unproven, and it is D03's and D01's to settle.

So the loop is not a later chunk of work queued behind this one. It is what several already-made rulings are quietly assuming, and the assumptions are not written down.

**Why this is the motivating instance for the banner at the top.** The loop commitments were made silently *because* the memory positions read as settled. A ledger that stayed a ledger would have surfaced them as open.
