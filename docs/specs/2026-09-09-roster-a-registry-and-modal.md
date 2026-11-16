# Roster A — the model registry, the roles table, and the tabbed `/model`

## Problem / Solution

Choosing what model fills a position is friction. A position's body and model are welded into the artifact that defines the position: main's in `[main]` of `arx.toml`, every other role's in its own `agents/*.md` frontmatter. So `hand-ex`, `hand-cx` and `hand-ds` are one position — read-only explorer — written out three times, once per body, and the only way to run the explorer on a different model is to dispatch a differently-named agent. Nothing can be rebound after startup: `config::load` runs once (`main.rs:2720`), the resulting `Resolved` is carried through the whole session, and no file is ever re-read. The single exception is main's own model through `/model`, which mutates a `String` on `cmd_run`'s stack (`main.rs:2826`) and dies with the process.

The fix is a registry and a roster. A `[[models]]` registry in `arx.toml` lists every available model by id, each entry carrying the body that runs it — so a position names a model id and the body follows, rather than the operator pairing the two by hand. A `[roles]` table gives each position its default id. Pins made at runtime persist in a file outside the repository and outlive the process. `/model` grows from a flat picker into a tabbed one, one tab per position, navigated with the keys the panes front already binds.

This is card A of wave 1. Wave 1 is the claude-code seat for main; the registry it ships carries every model that runs today, across all three bodies, because the hand and judge seats already run codex and native. Card B adds the `bolt` and `hand-do` positions on top of a working mechanism. Waves 2 and 3 put codex and a provider in main's seat, which is a different and much larger problem (a trait over `ClaudeCodeMain`, a second sidecar driver, and a protocol question `e652ce3a` records as unverified).

## Decisions locked

- **A position names a model id; the body follows from the registry.** No `(body, model)` pair at any choosing surface. `/model hand-ex gpt-5.6-sol` is the whole gesture.
- **The registry is `[[models]]` in `arx.toml`.** Each entry: `id`, `body`, and the body's own extras — `image` for a containerized body, `provider` for `native`. Adding a model is a config edit, never a release.
- **The registry seeds with every model that runs today**, across all three bodies: the claude-code ids from `main.models`, `gpt-5.6-sol` (codex) and `deepseek-v4-flash` (native, provider `deepseek`). Shipping a claude-code-only registry would regress the codex judge and delete the two cross-model hands outright; waves 2 and 3 are about main's *seat*, not the registry's contents.
- **`main.models` is retired.** The registry is the sole availables list for every position. `ConfigError::ModelNotInModels` and its siblings move to registry membership.
- **`[roles]` in `arx.toml` holds each position's default model id.** `agents/*.md` frontmatter keeps the position only — prompt body, `tools`, `write`, `env`, `window`, `steps`, `nudge`, `checkpoint`, `reserve`. `body`, `model`, `provider` and `image` leave the frontmatter entirely.
- **`hand-cx` and `hand-ds` collapse into `hand-ex`.** They were the same position forked by body; they become `/model hand-ex gpt-5.6-sol` and `/model hand-ex deepseek-v4-flash`.
- **Pins persist outside the repository and outlive the process.** One file under `~/.local/share/arx/roster/`, keyed by the absolute config path. `arx.toml` is never rewritten — a toml round-trip eats comments and formatting, and a personal model preference does not belong in a repo-tracked file.
- **Pin wins over the `[roles]` default, and the listing says which is which.** Every row renders as `pinned` or `from file`.
- **A dangling pin warns and falls back.** A pin naming an id no longer in the registry prints a notice on the operator surface and that position falls back to its `[roles]` default. It never refuses to start; it never runs silently.
- **`/models` does not exist.** `/model` is the single entry point. Bare `/model` opens the modal in the panes front and prints the roster in the line front. `/model <id>` still means main. `/model <position> <id>` sets a position. `/model clear <position>` drops a pin.
- **Typed forms apply immediately, in both fronts.** The Enter-commit belongs to the modal alone.
- **The modal is tabbed, one tab per position.** `j`/`k` moves the cursor, and the cursor *is* the pending choice. `h`/`l` walks tabs. Enter applies every tab at once. Esc discards all pending changes. A tab never visited cannot change.
- **Each tab's list opens with a `from file (<id>)` row above the registry models.** Selecting it clears that position's pin.
- **The judge pinned to main's own model family warns in the modal at pin time.** The dispatch-time warning (`main.rs:648-670`) is unchanged and still never refuses — D16 `1e6ad121` ruled that out.
- **A rebind binds at spawn.** Hands already running keep the model they started with; the new binding takes effect on the next dispatch.
- **The durable spawn row grows `body` and `model`.** `ledger.rs:368` writes `{kind, hand, agent, role, brief, root}` today, which was reconstructible only because config never moved.
- **One `Event::Model{position, from, to}` per changed row.** A commit changing three positions logs three lines, so the log reads the same whether the operator used the modal or three typed commands.
- **Main's body stays claude-code.** The `run` guard at `main.rs:2728` is untouched by this card.

## Execution plan

### Scope 1 — the registry and the roles table

- **Files:**
  - Modify `arx.toml`
  - Modify `crates/arx/src/config.rs`
  - Modify `agents/hand-ex.md`, `agents/judge.md`, `agents/drain.md`
  - Delete `agents/hand-cx.md`, `agents/hand-ds.md`
- **Build:** a `[[models]]` registry and a `[roles]` table parsed, validated and resolved; agent frontmatter reduced to the position; `main.models` gone.
- **Steps:**
  1. Add a `ModelEntry` struct to `config.rs` — `id: String`, `body: Body`, `image: Option<String>`, `provider: Option<String>`, `#[serde(deny_unknown_fields)]` — and a `models: Vec<ModelEntry>` field on the startup struct that parses `[[models]]`.
  2. Validate the registry at load: non-empty; `id` non-empty, trimmed (no leading or trailing whitespace) and unique; `provider` set if and only if `body == Body::Native`, reusing the existing `ProviderOnNonNative` / `NativeMissingProvider` errors with the model id in the message; a `provider` that names no `[providers.*]` entry is an error.
  3. Add a `roles: BTreeMap<String, String>` field parsing `[roles]`, mapping position name to model id. Validate every value is an id present in the registry; an unknown id is `ConfigError::UnknownModel { role, id }` listing the registry's ids. Validate every key is either `main` or a position with a matching `agents/<key>.md`; an unmatched key is an error naming the missing file.
  4. Require a `[roles]` entry for `main` and for `judge`. Every other position may be absent from `[roles]` only if no agent file exists for it — that is, every `agents/*.md` needs a row.
  5. Remove `models` from `RoleConfig` and delete `ConfigError::EmptyModelsEntry`, `NonCanonicalModelsEntry`, `DuplicateModel`, `ModelNotInModels` along with the `role == "main"` block in `check_role` that raises them (`config.rs:376-395`). The equivalent checks now run over the registry.
  6. Remove `body`, `model`, `provider` and `image` from `RoleConfig`'s accepted frontmatter keys. Because `RoleConfig` carries `#[serde(deny_unknown_fields)]`, any agent file still declaring one now fails to parse — the error message must name the key and say it moved to `[[models]]` and `[roles]` in `arx.toml`.
  7. `[main]` keeps `body`, `instructions`, `repo_instructions`, `nudge`, `checkpoint`, `steps`, `window`, `reserve`. Its `model` is now read from `[roles].main`; delete the `model` key from `[main]`. `EmptyModel` moves to the registry.
  8. In `resolve` (`config.rs:637-720`), build each `RoleRow` by looking its position up in `[roles]`, then the id up in the registry, and filling `body`, `model`, `provider`, `image` from the registry entry. `drive` still comes from `drive_surface(kind, body)` — the drive surface is never configured (D16 `0bd4df2f`).
  9. Keep the `find_drive_key` walk (`config.rs:415-445`) working over the two new tables; it recurses into arrays-of-tables already, so `[[models]]` is covered — add a test that a `drive` key inside a `[[models]]` entry is still refused.
  10. Rewrite `arx.toml`: drop `model` and `models` from `[main]`; add `[[models]]` entries for `claude-sonnet-5`, `claude-opus-5` (body `claude-code`, image `arx-hand-claude:2.1.258`), `gpt-5.6-sol` (body `codex`, image `arx-hand-codex:0.152.1`) and `deepseek-v4-flash` (body `native`, provider `deepseek`); add `[roles]` with `main = "claude-sonnet-5"`, `hand-ex = "claude-sonnet-5"`, `judge = "gpt-5.6-sol"`, `drain = "gpt-5.6-sol"`.
  11. Strip `body`, `model`, `image`, `provider` from the three surviving agent files, leaving `tools` and the prose. Delete `agents/hand-cx.md` and `agents/hand-ds.md`.
  12. Update every fixture and test constructing a `RoleConfig`, a `RoleRow` or a config string with `model =` / `models =` / frontmatter `body:`. `hand-cx` and `hand-ds` are referenced in tests — grep `crates/arx/tests/` and `crates/arx/src/` for both names and for `hand-ds`'s `deepseek-v4-flash` before assuming the list is short.
- **Verify:** `cargo run -p arx -- check-config ./arx.toml` succeeds and prints every position with its resolved body and model. `cargo test -p arx config` green. Negative checks, each expecting the named error text: an agent file carrying `model:`; a `[roles]` value naming an id not in the registry; two `[[models]]` entries sharing an id; a `[[models]]` entry with `body = "codex"` and a `provider`; a `[[models]]` entry with `body = "native"` and no `provider`.

### Scope 2 — the roster file and the effective binding

- **Files:**
  - Create `crates/arx/src/roster.rs`
  - Modify `crates/arx/src/main.rs`
- **Build:** durable per-position pins, read at startup and written on every change, resolved over the `[roles]` defaults.
- **Steps:**
  1. `roster.rs` owns `Roster` — a `BTreeMap<String, String>` of position to pinned model id — with `load(path)`, `save(path)`, `pin(position, id)`, `clear(position)` and `get(position)`. File format is toml, one `[pins]` table, so it is hand-readable and hand-editable.
  2. Path resolution: `~/.local/share/arx/roster/<name>.toml`, where `<name>` is the config file's parent directory name, a hyphen, and the first 16 hex characters of the sha256 of the config file's absolute path. `sha2` is already a dependency. Honour `ARX_ROSTER_DIR` as an override, mirroring the `ARX_USER_GATE` precedent (`config.rs:798`). Create the directory on first write, never at load.
  3. A missing roster file is an empty roster, not an error. A malformed one is an error printed as a notice with the path, after which the run continues on an empty roster — a corrupt preference file must not block a session.
  4. Add `effective_binding(position, &Resolved, &Roster) -> (String /* id */, &ModelEntry, Source)` in `main.rs`, where `Source` is `Pinned` or `FromFile`. Pin first, then `[roles]`.
  5. Dangling pin: at load, every pin naming an id absent from the registry emits one notice on the operator surface — `[arx] roster: <position> pinned to <id>, which is not in [[models]]; using <default-id> from arx.toml` — and is ignored for resolution. The pin stays in the file; it is not silently deleted, because the operator may be mid-edit of the registry.
  6. Seed `current_model` (`main.rs:2826`) from `effective_binding("main", ...)` rather than `resolved.main.model`.
  7. Because `crates/arx` has no `lib.rs` and integration tests `#[path]`-include `src/*.rs`, put `roster.rs`'s tests in `main.rs`'s test module, not in an inline `#[cfg(test)]` module inside `roster.rs`. An inline module in an included file registers twice and breaks locked test counts.
- **Verify:** `cargo test -p arx roster` green. Manual, with `ARX_ROSTER_DIR` pointed at a temp dir: run `arx run`, pin a position, confirm the file appears with the expected `[pins]` content; restart and confirm the pin is still in force. Hand-write a pin naming `no-such-model`, start, and confirm the notice prints and the position falls back.

### Scope 3 — spawn-time consumption

- **Files:** Modify `crates/arx/src/main.rs`, `crates/arx/src/ledger.rs`
- **Build:** dispatch resolves through the roster; the durable spawn row records what actually ran.
- **Steps:**
  1. `resolve_hand_row` (`main.rs:487-500`) and `build_hand_spec` (`main.rs:537-628`) take the roster and fill `model`, `body`, `image` and `provider` from `effective_binding` rather than from the `RoleRow`'s frozen copy. Everything else in the spec — `tools`, `write`, `env`, `window`, `steps`, the forced read-only for `Judge` and `Drain` at `main.rs:614` — is unchanged and still comes from the agent file.
  2. The `dispatch` tool's optional `body` tripwire (`main.rs:735-744`) compares the requested body against the *effective* binding's body, not the agent file's. Its refusal text through `ledger.refuse_dispatch` keeps its shape.
  3. `ledger.rs:368`'s durable spawn row gains `"body"` and `"model"`, taken from the `HandSpec` already in hand at `SpawnLedger::dispatch`. `BodyKind` serializes with the same strings config uses.
  4. The judge family check (`main.rs:648-670`) reads the effective binding, so a pinned judge is checked against main's effective model.
- **Verify:** `cargo test -p arx` green, with the pre-existing count plus the new tests, no deletions. A test asserting the spawn row carries `body` and `model`. A test pinning `hand-ex` to `gpt-5.6-sol` and asserting the resulting `HandSpec` has `body: Codex` and the codex image. A test asserting `dispatch` with `body: "codex"` against a claude-code-pinned `hand-ex` is refused, and is accepted once the pin moves to a codex model.

### Scope 4 — the `/model` grammar and the line front

- **Files:** Modify `crates/arx/src/main.rs`
- **Build:** the four command forms, the printed roster, immediate application of typed forms.
- **Steps:**
  1. Extend `parse_model_line` (`main.rs:3522`) from `List | Switch(&str)` to `Open | SetMain(&str) | Set(&str, &str) | Clear(&str)`. One argument that names a registry id is `SetMain`; one argument that names a position is an error telling the operator to give a model id too; two arguments are `Set`; `clear <position>` is `Clear`.
  2. `Open` in the line front prints the roster: every position, its effective model id, that model's body, and `pinned` or `from file`. `Open` in the panes front sends the terminal message that raises the modal (Scope 5).
  3. `SetMain` keeps its current behaviour — validate against the registry rather than `main.models`, call `adapter.set_model` (`claude_code.rs:327`), write `Event::Model` — and additionally pins `main` in the roster. `MODEL_SWITCH_BODY_UNSUPPORTED` (`main.rs:3520`) stays: main's body is claude-code for this card, and the guard remains for the day it is not.
  4. `Set` on a non-main position writes the pin and returns; there is no live child to notify, since a rebind binds at spawn. `Set` on `main` is `SetMain`.
  5. `Clear` removes the pin and reports the `[roles]` default it fell back to.
  6. Every form validates the position against the known set — `main` plus one per `agents/*.md` — and an unknown position lists the valid ones, the way `render_model_unlisted` (`main.rs:3540`) already lists models.
  7. `Event::Model` grows a `position` field. `SetMain` writes `position: "main"`.
- **Verify:** `cargo test -p arx` green. Piped-stdin session (the line front): `/model` prints the roster with four rows; `/model claude-opus-5` switches main and prints `main: claude-sonnet-5 -> claude-opus-5`; `/model hand-ex gpt-5.6-sol` prints the pin; `/model` now shows `hand-ex` as `pinned`; `/model clear hand-ex` reports the fallback; `/model nonesuch x` and `/model hand-ex nonesuch` both print a listing error. Confirm the transcript log holds one `kind: "model"` row per change with a `position`.

### Scope 5 — the tabbed modal

- **Files:** Modify `crates/arx/src/panes/app.rs`, `crates/arx/src/panes/ui.rs`, and the embedded key defaults backing `[terminal] keys`
- **Build:** `Modal::ModelPicker` becomes a tabbed roster editor.
- **Steps:**
  1. Widen the terminal message that carries the picker's contents from a flat model list to a roster: per position, the ordered model list, the cursor's starting index, the effective id, and whether it is pinned.
  2. Extend `Modal::ModelPicker` (`panes/app.rs:384-386`) with a tab index and a per-tab cursor index. Tab order is `main` first, then the positions in the order `agents/*.md` resolves.
  3. Each tab's list is a `from file (<id>)` row followed by every registry model, each rendered with its body so the operator can see what a pin implies. The cursor opens on the row matching the position's effective binding, or on the `from file` row when nothing is pinned.
  4. Bind `h` and `l` in modal mode to previous and next tab, matching their normal-mode meaning of previous and next panel. `j`/`k` are already bound in modal mode and keep their meaning. Add both to the key defaults by name so `keys.toml` can override them.
  5. Enter applies every tab at once: for each position whose cursor sits somewhere other than where it opened, write the pin (or clear it, for the `from file` row), emit one `Event::Model`, and echo one line per changed row. Nothing changed means one short acknowledgement and no roster lines. Esc discards every pending cursor move and writes nothing.
  6. Draw the tab strip and mark each row `pinned` or `from file` in `draw_model_picker` (`panes/ui.rs:1055-1067`). Keep the existing digit-selection shortcut working within the focused tab.
  7. When the judge's tab has its cursor on a model whose family matches main's effective model, render a warning line inside the modal. Family comparison reuses `judge::family` (`judge.rs`). It is a warning only — Enter still applies it.
  8. Update the slash popup's hint list (`panes/ui.rs:1014`) so `/model` reads as the roster command.
- **Verify:** `cargo test -p arx` green, including a test driving the modal's key handling — open, `l` to a hand tab, `j` twice, `h` back to main, Enter — and asserting exactly the expected pins were written and exactly the expected events logged. Live check in a pty session: `/model` opens the tabbed modal; `h`/`l` walk tabs; `j`/`k` move within one; Enter applies and echoes only changed rows; Esc after moving several cursors leaves the roster file byte-identical. Pin the judge to main's own family and confirm the in-modal warning renders.

## Out of scope

Done means: every position's model is chosen from one registry, pins survive a restart, and the tabbed `/model` is the way it is done.

Not in this card:
- `bolt` and `hand-do` as positions — card B.
- Codex or a provider in **main's** seat: the trait over `ClaudeCodeMain`, the app-server sidecar driver, the delta-shaped usage arithmetic, and the `e652ce3a` protocol spike. The `run` guard at `main.rs:2728` stays exactly as it is.
- A body change on main firing a reset, and what a position's `tools` list means on a body that ignores it (codex reads none of it, native narrows to `read_file`/`recall`). Both are wave 2 questions.
- A modal in the line front, and `/reset` or `/checkpoint` in the panes front.
- Any change to the gate, admission, the state object, the transcript log's format beyond `Event::Model`'s new field, or the container machinery.
- Migrating existing roster files: none exist.

## Review handoff

dispatch spec-runner on `docs/specs/2026-09-09-roster-a-registry-and-modal.md` (it self-runs codex and logs it); then dispatch spec-reviewer with the log path from its verdict.
