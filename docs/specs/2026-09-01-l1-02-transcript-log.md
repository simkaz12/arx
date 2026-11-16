# L1-02 — transcript log

Card: `docs/wayfinder/maps/arx-v0/L1-02-transcript-log.md`. The card is this brief's whole world.

**Ruled by:** D15 `eff1f01a` (look-back handles, per-thread append-only log, third persisted file, machine-local in v1), D16 `7b607c7e` (arx-written from the body's stream, the body's session file never read), D04 files-only persistence `ba6fd355` as grown by `eff1f01a`. If an edit below lands against one of these, STOP — the ruling reopens in the store first; do not bend the brief to fit.

## Problem / Solution

L1-01 shipped the workspace and config; a turn of main still has nowhere to land. Every later card that mints or resolves a handle — L1-03 streaming main's turns, D13's claim handle field, checkpoints and the state object header — needs the per-thread append-only transcript log to exist first. This card ships the log module in the shell crate: open a thread's log, append a turn (minting a `(thread, turn)` handle on every write), resolve a handle back to its turn's text — plus the card's four done-when proofs as tests. No body exists yet, so the stream is synthetic, exactly as the card's done-check demands.

## Decisions locked

- Per-thread append-only transcript log; `(thread, turn)` look-back handles minted on every write; independent of rewind — a rewind changes the window, never the log; machine-local in v1. The third persisted file beside the state object and the spawn ledger. (`eff1f01a`, growing `ba6fd355`)
- arx writes the log itself from the body's stream; the body's own session file is never read. At this card the stream is synthetic turn text handed to `append_turn` — nothing here reads any body file, which is the ruling holding trivially. (`7b607c7e`)
- v1 persistence is files-only — no session DB. (`ba6fd355`)
- Event lines (L2-01) are a later row in the same file; the line shape carries a `kind` field now so the format does not move. (card text)
- Brief-fixed, not rulings (cheap to change later): the format is JSONL, one entry per line, a turn line being `{"kind":"turn","turn":<n>,"text":"<text>"}`; the file is `<root>/<thread>.jsonl` with `root` a plain parameter — no config field, later cards wire the real path; turns are 1-based and numbered per thread counting turn lines only; a thread id matches `[A-Za-z0-9_-]+`, anything else is an error (it is a filename); readers skip lines whose `kind` is not `turn` — that tolerance is the room the kind field buys — and malformed JSON is a loud error naming the line; every append flushes; the module lives in `crates/arx` (harness persistence is the shell's, not the engine's judging half — `arx-engine` is untouched); `resolve` is a free function reading from disk, so a handle survives respawn without a live log value; no timestamps — D07 owns clock semantics and its cards add fields when they land.

## Execution plan

### Scope 1 — transcript module

- **Files:** Create `crates/arx/src/transcript.rs`; modify `crates/arx/src/main.rs` (declare `mod transcript`).
- **Build:** the log interface later cards call: `TranscriptLog::open`, `append_turn`, and free `resolve`.
- **Steps:**
  1. From the repo root: `cargo add -p arx serde_json`, `cargo add -p arx --dev tempfile`.
  2. Types in `transcript.rs` (one module doc comment naming what the file holds — the per-thread transcript log of `eff1f01a` — and nothing else):
     - `Handle { thread: String, turn: u64 }` — `Clone`, `Debug`, `PartialEq`, `Serialize`, `Deserialize` (later cards put handles in the state object header).
     - a serde line type for the wire shape `{ kind, turn, text }` — deserialization tolerant of unknown fields (no `deny_unknown_fields`; L2-01 widens rows in place).
     - `TranscriptError`, one variant per failure with a Display message: bad thread name (naming it), io (carrying path and source), unknown thread (no log file, carrying the resolved path), unresolvable turn (carrying thread and turn), malformed line (carrying path and line number).
     - `TranscriptLog { thread, path, file, next_turn }` — the struct exposes no truncate, no delete, no rewind; append-only is the whole interface.
  3. `TranscriptLog::open(root: &Path, thread: &str) -> Result<TranscriptLog, TranscriptError>`: validate the thread id; create `root` if missing; open `<root>/<thread>.jsonl` in create+append mode; recover `next_turn` by scanning existing lines — max `turn` among `kind == "turn"` lines plus one, 1 for an empty or new file, non-turn kinds skipped, malformed JSON an error.
  4. `append_turn(&mut self, text: &str) -> Result<Handle, TranscriptError>`: serialize the turn line, write it with a trailing newline, flush, mint and return the handle, bump `next_turn`.
  5. `resolve(root: &Path, handle: &Handle) -> Result<String, TranscriptError>`: validate `handle.thread` with the same id rule; open the thread's file read-only (unknown-thread error if absent); scan lines skipping non-turn kinds; return the matching turn's text; unresolvable-turn error otherwise.
  6. `#[cfg(test)]` unit tests in `transcript.rs`: bad thread id rejected on open and on resolve; fresh file starts at turn 1; recovery counts only turn lines; malformed line errors with its line number.
- **Verify:** `cargo build` — green. `cargo test -p arx` — new unit tests green, all pre-existing config tests unchanged and green.

### Scope 2 — done-when proofs

- **Files:** Create `crates/arx/tests/transcript_log.rs`.
- **Build:** the card's done-check, executable. Each test uses `tempfile::tempdir()` as `root`.
- **Steps:** one integration test per clause, plus two seams the module claims:
  1. `synthetic_stream_lands_n_entries` — append N=10 synthetic turns with awkward text (multi-line, unicode, embedded quotes); the file has exactly 10 lines, every line `kind == "turn"`, turns 1..=10 in order.
  2. `every_handle_resolves_to_its_turn` — collect the 10 handles as minted; `resolve` each against `root`; every result equals the exact text appended, awkward cases included.
  3. `window_truncation_leaves_log_byte_identical` — maintain a `Vec<String>` in-memory window alongside the appends; read the log file's bytes; truncate the window to 2 entries; read the bytes again; assert byte-identical. The log type offers no method the truncation could even reach — this pins it at run time.
  4. `second_thread_gets_own_log_handles_never_cross` — interleave appends to threads `alpha` and `beta` with distinct texts and identical turn numbers; every alpha handle resolves to alpha text and every beta handle to beta text; a handle naming thread `gamma` errors unknown-thread; a handle with a turn past beta's last errors unresolvable-turn.
  5. `reopen_continues_numbering` — open `alpha`, append 2 turns, drop the log, reopen, append 1 more: turns are 1, 2, 3 and all three handles resolve.
  6. `foreign_kind_row_is_skipped` — append a turn, then write a line `{"kind":"event","detail":"placeholder"}` straight into the file; reopen and append: numbering continues from turn lines only, prior handles still resolve, and the foreign line survives byte-for-byte — the file is never rewritten.
- **Verify:**
  - `cargo build` — green on both crates.
  - `cargo test -p arx` — everything green: the 6 new integration tests, the new transcript unit tests, and the pre-existing config suite untouched.
  - `cat` the log file produced by any one test run is not required; byte-level assertions live inside tests 3 and 6.

## Out of scope

Done means the Verify commands above pass — nothing more. No body, no SDK, no child process; the stream is synthetic. No spawn ledger (L1-07), no state object (L2-06), no event-line kinds beyond the room the `kind` field leaves (L2-01 owns them), no engine change, no config field, no CLI subcommand, no timestamps (D07's cards), no rotation or GC, no cross-machine sync — machine-local by ruling. Rewind mechanics do not exist here; the log's indifference to them is the deliverable. No commits — the working tree is the deliverable. No inline comments; the one module doc comment named in scope 1, step 2 is the exception.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-01-l1-02-transcript-log.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
