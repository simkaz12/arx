# Terminal, phase D: turns, the tool fold, markdown, the palette

Class: polishing brief under the owner's working-mode ruling (store decision `b51158b9`), the same class as phases A to C: one bounded hand-do brief, the diff left uncommitted for the owner to commit and use. Not a card. `gate_run: yes`.

Phase C (`2026-09-08-terminal-c-parties-and-split-page.md`) left main's page as a flat list of lines: an operator line is `> ` plus bold, main's text is markdown-lite where `**bold**` renders as the same bold, every gate verdict and every admission result is its own dim line, and the big view scrolls by row. The owner used it for a day (2026-09-08, two screenshots) and ruled the shape below. Vocabulary is `CONTEXT.md`'s; "turn" here means one unit on main's page, not a model turn.

## Problem / Solution

Reading main's page, the operator cannot tell at a glance which lines they typed and which main wrote, because the two share bold and nothing else. A run of fourteen tool calls is twenty-eight dim lines that say which tool ran but never which file, so the page scrolls away under them. Normal mode has vim keys, but they only scroll rows and only when the big view already has focus, so after Esc from typing nothing moves. Main's markdown is half rendered.

The fix: main's page becomes a list of turns. An operator turn, a main turn, a notice, or a run of tool calls folded into one line reading `· 14 tools used`. Each role has its own indent and colour, a dashed rule sits between turns, and a gutter bar marks the turn the cursor is on. Esc from insert lands on the big view, `j`/`k` walk the turns, Enter opens a fold and, on a call inside it, opens that call's trace. Main's text goes through a real markdown renderer. The colours are arx's own, olive and terracotta, pinned by hex and overridable in `arx.toml`.

## Decisions locked

- Esc from insert mode focuses the big view on the current main; `i` from anywhere enters insert as today.
- On main's page `j`/`k` move a cursor over stops, not rows. A stop is an operator turn, a main turn, a notice, a folded tool run, or, when the run is expanded, one call line inside it. `gg`/`G` go to the first and last stop. `Ctrl-d`/`Ctrl-u` move the view by half the pane height and move the cursor with it, to the first stop fully visible after a move down and the last after a move up. Cursor on the last stop follows new turns; anywhere else it stays.
- Consecutive allowed tool calls fold into one turn, `· N tools used`. Enter on it expands in place to one line per call: tool, subject, outcome, size, handle. Enter on a call line whose result was externalized opens that handle as a markdown document in the big view; Esc returns to the page with the cursor where it was. Enter on any other stop enters insert mode, as today.
- A denied or asked gate verdict, a nudge, and every other event stay their own dim line, never folded.
- Layout per role: operator at indent 0 with `>` in the operator colour and bold text; main at indent 2, markdown rendered, default foreground; tools at indent 4, dim, in the tool colour. Two gutter columns before the indent: the cursor bar and the role glyph.
- A dashed rule between every pair of turns, in the rule colour. The bar `▎` beside every line of the cursor's stop.
- Palette, pinned by hex, not by ANSI slot: olive `#8f8f4f` for `>`, the rules and the bar; terracotta `#b0503c` with the dim modifier for tool lines; the terminal's own foreground for body text and background everywhere. Overridable per role under `[terminal.palette]` in `arx.toml`; the override accepts `#rrggbb` or a ratatui colour name.
- Markdown through `ratskin` (termimad under it), rendered at the pane's width and re-rendered on resize. `tui-markdown` is the named fallback, taken only if the tripwire in scope 5 fires.
- The subject of a tool call reaches the front on a new `TerminalMsg::ToolCall { tool, subject }`, sent where main's own tools cross the gate. `events.rs` does not change; the locked event kinds stay seventeen.
- Hand and judge pages keep phase C's row scrolling and split page untouched. Only main's page gets the turn cursor.

## Execution plan

The working tree carries uncommitted phases A to C, the `list_files` tool, and earlier polishing diffs. Build on them. Scopes run in this order; each leaves `cargo build -p arx` green.

### 1. Turns on the page

- **Files:** Modify `crates/arx/src/panes/state.rs`; Modify `crates/arx/src/panes/ui.rs` and `crates/arx/src/panes/app.rs` only where they match on `PageLine`.
- **Build:** `PageLine` (state.rs:34) becomes `Turn`, the unit main's page holds:

  ```rust
  pub enum Turn {
      Operator(String),
      Main(String),
      Notice(String),
      Tools { calls: Vec<ToolCall>, expanded: bool },
  }
  pub struct ToolCall {
      pub tool: String,          // bare verb: read_file, list_files, dispatch, recall …
      pub subject: Option<String>,
      pub outcome: Option<String>,   // inline | externalized | …, from Admission
      pub bytes: Option<u64>,
      pub handle: Option<String>,    // threads/main.traces/230-5.md
  }
  ```

  `Block(Vec<String>)` goes; a code block is markdown and scope 5 renders it.
- **Steps:**
  1. In `apply_event` (state.rs:252): a `Gate` with verdict `allow` whose tool starts with `mcp__arx__` opens a `ToolCall` with the prefix stripped. If the page's last turn is `Tools` and not expanded, push onto it; otherwise push a new `Tools { calls: vec![call], expanded: false }`. A `Gate` with any other verdict, and every other event, pushes `Notice(event_line(ev))` as today, which also ends the current run.
  2. An `Admission { tool, outcome, bytes, handle }` fills the last open `ToolCall` whose `tool` matches and whose `outcome` is `None`. No match: push a `Notice` with the current `event_line` text, so nothing is lost.
  3. In `apply_terminal` (state.rs:188): `TerminalMsg::ToolCall { tool, subject }` (scope 2) fills `subject` on the last open call whose `tool` matches. Before scope 2 lands, the arm is absent and the field stays `None`.
  4. `Chunk`/`TurnEnd` push `Main` as today; the operator line pushes `Operator` as today (state.rs:177).
  5. Update every `match` on `PageLine` in ui.rs and app.rs to the new variants, rendering `Tools` for now as one dim line `· N tools used` and, when expanded, one dim line per call, so scope 1 builds and the page still draws.
- **Verify:** unit tests in state.rs's existing `#[cfg(test)]` module: three allow gates with their admissions become one `Tools` turn of three calls with outcomes and handles filled; a `deny` gate between two allows splits the run into two `Tools` turns with a `Notice` between; an admission with no open call becomes a `Notice`. `cargo test -p arx --bin arx panes::` green.

### 2. The subject over the terminal channel

- **Files:** Modify `crates/arx/src/terminal.rs`, `crates/arx/src/main.rs`.
- **Build:** `TerminalMsg::ToolCall { tool: String, subject: Option<String> }`. Sent from `gate_call` (main.rs:1461) right after it records an `allow` verdict, only when the gated tool name starts with `mcp__arx__`, with `tool` the name minus that prefix and `subject` extracted from the call's input by the same subject-key map the `Ask` message uses (gate.rs, the `match tool` block at lines 68 to 82, keyed on the full `mcp__arx__…` name). The line front's `render` arm for the variant is a no-op, so piped mode and every integration test print what they printed before.
- **Steps:**
  1. Add the variant to `TerminalMsg` (terminal.rs:7). Add the no-op arm to the line front. Add the arm in panes `apply_terminal` from scope 1 step 3.
  2. In `gate_call`, after the allow branch writes its `Event::Gate` (main.rs:1498) and before returning, render the message through the same `front_cell` the `Ask` branch uses (main.rs:1516). Hand tool calls (names without the prefix) send nothing.
  3. Test in `main.rs`'s own `#[cfg(test)]` module, never in an included file: a `gate_call` for `mcp__arx__read_file` with `{"path": "src/x.rs"}` and an allowing rule renders exactly one `ToolCall { tool: "read_file", subject: Some("src/x.rs") }` on a recording front; a `Bash` call from a hand renders none.
- **Verify:** that test; `cargo test -p arx` totals unchanged except for the added tests; `cargo run -p arx -- check-config ./arx.toml` output unchanged.

### 3. Focus, the cursor, the keys

- **Files:** Modify `crates/arx/src/panes/app.rs`, `crates/arx/src/panes/keys.toml`, `crates/arx/src/panes/keys.rs` if a new action name needs registering.
- **Build:** Esc from insert sets `focus = FocusTarget::BigView(PanelId::Main)` (the `Mode::Normal` transition at app.rs:709). A cursor per main page, `cursor: usize` over the page's stops, stored beside the page in `SessionState` or keyed by `PartyKey::Main(n)` in `App`, never reset while the session lives. The stop list is derived from the turns: one stop per `Operator`, `Main`, `Notice`, one per folded `Tools`, one per call of an expanded `Tools`.
- **Steps:**
  1. In `nav_down`/`nav_up` (app.rs:546, 553): when focus is `BigView(Main)`, move the cursor by one stop and clamp; the existing `pane_scroll_*` path stays for hand and judge pages and for the document view of scope 6.
  2. `nav_top`/`nav_bottom`: first and last stop on main's page. Landing on the last stop sets following, so new turns move the cursor.
  3. `nav_page` (app.rs:583): on main's page move the view by half the pane height (the height comes from the metrics the ui already reports back) and set the cursor to the first stop whose lines are all visible after a move down, the last such stop after a move up.
  4. Enter in normal mode (app.rs:656 decides insert): when focus is `BigView(Main)` and the cursor stop is a folded `Tools`, toggle `expanded` instead; when it is a call line with a `handle`, open the document (scope 6); otherwise fall through to insert as today. Add `expand = ["Enter"]` under `[normal]` in keys.toml for the help overlay, ordered before `insert`, and make the dispatcher try it first on main's page.
  5. The view offset for main's page is derived from the cursor every frame: the cursor's lines are kept fully on screen, moving the offset by the least amount; if the stop is taller than the pane, its first line sits at the top.
  6. `scroll_states` keeps serving hand and judge panes; main's `Pane::Single` entry is no longer read for scrolling.
- **Verify:** unit tests in app.rs: a page of operator, main, tools(3), main has four stops; `j` three times lands on the tools stop and Enter expands it to make six stops with the cursor on the first call; `k` from the first stop stays; `G` then a new turn arriving moves the cursor to it; Esc from insert yields `BigView(Main)`. `cargo test -p arx --bin arx panes::` green.

### 4. Layout, the rule, the gutter, the palette

- **Files:** Create `crates/arx/src/panes/palette.rs`; Modify `crates/arx/src/panes/mod.rs` (declare it), `crates/arx/src/panes/ui.rs`, `crates/arx/src/config.rs`.
- **Build:** `Palette { operator, rule, gutter, tool }`, each a `ratatui::style::Color`, defaults `Rgb(0x8f,0x8f,0x4f)` for the first three and `Rgb(0xb0,0x50,0x3c)` for `tool`. `page_lines` (ui.rs:75) renders turns to lines with this layout, widths in cells:

  ```
  col 0     cursor bar `▎` in gutter colour, or space
  col 1     space
  col 2     role glyph: `>` operator colour bold; space for main; `·` tool colour for a tool line
  col 3     space
  indent    0 for operator, 2 for main, 4 for tool lines, then the text
  ```

  Between every pair of turns one line: two spaces then `─ ─ ─ …` in the rule colour to the pane's right edge. Operator text bold, default foreground, wrapped at the pane width under the indent. Main text through the markdown renderer (scope 5; until then the existing `render_markdown`) at width minus the indent. A folded tools turn is one line `· N tools used`. An expanded one is one line per call: `· {tool}  {subject}  → {outcome} {size} → {handle basename}`, empty fields omitted, size as `NNNb` under 1000 bytes and `N.Nk` above. Tool lines get the dim modifier on top of the tool colour. Notices keep `DarkGray`.
- **Steps:**
  1. Write palette.rs with the struct, `Default`, and `fn from_config(&Option<PaletteConfig>) -> Palette` accepting `#rrggbb` or any name ratatui's `Color: FromStr` accepts; a bad value is a config error at load, not a fallback.
  2. `TerminalConfig` (config.rs:45) gains `pub palette: Option<PaletteConfig>` with `#[serde(deny_unknown_fields)]` and four optional string fields `operator`, `rule`, `gutter`, `tool`; flatten it into `Config` beside `terminal_keys` (config.rs:311) as `terminal_palette`. Tripwire: before adding the field, grep `terminal_keys` across `crates/arx/src` and `crates/arx/tests`; every struct literal found gets the new field in the same edit, and the count is reported in the verdict.
  3. Thread the palette from config into `App` at panes construction (mod.rs) and read it in ui.rs; replace the inline `Color::` literals on operator, rule, gutter and tool lines with palette fields. Leave the panel borders, status line and hand pages on their current colours.
  4. `page_lines` returns, besides the lines, the line range of every stop, which scope 3's view offset and the bar need.
- **Verify:** unit tests through `TestBackend` at 110x36: a page of operator, main, tools(2), main with the cursor on the main turn renders a fixed frame with the bar on the main turn's lines, `>` on the operator line, the rule between turns, the fold line at indent 4; the same page with the tools turn expanded and the cursor on the second call shows the bar on that line only; `Palette::from_config` on `operator = "#112233"` gives `Rgb(0x11,0x22,0x33)`, on `tool = "red"` gives `Color::Red`, on `rule = "olive"` errors. `check-config` on the repo's `arx.toml` prints exactly what it printed before; on a copy with `[terminal.palette] tool = "#b0503c"` it loads.

### 5. Markdown rendered

- **Files:** Modify `crates/arx/Cargo.toml`, `Cargo.lock`, `crates/arx/src/panes/ui.rs`.
- **Build:** `render_markdown(text, width) -> Vec<Line<'static>>` backed by `ratskin`, replacing the hand-rolled body at ui.rs:416. Every caller passes the width it draws into: main turns at pane width minus 2 for the indent plus the 4 gutter cells, hand brief and turns panes and `render_trace_turns` (ui.rs:151) at their own widths, the document view of scope 6 at the big view's width. The skin uses no RGB of its own: headings bold, emphasis italic, code in the terminal's cyan slot as today, tables with box borders, lists with the existing bullet. Rendered lines are cached per turn keyed on `(turn index, width)` and dropped on resize.
- **Steps:**
  1. Add `ratskin = "0.3"` to `[dependencies]`; run `cargo build -p arx` and record the resolved `ratskin` and `termimad` versions in the verdict.
  2. Tripwire, before touching ui.rs: render this fixture at widths 60 and 100 through ratskin in a scratch test and print the frames:

     ````md
     ## Repo root
     Two crates, **one** binary.
     - crates/arx
       - src/main.rs, `~4980` lines
       - src/panes/
     - crates/arx-engine
     | crate | role |
     |---|---|
     | arx | the binary |
     | arx-engine | the store |
     ```rust
     fn main() {}
     ```
     ````

     If the table comes back without box borders and column alignment, or the nested list loses its indent, STOP on ratskin: remove it, add `tui-markdown` (ratatui-core 0.1, which ratatui 0.30 re-exports) and continue with it, and say so in the verdict with the failing frame pasted. Do not patch around the bridge.
  3. Replace the body of `render_markdown`, keep the name, add the width parameter, fix every caller.
  4. Delete the markdown-lite helpers that nothing calls any more.
- **Verify:** the fixture above at width 60 through `TestBackend` matches a fixed frame committed with the test, borders and all; a plain paragraph longer than the width wraps under the indent, not at column 0; resizing 110x36 to 80x36 re-renders main's page without a panic and without stale lines. `cargo test -p arx --bin arx panes::` green.

### 6. The trace document

- **Files:** Modify `crates/arx/src/panes/app.rs`, `crates/arx/src/panes/ui.rs`, `crates/arx/src/panes/state.rs`.
- **Build:** Enter on an expanded call line with a `handle` reads the file at that handle relative to the run's cwd (`threads/main.traces/230-5.md`, the same root phase C's `read_trace` resolves under), renders it as markdown at the big view's width, and shows it in the big view titled with the handle. `j`/`k`, `gg`/`G`, `Ctrl-d`/`Ctrl-u` scroll rows through the existing `pane_scroll_*` path under a new `Pane::Doc` key on the main party; Esc returns to the page with the cursor and expansion state untouched. An unreadable file pushes `Notice("trace unreadable: <path>: <error>")` onto the page and stays on the page.
- **Steps:**
  1. `Pane::Doc` on the `Pane` enum (app.rs:70); `App` holds `doc: Option<Doc { title: String, body: String }>` for the main party; `draw_big_view` (ui.rs:312) draws it through `draw_scrollable_pane` when set.
  2. Enter dispatch from scope 3 step 4 sets it; `back` (Esc) clears it before it would leave the big view.
  3. The read is synchronous on the UI thread, once per open, as phase C reads a hand's trace.
- **Verify:** unit test: a page with an expanded tools turn whose second call has a handle pointing at a temp file; Enter on that stop sets `doc` with the file's text; Esc clears it and the cursor is still on the second call; a handle to a missing file pushes the notice and leaves `doc` `None`. `TestBackend` frame of the document view at 110x36 for a three-line markdown file.

## Out of scope

- `prompts/main.md` still says main does not read at scale and names only a whole file as a bounded read; the `list_files` line in it is its own polishing item, not this brief.
- Hand and judge pages: no turn cursor, no fold, no palette beyond what they inherit through `render_markdown`. Their split page, scroll memory and steer questions stay parked as phase C left them.
- No `/` search on the page, no yank, no mouse changes.
- `events.rs`, `crates/arx/tests/`, `sidecar/`, `docs/`, `prototypes/`: untouched.
- `arx.toml` in the repo: untouched; the palette table is documented here and defaults apply.

Done means: the six scopes' unit tests pass, the totals of `cargo test -p arx` before and after differ only by the tests added here, `cargo test --workspace` is green, `check-config` on the repo's `arx.toml` is byte-identical, piped mode with `front = "panes"` prints what it printed before, and the tmux capture below shows the page as drawn in the Decisions.

## Bounds

- Edits under `crates/arx/src/panes/`, plus `terminal.rs` (one variant and the line front's ignore arm), `main.rs` (the send in `gate_call` and its test), `config.rs` (the palette table), `crates/arx/Cargo.toml` and `Cargo.lock` (one dependency). Nothing else.
- Never `git stash`, never `git checkout --` a file, never revert a hunk you did not write. The owner keeps a live `arx run` open in this repo; never run `arx run` from the repo root and never write `threads/main.jsonl`. A live check builds a temp dir outside the repo holding a copy of `arx.toml` with `sidecar`, `instructions` and `agents` pointed at the repo by absolute path, keeps `[store] root`, and removes the dir after.
- No inline `#[cfg(test)]` module in a file an integration test `#[path]`-includes (`claude_code.rs`, `container.rs`, `events.rs`, `gate.rs`, `state_object.rs`, `transcript.rs`). The scope 2 test lives in `main.rs`'s module.
- Stub images: after any edit under `crates/arx/tests/fixtures/*-stub/` (none expected) `docker rmi` the tag first. `cargo test -p arx` needs the Docker daemon.

## Verify

1. Before any edit: `cargo test -p arx` green; record totals per binary.
2. After every scope: `cargo build -p arx` and `cargo test -p arx --bin arx panes::`.
3. At the end: `cargo test -p arx` with the same totals plus the added tests, listed by name; `cargo test --workspace`; `check-config` diff empty.
4. Live under tmux against the stub sidecar with `front = "panes"` from a temp dir as above: type a line, watch the reply render as markdown with a heading and a list, trigger tool calls through the stub so a fold appears, Esc, `j` onto the fold, Enter, `j` onto an externalized call, Enter, read the document, Esc, `G`, `Ctrl-u`, `gg`. Paste the captures. Kill the tmux session when done.
5. Piped mode against the same config prints exactly what it printed before this phase.

## Verdict

Return as text: what was built per scope, the file list with line counts, the `TerminalMsg` addition as landed, the tripwire outcome of scope 5 with the crate and version that stayed, the `terminal_keys` literal count from scope 4, the test totals before and after per binary, the last 10 lines of `cargo test -p arx`, the tmux captures fenced, the piped-mode diff (empty), `git status --short`, three things you would change next, and the codex gate outcome with dispositions and log path. Fence field: `gate_run: yes`.

## Review handoff

Polishing class, as phases A to C: dispatch hand-do on this brief; it self-runs the codex gate on the behaviour-change axis and reports dispositions with the log path. The diff stays uncommitted for the owner.
