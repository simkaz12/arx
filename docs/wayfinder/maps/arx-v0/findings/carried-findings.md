# Carried findings — the polish list for L3-04

**Date:** 2026-09-08, consolidated after L3-02 landed and layer 3's build cards closed. **Source:** the "findings carried" clauses of the landing entries in `../map.md` (L1-10, L2-12, L2-close, L3-03, L3-01, L3-02) plus what the two overnight live checks showed. Every item here was seen and left alone on purpose: the map takes no new cards, and L3-04 is the day the harness works on itself. An item is closed by a fix landed under arx, by a store record when it is a want rather than a defect, or by a ruling reopened in the store when building it shows a ruling wrong.

Classes: **correctness** (the harness does something wrong), **legibility** (the harness is right but tells the operator or main the wrong thing), **test gap** (behaviour verified by hand, no persisted check), **hazard** (a trap for whoever runs or builds arx), **want** (a convenience nobody ruled on).

## Main's window and tool surface

- [ ] **A hand's delivered verdict reaches main's window uncapped.** `format_delivery` in `crates/arx/src/main.rs` embeds the hand's full text with no size check; it is an injected message, not a tool result, so L3-01's admission never sees it. A hand that returns a 30k-token verdict fills main's window in one delivery. Class: correctness, the only unbounded entry into main's window left. Source: L3-01.
- [ ] **The echo preview and the trace frontmatter count different things.** The preview line quotes the trace file's byte and line counts; the frontmatter's `bytes` and `lines` are the body's. The live main compared the two and told the operator the file had grown. One extra word in either place settles it. Class: legibility. Source: L3-01 live check.
- [ ] **`arx_ping` never crosses the gate.** The sidecar answers it locally, so a gate rule on it is inert and the rehearsals had to ask on `recall` instead. Class: legibility (a rule that looks live and is not). Source: L2-14, L2-close.
- [ ] **A native hand's roster line hides its default tools.** `agents/hand-ds.md` carries no `tools:` line, main cannot see that `read_file` is the default, and it second-guessed a real read. Class: legibility. Source: L2-close.
- [ ] **`tap` does not do what the glossary says.** CONTEXT.md's Tap entry promises the hand's live transcript through the ordinary read tool; `SpawnLedger::tap` returns state, brief and stop question. Either the entry or the tool moves. Class: legibility. Source: L3-01 trace.

## Reset, reserve, gauge

- [ ] **The real Agent SDK string for a full-window rejection is unseen.** `is_context_window_message` in `crates/arx/src/claude_code.rs` matches five substrings guessed from the Anthropic and OpenAI shapes. Until a real rejection has been read once, the floor is a matcher nobody has watched fire. The day that hits it should quote the `fatal` message into the store. Class: hazard. Source: L3-02.
- [ ] **Occupancy lags the reply by a turn.** `transcript::occupancy` sums prompt-side tokens only, so the arm and the ceiling read one turn behind what the window actually holds after a long reply. Class: legibility, with a correctness edge near the ceiling. Source: L3-02 live check.
- [ ] **`arx gauge` prints the compaction constant as its window.** The line still reads `window 1000000 (body-reported)`; arx's own `[main] window` and `reserve` are the numbers an operator wants there. Class: legibility. Source: L3-02.
- [ ] **`fired` is written after a passed ritual, not at its start.** The L3-02 brief's prose and its own test ordering disagreed; the runner took the ordering, so the floor path logs `floor` and `respawned` with no `fired`. Fine if that is the meaning wanted; the store should say so if it is. Class: want (a ruling to write down). Source: L3-02.
- [ ] **The PID-reuse guard has a test that cannot fail.** `kill_and_reap`'s `try_wait` guard is right by inspection and Codex agreed; the regression test proves only that kill-then-respawn does not crash. Class: test gap, inherent to the primitive. Source: L3-02, and L1-04 before it.
- [ ] **The native hand's kill path is safe but not live.** `reqwest::blocking` cannot cancel a stalled read, so a kill during a hung provider call waits for the read to end. Class: correctness, low frequency. Source: L1-10, recorded on the L2-10 card.

## Store, drain, judge

- [x] **A fresh workspace needs `store/<scope>/` before `arx run` starts.** `Engine::open` refuses an unregistered scope directory by ruling (`f09b9ed4`); the refusal text should say `mkdir store/<scope>` rather than leave the operator to find it. Class: legibility. Source: L3-02 live check. Closed by L3-05: the refusal prints the absolute root and the `mkdir` line.
- [ ] **An all-drop drain commits nothing while deleting quarantine files one by one.** `Engine::drain()` in `crates/arx-engine`; reachable from the tool surface since L3-03. A crash mid-drop leaves a half-emptied quarantine with no commit to show it. Class: correctness. Source: L3-03 (pre-existing, out of that card's scope).
- [ ] **A claude-code judge fences its JSON and the validator refuses the fence.** Stripping a markdown fence before parsing would let a Claude judge pass the schema check; today its verdicts arrive malformed. Class: correctness, only when the judge is Claude-bodied. Source: L2-12 live check.
- [ ] **Family match for an OpenAI-compatible provider is by config key.** A provider named `gpt` matches a codex judge, one named `openai` does not. Class: correctness at the edge. Source: L2-12.
- [ ] **`agents/drain.md` with a bad body is refused, untested.** Verified by hand in L3-03, no persisted test. Class: test gap. Source: L3-03.

## Building arx under arx

- [ ] **Hands cannot run the container-backed half of the suite.** Hands run in containers with an empty allowlist and no Docker socket; `cargo test -p arx` needs the daemon for eleven of nineteen binaries. A hand polishing arx can edit and build and run the unit binary, the daemon-bound tests stay on the host. Whether the hand images carry a Rust toolchain at all is unchecked. Class: hazard, the first thing the L3-04 day meets. Source: overnight reasoning, not a landing entry.
- [ ] **Stub images are cached by tag, not content.** After editing a script under `crates/arx/tests/fixtures/*-stub/`, `docker rmi` the tag or the tests keep running the old stub. Already in `CLAUDE.md`; here so the list is whole. Class: hazard. Source: L3-03.
- [ ] **The `events.rs` module doc omits `miss` from its list of live kinds.** Every kind has an emitter now; the comment should say so plainly. Class: legibility. Source: L3-02 review.
- [ ] **`config_dir()` treated a bare `arx.toml` as parent `""`.** Fixed as an L1-11 ride-along; listed so nobody re-finds it. Class: closed. Source: L1-10.

## Process, not code

- [ ] **spec-reviewer stashed against its guardrail.** It ran `git stash -u` and pop on L3-01 with the read-only toolkit spelled out in the dispatch, then self-reported and redid the check through a worktree. The prohibition and the alternative belong in `~/.claude/agents/spec-reviewer.md`. Class: hazard, outside this repo. Source: L3-01 review.
- [ ] **spec-runner stalls after launching a codex round unless told to poll.** The L3-02 dispatch carried the instruction and ran four rounds without a stall; L3-01's did not and needed a resume. The durable fix is in `~/.claude/agents/spec-runner.md`. Class: hazard, outside this repo. Source: L3-01, L3-02.
- [ ] **spec-runner leaves its wait loops running after it returns.** Three shells from the two overnight runners lived six to eight hours after their verdicts: a codex poll loop whose grep pattern never matched the log's closing line, and the L3-02 live-check driver with its `until grep "driver done"` loop. Killed by hand at 10:30. Class: hazard, outside this repo. Source: this session.
- [ ] **arx-engine tests leave their temp directories under `/tmp`.** After a night of runs, about nine hundred `arx-engine-*` directories totalling 109 MB sat there, none removed by the tests that made them. Class: test gap. Source: this session.

- **A codex hand can dispatch outside arx's ledger** (hazard) — codex ships `spawn_agent`, `send_message`, `list_agents`, `interrupt_agent`, `followup_task` and `wait_agent` enabled, and roster C found no flag that switches them off; three candidates were probed live and none moved the token count. Contradicts `705e4df2`, arx owning dispatch and the process tree. Recheck this class on every new body.
- **An empty `tools:` means opposite things per body** (correctness, latent) — negates everything on codex after roster C, while `claude_hand.rs::effective_tools` reads it as the default set `Read, Glob, Grep, Bash`. Harmless while both empty-tools positions are codex-bodied; a lie to main the moment one is not.
- **`gate::append_rule` has no production caller** (legibility) — the a/A/d/D prompt that used it went with `Ask` in roster C, leaving it alive only through its own unit test.
