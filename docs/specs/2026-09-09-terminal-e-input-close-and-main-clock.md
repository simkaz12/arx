# Terminal, phase E: input editing, closing rows, main's clock and tokens

Class: polishing brief under the owner's working-mode ruling (store decision `b51158b9`), the same class as phases A to D: one bounded hand-do brief, the diff left uncommitted for the owner to commit and use. Not a card. `gate_run: yes`.

Phase D (`2026-09-09-terminal-d-turn-cursor-and-markdown.md`) landed the turn cursor, the tool fold, ratskin markdown and the palette. The owner used the result on 2026-09-09 and reported four gaps. Vocabulary is `CONTEXT.md`'s; "incarnation" below is one row of the main panel, one main between resets; "row" is one entry in the main, hands or judges panel.

## Problem / Solution

The input box is a bare string: typed characters append at the end, backspace removes the last one, Left and Right are dropped on the floor (`handle_insert_key`, app.rs:938, falls through `_ => {}`), and the box height counts literal newlines only (ui.rs:275), so a long line word-wraps inside a three-row box and the wrapped rows are clipped. After a few resets the main panel lists every incarnation and nothing removes one; the hands and judges panels keep every settled row the same way, and there is no way to stop a running hand from the front. A main row reads `main 2  claude-opus-5  live` with no clock and no token figure, while a hand row already ticks `4.2s` every redraw.

The fix: a cursor in the input with arrow, Home, End and Delete editing; one wrap function that decides the box height, the drawn rows and the cursor cell together, with the box growing to a third of the screen and scrolling past that; `x` on a panel row that removes a settled row or, on a running hand or judge, kills it through a new `/kill <id>` operator line that the harness intercepts beside `/reset`; and a main row that shows the running turn's elapsed time ticking and the incarnation's token total, fed by a new `TerminalMsg::Usage` sent once per turn.

## Decisions locked

- **Cursor.** `App` gains `cursor: usize`, a char index into `input`, beside the string. Typed characters insert at the cursor; Backspace deletes the char before it; Delete deletes the char at it; Alt-Enter inserts `\n` at it. Left and Right move one char and stop at the ends. Home and End (also `Ctrl-a`, `Ctrl-e`) go to the start and end of the visual row the cursor is on. Submit clears the string and puts the cursor at 0. History recall and Tab completion replace the buffer as today and put the cursor at the end.
- **Up and Down in insert.** `Up` stays bound to `history_up` and `Down` to `history_down`, and the actions apply a row rule first: when the cursor is not on the first visual row, Up moves it up one row keeping the column where the row allows; when it is not on the last row, Down moves it down one row; otherwise the action recalls history as today.
- **One wrap.** `wrap_input(text: &str, width: usize) -> Vec<Row>` in the panes module, where `Row` is a char range `start..end` of the buffer. Word wrap at spaces, hard break at `width` when a word is longer, `\n` forces a break and is not drawn. Width is the box's inner width. The height, the drawn lines and the cursor `(row, col)` all come from this one call; `draw_input` draws the rows itself and stops using `Paragraph`'s `Wrap`. Column width in cells through `unicode-width` as the rest of ui.rs measures.
- **Height and viewport.** The box is `rows + 2` tall, clamped to `3..=cap + 2` where `cap = max(1, area.height / 3)` for the area the input and big view share (ui.rs:277). The big view keeps `Constraint::Min(3)` and shrinks. When `rows > cap`, the box shows `cap` rows and keeps the cursor's row visible by moving the top row the least amount, the same rule main's page uses for its cursor; the scroll top persists between frames.
- **Keys.** `[insert]` gains `left = ["Left"]`, `right = ["Right"]`, `home = ["Home", "Ctrl-a"]`, `end = ["End", "Ctrl-e"]`, `delete = ["Delete"]`. `key_token` (keys.rs:156) gains tokens for `Home`, `End` and `Delete`; `Left` and `Right` already exist. `[normal]` gains `close = ["x"]`, ordered after `back`. The help overlay lists them as it lists the rest.
- **`x` on a panel row.** Acts only when focus is on the main, hands or judges panel, on the selected row; in the big view, in insert and in a modal it does nothing. Hands and judges panel, row settled (delivered, killed, or a judge with a malformed schema): the row and its page are removed at once, no confirmation. Row not settled (running or stopped): a confirmation modal `kill h3? Enter kills, Esc keeps`, through the existing `[modal]` context; on confirm the front pushes the line `/kill h3` onto `outbox` and the row's status cell reads `killing` until the delivery settles it. Main panel, ended incarnation: removed at once. Main panel, the live incarnation: nothing removed, the status line reads `live main: /reset ends it` until the next key.
- **After removal.** Selection moves to the row after the removed one, else the row before, else the panel is empty. If the big view was showing the removed party's page it shows the newly selected row's page, or the live main's page when the panel is empty. A `TerminalMsg` or `Event` that names a removed hand id is dropped, never re-creates the row.
- **`/kill <id>` in the harness.** Intercepted where `/reset` is (main.rs:3146), before the line becomes a turn: `id` is the rest of the line trimmed; the harness calls the same ledger path the `kill` MCP tool handler calls (main.rs:808, `SpawnLedger::kill`, ledger.rs:483), so the `Control::Kill`, the `Killed` mark and the killed delivery are the ones that exist. The line is not written to the transcript as a turn; the killed delivery is the record main sees. An unknown or already-settled id gets the error text back to the front the way a bad `/model` argument does, and nothing else happens. Typing `/kill h3` by hand does exactly what `x` does.
- **Main row clock.** `MainIncarnation` gains `started: Instant`, `turn_started: Option<Instant>` and `tokens: u64`. `turn_started` is set when an operator turn is pushed onto the incarnation's page (state.rs:177) or when a `Chunk` arrives with no turn open, and cleared on `TurnEnd`; a `Reset` that ends the incarnation clears it. The row's status cell: `running · 12.4s · 38.2k tok` while a turn is open, elapsed since `turn_started`, ticking on the existing 33 ms redraw (mod.rs:27) exactly as `HandRow::status_cell` ticks; `live · 38.2k tok` when idle; `ended · 38.2k tok` when ended. Token figure formatted `NNN tok` under 1000, `N.Nk tok` under a million, `N.NM tok` above; `0 tok` before any usage.
- **Token total.** The incarnation's `tokens` is the sum over its turns of `input_tokens + output_tokens + cache_creation_input_tokens + cache_read_input_tokens` as the body reports them, the same basis Claude Code's `/cost` calls total tokens. Occupancy stays where it is, in the session panel. Missing keys count as zero.
- **`TerminalMsg::Usage { input: u64, output: u64, cache_creation: u64, cache_read: u64 }`.** Sent from `run_turn_cycle` where `outcome.usage` is in hand (main.rs:2031), before the `Status` message of that cycle, reading the keys the way `transcript::occupancy` reads them. `LineFront` ignores it. The sidecar reports usage once per turn on the `result` line and is not touched, so the figure moves at turn end while the clock moves live.
- `events.rs` does not change; the locked event kinds stay as they are.

## Execution plan

The working tree carries uncommitted phases A to D and earlier polishing diffs. Build on them. Scopes run in this order; each leaves `cargo build -p arx` green.

### 1. The cursor and the wrap

- **Files:** Modify `crates/arx/src/panes/app.rs`, `crates/arx/src/panes/ui.rs`, `crates/arx/src/panes/keys.rs`, `crates/arx/src/panes/keys.toml`.
- **Build:** the cursor, the five insert actions, the row rule on `history_up`/`history_down`, `wrap_input`, the box height and viewport, `draw_input` drawing pre-wrapped rows and placing the terminal cursor from the same rows.
- **Steps:**
  1. Add `cursor` to `App`; make every mutation of `input` cursor-aware (`Char(c)`, `Backspace`, the `newline` action, `submit`, history recall, completion). Grep `self.input` in app.rs first and list every site in the verdict.
  2. Register the five insert actions in `INSERT_ACTIONS` (keys.rs) and bind them in keys.toml; add the three tokens to `key_token`.
  3. Write `wrap_input` with its own unit tests before touching ui.rs.
  4. Rewrite the height at ui.rs:275 and `draw_input` (ui.rs:339) on top of `wrap_input`; add the viewport top and the least-movement rule.
- **Verify:** unit tests: `wrap_input` on an empty string, a string shorter than the width, a 3-word string that wraps once at a space, a 40-char word at width 10, and `"a\nb"`; cursor tests: typing `abc`, Left twice, typing `X` gives `aXbc` with the cursor at 2; Home and End on the second row of a wrapped buffer; Up on the first row recalls history, Up on the second row moves up; Backspace at 0 and Delete at the end are no-ops. `TestBackend` at 110x36: a 300-char input draws a box of the right height with three text rows and the terminal cursor on the last row after the last char; a 2000-char input caps at `area.height / 3` rows with the cursor row visible. `cargo test -p arx --bin arx panes::` green.

### 2. Closing rows and `/kill`

- **Files:** Modify `crates/arx/src/panes/app.rs`, `crates/arx/src/panes/state.rs`, `crates/arx/src/panes/ui.rs`, `crates/arx/src/panes/keys.toml`, `crates/arx/src/main.rs`, `CONTEXT.md`.
- **Build:** the `close` action and its four cases, the removal methods on `SessionState`, the confirm modal, the `killing` cell, the harness interception, and one line in `CONTEXT.md`'s operator surface section naming `/kill <id>` beside `/reset`.
- **Steps:**
  1. `SessionState` gains `remove_hand(id)`, `remove_judge(id)`, `remove_main(index)`; each drops the row, its page and its scroll state, and the `apply_*` paths ignore ids they no longer hold. Check what `apply_terminal` does today with an unknown hand id in `HandDelivered`/`HandStopped`; if it inserts, make it drop.
  2. The `close` action in the normal dispatcher: resolve focus and selection through `selection_mut` (app.rs:769); branch on settled or not; open the modal or remove. Reuse the existing modal type if it can carry a confirm with a pending action, else add a `Confirm` variant; do not add a second modal mechanism.
  3. On confirm: push `/kill <id>` to `outbox`, set the row's cell to `killing`. The existing killed delivery path (`HandDelivered { outcome: "killed" }`) settles it.
  4. In main.rs, the interception beside `/reset`. Tripwire: check whether deliveries are pumped while main is idle at the prompt. If a killed hand's delivery only lands at the next step boundary, so the row stays `killing` until the operator types a line, report that in the verdict with the anchor and do not add a new pump; the `killing` cell is the visible acknowledgement.
  5. The `CONTEXT.md` line, prose, in the operator surface section.
- **Verify:** unit tests: `x` on a delivered hand removes it and selects the next row; `x` on the last remaining row leaves the panel empty and the big view on the live main; `x` on a running hand opens the modal, Enter pushes exactly `/kill h1` to `outbox` and the cell reads `killing`, Esc pushes nothing; `x` on an ended main removes it; `x` on the live main removes nothing and sets the status line; a late `HandDelivered` for a removed id changes nothing. Harness test in `main.rs`'s own `#[cfg(test)]` module, never in an included file: the interception on `/kill h1` for a spawned fake hand calls the ledger kill and the recording front receives a killed delivery; `/kill zz` yields the error text and no ledger change. `cargo test -p arx --bin arx panes::` green.

### 3. Main's clock and tokens

- **Files:** Modify `crates/arx/src/panes/state.rs`, `crates/arx/src/panes/ui.rs`, `crates/arx/src/terminal.rs`, `crates/arx/src/main.rs`.
- **Build:** the three fields on `MainIncarnation`, the status cell in `draw_main_panel` (ui.rs:802), the `Usage` variant, its send in `run_turn_cycle`, the line front's no-op arm, the `apply_terminal` arm that adds the four figures to the live incarnation's `tokens`.
- **Steps:**
  1. Tripwire before adding the fields: grep `MainIncarnation {` across `crates/arx/src`; every struct literal found gets the fields in the same edit, and the count goes in the verdict.
  2. `turn_started` set and cleared as locked; a `status_cell()` on `MainIncarnation` mirroring `HandRow::status_cell`.
  3. The `Usage` variant, the send, the arms.
- **Verify:** unit tests: an operator turn then `TurnEnd` opens and closes the turn; a `Chunk` on an idle incarnation opens one; a `Usage` of `1000/200/0/300` gives `1.5k tok`; two usages sum; a `Reset` that respawns leaves the old incarnation `ended · N tok` and the new one `live · 0 tok`; the formatter on 999, 1000, 38200, 1200000. `TestBackend` frame at 110x36 of the main panel with one running incarnation matching `running · 0.0s · 1.5k tok` (elapsed measured at construction, as the hand-row tests do) and one ended. Harness test in `main.rs`'s module: a turn cycle against the fake body whose usage row carries the four keys renders exactly one `Usage` on a recording front, before that cycle's `Status`. `cargo test -p arx --bin arx panes::` green.

## Out of scope

- Mouse changes, bracketed paste, word-wise cursor moves, `/` search, yank. The mouse wheel on main's page stays the named follow-up it already is.
- Per-message token streaming: the sidecar is untouched, so no figure moves inside a turn.
- Hand and judge pages: no change beyond the `killing` cell and removal.
- `x` on the live main: `/reset` is the only way to end it, unchanged.
- `events.rs`, `crates/arx/tests/`, `sidecar/`, `prompts/main.md`, `docs/` other than this brief, `prototypes/`: untouched. `prompts/main.md` stays because main never sees `/kill`; the killed delivery it does see already exists.
- `arx.toml` in the repo: untouched.

Done means: the three scopes' unit tests pass, the totals of `cargo test -p arx` before and after differ only by the tests added here, `cargo test --workspace` is green, `check-config` on the repo's `arx.toml` is byte-identical, piped mode with `front = "panes"` prints what it printed before, and the tmux capture shows the box growing, a row removed, a running hand killed from the panel, and a main row ticking.

## Bounds

- Edits under `crates/arx/src/panes/`, plus `terminal.rs` (one variant and the line front's ignore arm), `main.rs` (the `/kill` interception, the `Usage` send, their tests), and one line in `CONTEXT.md`. Nothing else. No new dependency; `unicode-width` is already in the tree through ratatui.
- Never `git stash`, never `git checkout --` a file, never revert a hunk you did not write. The owner keeps a live `arx run` open in this repo; never run `arx run` from the repo root and never write `threads/main.jsonl`. A live check builds a temp dir outside the repo holding a copy of `arx.toml` with `sidecar`, `instructions` and `agents` pointed at the repo by absolute path, keeps `[store] root`, and removes the dir after.
- No inline `#[cfg(test)]` module in a file an integration test `#[path]`-includes (`claude_code.rs`, `container.rs`, `events.rs`, `gate.rs`, `state_object.rs`, `transcript.rs`). The harness tests live in `main.rs`'s module.
- Stub images: after any edit under `crates/arx/tests/fixtures/*-stub/` (none expected) `docker rmi` the tag first. `cargo test -p arx` needs the Docker daemon.

## Verify

1. Before any edit: `cargo test -p arx` green; record totals per binary.
2. After every scope: `cargo build -p arx` and `cargo test -p arx --bin arx panes::`.
3. At the end: `cargo test -p arx` with the same totals plus the added tests, listed by name; `cargo test --workspace`; `check-config` diff empty.
4. Live under tmux against the stub sidecar and the fake hand body with `front = "panes"` from a temp dir as above: type a line longer than the width and watch the box grow; Left, Home, type, End; Alt-Enter twice and Up, Down between rows; dispatch two hands through the stub, `x` one while running, Enter, watch `killing` settle to `killed`, `x` it again to remove it; `/reset`, then `x` the ended main; send a turn and watch the main row tick, then read the `tok` figure after it ends. Paste the captures. Kill the tmux session when done.
5. Piped mode against the same config prints exactly what it printed before this phase.

## Verdict

Return as text: what was built per scope, the file list with line counts, the `TerminalMsg` addition as landed, the `self.input` site list from scope 1, the `MainIncarnation {` literal count from scope 3, the idle-pump tripwire outcome from scope 2 with its anchor, the test totals before and after per binary, the last 10 lines of `cargo test -p arx`, the tmux captures fenced, the piped-mode diff (empty), `git status --short`, three things you would change next, and the codex gate outcome with dispositions and log path. Fence field: `gate_run: yes`.

## Review handoff

Polishing class, as phases A to D: dispatch hand-do on this brief; it self-runs the codex gate on the behaviour-change axis and reports dispositions with the log path. The diff stays uncommitted for the owner.
