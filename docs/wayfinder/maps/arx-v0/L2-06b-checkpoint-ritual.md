---
id: L2-06b
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-06a
split-from: L2-06
ruled-by: D15 (`adec3734` — checkpoints fire on a token-delta clock between turns or by operator command, every one runs the full ritual, each authored from the window and never from the previous object, hands in flight run on across it and the object lists them; `18fec68a` — authoring turn, harness-prompted truth-check turn, mechanical check blocking on failure; `18b4928b` — the prior object is on disk, never the template; `25664aa5` — the delta is config with a default set from the first real sizes), D07 (`7dae481c` — no rewind on a pressure clock), D16 (`952d3b69` — under a CLI main the ritual turns stay in the window; rewind is native-only), D05 (`23642b44` — dispatch is non-blocking; hands belong to the ledger, not the window)
---

# L2-06b — the checkpoint ritual in the loop

Cut from L2-06 on 2026-09-04 by main under the owner's overnight clearance; the second half, on top of L2-06a's artifact and check.

## Deliverable

The checkpoint in `arx run`. It fires between turns when the occupancy the nudge already reads has grown by the configured token delta since the last checkpoint, or when the operator types the checkpoint command on the terminal (a line arx takes for itself, never a turn). Every checkpoint runs the full ritual: arx prompts an authoring turn as itself, carrying the ledger's in-flight hands by id so the object lists them; arx prompts a truth-check turn with the authored object in hand and takes the reply as the object or its correction; arx runs L2-06a's mechanical check and writes the file with its header only on pass, sending the failing probe back for one more authoring round and giving up loud after that. Each object is authored from the window, never from the previous one; under a CLI-bodied main the ritual turns stay in the window, no rewind. `Event::Checkpoint { check, delta }` lands in the transcript log for every attempt. Hands in flight run on across the checkpoint and their verdicts land at the next turn boundary after it, as they do today. The reset itself, the respawn and the orientation turn are L3-02's.

## Done when

A synthetic thread whose stub body reports growing usage crosses the token delta twice and produces two state objects, ordinals 1 and 2, each passing the check; a deliberately false position in the authoring turn is replaced by the truth-check turn's correction and the file carries the correction; a body failing the check produces `check: fail`, no file, and a second authoring round that passes; the header handle of each file resolves in the transcript log; the operator's checkpoint command fires the same ritual with no delta crossed; a fake hand in flight across the checkpoint is named in the object's hands-in-flight section and its verdict lands in the window afterwards; the nudge and every existing test keep their behaviour.

## Carried from the split

The trace behind this split found that hand deliveries reach the window only between full turns under the Claude-Code-bodied main (the sidecar yields no step boundary), so "the respawned main's first step boundary" in `adec3734` reads as the first turn boundary for this body. It also found that whether a reset respawns the sidecar child inside the running `arx run` or the whole binary is settled nowhere; the ledger's in-memory hands do not survive a process restart. Both belong to L3-02, not here.

## Ruled by

D15 (`adec3734`, `18fec68a`, `18b4928b`, `25664aa5`), D07 (`7dae481c`), D16 (`952d3b69`), D05 (`23642b44`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

The clock lives on a `Checkpoint { delta, base, last_occupancy }` beside `Nudge`; `run_turn_cycle` records `last_occupancy` from every turn's usage, operator or ritual alike, right after `append_usage`. `base` starts at 0 for the run and only moves on a passing checkpoint, to the occupancy measured at the ritual's last turn. The operator loop checks `Checkpoint::due()` after every operator turn and before `pump_deliveries!`; a stdin line trimming to exactly `/checkpoint` is caught before it becomes a turn and fires the ritual on the spot.

Each ritual runs `run_checkpoint_ritual`, a free function taking the same session state `run_turn_cycle` does, calling it directly for the two prompt turns without `pump_deliveries!` between them. The ordinal comes from `state_object::latest` plus one. The authoring prompt lists the ledger's in-flight hands via the new `SpawnLedger::in_flight_hands`, `- <id> <agent>` per line or `- none`; the truth-check prompt carries the authoring reply verbatim and a bare `ok` (case-insensitive) keeps that reply as the body, anything else replaces it. Both prompts are exactly as ruled:

```
[arx] checkpoint {n}: author the state object from this window, not from any earlier object. Reply with the object body only, in this shape and nothing else:
## Position
<what is in flight, what is decided, what is open, the next concrete move, which files are hot>
## Target
<the target>
## Drift
<whether the target drifted, and how>
## Hands in flight
{hands}
Cite a file as `- file: <path> @ <hash>` where <hash> is the output of `git hash-object <path>` run now, and a decision as `- decision: <id>`. Do not cite anything you have not verified in this window.
```

```
[arx] checkpoint {n}: truth-check. Below is the object you just authored. Check every line against this window and against disk. Reply `ok` if every line holds; otherwise reply with the corrected object body in full, same shape.
{body}
```

The reply handle needed for the header's `turn` field never widened `run_turn_cycle`'s signature — every call already threads a `last_handle: &mut Handle` through it, and `TranscriptLog::last_handle()` already answers the same question from the log itself, so the ritual reads that back after each turn and resolves the text through `transcript::resolve`. That is the smaller diff of the two the brief named.

L2-06a's `check` runs against `config_dir` as root and `resolved.store_scope` as scope; on pass, `write_checked` writes the header arx mints (`Header { thread, checkpoint, at, turn }`, `at` an RFC 3339 UTC-seconds stamp built from `SystemTime` with no new dependency) and `Event::Checkpoint { check: "pass", delta }` lands before `base` moves. On failure the event carries `check: "fail"` and the same delta, a terminal notice names every failing probe and entry, and one repair round runs: a second authoring prompt with the failures appended before the shape, then its own truth-check, then the check again. A second failure abandons the checkpoint loud, writes nothing, and leaves `base` where it was. `pump_deliveries!` never runs between the ritual's turns or after a `/checkpoint` command; hands in flight keep running and their verdicts land at the next real turn boundary, same as before.

The stub sidecar answers `[arx] checkpoint ` prompts containing `author the state object` from `checkpoint-author.md` on the first such prompt in the process and `checkpoint-author-2.md` on the second, falling back to the first file when the second is absent; prompts containing `truth-check` answer from `checkpoint-truth.md`, falling back to `ok`. An `ARX_STUB_TOKENS_PER_TURN` env var, read once per answered turn of any kind, reports `input_tokens` as that value times the running count of turns answered in the process and zeroes both cache fields; unset, every existing usage constant is untouched.

One live run against the real Claude Code body found that its agent-sdk drive carries no Bash or file-reading tool at all — unlike a hand's `claude -p` drive — so the model could not run `git hash-object` and correctly cited nothing rather than guess; both checkpoints still passed on the first round with empty provenance, since a body with no file or decision entries has nothing for the check to fail. The ritual and the check behave exactly as designed either way; this is a fact about the current sidecar wiring for the owner to weigh before the first real objects are sized.

Tests: `cargo test -p arx` — unittests 155 (151 plus `in_flight_hands_lists_running_hands_with_their_agent`, `authoring_prompt_lists_hands_or_none`, `truth_check_reply_ok_keeps_the_authored_body`, `clock_fires_on_the_delta_and_rebases`), checkpoint 6, claude_main 50 (the brief's Scope 2 Verify names 49, a stale figure carried over from L2-06a's baseline before L2-10a; this card touches no file in `tests/claude_main.rs` and the count is unchanged from this card's own `2f6ca32` baseline), every other binary unchanged. `cargo test --workspace` — 335 passed, 1 ignored.
