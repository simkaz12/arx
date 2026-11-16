---
id: L2-06a
layer: 2
labels: wayfinder:task
status: closed
assignee: spec-runner
blocked-by: L2-01
split-from: L2-06
ruled-by: D15 (`18b4928b` — header-level provenance, authored fresh each time; `18fec68a` — the mechanical check: file exists, decision id resolves, path still carries that sha, harness-side on every write, blocking on failure; `eff1f01a` — files-only persistence, the transcript log as the handle address space, the harness mints handles mechanically; `b03f77fc` — the falsehood residual: the check covers paths, shas and decision ids and nothing else), D16 (`952d3b69` — the reset apparatus binds every body)
---

# L2-06a — the state object and its mechanical check

Cut from L2-06 on 2026-09-04 by main under the owner's overnight clearance: the artifact and its check are a session on their own, and L2-06b's ritual has nothing to write until they exist.

## Deliverable

The state object as a file arx reads and writes, and the check that guards every write. One markdown file per checkpoint under the thread's directory, with a header arx writes — checkpoint ordinal, timestamp, the `(thread, turn)` handle into the transcript log at authoring — and a body main authors: position (what is in flight, what is decided, what is open, the next concrete move, which files are hot), the target, whether the target drifted, and the hands in flight by ledger id. Entries carry no anchors beyond what the check needs: a path with the content hash it carried at authoring, and a decision id. A parser that reads the file back into a typed object and a writer that produces it, both under one module. The mechanical check as a function over a body: every referenced file exists, every referenced path still carries its hash, every decision id resolves through `Store::decision`; it names the failing probe and the entry, and a caller that writes only on pass is the whole write path. The content hash needs no new dependency. The `[main] checkpoint` key parsed into config (the token delta; absent means off, reset-only) so L2-06b has nothing to add to the schema.

## Done when

A state object written through the module reads back equal; its header handle resolves in the transcript log to the turn it names; the check passes on a body whose paths carry their hashes and whose decision ids resolve in a scratch engine, and fails naming the probe and the entry on a missing file, a changed file, and an unknown decision id; a body listing two hands in flight by id parses into two ids; `check-config` output is byte-identical with and without the `checkpoint` key; the hash of a file is the same value two callers compute.

## Ruled by

D15 (`18b4928b`, `18fec68a`, `eff1f01a`, `b03f77fc`), D16 (`952d3b69`) — read each before writing the brief; a card that lands against one stops and reopens it in the store first (map Notes).

## Shipped

One module, `crates/arx/src/state_object.rs`, importing only `std`, `serde`, `serde_yaml` and `arx_engine`, `#[path]`-included by `crates/arx/tests/state_object.rs` and `crates/arx/tests/transcript_log.rs`, no inline test module. A state object is one file at `threads/<thread>.checkpoints/<ordinal>.md`, `checkpoints_dir(threads_dir, thread)` naming the directory and `latest(dir)` reading the highest ordinal by parsing `<u64>.md` stems, ignoring anything else, `None` on an empty or missing directory. The header is `Header { thread, checkpoint, at, turn }`, a fixed field order carried into YAML by struct declaration order; `render` writes `---\n<yaml>---\n<body>`, `parse` splits on the first two `---` lines and keeps the body verbatim, and `render` then `parse` round-trips both the header and the body byte for byte.

The body carries provenance as bullet lines anywhere in the text, `- file: <path> @ <40 lowercase hex>` and `- decision: <id>`; a hash that is not 40 lowercase hex drops the line as prose rather than as an entry. A third shape, `- <id> <rest of line>` with `<id>` matching `h[0-9]+`, is read only under a `## Hands in flight` heading and only for the reader — `entries(body)` returns the three lists but the check never touches the hand list. The hash itself is `git hash-object`'s blob id, `hash_file(path)` shelling out with no repository required.

`check(body, root, store, scope)` walks the body once, in the order the entries appear, and probes each: a file entry against `FileExists` then, only if the file is there, `FileHash`; a decision entry against `DecisionResolves` through `store.decision(scope, id)`, where `Ok(None)` and `Err` both count as a failure. Every failure is collected and returned together as `Vec<CheckFailure>`, each naming its `Probe`, the entry's own text and a detail, so one authoring round sees everything wrong at once. `write_checked(dir, obj, root, store, scope)` runs `check` on the object's body and, only on a clean pass, writes `dir/<checkpoint>.md`; an ordinal already on disk is refused as its own `CheckFailure` (`FileExists`, the target path, `already exists`) rather than overwritten, so a prior checkpoint is never lost.

`RoleConfig` gained `checkpoint: Option<u64>`, additive, `deny_unknown_fields` untouched; `RoleRow` and `Resolved` both carry the resolved value, and `0` is refused in `check_role` with `main.checkpoint must be positive`, the same place and shape as the `nudge` check beside it. No default is chosen — absent means off, matching `25664aa5`. `check-config ./arx.toml` was diffed byte-for-byte against a scratch build of `main` and is unchanged; the key adds no column.

`crates/arx/tests/transcript_log.rs` gained the `#[path]` include and one test proving the loop this card sets up for L2-06b: a header's `turn` field, taken from a real `TranscriptLog::append_turn_as` handle, resolves back through `transcript::resolve` to the exact text that turn carried, after a `render`/`parse` round trip.

One call the brief left to the runner: `Resolved.checkpoint` is populated from the already-built `main: RoleRow`'s own field rather than re-reading `startup.main.checkpoint`, since the two are definitionally the same value and reading it off the row avoids relying on partial-move semantics elsewhere in `load`. `write_checked`'s directory creation and file write use `expect`, not a `CheckFailure`, on an I/O error — the three probes are the only failure shape the brief names, and a create/write failure on a caller-owned path is an environment fault, not something an authoring round can fix by editing the body. No other deviation.

Tests: `cargo test -p arx` — unittests 142 (140 plus `main_checkpoint_parses`, `main_checkpoint_zero_is_refused`), `check_config` 8, `state_object` 7, `transcript_log` 27, every other binary unchanged from the L2-08 merge. `cargo test --workspace` — 315 passed, 1 ignored.
