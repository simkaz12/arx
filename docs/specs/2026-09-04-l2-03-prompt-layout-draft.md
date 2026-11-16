# L2-03 — system prompt layout and cache discipline: the draft, second cut

Card `docs/wayfinder/maps/arx-v0/L2-03-prompt-layout-and-cache.md` (prototype, HITL, gate exempt). Ruled by `bb81d2c6` (layout and cache discipline are decided at build against the real prompt and the real tool surface), D16 `68c366dc` (arx's own system prompt replaces the body's preset; new-thread-only, which fits reset as kill-and-respawn) and `ceb0390f` (arx includes the repo instruction file itself when the definition says so, so the log records what the model saw), `952d3b69` (reset under a CLI body is kill-and-respawn with the state object as orientation; rewind is native-only), `a82c08c5` (recall appends at the tail; the cache breaks only before the divergence point), D02 `975f65cd` (spawn-time composition named by the definition).

What the first cut got wrong, and this one fixes: it restated the ten tool signatures in the prompt and then asked whether they should be generated; it invented a prefix check that would refuse to start a thread; it measured the cache against a byte count that ignores the 1024-token floor and the cache TTL; and it left six questions open, five of which the overnight rule says main answers. One question remains for the owner, at the end.

Reaction 2026-09-05, folded in below: the owner struck the always-dispatch line. Escalation between hand tiers is main's judgement in the prompt, never a definition property, and main may jump a tier; the prompt gains a size rule and an escalation rule, and the build gains a bounded read on main so a verdict can be checked before a redispatch. Second reaction, same day: minimal edits by main are permitted on the same footing as reads, since a small fix main can see whole costs less than re-initialising a hand. Third, on main's own review of the draft, agreed by the owner: a hand roster joins the per-thread band, lines ahead of the code leave the standing prompt, bounded is the read tool's property, and band two's lifetime is the file. The build is split: L2-03a ships the read and edit tools first, L2-03b the layout, so the standing prompt never names a tool that does not exist; and the file is `prompts/main.md`, because `config::load` refuses any `agents/main.md` by name (the no-body-override guard, `0bd4df2f`, locked by a `check_config` test).

## Ground truth (2026-09-04, against the tree after L2-11)

- **The prompt today.** `compose_system_prompt` is one string: the `[main] instructions` file or the one-sentence default, then `Repo instructions (<path>):` and the file if `[main] repo_instructions` is set, then `Store grounding (scope <scope>):` with one `- [<label>] <question> — <answer>` line per hot and ember claim and `Quarantined this thread:` with one `- [<kind>] <question>` line per candidate under `store/quarantine/main`. The sidecar passes it as `systemPrompt` with `settingSources: []` and `tools: []`, so the model has the ten arx MCP tools and nothing else. The repo's `arx.toml` sets neither path.
- **The tool descriptions are one-liners.** `recall`: "Substring-recall claims from a scope." `steer`: "Push a message into one running hand." `kill`: "Tear down a hand; its exit is stubbed into the transcript." The schema is where a model learns what a tool is; today it learns almost nothing there.
- **Main learns no definition names today.** Nothing in the prompt or the tool descriptions lists the hand definitions; `dispatch` takes a name main has to know already.
- **The SDK has the seam this card needs.** `@anthropic-ai/claude-agent-sdk` 0.3.251, `sdk.d.ts:2107`: `systemPrompt` is `string | string[] | { type: 'preset', … }`. In the array form, the element `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` marks "the split between the static (globally-cacheable) prefix and the dynamic (session-specific) suffix. Blocks before the marker are eligible for cross-session prompt caching; blocks after it are not." No `cache_control` option exists; the CLI places the breakpoints.
- **How the vendor caches.** The prefix is cached in the order tools, then system, then messages; a byte changed anywhere invalidates everything after it. The minimum cacheable prefix is 1024 tokens on Sonnet and Opus. The default TTL is five minutes, refreshed on every hit.
- **The log already records the numbers.** The sidecar forwards the SDK result's `usage` object verbatim (`claude-main.mjs:198`); `append_usage` writes it as the `usage` row; `transcript::occupancy` sums `input_tokens`, `cache_creation_input_tokens` and `cache_read_input_tokens`; `arx gauge` reads the last row. No new plumbing is needed to measure.
- **Main has no file or shell tool today** (L2-06b's live check), and `tap` returns a hand's state, brief and stop question, not its trace. The size rule and the escalation rule in band two assume a bounded read and a bounded edit on main's surface, both crossing the gate like every main tool call; they are build items, not prompt lines.

## The layout: four bands, by how long they live

| Band | Lives for | Sent as | Changes when |
|------|-----------|---------|--------------|
| Tool schemas | the binary | the tools block, from the sidecar's registrations | a release |
| Standing prompt | the file | `systemPrompt[0]`, the body of `prompts/main.md` | `prompts/main.md` changes |
| Per-thread section | one window | `systemPrompt[2]`, after `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` | a spawn or respawn |
| Tail | one turn each | messages | every turn, append only |

The first two bands are the cross-session prefix: identical bytes for every thread and every respawn on the same binary, so a respawn reads them from cache and pays only for the per-thread section. The per-thread section is composed once at spawn and never edited. The tail is where everything that moves lives: operator turns, replies, `[hand …]` deliveries, `[arx]` lines, checkpoint ritual turns, and every tool result including recall (`a82c08c5`).

### Band two, the standing prompt (`prompts/main.md`, verbatim)

```
You are arx's main: the deciding loop the operator talks to. Everything else in this harness works on your behalf. You decide, dispatch and judge. You do not read, search or change things at scale; a hand does that and leaves a trace.

How work moves
- A hand starts from its brief and nothing else. It does not see this window, the store or the state object. Write the brief so a stranger could act on it: the goal, the surface, what done looks like, what it must not touch.
- Do it yourself when one bounded read or one small edit settles it: a file you can see whole, a change you can state in a sentence. Dispatch when the work is a search, a change across files, or anything whose answer would not fit in a few screens. Past that point a hand's window is cheaper than yours; short of it, a hand's spawn costs more than the fix.
- Start with the smallest hand the brief permits. When a verdict does not survive a look at its trace, redispatch to a stronger hand with the gap written into the brief, and skip a tier when the gap warrants it. The tier is your call, never the hand's.
- dispatch returns at once and your reply ends your turn. A hand's verdict, stop or exit arrives as a `[hand …]` line at the start of a later turn, with a trace handle. Nothing waits for you.
- A verdict is a claim. The trace under its handle is the evidence. Do not tell the operator a result is true until a hand has shown it.
- A hand that stops with a question holds until you steer it. A hand you will not answer, kill.
- Hands never commit. A write hand's work is the diff in its trace; applying it is a decision you say out loud.

The store
- The grounding below is what the store held in force for this scope when this window started. It does not update while the window lives; recall and decisions_query do.
- save puts a claim in this thread's quarantine. drain promotes the quarantine once, when the work is done. Nothing reaches the store before drain.
- A decision names what was rejected and why. A fact is a standing statement the code does not show. Do not save narrative or status.

Lines from the harness
- A line beginning `[arx]` is the harness, not the operator. A nudge is information about your window. A checkpoint asks for a state object: answer with the object body alone, then with `ok` or a corrected body, and nothing else.
- Hands in flight run on across a checkpoint. List them as in flight in the object; their verdicts land after the ritual.

The operator
- The operator sees your reply and the notices arx writes. Answer what was asked, name what you decided and what you will not do, and stop.
```

Per-tool semantics do not appear here. They live in the sidecar's descriptions, which the build rewrites from one-liners into the sentences a caller needs (what the argument is, what comes back, what it does not do). One source; nothing to drift.

### Band three, the per-thread section (rendered by arx at spawn)

```
Workspace <config_dir>. Store scope <scope>.

Repo instructions (<path>):
<the file, verbatim>                                  (only when [main] repo_instructions is set)

Store grounding (scope <scope>), as of this window:
- [hot] <question> — <answer>
- [ember] <question> — <answer>

Quarantined this thread, as of this window:
- [<kind>] <question>

Hands available:
- <name>: <model>, <body>, tools <list>. <role line from the definition>

State object at respawn:
<the checked object's body, verbatim>                (only on a respawn)
```

Empty parts are omitted, as today. "As of this window" is in the text because it is true: a save mid-window does not re-render the listing, and a grounding push is a respawn, not an edit. The roster is rendered from the hand definitions the config names, in the config's order; it is what gives the escalation rule its vocabulary.

## The discipline

- **Nothing in the three prefix bands changes while a session lives.** This holds by construction, not by a check: arx composes the array once and the sidecar passes it once at `init`; no code path edits it, and none is added. The first cut's "refuse to start if the prefix differs from the last thread's" is dropped; the same binary composes the same bytes.
- **The per-thread section changes only at a spawn.** A grounding push, a new repo instruction file, a respawn with a state object: each is a new session (`952d3b69`), and the new session's first turn pays for band three only.
- **The tail only appends.** Under the claude-code body arx cannot rewind, so checkpoint ritual turns stay in the window (`952d3b69`); that is a window cost, not a cache cost, because they append like everything else.
- **The standing prompt says only what this binary does.** A card that changes what main experiences edits `prompts/main.md` in the same card. A line ahead of the code is a lie the model acts on; the second cut's respawn line is out until L3-02 ships the mechanic.
- **One author, one source.** The standing prompt is one file, the tool semantics are the sidecar's descriptions, the per-thread section is arx's renderer. The transcript log's system row records the two blocks as sent, so a cache miss is attributable to the band that moved.

## The measurement, which is the card's done-when

Read from the log's `usage` rows on the owner's login, all turns within the five-minute TTL:

1. First turn of a fresh spawn: `cache_creation_input_tokens` is non-zero and covers the three prefix bands.
2. Second turn on the same session: `cache_read_input_tokens` is at least the first turn's `cache_creation_input_tokens`; `input_tokens` is the new turn alone.
3. First turn of a second `arx run` within the TTL, with the store changed between runs so band three differs: `cache_read_input_tokens` is non-zero and at least the token size of bands one and two, and `cache_creation_input_tokens` is of the order of band three. That is a respawn's cache shape, and it runs before L3-02 exists.

If the three prefix bands together fall under 1024 tokens the first check cannot pass on any layout. Ten schemas with real descriptions clear it; the live run counts it.

The hermetic half runs on the stub sidecar: two spawns of the same config produce byte-identical `systemPrompt[0]`, and a grounding change between them alters `systemPrompt[2]` only; the stub replies with a scripted `usage` and the log row carries it.

## Decisions taken, no grill

- **Escalation is main's judgement, in the prompt.** Owner ruling 2026-09-05: a weaker model in the ladder would over-reach to delegation, and main may jump a tier. Rejected: the definition naming its escalation target; the first cut's always-dispatch line.
- **Main may make minimal edits itself.** Owner ruling 2026-09-05: a small fix main can see whole costs less than a hand's spawn and brief, and f5b5caf4 already places the discipline in prompt taste where main has edit tools. Rejected: a read-only main; a line-count limit in the prompt.
- **A hand roster in the per-thread band.** The escalation rule needs names and a ranking; the definitions are config, so the roster is rendered at spawn rather than written into the standing prompt. Rejected: main guessing names; the roster in band two.
- **Bounded is the tool's property.** Main cannot know a file's size before reading it; the read on main caps its return and says when it truncated, and a truncated read is the signal to dispatch. Rejected: asking main to judge size before it reads.
- **No tool list in the prompt.** The schemas are already in the cached prefix; a second copy is a second author. Rejected: the first cut's hand-written list; a list generated from the sidecar (still a second copy).
- **The SDK's boundary marker, not one string.** One string is never cross-session cacheable, so every respawn re-pays the whole prompt. Rejected: one string; the `claude_code` preset with `excludeDynamicSections` (D16 rejects the preset).
- **The state object stays in the per-thread section**, where the card put it. Same cache cost as a first user turn, and it is not a message anyone sent. Rejected: the first user turn.
- **The quarantine listing stays, stamped "as of this window".** At a respawn it is what the new window would otherwise not know it had saved. Rejected: dropping it; a tool to list it.
- **No prefix hash, no prefix check.** Rejected: the first cut's `prefix_sha` and the refusal rule.
- **`repo_instructions` stays unset in arx's own `arx.toml`.** The repo's `CLAUDE.md` is written for a body with file tools; for main, the grounding is the repo knowledge. Rejected: pointing the option at `CLAUDE.md`.
- **The hash question leaves this card.** How a checkpoint cites a file when main has no file tool is L2-06b's residual, not a layout question; it goes to the map's Notes at close. Rejected: settling it here.

## Owner's reaction, 2026-09-05

Read and ruled in one sitting: the always-dispatch line struck; escalation is main's judgement, never the definition's, and may jump a tier; minimal edits permitted on the same footing as reads; the four fixes from main's own review agreed. The standing prompt in band two stands as written above. Nothing is left open; the next step is the build brief.

## The build that follows the reaction

`prompts/main.md` in the repo with the standing prompt as its body and `[main] instructions = "prompts/main.md"` in `arx.toml`; `compose_system_prompt` returning two blocks and the sidecar passing `[static, SYSTEM_PROMPT_DYNAMIC_BOUNDARY, dynamic]`; the sidecar's ten descriptions rewritten; `read_file` and `edit_file` on main's MCP surface, bounded to the workspace and `threads/` the way L2-11 bounds a hand's and crossing the gate on the host, so main can look at a trace before it escalates and make the small fix a respawned hand would cost more than, with `read_file` capping its return and reporting truncation (new scope, from the owner's reaction); the hand roster rendered into band three from the definitions; the system row carrying both blocks; the hermetic test above; the live measurement on the owner's login. Behaviour-changing, so the codex gate runs (credits return 2026-09-07).
