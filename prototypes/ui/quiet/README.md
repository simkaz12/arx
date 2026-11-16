# quiet

A terminal front for arx that never leaves the line-stream model: no alternate screen, no
viewport, no TUI library. Piped output stays byte-identical to `crates/arx/src/terminal.rs`
today; a tty gets a status line, gutters, a small line editor, an interactive gate ask, and a
`/model` picker, all drawn in place with plain cursor moves.

## Layout

- `src/scenario.rs` — the scripted session engine. Owns the event enum and session state,
  runs on its own thread, talks to whatever is rendering over two channels (`Event` out,
  `OperatorInput` in). This is the seam the real L1-04 loop would plug into.
- `src/driver.rs` — the `--auto` player. Watches the same event stream a renderer sees and
  feeds scripted operator lines back at the right points, so a scene never advances on a
  wall-clock guess.
- `src/piped.rs` — non-tty rendering. Chunk/TurnEnd/Notice/Ask reproduce `terminal.rs`'s exact
  bytes; everything else (tool calls, dispatch, usage, checkpoint numbers) is data that
  arx already holds today but never shows an operator, so piped mode stays silent on it too,
  matching current behaviour rather than inventing a new shape for it.
- `src/tty.rs` — the interactive front: status line, gutters, the line editor, the ask and
  picker widgets, streaming with a line-buffered markdown pass.
- `src/snapshot.rs` — `--snapshot`'s reviewer view: runs the same engine, renders the tty
  vocabulary as plain grouped text, ANSI stripped.
- `src/editor.rs`, `src/format.rs`, `src/state.rs` — line editing, text shaping shared by
  the tty and snapshot renderers, and the fixed session facts from `SCENARIO.md`.

## Run it

```
cargo build
./target/debug/quiet                  # interactive, real keys
./target/debug/quiet --auto           # interactive, scripted operator, real pacing
./target/debug/quiet --auto --fast    # same, no delays
./target/debug/quiet --auto --fast | cat   # forces piped mode
./target/debug/quiet --snapshot       # plain-text dump of the whole session, scene by scene
```

A finished `--auto` run sits at the idle prompt afterward, same as a real session that has
run out of things to say — it does not exit on its own.

## Two proposals

`SCENARIO.md` asks two things of every prototype that are not how arx behaves today; both are
implemented here, not just described.

- **A transient provider error stops being fatal.** Today `overloaded_error` and its kin kill
  the process. Here it prints as a notice, retries once after a wait, and the turn completes.
  This is a real behaviour change, not a rendering choice — worth grilling before it lands
  anywhere real, since retry-once could mask a genuinely broken provider.
- **An unrecognized slash command stops going to main as chat.** Today `/reste` would be sent
  as a turn. Here it prints a hint naming the three known commands and the line goes nowhere.
  Smaller change, same caveat: it's new behaviour, not just new paint.

## Known gaps

- Inline code coloring keeps the backticks (`` `code` `` in cyan, not bare colored text) so the
  ANSI-stripped snapshot still shows what was marked as code. A cleaner look drops the
  backticks and leans on color alone, which only works with color.
- Markdown-lite resolves one line at a time, buffered until the line's newline arrives (or the
  turn ends, or an abort flushes it). A reply with one very long paragraph pops in as a single
  block rather than word by word; only line-structured text (the bulk of what main writes)
  streams smoothly throughout.
- The `/model` picker only gets exercised end to end under a real keyboard. `--auto`'s tty path
  shows the picker rendering (bare `/model` opens it) and then resolves it with a synthetic
  `/model claude-sonnet-5` line rather than simulated arrow/digit presses, so the picker's own
  key handling is proven by manual use, not by the scripted run.

## If this direction wins

- Give inline code a real choice between "keep the backticks" and "color only, no tty
  fallback needed" instead of splitting the difference by default.
- Make the picker's arrow/digit navigation part of the scripted `--auto` path too, with
  synthetic key events rather than a direct command, so a reviewer sees the actual keys work
  without touching a keyboard.
- Move the two behaviour proposals (recoverable provider error, unknown-command hint) to a
  decision of their own before they ride along with whatever terminal direction wins — they
  are not specific to this prototype and should not get adopted by default just because this
  one happened to implement them.
