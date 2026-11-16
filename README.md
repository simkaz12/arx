# arx

arx is an agent harness written in Rust. A "main" model talks to the operator and decides what to do. It hands real work to short-lived "hand" workers (Claude Code, Codex, or a model reached through an HTTP API), each running in its own Docker container. A second worker, the reviewer, checks a result and returns a PASS or FAIL verdict as JSON.

## Status

A prototype built over a few weeks in 2026 (late August to mid September). I set it aside in favour of an existing harness. It is not maintained, and some of it is rough. It is published to show the design and the code, not as a tool to depend on.

## How it works

### The shell and the model loop

`crates/arx` is the binary. `arx run <arx.toml>` starts main's session, reads operator lines from the terminal, sends them to main's body and logs every turn to a transcript on disk. Other subcommands: `check-config` (load a config and print the resolved role table), `gauge` (print the body's usage numbers for main's thread), `events` (group a thread's transcript rows by kind) and `ember` (write operator guidance into the store).

Main can run on three bodies, chosen in `arx.toml`:

- `claude-code`: a Node sidecar (`sidecar/claude-main.mjs`) built on `@anthropic-ai/claude-agent-sdk`. The Rust shell starts it with `node` and talks to it in JSON lines over stdin and stdout.
- `codex`: the shell starts `codex app-server --listen stdio://` with the shell tool, web search and approval prompts switched off, and drives it directly.
- `native`: a provider adapter in the shell itself (an Anthropic adapter and an OpenAI-compatible adapter, with streaming, provider error classification and a usage gauge).

Main sees a small set of tools (`sidecar/main-tools.json`): bounded file read and edit, dispatch, steer and kill for hands, and the store verbs (save, recall, decisions_query, decision, drain). It is told to decide and dispatch, not to read or change things at scale. A hand's verdict arrives as a line at the start of a later turn.

Around the loop the shell adds:

- Admission control: a tool result over a token threshold does not enter main's window whole. Main gets a head and a handle to a file under `threads/`, and reads the rest by line range.
- A gate on tool calls with deny, ask and allow rules and external hook scripts, read from the repo's `arx.toml` and a user-level file.
- A checkpoint and reset ritual: main is asked for a state object, the object is checked against the files it names, and a fresh main takes over from it and the transcript. Switching between the Claude Code and Codex bodies runs the same ritual.
- A spawn ledger that records every hand's dispatch, steering, kill and delivery.
- Typed instrumentation events written to the transcript.

### The terminal

The front end is a ratatui TUI (`crates/arx/src/panes`), selected by `[terminal] front = "panes"`. It shows main's turns and the running hands and reviewers, with a trace view for each. Keys are vim-like and live in `crates/arx/src/panes/keys.toml`. A plain line front is the other option.

### The engine crate

`crates/arx-engine` implements the store behind the memory verbs. It is a git-backed directory of one markdown file per claim, grouped by scope. A claim is a fact, a decision or an ember (operator guidance). The engine provides:

- recall by BM25, optionally blended with embeddings from an Ollama embedder when `[store] embedder` names a URL (the engine also has a hashing embedder, which the engine tests use);
- supersession and invalidation of claims;
- a quarantine area for claims saved during a thread;
- a landscape lookup, which returns the nearest existing claims for a candidate.

Memory is entirely local to this repo. There is no external memory service. Main's `save` writes to the quarantine only. A `drain` hand reads the quarantine and proposes, per candidate, to promote it, promote it over an in-force claim, or drop it. Nothing reaches the store until that proposal is applied. At startup, the claims in force for the configured scope are put into main's prompt as grounding. The store root is set in `arx.toml` (`[store] root`).

### Hands in containers

Each hand runs through `docker run --rm` with the image named for its model in `arx.toml`. The container code is in `crates/arx/src/container.rs` and `container_hand.rs`. For a hand:

- The repo is bind-mounted at `/hand/repo`. A read-only hand gets it read-only. A write hand gets a fresh `git worktree` instead, so its changes never touch the main checkout.
- A throwaway home at `/hand/home` and an output directory at `/hand/out` are mounted read-write. The store root and the credential directory are masked with read-only tmpfs mounts if they sit inside the repo.
- The container runs as the host's uid and gid. Only the environment variables listed in the hand's frontmatter are forwarded.
- The Claude Code or Codex login credential is copied from the host into the throwaway home with restrictive file modes. The host's own credential directory is not mounted.
- Hands never commit. When a write hand finishes, its diff is saved as `worktree.diff` in its trace, and applying it is left to main.
- The Claude Code hand runs `claude` in the container, the Codex hand runs `codex exec`, and a native hand runs the same provider loop used for the native main, with a model API key read from the environment variable named in the provider config.

The two images are in `docker/claude-hand` and `docker/codex-hand`. Each is `node:22` plus a pinned global install of the vendor CLI.

### Configuration

`arx.toml` declares the main body, the available models (each with a body, and an image or provider), the role-to-model table, the terminal front, the store and any providers. Hand roles are markdown files in `agents/` with a small YAML frontmatter (tools, write, env) and a one-line role description. `prompts/main.md` is main's instruction text. `arx check-config` validates all of it.

## Requirements

- Rust toolchain with edition 2024 support (Cargo 1.85 or newer)
- Git on `PATH`. Hands need the config directory to be inside a git repository with at least one commit.
- Node.js for the sidecar (the hand images use Node 22)
- Docker, for running hands and for the container tests
- A logged-in Claude Code and/or Codex CLI on the host for those bodies and hands
- For native hands and mains: an API key in the environment variable named by `api_key_env` in the provider block (the shipped `arx.toml` uses `DEEPSEEK_API_KEY`)
- Optional: Ollama with `nomic-embed-text` for embedding-backed recall

## Build and run

```
cargo build --workspace
(cd sidecar && npm ci)
docker build -t arx-hand-claude:2.1.258 docker/claude-hand
docker build -t arx-hand-codex:0.152.1 docker/codex-hand
cargo run -p arx -- check-config ./arx.toml
cargo run -p arx -- run ./arx.toml
```

The `arx.toml` in the repo root is a sample. Model ids, image tags and the store path in it are examples, so edit them before a real run.

## Tests

```
cargo test --workspace
(cd sidecar && npm test)
```

The Rust tests exercise the shell against stub binaries. `crates/arx/tests/fixtures` holds stub `claude` and `codex` executables, a stub sidecar, stub Docker images, config fixtures and hook scripts, so no paid model is called. The container tests need a working Docker daemon. One test (`installed_codex_tool_interrupt_followup_and_model_change`) is ignored by default because it uses a real Codex install and a paid account. One recall test is ignored unless Ollama is running. `crates/arx/tests/layer2_close.rs` compares a script against its copy on the `main` branch with `git show`, so it only passes inside a git checkout whose branch is named `main`.

## Layout

- `crates/arx`: the binary, bodies, gate, ledger, transcript, container hands, TUI
- `crates/arx-engine`: the claim store, recall and drain
- `sidecar`: the Claude Agent SDK process for the `claude-code` main body, with Node tests
- `docker`: hand images
- `agents`, `prompts`, `arx.toml`: sample role and main configuration
- `docs`: the design records. `docs/wayfinder/maps` holds the design and build maps with their findings and debate ledgers, `docs/specs` the per-card build specs, `docs/research` the memory research, `docs/verification` live check notes
- `SURVEY.md`, `POSITION.md`, `CONTEXT.md`, `CLAUDE.md`: the harness survey, the position paper, the glossary and the working rules used during development
- `prototypes/ui`: three standalone ratatui UI prototypes (`inline`, `panes`, `quiet`) used to try layouts for the TUI in `crates/arx`; they are not workspace members

Start with `POSITION.md` and `docs/wayfinder/maps/arx-design/map.md` for the reasoning behind the design.
