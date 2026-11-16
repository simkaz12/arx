mod admission;
mod anthropic;
mod claude_code;
mod claude_hand;
mod codex_auth;
mod codex_hand;
mod codex_main;
mod config;
mod container;
mod container_hand;
mod drain;
mod events;
mod gate;
mod reviewer;
mod ledger;
mod main_body;
mod main_tools;
mod native_hand;
mod openai_compat;
mod panes;
mod provider;
mod roster;
mod state_object;
mod terminal;
mod transcript;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::{self, IsTerminal};
use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};

use arx_engine::{ClaimHandle, Engine, OllamaEmbedder, SaveIntent, Store, StoreCall, StoreError, Suggestion, SuggestionAction};
use config::{Body, FrontKind, ModelEntry, Resolved, RoleRow};
use events::Event;
use gate::{Decision, DecisionNote, Gate, GateCall, GateVerdict, Table as GateTable, Tier as GateTier};
use ledger::{BodyKind, Delivery, HandId, HandRole, HandSpec, ProviderSpec, SpawnLedger, SteerOutcome, Timing};
use main_body::{FileCall, HandCall, MainBody, SpawnOpts, SystemPrompt};
use panes::PanesFront;
use roster::Roster;
use state_object::{CheckFailure, Header, Probe, StateObject};
use terminal::{Front, LineFront, OperatorMsg, Poll, RosterModel, RosterTab, TerminalMsg};
use transcript::TranscriptLog;

const DEFAULT_MAIN_INSTRUCTIONS: &str =
    "You are arx's main, the deciding loop the operator talks to directly. Everything else in this harness works on your behalf.";

const COMPACT_WINDOW: u64 = 1_000_000;

#[derive(Parser)]
#[command(name = "arx", about = "arx: an agent harness driving a configured main, hands and reviewer")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Load a config, resolve every role, print the resolved table")]
    CheckConfig {
        #[arg(default_value = "./arx.toml", value_name = "PATH", help = "Path to arx.toml")]
        path: PathBuf,
    },
    #[command(about = "Start main's session: spawn its body, stream stdin prompts to it, log every turn")]
    Run {
        #[arg(default_value = "./arx.toml", value_name = "PATH", help = "Path to arx.toml")]
        path: PathBuf,
    },
    #[command(about = "Print the body's own usage numbers for main's thread and their occupancy sum")]
    Gauge {
        #[arg(default_value = "./arx.toml", value_name = "PATH", help = "Path to arx.toml")]
        path: PathBuf,
    },
    #[command(about = "Print a thread's transcript rows grouped by kind, with counts")]
    Events {
        #[arg(long = "path", default_value = "./arx.toml", value_name = "PATH", help = "Path to arx.toml")]
        path: PathBuf,
        #[arg(default_value = "main", value_name = "THREAD", help = "Thread id")]
        thread: String,
    },
    #[command(about = "Mint the owner's guidance into the store, handled from main thread's last committed turn")]
    Ember {
        #[arg(long = "path", default_value = "./arx.toml", value_name = "PATH", help = "Path to arx.toml")]
        path: PathBuf,
        #[arg(value_name = "SCOPE", help = "Scope to write the ember into")]
        scope: String,
        #[arg(value_name = "TEXT", help = "The ember's text")]
        text: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::CheckConfig { path } => cmd_check_config(&path),
        Command::Run { path } => cmd_run(&path),
        Command::Gauge { path } => cmd_gauge(&path),
        Command::Events { path, thread } => cmd_events(&path, &thread),
        Command::Ember { path, scope, text } => cmd_ember(&path, &scope, &text),
    }
}

fn gate_tier_is_empty(cfg: &config::GateConfig) -> bool {
    cfg.deny.is_empty() && cfg.ask.is_empty() && cfg.allow.is_empty() && cfg.hooks.is_empty()
}

fn cmd_check_config(path: &Path) -> ExitCode {
    let resolved = match config::load(path) {
        Ok(resolved) => resolved,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let repo_label = path.display().to_string();
    if let Err(err) = config::check_gate(&resolved.gate, &repo_label) {
        eprintln!("error: {err}");
        return ExitCode::FAILURE;
    }

    let user_gate_path = config::user_gate_path();
    let user_label = user_gate_path.display().to_string();
    let user_gate = match config::load_user_gate(&user_gate_path) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = config::check_gate(&user_gate, &user_label) {
        eprintln!("error: {err}");
        return ExitCode::FAILURE;
    }

    print!("{}", render_table(&resolved));

    for text in config::inert_tools_notices(resolved.positions()) {
        println!("{text}");
    }

    if !gate_tier_is_empty(&resolved.gate) || !gate_tier_is_empty(&user_gate) {
        println!(
            "gate: repo deny {} ask {} allow {} hooks {} · user deny {} ask {} allow {} hooks {}",
            resolved.gate.deny.len(),
            resolved.gate.ask.len(),
            resolved.gate.allow.len(),
            resolved.gate.hooks.len(),
            user_gate.deny.len(),
            user_gate.ask.len(),
            user_gate.allow.len(),
            user_gate.hooks.len(),
        );
    }

    ExitCode::SUCCESS
}

fn config_dir(path: &Path) -> PathBuf {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn render_hand_roster(runner: &RoleRow, reviewer: &RoleRow, hands: &[RoleRow]) -> Option<String> {
    let mut section = "Hands available:\n".to_string();
    for hand in std::iter::once(runner).chain(hands.iter()) {
        let tools = if hand.tools.is_empty() {
            "none".to_string()
        } else {
            hand.tools.join(", ")
        };
        section.push_str(&format!("- {}: {}, {}, tools {tools}", hand.role, hand.model, hand.body));
        if let Some(role_line) = hand.role_line.as_deref().filter(|line| !line.is_empty()) {
            section.push_str(&format!(". {role_line}"));
        }
        section.push('\n');
    }
    section.push_str(&format!(
        "- {}: {}, {}, read-only; returns a JSON verdict under arx's schema (verdict PASS|FAIL, must_fix)\n",
        reviewer.role, reviewer.model, reviewer.body
    ));
    Some(section.trim_end_matches('\n').to_string())
}

fn compose_dynamic_block(resolved: &Resolved, config_dir: &Path, store: &mut dyn Store) -> Result<String, String> {
    let absolute_dir = std::env::current_dir()
        .map(|cwd| cwd.join(config_dir))
        .unwrap_or_else(|_| config_dir.to_path_buf());
    let mut parts: Vec<String> = Vec::new();

    let mut workspace_line = format!("Workspace {}.", absolute_dir.display());
    if let Some(scope) = &resolved.store_scope {
        workspace_line.push_str(&format!(" Store scope {scope}."));
    }
    parts.push(workspace_line);

    if let Some(repo_path) = &resolved.repo_instructions {
        let repo_text =
            fs::read_to_string(repo_path).map_err(|source| format!("{}: {source}", repo_path.display()))?;
        parts.push(format!("Repo instructions ({}):\n{repo_text}", repo_path.display()));
    }

    if let Some(scope) = &resolved.store_scope {
        let grounded = store.grounding(scope).map_err(|err| err.to_string())?;
        if !grounded.is_empty() {
            let mut section = format!("Store grounding (scope {scope}), as of this window:\n");
            for claim in &grounded {
                let label = claim.label.as_deref().unwrap_or("");
                section.push_str(&format!("- [{label}] {} — {}\n", claim.question, claim.answer));
            }
            parts.push(section.trim_end_matches('\n').to_string());
        }

        let quarantine_dir = resolved.store_root.join("quarantine").join("main");
        let quarantined = read_quarantine_listing(&quarantine_dir)?;
        if !quarantined.is_empty() {
            let mut section = "Quarantined this thread, as of this window:\n".to_string();
            for row in &quarantined {
                let kind = row.get("kind").and_then(|v| v.as_str()).unwrap_or_default();
                let question = row.get("question").and_then(|v| v.as_str()).unwrap_or_default();
                section.push_str(&format!("- [{kind}] {question}\n"));
            }
            parts.push(section.trim_end_matches('\n').to_string());
        }
    }

    if let Some(section) = render_hand_roster(&resolved.runner, &resolved.reviewer, &resolved.hands) {
        parts.push(section);
    }

    Ok(parts.join("\n\n"))
}

fn compose_system_prompt(resolved: &Resolved, config_dir: &Path, store: &mut dyn Store) -> Result<SystemPrompt, String> {
    let static_block = match &resolved.instructions {
        Some(path) => fs::read_to_string(path).map_err(|source| format!("{}: {source}", path.display()))?,
        None => DEFAULT_MAIN_INSTRUCTIONS.to_string(),
    };
    let dynamic = compose_dynamic_block(resolved, config_dir, store)?;
    Ok(SystemPrompt { static_block, dynamic })
}

fn read_quarantine_listing(dir: &Path) -> Result<Vec<serde_json::Value>, String> {
    let mut entries: Vec<(u64, PathBuf)> = Vec::new();
    match fs::read_dir(dir) {
        Ok(read_dir) => {
            for entry in read_dir {
                let entry = entry.map_err(|err| format!("{}: {err}", dir.display()))?;
                let candidate_path = entry.path();
                if candidate_path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let stem = candidate_path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                let seq: u64 = stem.split('-').next().and_then(|s| s.parse().ok()).unwrap_or(0);
                entries.push((seq, candidate_path));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("{}: {err}", dir.display())),
    }
    entries.sort_by_key(|(seq, _)| *seq);

    entries
        .into_iter()
        .map(|(_, candidate_path)| {
            let text = fs::read_to_string(&candidate_path).map_err(|err| format!("{}: {err}", candidate_path.display()))?;
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(|err| format!("{}: {err}", candidate_path.display()))?;
            let require_str = |field: &str| -> Result<String, String> {
                value
                    .get(field)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| format!("{}: missing or non-string '{field}'", candidate_path.display()))
            };
            require_str("id")?;
            require_str("scope")?;
            let kind = require_str("kind")?;
            if kind != "fact" && kind != "decision" {
                return Err(format!("{}: invalid 'kind' '{kind}'", candidate_path.display()));
            }
            require_str("question")?;
            require_str("answer")?;
            require_str("why")?;
            let handle = value
                .get("handle")
                .ok_or_else(|| format!("{}: missing 'handle'", candidate_path.display()))?;
            handle
                .get("thread")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("{}: missing or non-string 'handle.thread'", candidate_path.display()))?;
            handle
                .get("turn")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| format!("{}: missing or non-integer 'handle.turn'", candidate_path.display()))?;
            Ok(value)
        })
        .collect()
}

fn on_store_save(
    store: &mut dyn Store,
    args: &serde_json::Value,
    handle: &ClaimHandle,
    front: &RefCell<&mut dyn Front>,
) -> Result<serde_json::Value, StoreError> {
    let intent: SaveIntent = match serde_json::from_value(args.clone()) {
        Ok(intent) => intent,
        Err(err) => return Ok(serde_json::json!({"error": err.to_string()})),
    };
    let landscape = store.save_with(&intent, handle)?;
    if let Some(reason) = &landscape.degraded {
        let _ = front.borrow_mut().render(&TerminalMsg::Notice {
            text: format!("[arx] store: landscape degraded — {reason}"),
        });
    }
    Ok(serde_json::to_value(landscape.claims).expect("claims always serialize"))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DrainArgs {
    #[serde(default)]
    proposal: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    overrides: Option<Vec<Suggestion>>,
}

#[allow(clippy::too_many_arguments)]
fn on_store_drain(
    store: &mut dyn Store,
    args: &serde_json::Value,
    handle: &ClaimHandle,
    resolved: &Resolved,
    roster: &Roster,
    config_dir: &Path,
    user_gate: &config::GateConfig,
    ledger: &RefCell<&mut SpawnLedger>,
    log: &RefCell<&mut TranscriptLog>,
    front: &RefCell<&mut dyn Front>,
    event_err: &RefCell<Option<transcript::TranscriptError>>,
    adapter_for_kill: &mut ManuallyDrop<MainBody>,
    pending_drains: &mut HashMap<HandId, Vec<String>>,
    proposals: &mut HashMap<HandId, Vec<Suggestion>>,
) -> Result<serde_json::Value, StoreError> {
    let drain_args: DrainArgs = match serde_json::from_value(args.clone()) {
        Ok(value) => value,
        Err(err) => return Ok(serde_json::json!({"error": err.to_string()})),
    };

    if drain_args.proposal.is_none() && drain_args.overrides.is_some() {
        return Ok(serde_json::json!({"error": "drain: overrides need a proposal"}));
    }

    if let Some(proposal_hand) = &drain_args.proposal {
        let Some(base_proposal) = proposals.get(proposal_hand) else {
            return Ok(serde_json::json!({
                "error": format!("drain: no proposal from {proposal_hand} in this session — run drain again")
            }));
        };
        let merged = match &drain_args.overrides {
            Some(overrides) => {
                let mut merged = base_proposal.clone();
                let mut bad_id: Option<String> = None;
                for override_item in overrides {
                    match merged.iter_mut().find(|suggestion| suggestion.id == override_item.id) {
                        Some(slot) => *slot = override_item.clone(),
                        None => {
                            bad_id = Some(override_item.id.clone());
                            break;
                        }
                    }
                }
                if let Some(bad_id) = bad_id {
                    return Ok(serde_json::json!({
                        "error": format!("drain: override names {bad_id}, which is not in {proposal_hand}'s proposal")
                    }));
                }
                merged
            }
            None => base_proposal.clone(),
        };
        let overrides_count = drain_args.overrides.as_ref().map(|list| list.len()).unwrap_or(0);
        let report = store.drain_with(Some(&merged), handle)?;
        if let Err(err) = log_event(
            &mut **log.borrow_mut(),
            &mut **front.borrow_mut(),
            &Event::Drain {
                hand: proposal_hand.clone(),
                promoted: report.promoted,
                dropped: report.dropped,
                overrides: overrides_count,
            },
        ) {
            *event_err.borrow_mut() = Some(err);
            adapter_for_kill.kill();
            return Ok(serde_json::json!({"error": "event line failed to write"}));
        }
        proposals.remove(proposal_hand);
        return Ok(serde_json::to_value(report).expect("drain report always serializes"));
    }

    let Some(drain_row) = resolved.drain.as_ref() else {
        return Ok(serde_json::json!({"error": "drain: no drain agent — add agents/drain.md"}));
    };

    let quarantine_dir = resolved.store_root.join("quarantine").join("main");
    let candidates = match read_quarantine_listing(&quarantine_dir) {
        Ok(list) => list,
        Err(message) => return Ok(serde_json::json!({"error": message})),
    };
    if candidates.is_empty() {
        return Ok(serde_json::json!({"error": "drain: quarantine is empty"}));
    }

    let mut enriched: Vec<serde_json::Value> = Vec::new();
    let mut snapshot_ids: Vec<String> = Vec::new();
    for mut row in candidates {
        let scope = row.get("scope").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let question = row.get("question").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let answer = row.get("answer").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let id = row.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let landscape = store.landscape(&scope, &question, &answer)?;
        if let Some(reason) = &landscape.degraded {
            let _ = front.borrow_mut().render(&TerminalMsg::Notice {
                text: format!("[arx] store: landscape degraded — {reason}"),
            });
        }
        let landscape_json: Vec<serde_json::Value> = landscape
            .claims
            .iter()
            .map(|claim| {
                serde_json::json!({
                    "id": claim.id,
                    "kind": claim.kind,
                    "question": claim.question,
                    "answer": claim.answer,
                    "why": claim.why,
                })
            })
            .collect();
        if let Some(obj) = row.as_object_mut() {
            obj.insert("landscape".to_string(), serde_json::Value::Array(landscape_json));
        }
        snapshot_ids.push(id);
        enriched.push(row);
    }

    let input_text = drain::build_drain_input("main", &enriched);
    let brief_text = drain::drain_brief(drain_args.note.as_deref());

    let spec = match build_hand_spec(drain_row, HandRole::Drain, "drain", resolved, roster, config_dir, user_gate, Some(input_text)) {
        Ok(spec) => spec,
        Err(message) => return Ok(serde_json::json!({"error": message})),
    };

    let (_, drain_entry, _) = effective_binding("drain", resolved, roster);
    let drain_family = reviewer::family_of(drain_entry.body, drain_entry.provider.as_deref(), &resolved.providers);
    let (_, main_entry, _) = main_effective_binding(resolved, roster);
    let main_family = reviewer::family_of(main_entry.body, main_entry.provider.as_deref(), &resolved.providers);

    match dispatch_and_record(
        &mut **ledger.borrow_mut(),
        event_err,
        adapter_for_kill,
        spec,
        Some((drain_family, main_family)),
        &brief_text,
        log,
        front,
    ) {
        Ok(hand) => {
            pending_drains.insert(hand.clone(), snapshot_ids);
            Ok(serde_json::json!({"hand": hand}))
        }
        Err(value) => Ok(value),
    }
}

fn body_kind(body: Body) -> BodyKind {
    match body {
        Body::ClaudeCode => BodyKind::ClaudeCode,
        Body::Codex => BodyKind::Codex,
        Body::Native => BodyKind::Native,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelSource {
    Pinned,
    FromFile,
}

/// Pin first, then the `[roles]` default — the roster's own resolution order.
/// `position` must be a known row (`Resolved::position`); callers only ever
/// pass names drawn from `Resolved::positions()`.
fn effective_binding<'a>(position: &str, resolved: &'a Resolved, roster: &Roster) -> (String, &'a ModelEntry, ModelSource) {
    let row = resolved
        .position(position)
        .unwrap_or_else(|| panic!("effective_binding called with unknown position '{position}'"));
    if let Some(id) = roster.get(position) {
        if let Some(entry) = resolved.model_entry(id) {
            return (id.to_string(), entry, ModelSource::Pinned);
        }
    }
    let entry = resolved
        .model_entry(&row.model)
        .unwrap_or_else(|| panic!("'{position}' [roles] default '{}' is missing from the resolved registry", row.model));
    (row.model.clone(), entry, ModelSource::FromFile)
}

fn effective_rows_for_notices(resolved: &Resolved, roster: &Roster) -> Vec<RoleRow> {
    resolved
        .positions()
        .into_iter()
        .map(|row| {
            let (effective_id, entry, _source) = effective_binding(&row.role, resolved, roster);
            let mut row = row.clone();
            row.model = effective_id;
            row.body = entry.body;
            row
        })
        .collect()
}

/// Saves `roster`; on failure, rolls the just-made mutation for `position`
/// back to `previous` (its pin if it had one, cleared otherwise) so the
/// in-memory roster never diverges from the durable file, and renders the
/// failure on the operator surface — a retry of the identical command sees
/// the same unpinned or differently-pinned state, not a false "already".
fn save_roster_or_notice(
    roster: &mut Roster,
    roster_path: &Path,
    position: &str,
    previous: Option<&str>,
    front: &mut dyn Front,
) -> bool {
    match roster.save(roster_path) {
        Ok(()) => true,
        Err(err) => {
            match previous {
                Some(value) => roster.pin(position, value),
                None => {
                    roster.clear(position);
                }
            }
            let _ = front.render(&TerminalMsg::Notice {
                text: format!("[arx] roster: {err} — {position}'s change was not saved; the pin did not take"),
            });
            false
        }
    }
}

fn dangling_pin_notices(roster: &Roster, resolved: &Resolved) -> Vec<String> {
    let mut notices = Vec::new();
    for (position, id) in roster.pins() {
        let Some(row) = resolved.position(position) else {
            continue;
        };
        if resolved.model_entry(id).is_none() {
            notices.push(format!(
                "[arx] roster: {position} pinned to {id}, which is not in [[models]]; using {} from arx.toml",
                row.model
            ));
        }
    }
    notices
}

fn main_effective_binding<'a>(resolved: &'a Resolved, roster: &Roster) -> (String, &'a ModelEntry, ModelSource) {
    let (id, entry, source) = effective_binding("main", resolved, roster);
    if source == ModelSource::Pinned && !matches!(entry.body, Body::ClaudeCode | Body::Codex) {
        let fallback_id = resolved.main.model.clone();
        let fallback_entry = resolved
            .model_entry(&fallback_id)
            .expect("main's [roles] default always resolves in the registry");
        return (fallback_id, fallback_entry, ModelSource::FromFile);
    }
    (id, entry, source)
}

/// The startup-only wrapper that also renders the one-time notice when a
/// pin gets rejected by `main_effective_binding`'s body check.
fn main_current_model(resolved: &Resolved, roster: &Roster) -> (String, Option<String>) {
    let (raw_id, raw_entry, raw_source) = effective_binding("main", resolved, roster);
    if raw_source == ModelSource::Pinned && !matches!(raw_entry.body, Body::ClaudeCode | Body::Codex) {
        let fallback = resolved.main.model.clone();
        let notice = format!(
            "[arx] roster: main pinned to {raw_id}, whose body '{}' does not fit main's supported seats; using {fallback} from arx.toml",
            raw_entry.body
        );
        return (fallback, Some(notice));
    }
    (raw_id, None)
}

fn resolve_hand_row<'a>(agent: &str, resolved: &'a Resolved) -> Result<(&'a RoleRow, HandRole), String> {
    if agent == resolved.runner.role {
        return Ok((&resolved.runner, HandRole::Runner));
    }
    if agent == resolved.reviewer.role {
        return Ok((&resolved.reviewer, HandRole::Reviewer));
    }
    let found = resolved.hands.iter().find(|row| row.role == agent);
    let Some(found) = found else {
        let mut known: Vec<&str> = [resolved.runner.role.as_str(), resolved.reviewer.role.as_str()]
            .into_iter()
            .chain(resolved.hands.iter().map(|row| row.role.as_str()))
            .collect();
        known.sort();
        return Err(format!("unknown agent '{agent}' — known definitions: {}", known.join(", ")));
    };
    Ok((found, HandRole::Hand))
}

fn resolve_provider(agent: &str, kind: BodyKind, provider: Option<&str>, resolved: &Resolved) -> Result<Option<ProviderSpec>, String> {
    if kind != BodyKind::Native {
        return Ok(None);
    }
    let provider_name = provider.unwrap_or_default();
    let provider_row = resolved
        .providers
        .get(provider_name)
        .ok_or_else(|| format!("internal error: role '{agent}' names provider '{provider_name}' with no resolved row"))?;
    let key = std::env::var(&provider_row.api_key_env).unwrap_or_default();
    if key.is_empty() {
        return Err(format!(
            "provider '{provider_name}' needs env {} set for a native dispatch",
            provider_row.api_key_env
        ));
    }
    Ok(Some(ProviderSpec {
        kind: provider_row.kind,
        base_url: provider_row.base_url.clone(),
        api_key_env: provider_row.api_key_env.clone(),
    }))
}

fn resolve_native_tools(agent: &str, tools: &[String]) -> Result<Vec<String>, String> {
    if tools.is_empty() {
        return Ok(vec!["read_file".to_string()]);
    }
    for tool in tools {
        if tool != "read_file" && tool != "recall" {
            return Err(format!("dispatch: {agent} lists an unknown native tool {tool}"));
        }
    }
    Ok(tools.to_vec())
}

fn resolve_hand_spec(
    agent: &str,
    resolved: &Resolved,
    roster: &Roster,
    config_dir: &Path,
    user_gate: &config::GateConfig,
) -> Result<HandSpec, String> {
    let threads_dir = config_dir.join("threads");
    let cwd = config_dir.to_path_buf();
    let thread = "main".to_string();
    let store_root = resolved.store_root.clone();

    if agent == "fake" || agent == "fake-write" {
        return Ok(HandSpec {
            agent: agent.to_string(),
            kind: BodyKind::Fake,
            role: HandRole::Hand,
            model: String::new(),
            cwd,
            threads_dir,
            thread,
            provider: None,
            write: agent == "fake-write",
            image: ledger::DEFAULT_HAND_IMAGE.to_string(),
            env: Vec::new(),
            store_root,
            config_dir: config_dir.to_path_buf(),
            tools: Vec::new(),
            steps: config::DEFAULT_STEPS,
            window: None,
            gate: resolved.gate.clone(),
            user_gate: user_gate.clone(),
            store_scope: resolved.store_scope.clone(),
            drain_input: None,
            admission: resolved.admission.clone(),
            role_line: None,
        });
    }

    let (row, role) = resolve_hand_row(agent, resolved)?;
    build_hand_spec(row, role, agent, resolved, roster, config_dir, user_gate, None)
}

#[allow(clippy::too_many_arguments)]
fn build_hand_spec(
    row: &RoleRow,
    role: HandRole,
    agent: &str,
    resolved: &Resolved,
    roster: &Roster,
    config_dir: &Path,
    user_gate: &config::GateConfig,
    drain_input: Option<String>,
) -> Result<HandSpec, String> {
    let threads_dir = config_dir.join("threads");
    let cwd = config_dir.to_path_buf();
    let thread = "main".to_string();
    let store_root = resolved.store_root.clone();

    let (effective_id, entry, _source) = effective_binding(agent, resolved, roster);
    let kind = body_kind(entry.body);
    if role == HandRole::Runner && kind == BodyKind::Native {
        return Err(format!(
            "dispatch: runner model '{}' uses native body; runner requires claude-code or codex so it can implement the brief",
            entry.id
        ));
    }
    let provider = resolve_provider(agent, kind, entry.provider.as_deref(), resolved)?;

    if matches!(kind, BodyKind::ClaudeCode | BodyKind::Codex) && entry.image.is_none() {
        return Err(format!(
            "dispatch: {agent} is a {} hand with no image; set image: on model '{}' in [[models]]",
            kind.label(),
            entry.id
        ));
    }

    let tools = if kind == BodyKind::Native {
        resolve_native_tools(agent, &row.tools)?
    } else {
        row.tools.clone()
    };

    Ok(HandSpec {
        agent: agent.to_string(),
        kind,
        role,
        model: effective_id,
        cwd,
        threads_dir,
        thread,
        provider,
        write: !matches!(role, HandRole::Reviewer | HandRole::Drain) && row.write,
        image: entry.image.clone().unwrap_or_else(|| ledger::DEFAULT_HAND_IMAGE.to_string()),
        env: row.env.clone(),
        store_root,
        config_dir: config_dir.to_path_buf(),
        tools,
        steps: row.steps,
        window: row.window,
        gate: resolved.gate.clone(),
        user_gate: user_gate.clone(),
        store_scope: resolved.store_scope.clone(),
        drain_input,
        admission: resolved.admission.clone(),
        role_line: row.role_line.clone(),
    })
}

const CANCEL_DISPATCHED_HAND_DEADLINE: Duration = Duration::from_secs(30);
const CANCEL_DISPATCHED_HAND_POLL: Duration = Duration::from_millis(50);

fn cancel_dispatched_hand(ledger: &mut SpawnLedger, hand: &str) {
    let deadline = Instant::now() + CANCEL_DISPATCHED_HAND_DEADLINE;
    let mut signaled = ledger.kill(hand).is_ok();

    while ledger.in_flight_hands().iter().any(|(id, _)| id == hand) {
        if Instant::now() >= deadline {
            return;
        }
        if !signaled {
            signaled = ledger.kill(hand).is_ok();
        }
        if let Ok(None) = ledger.try_next_timed() {
            std::thread::sleep(CANCEL_DISPATCHED_HAND_POLL);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_and_record(
    ledger: &mut SpawnLedger,
    event_err: &RefCell<Option<transcript::TranscriptError>>,
    adapter_for_kill: &mut ManuallyDrop<MainBody>,
    spec: HandSpec,
    family_check: Option<(String, String)>,
    brief: &str,
    log: &RefCell<&mut TranscriptLog>,
    front: &RefCell<&mut dyn Front>,
) -> Result<HandId, serde_json::Value> {
    let spawn_agent = spec.agent.clone();
    let spawn_role = spec.role.label().to_string();
    let spawn_body = spec.kind.label().to_string();
    let spawn_image = spec.image.clone();
    let spawn_model = spec.model.clone();
    match ledger.dispatch(spec, brief) {
        Ok(hand) => {
            if let Some((row_family, main_family)) = family_check {
                if row_family == main_family {
                    let _ = front.borrow_mut().render(&TerminalMsg::Notice {
                        text: format!(
                            "[arx] {spawn_role} '{spawn_agent}' is {row_family}, the same family as main; its verdict is not independent"
                        ),
                    });
                    if let Err(err) = log_event(
                        &mut **log.borrow_mut(),
                        &mut **front.borrow_mut(),
                        &Event::Family {
                            agent: spawn_agent.clone(),
                            family: row_family,
                        },
                    ) {
                        *event_err.borrow_mut() = Some(err);
                        cancel_dispatched_hand(ledger, &hand);
                        adapter_for_kill.kill();
                        return Err(serde_json::json!({"error": "event line failed to write"}));
                    }
                }
            }
            if let Err(err) = log_event(
                &mut **log.borrow_mut(),
                &mut **front.borrow_mut(),
                &Event::Spawn {
                    hand: hand.clone(),
                    agent: spawn_agent.clone(),
                    role: spawn_role.clone(),
                },
            ) {
                *event_err.borrow_mut() = Some(err);
                adapter_for_kill.kill();
                return Err(serde_json::json!({"error": "event line failed to write"}));
            }
            let _ = front.borrow_mut().render(&TerminalMsg::HandSpawned {
                hand: hand.clone(),
                agent: spawn_agent,
                role: spawn_role,
                body: spawn_body,
                image: spawn_image,
                model: spawn_model,
                brief: brief.to_string(),
            });
            Ok(hand)
        }
        Err(err) => Err(serde_json::json!({"error": err.to_string()})),
    }
}

#[allow(clippy::too_many_arguments)]
fn on_hand_call(
    ledger: &RefCell<&mut SpawnLedger>,
    hand_err: &mut Option<ledger::LedgerError>,
    event_err: &RefCell<Option<transcript::TranscriptError>>,
    adapter_for_hand_kill: &mut ManuallyDrop<MainBody>,
    call: &HandCall,
    resolved: &Resolved,
    roster: &Roster,
    config_dir: &Path,
    user_gate: &config::GateConfig,
    log: &RefCell<&mut TranscriptLog>,
    front: &RefCell<&mut dyn Front>,
) -> serde_json::Value {
    if hand_err.is_some() {
        return serde_json::json!({"error": "session failing"});
    }
    match call.verb.as_str() {
        "dispatch" => {
            let agent = call.args.get("agent").and_then(|v| v.as_str()).unwrap_or_default();
            let brief = call.args.get("brief").and_then(|v| v.as_str()).unwrap_or_default();
            let requested = call.args.get("body").and_then(|v| v.as_str());

            if agent == "drain" {
                return serde_json::json!({"error": "dispatch: drain runs through the drain tool"});
            }

            let definition_kind = if agent == "fake" || agent == "fake-write" {
                Some(BodyKind::Fake)
            } else {
                resolve_hand_row(agent, resolved)
                    .ok()
                    .map(|_| body_kind(effective_binding(agent, resolved, roster).1.body))
            };

            if let (Some(body), Some(definition_kind)) = (requested, definition_kind) {
                let matches = serde_json::from_value::<Body>(serde_json::Value::String(body.to_string()))
                    .map(body_kind)
                    .map(|kind| kind == definition_kind)
                    .unwrap_or(false);
                if !matches {
                    return match ledger.borrow_mut().refuse_dispatch(agent, body, definition_kind) {
                        Ok(text) => serde_json::json!({"error": text}),
                        Err(err) => serde_json::json!({"error": err.to_string()}),
                    };
                }
            }

            match resolve_hand_spec(agent, resolved, roster, config_dir, user_gate) {
                Ok(spec) => {
                    let family_check = if spec.role == HandRole::Reviewer {
                        let (_, entry, _) = effective_binding(agent, resolved, roster);
                        let row_family = reviewer::family_of(entry.body, entry.provider.as_deref(), &resolved.providers);
                        let (_, main_entry, _) = main_effective_binding(resolved, roster);
                        let main_family = reviewer::family_of(main_entry.body, main_entry.provider.as_deref(), &resolved.providers);
                        Some((row_family, main_family))
                    } else {
                        None
                    };
                    match dispatch_and_record(
                        &mut **ledger.borrow_mut(),
                        event_err,
                        adapter_for_hand_kill,
                        spec,
                        family_check,
                        brief,
                        log,
                        front,
                    ) {
                        Ok(hand) => serde_json::json!({"hand": hand}),
                        Err(value) => value,
                    }
                }
                Err(message) => serde_json::json!({"error": message}),
            }
        }
        "tap" => {
            let hand = call.args.get("hand").and_then(|v| v.as_str()).unwrap_or_default();
            match ledger.borrow_mut().tap(hand) {
                Some(value) => {
                    if let Err(err) =
                        log_event(&mut **log.borrow_mut(), &mut **front.borrow_mut(), &Event::Tap { hand: hand.to_string() })
                    {
                        *event_err.borrow_mut() = Some(err);
                        adapter_for_hand_kill.kill();
                        return serde_json::json!({"error": "event line failed to write"});
                    }
                    value
                }
                None => serde_json::json!({"error": "unknown hand"}),
            }
        }
        "steer" => {
            let hand = call.args.get("hand").and_then(|v| v.as_str()).unwrap_or_default();
            let message = call.args.get("message").and_then(|v| v.as_str()).unwrap_or_default();
            match ledger.borrow_mut().steer(hand, message) {
                Ok(SteerOutcome::Accepted) => {
                    if let Err(err) =
                        log_event(&mut **log.borrow_mut(), &mut **front.borrow_mut(), &Event::Steer { hand: hand.to_string() })
                    {
                        *event_err.borrow_mut() = Some(err);
                        adapter_for_hand_kill.kill();
                        return serde_json::json!({"error": "event line failed to write"});
                    }
                    serde_json::json!({"ok": true})
                }
                Ok(SteerOutcome::NotEligible) => {
                    serde_json::json!({"error": "hand not eligible for steer"})
                }
                Ok(SteerOutcome::Refused(text)) => serde_json::json!({"error": text}),
                Err(err) => {
                    *hand_err = Some(err);
                    adapter_for_hand_kill.kill();
                    serde_json::json!({"error": "ledger write failed"})
                }
            }
        }
        "kill" => {
            let hand = call.args.get("hand").and_then(|v| v.as_str()).unwrap_or_default();
            match ledger.borrow_mut().kill(hand) {
                Ok(true) => {
                    if let Err(err) =
                        log_event(&mut **log.borrow_mut(), &mut **front.borrow_mut(), &Event::Kill { hand: hand.to_string() })
                    {
                        *event_err.borrow_mut() = Some(err);
                        adapter_for_hand_kill.kill();
                        return serde_json::json!({"error": "event line failed to write"});
                    }
                    serde_json::json!({"ok": true})
                }
                Ok(false) => serde_json::json!({"error": "hand not eligible for kill"}),
                Err(err) => {
                    *hand_err = Some(err);
                    adapter_for_hand_kill.kill();
                    serde_json::json!({"error": "ledger write failed"})
                }
            }
        }
        other => serde_json::json!({"error": format!("unknown verb '{other}'")}),
    }
}

fn delivery_hand_and_outcome(delivery: &Delivery) -> (&str, &'static str) {
    match delivery {
        Delivery::Verdict { hand, .. } => (hand, "verdict"),
        Delivery::Stop { hand, .. } => (hand, "stop"),
        Delivery::Killed { hand, .. } => (hand, "killed"),
    }
}

fn delivery_trace_and_bytes(delivery: &Delivery) -> (Option<String>, Option<u64>) {
    match delivery {
        Delivery::Verdict { trace, bytes, .. } => (trace.clone(), *bytes),
        Delivery::Stop { .. } => (None, None),
        Delivery::Killed { trace, bytes, .. } => (trace.clone(), *bytes),
    }
}

fn reviewer_schema_outcome(reviewer_event: &Option<(Event, String)>) -> Option<String> {
    reviewer_event.as_ref().and_then(|(event, _)| match event {
        Event::Schema { outcome, .. } => Some(outcome.clone()),
        _ => None,
    })
}

fn send_delivery_party_msg(front: &mut dyn Front, delivery: &Delivery, timing: &Timing, schema: Option<String>) {
    let msg = match delivery {
        Delivery::Stop { hand, question, .. } => TerminalMsg::HandStopped { hand: hand.clone(), question: question.clone() },
        Delivery::Verdict { hand, text, trace, bytes, diff, .. } => TerminalMsg::HandDelivered {
            hand: hand.clone(),
            outcome: "verdict".to_string(),
            text: Some(text.clone()),
            trace: trace.clone(),
            bytes: *bytes,
            elapsed_ms: timing.elapsed_ms,
            lag_ms: timing.lag_ms,
            schema,
            diff: diff.clone(),
        },
        Delivery::Killed { hand, trace, bytes } => TerminalMsg::HandDelivered {
            hand: hand.clone(),
            outcome: "killed".to_string(),
            text: None,
            trace: trace.clone(),
            bytes: *bytes,
            elapsed_ms: timing.elapsed_ms,
            lag_ms: timing.lag_ms,
            schema: None,
            diff: None,
        },
    };
    let _ = front.render(&msg);
}

fn delivery_walls(delivery: &Delivery) -> &[String] {
    match delivery {
        Delivery::Verdict { walls, .. } => walls,
        Delivery::Stop { .. } | Delivery::Killed { .. } => &[],
    }
}

fn delivery_miss(delivery: &Delivery) -> Option<u64> {
    match delivery {
        Delivery::Verdict { miss, .. } => *miss,
        Delivery::Stop { .. } | Delivery::Killed { .. } => None,
    }
}

fn delivery_stop_wall(delivery: &Delivery) -> Option<(&str, &str)> {
    match delivery {
        Delivery::Stop { hand, wall: Some(tool), .. } => Some((hand.as_str(), tool.as_str())),
        _ => None,
    }
}

fn format_delivery(delivery: &Delivery) -> String {
    match delivery {
        Delivery::Verdict { hand, text, trace: None, .. } => format!("[hand {hand} verdict] {text}"),
        Delivery::Verdict { hand, text, trace: Some(trace), diff: None, .. } => format!("[hand {hand} verdict] {text} (trace: {trace})"),
        Delivery::Verdict {
            hand,
            text,
            trace: Some(trace),
            diff: Some(diff),
            ..
        } => format!("[hand {hand} verdict] {text} (trace: {trace}, diff: {diff})"),
        Delivery::Stop { hand, question, wall: Some(_) } => format!("[hand {hand} stop] {question}"),
        Delivery::Stop { hand, question, wall: None } => format!("[hand {hand} stop] {question} — answer with the steer tool"),
        Delivery::Killed { hand, trace: None, .. } => format!("[hand {hand} killed] exit: killed"),
        Delivery::Killed { hand, trace: Some(trace), .. } => format!("[hand {hand} killed] exit: killed (trace: {trace})"),
    }
}

fn admission_trace_root(config_dir: &Path, thread: &str) -> admission::TraceRoot {
    let threads_dir = config_dir.join("threads");
    let threads_dir = threads_dir.canonicalize().unwrap_or(threads_dir);
    admission::TraceRoot {
        host_dir: threads_dir.join(format!("{thread}.traces")),
        handle_prefix: format!("threads/{thread}.traces"),
        fallback_dir: std::env::temp_dir().join("arx-traces").join(thread),
    }
}

fn bound_verdict_delivery(
    delivery: &Delivery,
    text: String,
    policy: &admission::AdmissionPolicy,
    root: &admission::TraceRoot,
    seq: &mut u64,
    ordinal: u64,
) -> (String, Vec<Event>) {
    if !matches!(delivery, Delivery::Verdict { .. }) {
        return (text, Vec::new());
    }
    let (hand, _) = delivery_hand_and_outcome(delivery);
    match admission::admit_verdict(policy, root, seq, admission::VerdictRequest { hand, body: &text, ordinal }) {
        Ok(admitted) => (admitted.text, admitted.events),
        Err(err) => (
            format!(
                "[arx] verdict too large to externalize ({err}); showing a bounded head:\n{}",
                admission::cut_at_char_boundary(&text, admission::PREVIEW_HEAD_BYTES)
            ),
            Vec::new(),
        ),
    }
}

fn reviewer_verdict_event(ledger: &SpawnLedger, resolved: &Resolved, hand: &str, delivery: &Delivery) -> Option<(Event, String)> {
    let Delivery::Verdict { text, trace, .. } = delivery else {
        return None;
    };
    let is_reviewer = ledger
        .hand_identity(hand)
        .map(|(agent, _)| agent == resolved.reviewer.role)
        .unwrap_or(false);
    if !is_reviewer {
        return None;
    }
    match reviewer::validate_verdict(text) {
        Ok(()) => Some((
            Event::Schema {
                hand: hand.to_string(),
                outcome: "ok".to_string(),
                reason: None,
            },
            format_delivery(delivery),
        )),
        Err(reason) => {
            let mut cutoff = text.len().min(500);
            while cutoff > 0 && !text.is_char_boundary(cutoff) {
                cutoff -= 1;
            }
            let raw = &text[..cutoff];
            let formatted = match trace {
                Some(trace) => format!("[hand {hand} verdict] reviewer verdict malformed: {reason}. Raw: {raw} (trace: {trace})"),
                None => {
                    format!("[hand {hand} verdict] reviewer verdict malformed: {reason}. Raw: {raw}")
                }
            };
            Some((
                Event::Schema {
                    hand: hand.to_string(),
                    outcome: "malformed".to_string(),
                    reason: Some(reason),
                },
                formatted,
            ))
        }
    }
}

fn drain_proposal_event(
    hand: &str,
    delivery: &Delivery,
    pending_drains: &mut HashMap<HandId, Vec<String>>,
    proposals: &mut HashMap<HandId, Vec<Suggestion>>,
) -> Option<(Event, String)> {
    if !pending_drains.contains_key(hand) {
        return None;
    }
    let Delivery::Verdict { text, trace, .. } = delivery else {
        pending_drains.remove(hand);
        return None;
    };
    let snapshot_ids = pending_drains.remove(hand).unwrap_or_default();
    let trace_text = trace.clone().unwrap_or_default();
    match drain::validate_proposal(text, &snapshot_ids) {
        Ok(suggestions) => {
            let formatted = drain::render_proposal(hand, &trace_text, &suggestions);
            proposals.insert(hand.to_string(), suggestions);
            Some((
                Event::Schema {
                    hand: hand.to_string(),
                    outcome: "ok".to_string(),
                    reason: None,
                },
                formatted,
            ))
        }
        Err(reason) => {
            let formatted = format!("[hand {hand} drain proposal malformed] {reason} (trace: {trace_text})");
            Some((
                Event::Schema {
                    hand: hand.to_string(),
                    outcome: "malformed".to_string(),
                    reason: Some(reason),
                },
                formatted,
            ))
        }
    }
}

struct Nudge {
    threshold: u64,
    said: bool,
    pending: Option<String>,
}

struct Checkpoint {
    delta: Option<u64>,
    base: u64,
    last_occupancy: u64,
    number: Option<u64>,
    written_delta: Option<u64>,
}

impl Checkpoint {
    fn due(&self) -> bool {
        match self.delta {
            Some(delta) => self.last_occupancy.saturating_sub(self.base) >= delta,
            None => false,
        }
    }
}

struct Reserve {
    window: u64,
    reserve: u64,
    armed: bool,
}

fn build_status(resolved: &Resolved, checkpoint: &Checkpoint, nudge: &Nudge, current_model: &str, turn: u64) -> TerminalMsg {
    let (window, reserve) = config::main_window(&resolved.main);
    TerminalMsg::Status {
        model: current_model.to_string(),
        occupancy: checkpoint.last_occupancy,
        window,
        reserve,
        nudge: nudge.threshold,
        checkpoint: checkpoint.number.map(|n| (n, checkpoint.written_delta.unwrap_or(0))),
        turn,
    }
}

impl Reserve {
    fn crossed_arm(&self, occupancy: u64) -> bool {
        occupancy >= self.window.saturating_sub(self.reserve)
    }

    fn at_ceiling(&self, occupancy: u64) -> bool {
        occupancy >= self.window
    }
}

#[derive(Clone, Copy)]
enum Fire {
    Switch,
    Operator,
    Harness,
}

impl Fire {
    fn label(&self) -> &'static str {
        match self {
            Fire::Switch => "body_switch",
            Fire::Operator => "operator",
            Fire::Harness => "harness",
        }
    }
}

enum RitualOutcome {
    Written(u64),
    Abandoned,
}

fn format_hands_block(hands: &[(HandId, String)]) -> String {
    if hands.is_empty() {
        "- none".to_string()
    } else {
        hands.iter().map(|(id, agent)| format!("- {id} {agent}")).collect::<Vec<_>>().join("\n")
    }
}

fn checkpoint_authoring_prompt(n: u64, hands_block: &str, failure_note: Option<&str>) -> String {
    let mut prompt = format!(
        "[arx] checkpoint {n}: author the state object from this window, not from any earlier object. Reply with the object body only, in this shape and nothing else:\n"
    );
    if let Some(note) = failure_note {
        prompt.push_str(note);
        prompt.push('\n');
    }
    prompt.push_str(
        "## Position\n<what is in flight, what is decided, what is open, the next concrete move, which files are hot>\n## Target\n<the target>\n## Drift\n<whether the target drifted, and how>\n## Hands in flight\n",
    );
    prompt.push_str(hands_block);
    prompt.push('\n');
    prompt.push_str(
        "Cite a file as `- file: <path> @ <hash>` where <hash> is the output of `git hash-object <path>` run now, and a decision as `- decision: <id>`. Do not cite anything you have not verified in this window.",
    );
    prompt
}

fn checkpoint_truth_check_prompt(n: u64, body: &str) -> String {
    format!(
        "[arx] checkpoint {n}: truth-check. Below is the object you just authored. Check every line against this window and against disk. Reply `ok` if every line holds; otherwise reply with the corrected object body in full, same shape.\n{body}"
    )
}

fn resolve_truth_check(authored: &str, reply: &str) -> String {
    if reply.trim().eq_ignore_ascii_case("ok") {
        authored.to_string()
    } else {
        reply.to_string()
    }
}

fn render_check_failures(failures: &[CheckFailure]) -> String {
    failures
        .iter()
        .map(|failure| format!("{:?} {} ({})", failure.probe, failure.entry, failure.detail))
        .collect::<Vec<_>>()
        .join("; ")
}

fn apply_no_store_scope_override(failures: Vec<CheckFailure>, no_scope: bool) -> Vec<CheckFailure> {
    if !no_scope {
        return failures;
    }
    failures
        .into_iter()
        .map(|failure| {
            if failure.probe == Probe::DecisionResolves {
                CheckFailure {
                    probe: failure.probe,
                    entry: failure.entry,
                    detail: "no store scope".to_string(),
                }
            } else {
                failure
            }
        })
        .collect()
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn rfc3339_utc_seconds(total_secs: u64) -> String {
    let days = (total_secs / 86_400) as i64;
    let secs_of_day = total_secs % 86_400;
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn rfc3339_utc_seconds_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    rfc3339_utc_seconds(secs)
}

fn hand_nudge(delivery: &Delivery, threshold: u64) -> Option<Event> {
    let Delivery::Verdict { hand, usage: Some(usage), .. } = delivery else {
        return None;
    };
    let occupancy = transcript::occupancy(usage);
    if occupancy < threshold {
        return None;
    }
    Some(Event::Nudge {
        seat: "hand".to_string(),
        hand: Some(hand.clone()),
        occupancy,
        threshold,
    })
}

enum CycleFailure {
    Direct,
    ViaClone,
    WindowExceeded,
    BodyUnavailable,
}

fn decide(gate: &mut dyn Gate, gate_note: &RefCell<Option<DecisionNote>>, call: &GateCall) -> Decision {
    let verdict = gate.decide(call);
    match gate_note.borrow_mut().take() {
        Some(DecisionNote { by, reason }) => Decision { verdict, by, reason },
        None => Decision {
            verdict,
            by: None,
            reason: None,
        },
    }
}

fn canonicalize_in_workspace(root: &Path, path: &str, tool: &str, extra_root: Option<&Path>) -> Result<(PathBuf, PathBuf, String), String> {
    let canonical_root = root.canonicalize().map_err(|err| format!("{tool} error: {path}: {err}"))?;
    let candidate = root.join(path);
    let canonical = candidate.canonicalize().map_err(|err| format!("{tool} error: {path}: {err}"))?;
    if canonical.starts_with(&canonical_root) {
        let rel = canonical.strip_prefix(&canonical_root).unwrap_or(&canonical).display().to_string();
        return Ok((canonical, canonical_root, rel));
    }
    if let Some(extra_root) = extra_root {
        if let Ok(canonical_extra_root) = extra_root.canonicalize() {
            if canonical.starts_with(&canonical_extra_root) {
                let rel = canonical.display().to_string();
                return Ok((canonical, canonical_root, rel));
            }
        }
    }
    Err(format!("{tool} error: path escapes the workspace"))
}

fn is_arx_state(canonical_root: &Path, canonical: &Path) -> bool {
    canonical.starts_with(canonical_root.join(config::DEFAULT_STORE_DIRNAME))
        || canonical.starts_with(canonical_root.join("threads"))
        || canonical == canonical_root.join("pid.txt")
}

fn read_file_call(root: &Path, args: &serde_json::Value, ctx: &mut AdmissionCtx) -> (serde_json::Value, String, Vec<Event>) {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or_default();
    let offset = args.get("offset").and_then(|v| v.as_u64());
    let limit = args.get("limit").and_then(|v| v.as_u64());
    let (canonical, _canonical_root, rel) = match canonicalize_in_workspace(root, path, "read_file", Some(&ctx.root.fallback_dir)) {
        Ok(v) => v,
        Err(err) => {
            return (serde_json::json!({"error": err.clone()}), format!("{path}: {err}"), Vec::new());
        }
    };
    let raw = match fs::read(&canonical) {
        Ok(b) => b,
        Err(err) => {
            let err_text = format!("read_file error: {path}: {err}");
            return (serde_json::json!({"error": err_text.clone()}), format!("{rel}: {err_text}"), Vec::new());
        }
    };
    let raw_len = raw.len();
    let sha = admission::sha256_hex(&raw);
    let content = match String::from_utf8(raw) {
        Ok(s) => s,
        Err(_) => {
            let err_text = format!("read_file error: {rel} is not UTF-8");
            return (serde_json::json!({"error": err_text.clone()}), format!("{rel}: {err_text}"), Vec::new());
        }
    };
    let ranged = offset.is_some() || limit.is_some();
    let sliced = match admission::slice_lines(&content, offset, limit) {
        Ok(s) => s,
        Err(err) => {
            return (serde_json::json!({"error": err.clone()}), format!("{rel}: {err}"), Vec::new());
        }
    };
    let source = if ranged {
        let total_lines = content.lines().count() as u64;
        let start_line = offset.unwrap_or(1);
        let end_line = match limit {
            Some(l) => std::cmp::min(total_lines, start_line.saturating_sub(1).saturating_add(l)),
            None => total_lines,
        };
        format!("{rel}#L{start_line}-L{end_line}")
    } else {
        rel.clone()
    };
    let trace_rooted = admission::is_trace_rooted(&ctx.threads_dir, &canonical).then(|| rel.clone());
    let req = admission::Request {
        tool: "read_file",
        args,
        body: &sliced,
        provenance: admission::Provenance { source, sha256: Some(sha) },
        trace_rooted,
        ranged,
        ordinal: ctx.turn,
    };
    match admission::admit(ctx.policy, &mut *ctx.cache, &ctx.root, &mut *ctx.seq, req) {
        Ok(admitted) => (
            serde_json::json!({"text": admitted.text}),
            format!("{rel}: read {raw_len} bytes, {}", admitted.outcome),
            admitted.events,
        ),
        Err(err) => (serde_json::json!({"error": err.clone()}), format!("{rel}: {err}"), Vec::new()),
    }
}

fn edit_file_call(root: &Path, args: &serde_json::Value, ctx: &mut AdmissionCtx) -> (serde_json::Value, String, Vec<Event>) {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or_default();
    let old = args.get("old").and_then(|v| v.as_str()).unwrap_or_default();
    let new = args.get("new").and_then(|v| v.as_str()).unwrap_or_default();
    let (canonical, canonical_root, rel) = match canonicalize_in_workspace(root, path, "edit_file", None) {
        Ok(v) => v,
        Err(err) => {
            return (serde_json::json!({"error": err.clone()}), format!("{path}: {err}"), Vec::new());
        }
    };
    if is_arx_state(&canonical_root, &canonical) {
        let err_text = format!("edit_file error: {rel} is arx's own state");
        return (serde_json::json!({"error": err_text.clone()}), format!("{rel}: {err_text}"), Vec::new());
    }
    let bytes = match fs::read(&canonical) {
        Ok(b) => b,
        Err(err) => {
            let err_text = format!("edit_file error: {path}: {err}");
            return (serde_json::json!({"error": err_text.clone()}), format!("{rel}: {err_text}"), Vec::new());
        }
    };
    let content = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            let err_text = format!("edit_file error: {rel} is not UTF-8");
            return (serde_json::json!({"error": err_text.clone()}), format!("{rel}: {err_text}"), Vec::new());
        }
    };
    let count = content.matches(old).count();
    if count != 1 {
        let err_text = format!("edit_file error: old text matches {count} times");
        return (serde_json::json!({"error": err_text.clone()}), format!("{rel}: {err_text}"), Vec::new());
    }
    let updated = content.replacen(old, new, 1);
    if let Err(err) = fs::write(&canonical, updated) {
        let err_text = format!("edit_file error: {path}: {err}");
        return (serde_json::json!({"error": err_text.clone()}), format!("{rel}: {err_text}"), Vec::new());
    }
    let confirmation = format!("edited {rel}");
    let req = admission::Request {
        tool: "edit_file",
        args,
        body: &confirmation,
        provenance: admission::Provenance {
            source: rel.clone(),
            sha256: None,
        },
        trace_rooted: None,
        ranged: false,
        ordinal: ctx.turn,
    };
    match admission::admit(ctx.policy, &mut *ctx.cache, &ctx.root, &mut *ctx.seq, req) {
        Ok(admitted) => (
            serde_json::json!({"text": admitted.text}),
            format!("{rel}: edit\n--- old\n{old}\n+++ new\n{new}"),
            admitted.events,
        ),
        Err(err) => (serde_json::json!({"error": err.clone()}), format!("{rel}: {err}"), Vec::new()),
    }
}

fn list_files_call(root: &Path, args: &serde_json::Value, ctx: &mut AdmissionCtx) -> (serde_json::Value, String, Vec<Event>) {
    const MAX_ENTRIES: usize = 2000;
    let raw_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let path = if raw_path.is_empty() { "." } else { raw_path };
    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2).clamp(1, 8);
    let (canonical, canonical_root, rel) = match canonicalize_in_workspace(root, path, "list_files", None) {
        Ok(v) => v,
        Err(err) => {
            return (serde_json::json!({"error": err.clone()}), format!("{path}: {err}"), Vec::new());
        }
    };
    let rel = if rel.is_empty() { String::from(".") } else { rel };
    if !canonical.is_dir() {
        let err_text = format!("list_files error: {rel} is not a directory");
        return (serde_json::json!({"error": err_text.clone()}), format!("{rel}: {err_text}"), Vec::new());
    }
    let mut lines: Vec<String> = Vec::new();
    let mut truncated = false;
    let mut stack: Vec<(PathBuf, u64)> = vec![(canonical.clone(), 0)];
    while let Some((dir, level)) = stack.pop() {
        let mut entries: Vec<PathBuf> = match fs::read_dir(&dir) {
            Ok(read_dir) => read_dir.filter_map(|entry| entry.ok()).map(|entry| entry.path()).collect(),
            Err(_) => continue,
        };
        entries.sort();
        for entry in entries {
            if lines.len() >= MAX_ENTRIES {
                truncated = true;
                break;
            }
            let name = entry.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            if is_arx_state(&canonical_root, &entry) {
                continue;
            }
            let shown = entry.strip_prefix(&canonical_root).unwrap_or(&entry).display().to_string();
            if entry.is_dir() {
                lines.push(format!("{shown}/"));
                let stays_in_root = entry.canonicalize().map(|canonical| canonical.starts_with(&canonical_root)).unwrap_or(false);
                if stays_in_root && level + 1 < depth {
                    stack.push((entry, level + 1));
                }
            } else {
                lines.push(shown);
            }
        }
        if truncated {
            break;
        }
    }
    lines.sort();
    let count = lines.len();
    let mut body = if lines.is_empty() { String::from("(no entries)") } else { lines.join("\n") };
    if truncated {
        body.push_str(&format!("\n[arx] listing truncated at {MAX_ENTRIES} entries; narrow path or lower depth"));
    }
    let sha = admission::sha256_hex(body.as_bytes());
    let req = admission::Request {
        tool: "list_files",
        args,
        body: &body,
        provenance: admission::Provenance { source: format!("{rel}#depth{depth}"), sha256: Some(sha) },
        trace_rooted: None,
        ranged: false,
        ordinal: ctx.turn,
    };
    match admission::admit(ctx.policy, &mut *ctx.cache, &ctx.root, &mut *ctx.seq, req) {
        Ok(admitted) => (
            serde_json::json!({"text": admitted.text}),
            format!("{rel}: list {count} entries, depth {depth}, {}", admitted.outcome),
            admitted.events,
        ),
        Err(err) => (serde_json::json!({"error": err.clone()}), format!("{rel}: {err}"), Vec::new()),
    }
}

fn file_call(root: &Path, call: &FileCall, ctx: &mut AdmissionCtx) -> (serde_json::Value, String, Vec<Event>) {
    match call.tool.as_str() {
        "read_file" => read_file_call(root, &call.args, ctx),
        "edit_file" => edit_file_call(root, &call.args, ctx),
        "list_files" => list_files_call(root, &call.args, ctx),
        other => {
            let err_text = format!("{other} error: unknown file tool");
            (serde_json::json!({"error": err_text.clone()}), err_text, Vec::new())
        }
    }
}

fn handler_gate_call(verb_or_tool: &str, args: &serde_json::Value) -> GateCall {
    GateCall {
        tool: format!("mcp__arx__{verb_or_tool}"),
        input: args.clone(),
    }
}

fn gate_denied_result(tool: &str, reason: String) -> serde_json::Value {
    let reason = if reason.is_empty() { "deny".to_string() } else { reason };
    serde_json::json!({"error": format!("gate denied {tool}: {reason}")})
}

fn log_event(log: &mut TranscriptLog, front: &mut dyn Front, event: &Event) -> Result<(), transcript::TranscriptError> {
    log.append_event(event)?;
    let _ = front.event(event);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn gate_call(
    gate: &RefCell<&mut dyn Gate>,
    gate_note: &RefCell<Option<DecisionNote>>,
    call: &GateCall,
    log_cell: &RefCell<&mut TranscriptLog>,
    front_cell: &RefCell<&mut dyn Front>,
    updated_input_notice: &RefCell<Option<String>>,
    repo_gate_path: &Path,
    repo_tier: &RefCell<GateTier>,
    gate_err: &RefCell<Option<transcript::TranscriptError>>,
    gate_io_err: &RefCell<Option<io::Error>>,
    ask_err: &RefCell<Option<String>>,
    adapter_for_kill: &mut ManuallyDrop<MainBody>,
) -> GateVerdict {
    if gate_err.borrow().is_some() || gate_io_err.borrow().is_some() || ask_err.borrow().is_some() {
        return GateVerdict::Deny {
            reason: "session failing".to_string(),
        };
    }
    let decision = decide(&mut **gate.borrow_mut(), gate_note, call);
    if let Some(message) = updated_input_notice.borrow_mut().take() {
        if let Err(err) = front_cell.borrow_mut().render(&TerminalMsg::Notice { text: message }) {
            *gate_io_err.borrow_mut() = Some(err);
            adapter_for_kill.kill();
            return GateVerdict::Deny {
                reason: "session failing".to_string(),
            };
        }
    }
    let label = match &decision.verdict {
        GateVerdict::Allow => "allow",
        GateVerdict::Deny { .. } => "deny",
    };
    if let Err(err) = log_event(
        &mut **log_cell.borrow_mut(),
        &mut **front_cell.borrow_mut(),
        &Event::Gate {
            tool: call.tool.clone(),
            verdict: label.to_string(),
            by: decision.by,
            reason: decision.reason,
        },
    ) {
        *gate_err.borrow_mut() = Some(err);
        adapter_for_kill.kill();
        return GateVerdict::Deny {
            reason: "gate line failed to write".to_string(),
        };
    }
    if matches!(decision.verdict, GateVerdict::Allow) {
        if let Some(tool) = call.tool.strip_prefix("mcp__arx__") {
            let _ = front_cell.borrow_mut().render(&TerminalMsg::ToolCall {
                tool: tool.to_string(),
                subject: gate::subject(call),
            });
        }
    }
    decision.verdict
}

struct AdmissionCtx<'a> {
    policy: &'a admission::AdmissionPolicy,
    cache: &'a mut admission::Cache,
    root: admission::TraceRoot,
    threads_dir: PathBuf,
    turn: u64,
    seq: &'a mut u64,
}

struct PendingMainSelection {
    model: String,
    clear: bool,
    path: PathBuf,
    previous_pin: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn run_turn_cycle(
    role: &str,
    text: &str,
    images: &[main_body::Attachment],
    adapter: &mut MainBody,
    adapter_for_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_model_fallback_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_store_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_hand_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_file_kill: &mut ManuallyDrop<MainBody>,
    log: &mut TranscriptLog,
    front: &mut dyn Front,
    gate: &mut dyn Gate,
    gate_note: &RefCell<Option<DecisionNote>>,
    repo_tier: &RefCell<GateTier>,
    updated_input_notice: &RefCell<Option<String>>,
    repo_gate_path: &Path,
    store: &mut dyn Store,
    ledger: &mut SpawnLedger,
    last_handle: &mut transcript::Handle,
    resolved: &Resolved,
    roster: &mut Roster,
    pending_model: &mut Option<PendingMainSelection>,
    current_model: &mut String,
    config_dir: &Path,
    user_gate: &config::GateConfig,
    nudge: &mut Nudge,
    checkpoint: &mut Checkpoint,
    pending_drains: &mut HashMap<HandId, Vec<String>>,
    proposals: &mut HashMap<HandId, Vec<Suggestion>>,
    admission_cache: &mut admission::Cache,
    admission_seq: &mut u64,
) -> Result<(), CycleFailure> {
    let text = match nudge.pending.take() {
        Some(pending) => format!("{pending}\n\n{text}"),
        None => text.to_string(),
    };
    let text = text.as_str();

    match log.append_turn_as(role, text) {
        Ok(handle) => *last_handle = handle,
        Err(err) => {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
    }

    if role == "arx" {
        if let Err(err) = front.render(&TerminalMsg::Notice { text: text.to_string() }) {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
    }

    let mut chunk_err: Option<io::Error> = None;
    let mut store_err: Option<StoreError> = None;
    let mut hand_err: Option<ledger::LedgerError> = None;
    let mut file_err: Option<transcript::TranscriptError> = None;

    let log_cell = RefCell::new(log);
    let front_cell: RefCell<&mut dyn Front> = RefCell::new(front);
    let gate_cell: RefCell<&mut dyn Gate> = RefCell::new(gate);
    let gate_err_cell: RefCell<Option<transcript::TranscriptError>> = RefCell::new(None);
    let gate_io_err_cell: RefCell<Option<io::Error>> = RefCell::new(None);
    let ask_err_cell: RefCell<Option<String>> = RefCell::new(None);
    let event_err_cell: RefCell<Option<transcript::TranscriptError>> = RefCell::new(None);
    let ledger_cell: RefCell<&mut SpawnLedger> = RefCell::new(ledger);

    let threads_dir = config_dir.join("threads");
    let threads_dir = threads_dir.canonicalize().unwrap_or(threads_dir);
    let admission_root = admission::TraceRoot {
        host_dir: threads_dir.join(format!("{}.traces", last_handle.thread)),
        handle_prefix: format!("threads/{}.traces", last_handle.thread),
        fallback_dir: std::env::temp_dir().join("arx-traces").join(&last_handle.thread),
    };
    let admission_ctx_cell = RefCell::new(AdmissionCtx {
        policy: &resolved.admission,
        cache: admission_cache,
        root: admission_root,
        threads_dir,
        turn: last_handle.turn,
        seq: admission_seq,
    });

    let turn_started = Instant::now();
    let outcome = adapter.run_turn_controlled(
        text,
        images,
        |delta| {
            if chunk_err.is_some() {
                return;
            }
            if let Err(err) = front_cell.borrow_mut().render(&TerminalMsg::Chunk {
                text: delta.to_string(),
            }) {
                chunk_err = Some(err);
                adapter_for_kill.kill();
            }
        },
        |from, to| {
            if event_err_cell.borrow().is_some() {
                return;
            }
            if let Err(err) = log_event(
                &mut **log_cell.borrow_mut(),
                &mut **front_cell.borrow_mut(),
                &Event::Model {
                    position: "main".to_string(),
                    from: from.to_string(),
                    to: to.to_string(),
                },
            ) {
                *event_err_cell.borrow_mut() = Some(err);
                adapter_for_model_fallback_kill.kill();
                return;
            }
            *current_model = to.to_string();
        },
        |call: &StoreCall| -> serde_json::Value {
            if store_err.is_some() {
                return serde_json::json!({"error": "session failing"});
            }
            let gate_input = handler_gate_call(&call.verb, &call.args);
            if let GateVerdict::Deny { reason } = gate_call(
                &gate_cell,
                gate_note,
                &gate_input,
                &log_cell,
                &front_cell,
                updated_input_notice,
                repo_gate_path,
                repo_tier,
                &gate_err_cell,
                &gate_io_err_cell,
                &ask_err_cell,
                adapter_for_store_kill,
            ) {
                return gate_denied_result(&gate_input.tool, reason);
            }
            let claim_handle = ClaimHandle {
                thread: last_handle.thread.clone(),
                turn: last_handle.turn,
            };
            match call.verb.as_str() {
                "save" => match on_store_save(store, &call.args, &claim_handle, &front_cell) {
                    Ok(value) => value,
                    Err(StoreError::Io { path, source }) => {
                        store_err = Some(StoreError::Io { path, source });
                        adapter_for_store_kill.kill();
                        serde_json::json!({"error": "store write failed"})
                    }
                    Err(err) => serde_json::json!({"error": err.to_string()}),
                },
                "drain" => match on_store_drain(
                    store,
                    &call.args,
                    &claim_handle,
                    resolved,
                    roster,
                    config_dir,
                    user_gate,
                    &ledger_cell,
                    &log_cell,
                    &front_cell,
                    &event_err_cell,
                    adapter_for_store_kill,
                    pending_drains,
                    proposals,
                ) {
                    Ok(value) => value,
                    Err(StoreError::Io { path, source }) => {
                        store_err = Some(StoreError::Io { path, source });
                        adapter_for_store_kill.kill();
                        serde_json::json!({"error": "store write failed"})
                    }
                    Err(err) => serde_json::json!({"error": err.to_string()}),
                },
                _ => arx_engine::serve(store, &call.verb, &call.args, &claim_handle),
            }
        },
        |call: &HandCall| -> serde_json::Value {
            let gate_input = handler_gate_call(&call.verb, &call.args);
            if let GateVerdict::Deny { reason } = gate_call(
                &gate_cell,
                gate_note,
                &gate_input,
                &log_cell,
                &front_cell,
                updated_input_notice,
                repo_gate_path,
                repo_tier,
                &gate_err_cell,
                &gate_io_err_cell,
                &ask_err_cell,
                adapter_for_hand_kill,
            ) {
                return gate_denied_result(&gate_input.tool, reason);
            }
            let value = on_hand_call(
                &ledger_cell,
                &mut hand_err,
                &event_err_cell,
                adapter_for_hand_kill,
                call,
                resolved,
                roster,
                config_dir,
                user_gate,
                &log_cell,
                &front_cell,
            );
            if hand_err.is_some() || event_err_cell.borrow().is_some() {
                return value;
            }
            let body = serde_json::to_string(&value).unwrap_or_default();
            let mut ctx_guard = admission_ctx_cell.borrow_mut();
            let ctx: &mut AdmissionCtx = &mut ctx_guard;
            let req = admission::Request {
                tool: &call.verb,
                args: &call.args,
                body: &body,
                provenance: admission::Provenance {
                    source: call.verb.clone(),
                    sha256: None,
                },
                trace_rooted: None,
                ranged: false,
                ordinal: ctx.turn,
            };
            match admission::admit(ctx.policy, &mut *ctx.cache, &ctx.root, &mut *ctx.seq, req) {
                Ok(admitted) => {
                    for event in &admitted.events {
                        if let Err(err) = log_event(&mut **log_cell.borrow_mut(), &mut **front_cell.borrow_mut(), event) {
                            *event_err_cell.borrow_mut() = Some(err);
                            adapter_for_hand_kill.kill();
                            return serde_json::json!({"error": "event line failed to write"});
                        }
                    }
                    if admitted.outcome == "inline" {
                        value
                    } else {
                        serde_json::json!({"text": admitted.text})
                    }
                }
                Err(err) => serde_json::json!({"error": err}),
            }
        },
        |call: &FileCall| -> serde_json::Value {
            if file_err.is_some() {
                return serde_json::json!({"error": "session failing"});
            }
            let gate_input = handler_gate_call(&call.tool, &call.args);
            if let GateVerdict::Deny { reason } = gate_call(
                &gate_cell,
                gate_note,
                &gate_input,
                &log_cell,
                &front_cell,
                updated_input_notice,
                repo_gate_path,
                repo_tier,
                &gate_err_cell,
                &gate_io_err_cell,
                &ask_err_cell,
                adapter_for_file_kill,
            ) {
                return gate_denied_result(&gate_input.tool, reason);
            }
            let (result, row_text, events) = file_call(config_dir, call, &mut admission_ctx_cell.borrow_mut());
            if let Err(err) = log_cell.borrow_mut().append_file(&row_text) {
                file_err = Some(err);
                adapter_for_file_kill.kill();
                return serde_json::json!({"error": "event line failed to write"});
            }
            for event in &events {
                if let Err(err) = log_event(&mut **log_cell.borrow_mut(), &mut **front_cell.borrow_mut(), event) {
                    file_err = Some(err);
                    adapter_for_file_kill.kill();
                    return serde_json::json!({"error": "event line failed to write"});
                }
            }
            result
        },
        &mut || front_cell.borrow_mut().poll_control(),
    );
    let turn_elapsed_ms = turn_started.elapsed().as_millis() as u64;

    let log = log_cell.into_inner();
    let front = front_cell.into_inner();
    drop(gate_cell);

    if let Some(err) = ask_err_cell.into_inner() {
        eprintln!("error: {err}");
        return Err(CycleFailure::ViaClone);
    }

    if let Some(err) = gate_io_err_cell.into_inner() {
        eprintln!("error: {err}");
        return Err(CycleFailure::ViaClone);
    }

    if let Some(err) = gate_err_cell.into_inner() {
        eprintln!("error: {err}");
        return Err(CycleFailure::ViaClone);
    }

    if let Some(err) = store_err {
        eprintln!("error: {err}");
        return Err(CycleFailure::ViaClone);
    }

    if let Some(err) = chunk_err {
        eprintln!("error: {err}");
        return Err(CycleFailure::ViaClone);
    }

    if let Some(err) = hand_err {
        eprintln!("error: {err}");
        return Err(CycleFailure::ViaClone);
    }

    if let Some(err) = event_err_cell.into_inner() {
        eprintln!("error: {err}");
        return Err(CycleFailure::ViaClone);
    }

    if let Some(err) = file_err {
        eprintln!("error: {err}");
        return Err(CycleFailure::ViaClone);
    }

    let selection_result = adapter.take_model_selection_result();
    if let Some(selection) = pending_model.take() {
        let notice = match selection_result {
            Some(Ok(model)) if model == selection.model => {
                let from = std::mem::replace(current_model, model.clone());
                log_event(log, front, &Event::Model { position: "main".into(), from: from.clone(), to: model.clone() })
                    .map_err(|_| CycleFailure::Direct)?;
                if selection.clear {
                    format!("[arx] model: main cleared; falling back to {model} from arx.toml")
                } else {
                    render_model_switched(&from, &model)
                }
            }
            result => {
                let mut trial = roster.clone();
                match selection.previous_pin {
                    Some(pin) => trial.pin("main", &pin),
                    None => { trial.clear("main"); },
                }
                if let Err(err) = trial.save(&selection.path) {
                    eprintln!("error: failed to restore previous main pin: {err}; main stopped");
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
                *roster = trial;
                let message = match result {
                    Some(Err(message)) => message,
                    _ => "turn did not activate the pending model; selection was not changed".into(),
                };
                render_model_switch_failed(&message)
            }
        };
        *last_handle = log.append_turn_as("arx", &notice).map_err(|_| CycleFailure::Direct)?;
        front.render(&TerminalMsg::Notice { text: notice }).map_err(|_| CycleFailure::Direct)?;
    }

    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(main_body::MainError::VendorCompaction { text }) => {
            if !text.is_empty() {
                *last_handle = log.append_turn_as("main", &text).map_err(|_| CycleFailure::Direct)?;
            }
            log_event(
                log,
                front,
                &Event::Reset {
                    phase: "recovery".into(),
                    check: Some("codex_compaction".into()),
                },
            )
            .map_err(|_| CycleFailure::Direct)?;
            front
                .render(&TerminalMsg::Notice {
                    text: "[arx] Codex compacted its context; recovering from the last checked state and transcript".into(),
                })
                .map_err(|_| CycleFailure::Direct)?;
            adapter.kill();
            return Err(CycleFailure::WindowExceeded);
        }
        Err(main_body::MainError::ContextWindowExceeded { .. }) => {
            adapter.kill();
            return Err(CycleFailure::WindowExceeded);
        }
        Err(main_body::MainError::UsageLimitReached { message }) => {
            if let Err(err) = front.render(&TerminalMsg::Notice {
                text: format!("[arx] main unavailable: {message}"),
            }) {
                eprintln!("error: {err}");
            }
            adapter.kill();
            return Err(CycleFailure::BodyUnavailable);
        }
        Err(err) => {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
    };

    match log.append_turn_as("main", &outcome.text) {
        Ok(handle) => *last_handle = handle,
        Err(err) => {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
    }

    if let Some(usage) = &outcome.usage {
        if let Err(err) = front.render(&TerminalMsg::Usage {
            input: usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            output: usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            cache_creation: usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            cache_read: usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        }) {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
        let mut usage_row = usage.clone();
        let model_mismatch = apply_usage_model_fields(&mut usage_row, outcome.models.as_deref(), current_model);
        if let Err(err) = log.append_usage_timed(&usage_row, Some(turn_elapsed_ms)) {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
        if let Some(text) = model_mismatch {
            if let Err(err) = front.render(&TerminalMsg::Notice { text }) {
                eprintln!("error: {err}");
                adapter.kill();
                return Err(CycleFailure::Direct);
            }
        }
        checkpoint.last_occupancy = transcript::occupancy(usage);
        if !nudge.said {
            let occupancy = transcript::occupancy(usage);
            let threshold = nudge.threshold;
            if occupancy >= threshold {
                nudge.said = true;
                if let Err(err) = log_event(
                    log,
                    front,
                    &Event::Nudge {
                        seat: "main".to_string(),
                        hand: None,
                        occupancy,
                        threshold,
                    },
                ) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
                let notice = format!("nudge: main at {occupancy} tokens, past {threshold}");
                if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
                nudge.pending = Some(format!(
                    "[arx] nudge: this window holds {occupancy} tokens, past the {threshold} line. No action is required."
                ));
            }
        }
    }

    if let Err(err) = front.render(&TerminalMsg::TurnEnd {
        text: outcome.text.clone(),
    }) {
        eprintln!("error: {err}");
        adapter.kill();
        return Err(CycleFailure::Direct);
    }

    if outcome.stopped {
        if let Err(err) = front.render(&TerminalMsg::Stopped) {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
    }

    if let Err(err) = front.render(&build_status(resolved, checkpoint, nudge, current_model, last_handle.turn)) {
        eprintln!("error: {err}");
        adapter.kill();
        return Err(CycleFailure::Direct);
    }

    Ok(())
}

fn apply_usage_model_fields(usage_row: &mut serde_json::Value, models: Option<&[String]>, current_model: &str) -> Option<String> {
    let fields = usage_row.as_object_mut()?;
    match models {
        Some([single]) => {
            fields.insert("model".to_string(), serde_json::Value::String(single.clone()));
            fields.insert("model_source".to_string(), serde_json::Value::String("sdk".to_string()));
            if single.as_str() != current_model {
                return Some(format!("[arx] model: sidecar reports {single}, arx expected {current_model}"));
            }
            None
        }
        Some(models) if !models.is_empty() => {
            fields.insert("models".to_string(), serde_json::Value::String(models.join("+")));
            fields.insert("model_source".to_string(), serde_json::Value::String("sdk".to_string()));
            None
        }
        _ => {
            fields.insert("model".to_string(), serde_json::Value::String(current_model.to_string()));
            fields.insert("model_source".to_string(), serde_json::Value::String("arx".to_string()));
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_checkpoint_ritual(
    adapter: &mut MainBody,
    adapter_for_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_model_fallback_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_store_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_hand_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_file_kill: &mut ManuallyDrop<MainBody>,
    log: &mut TranscriptLog,
    front: &mut dyn Front,
    gate: &mut dyn Gate,
    gate_note: &RefCell<Option<DecisionNote>>,
    repo_tier: &RefCell<GateTier>,
    updated_input_notice: &RefCell<Option<String>>,
    repo_gate_path: &Path,
    store: &mut dyn Store,
    ledger: &mut SpawnLedger,
    last_handle: &mut transcript::Handle,
    resolved: &Resolved,
    roster: &mut Roster,
    pending_model: &mut Option<PendingMainSelection>,
    current_model: &mut String,
    config_dir: &Path,
    user_gate: &config::GateConfig,
    nudge: &mut Nudge,
    checkpoint: &mut Checkpoint,
    pending_drains: &mut HashMap<HandId, Vec<String>>,
    proposals: &mut HashMap<HandId, Vec<Suggestion>>,
    admission_cache: &mut admission::Cache,
    admission_seq: &mut u64,
) -> Result<RitualOutcome, CycleFailure> {
    let threads_dir = config_dir.join("threads");
    let checkpoints_dir = state_object::checkpoints_dir(&threads_dir, "main");
    let n = match state_object::latest(&checkpoints_dir) {
        Ok(Some((ordinal, _))) => ordinal + 1,
        Ok(None) => 1,
        Err(err) => {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
    };

    let hands = ledger.in_flight_hands();
    let hands_block = format_hands_block(&hands);
    let store_scope = resolved.store_scope.clone();
    let scope = store_scope.as_deref().unwrap_or("");

    let mut failure_note: Option<String> = None;

    for round in 1..=2u32 {
        let authoring_prompt = checkpoint_authoring_prompt(n, &hands_block, failure_note.as_deref());
        run_turn_cycle(
            "arx",
            &authoring_prompt,
            &[],
            adapter,
            adapter_for_kill,
            adapter_for_model_fallback_kill,
            adapter_for_store_kill,
            adapter_for_hand_kill,
            adapter_for_file_kill,
            log,
            front,
            gate,
            gate_note,
            repo_tier,
            updated_input_notice,
            repo_gate_path,
            store,
            ledger,
            last_handle,
            resolved,
            roster,
            pending_model,
            &mut *current_model,
            config_dir,
            user_gate,
            nudge,
            checkpoint,
            pending_drains,
            proposals,
            admission_cache,
            admission_seq,
        )?;
        let authored_handle = log.last_handle().expect("checkpoint authoring turn was just committed");
        let authored_body = match transcript::resolve(&threads_dir, &authored_handle) {
            Ok(text) => text,
            Err(err) => {
                eprintln!("error: {err}");
                adapter.kill();
                return Err(CycleFailure::Direct);
            }
        };

        let truth_check_prompt = checkpoint_truth_check_prompt(n, &authored_body);
        run_turn_cycle(
            "arx",
            &truth_check_prompt,
            &[],
            adapter,
            adapter_for_kill,
            adapter_for_model_fallback_kill,
            adapter_for_store_kill,
            adapter_for_hand_kill,
            adapter_for_file_kill,
            log,
            front,
            gate,
            gate_note,
            repo_tier,
            updated_input_notice,
            repo_gate_path,
            store,
            ledger,
            last_handle,
            resolved,
            roster,
            pending_model,
            &mut *current_model,
            config_dir,
            user_gate,
            nudge,
            checkpoint,
            pending_drains,
            proposals,
            admission_cache,
            admission_seq,
        )?;
        let truth_handle = log.last_handle().expect("checkpoint truth-check turn was just committed");
        let truth_reply = match transcript::resolve(&threads_dir, &truth_handle) {
            Ok(text) => text,
            Err(err) => {
                eprintln!("error: {err}");
                adapter.kill();
                return Err(CycleFailure::Direct);
            }
        };

        let body = resolve_truth_check(&authored_body, &truth_reply);
        let turn_handle = if truth_reply.trim().eq_ignore_ascii_case("ok") {
            authored_handle
        } else {
            truth_handle
        };

        let obj = StateObject {
            header: Header {
                thread: "main".to_string(),
                checkpoint: n,
                at: rfc3339_utc_seconds_now(),
                turn: turn_handle.turn,
            },
            body,
        };

        let delta = checkpoint.last_occupancy.saturating_sub(checkpoint.base);
        match state_object::write_checked(&checkpoints_dir, &obj, config_dir, store, scope) {
            Ok(_) => {
                if let Err(err) = log_event(
                    log,
                    front,
                    &Event::Checkpoint {
                        check: "pass".to_string(),
                        delta,
                    },
                ) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
                checkpoint.number = Some(n);
                checkpoint.written_delta = Some(delta);
                checkpoint.base = checkpoint.last_occupancy;
                let notice = format!("[arx] checkpoint {n} written: threads/main.checkpoints/{n}.md");
                if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
                return Ok(RitualOutcome::Written(n));
            }
            Err(failures) => {
                let failures = apply_no_store_scope_override(failures, store_scope.is_none());
                if let Err(err) = log_event(
                    log,
                    front,
                    &Event::Checkpoint {
                        check: "fail".to_string(),
                        delta,
                    },
                ) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
                let joined = render_check_failures(&failures);
                let notice = format!("[arx] checkpoint {n}: check failed — {joined}");
                if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
                if round == 2 {
                    let notice = format!("[arx] checkpoint {n} abandoned after two rounds");
                    if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return Err(CycleFailure::Direct);
                    }
                    return Ok(RitualOutcome::Abandoned);
                }
                failure_note = Some(format!("The mechanical check failed on your last attempt: {joined} Fix every one."));
            }
        }
    }

    unreachable!("checkpoint ritual always returns within two rounds")
}

fn orientation_prompt_with_object(
    thread: &str,
    n: u64,
    t: u64,
    body: &str,
    gap: Option<&str>,
    note: Option<&str>,
) -> String {
    let mut prompt = format!("[arx] this window is a respawn after reset in thread {thread}. ");
    if let Some(gap) = gap {
        prompt.push_str(gap);
        prompt.push(' ');
    }
    prompt.push_str(&format!(
        "Your predecessor's state object, checkpoint {n}, is below; the transcript is at threads/{thread}.jsonl, turns 1 to {t}, readable with read_file. Report in a few lines what you can resume from the object and what you cannot determine, then stop; the operator reads this before typing.\n\n{body}"
    ));
    if let Some(note) = note {
        prompt.push_str(&format!("\nOperator note: {note}"));
    }
    prompt
}

fn orientation_prompt_without_object(thread: &str, t: u64, gap: Option<&str>) -> String {
    let mut prompt = format!("[arx] this window is a respawn after reset in thread {thread}. ");
    if let Some(gap) = gap {
        prompt.push_str(gap);
        prompt.push(' ');
    }
    prompt.push_str(&format!(
        "No state object exists for this thread; orient from the transcript at threads/{thread}.jsonl, turns 1 to {t}, readable with read_file. Report in a few lines what you can determine from it and what you cannot, then stop; the operator reads this before typing."
    ));
    prompt
}

#[allow(clippy::too_many_arguments)]
fn run_reset(
    destination: Option<(MainBody, String)>,
    fire: Fire,
    skip_ritual: bool,
    adapter: &mut MainBody,
    adapter_for_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_model_fallback_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_store_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_hand_kill: &mut ManuallyDrop<MainBody>,
    adapter_for_file_kill: &mut ManuallyDrop<MainBody>,
    log: &mut TranscriptLog,
    front: &mut dyn Front,
    gate: &mut dyn Gate,
    gate_note: &RefCell<Option<DecisionNote>>,
    repo_tier: &RefCell<GateTier>,
    updated_input_notice: &RefCell<Option<String>>,
    repo_gate_path: &Path,
    store: &mut dyn Store,
    ledger: &mut SpawnLedger,
    last_handle: &mut transcript::Handle,
    resolved: &Resolved,
    roster: &mut Roster,
    pending_model: &mut Option<PendingMainSelection>,
    current_model: &mut String,
    config_dir: &Path,
    user_gate: &config::GateConfig,
    nudge: &mut Nudge,
    checkpoint: &mut Checkpoint,
    reserve: &mut Reserve,
    pending_drains: &mut HashMap<HandId, Vec<String>>,
    proposals: &mut HashMap<HandId, Vec<Suggestion>>,
    admission_cache: &mut admission::Cache,
    admission_seq: &mut u64,
) -> Result<(), CycleFailure> {
    let threads_dir = config_dir.join("threads");
    let checkpoints_dir = state_object::checkpoints_dir(&threads_dir, "main");
    let has_destination = destination.is_some();

    enum RitualEnd {
        Written(u64),
        Floor,
        SwitchFallback(SwitchFallback),
    }

    enum SwitchFallback {
        OutgoingUnavailable,
        RestorePrevious,
    }

    let end = if skip_ritual {
        if matches!(fire, Fire::Switch) {
            RitualEnd::SwitchFallback(SwitchFallback::RestorePrevious)
        } else {
            RitualEnd::Floor
        }
    } else {
        let ritual = run_checkpoint_ritual(
            adapter,
            adapter_for_kill,
            adapter_for_model_fallback_kill,
            adapter_for_store_kill,
            adapter_for_hand_kill,
            adapter_for_file_kill,
            log,
            front,
            gate,
            gate_note,
            repo_tier,
            updated_input_notice,
            repo_gate_path,
            store,
            ledger,
            last_handle,
            resolved,
            roster,
            pending_model,
            &mut *current_model,
            config_dir,
            user_gate,
            nudge,
            checkpoint,
            pending_drains,
            proposals,
            admission_cache,
            admission_seq,
        );
        match ritual {
            Ok(RitualOutcome::Written(n)) => RitualEnd::Written(n),
            Ok(RitualOutcome::Abandoned) => RitualEnd::Floor,
            Err(CycleFailure::WindowExceeded) => RitualEnd::Floor,
            Err(CycleFailure::BodyUnavailable) if has_destination => {
                RitualEnd::SwitchFallback(SwitchFallback::OutgoingUnavailable)
            }
            Err(other) => return Err(other),
        }
    };

    let thread = last_handle.thread.clone();
    let source: String;
    let orientation_prompt: String;
    let mut note: Option<String> = None;

    match end {
        RitualEnd::Written(n) => {
            if let Err(err) = log_event(
                log,
                front,
                &Event::Reset {
                    phase: "fired".to_string(),
                    check: Some(fire.label().to_string()),
                },
            ) {
                eprintln!("error: {err}");
                adapter.kill();
                return Err(CycleFailure::Direct);
            }

            let path = checkpoints_dir.join(format!("{n}.md"));
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(err) => {
                    eprintln!("error: {}: {err}", path.display());
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
            };
            let obj = match state_object::parse(&text) {
                Ok(obj) => obj,
                Err(err) => {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
            };

            match fire {
                Fire::Switch => {}
                Fire::Operator => {
                    if let Err(err) = front.render(&TerminalMsg::Notice { text: obj.body.clone() }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return Err(CycleFailure::Direct);
                    }
                    let confirm = format!(
                        "[arx] state object {n} above. ok to tear down and respawn; any other line rides into the new main's first turn as an operator note."
                    );
                    if let Err(err) = front.render(&TerminalMsg::Confirm { text: confirm }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return Err(CycleFailure::Direct);
                    }
                    match front.read_answer(None) {
                        Ok(Poll::Line(OperatorMsg::Line(line))) | Ok(Poll::Line(OperatorMsg::Turn { text: line, .. })) => {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("ok") {
                                note = Some(trimmed.to_string());
                            }
                        }
                        Ok(Poll::Eof) | Ok(Poll::Idle) => {}
                        Err(err) => {
                            eprintln!("error: {err}");
                            adapter.kill();
                            return Err(CycleFailure::Direct);
                        }
                    }
                }
                Fire::Harness => {
                    let ceiling_notice = format!(
                        "[arx] reset fired at the window ceiling: state object {n} written and shown below; respawning."
                    );
                    if let Err(err) = front.render(&TerminalMsg::Notice { text: ceiling_notice }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return Err(CycleFailure::Direct);
                    }
                    if let Err(err) = front.render(&TerminalMsg::Notice { text: obj.body.clone() }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return Err(CycleFailure::Direct);
                    }
                }
            }

            let t = last_handle.turn;
            orientation_prompt = orientation_prompt_with_object(&thread, n, t, &obj.body, None, note.as_deref());
            source = format!("checkpoint {n}");
        }
        RitualEnd::Floor => {
            let overshoot = checkpoint.last_occupancy.saturating_sub(checkpoint.base);
            if let Err(err) = log_event(log, front, &Event::Floor { overshoot }) {
                eprintln!("error: {err}");
                adapter.kill();
                return Err(CycleFailure::Direct);
            }

            match state_object::latest(&checkpoints_dir) {
                Ok(Some((n, path))) => {
                    let notice = format!(
                        "[arx] floor: the window grew {overshoot} tokens past the last checkpoint and the reset ritual could not complete; respawning from checkpoint {n}"
                    );
                    if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return Err(CycleFailure::Direct);
                    }
                    let text = match fs::read_to_string(&path) {
                        Ok(text) => text,
                        Err(err) => {
                            eprintln!("error: {}: {err}", path.display());
                            adapter.kill();
                            return Err(CycleFailure::Direct);
                        }
                    };
                    let obj = match state_object::parse(&text) {
                        Ok(obj) => obj,
                        Err(err) => {
                            eprintln!("error: {err}");
                            adapter.kill();
                            return Err(CycleFailure::Direct);
                        }
                    };
                    let k = obj.header.turn + 1;
                    let t = last_handle.turn;
                    let gap = format!(
                        "The reset ritual could not complete: the window had grown {overshoot} tokens past checkpoint {n}, so turns {k} to {t} are not in the object."
                    );
                    orientation_prompt = orientation_prompt_with_object(
                        &thread,
                        n,
                        t,
                        &obj.body,
                        Some(&gap),
                        None,
                    );
                    source = format!("checkpoint {n}");
                }
                Ok(None) => {
                    let notice = format!(
                        "[arx] floor: the window grew {overshoot} tokens past the last checkpoint and the reset ritual could not complete; respawning from the transcript alone"
                    );
                    if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return Err(CycleFailure::Direct);
                    }
                    let t = last_handle.turn;
                    orientation_prompt = orientation_prompt_without_object(&thread, t, None);
                    source = "transcript".to_string();
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
            }
        }
        RitualEnd::SwitchFallback(cause) => {
            let outgoing_unavailable = matches!(cause, SwitchFallback::OutgoingUnavailable);
            match state_object::latest(&checkpoints_dir) {
                Ok(Some((n, path))) => {
                    let notice = if outgoing_unavailable {
                        format!(
                            "[arx] switch: the outgoing main became unavailable during the reset ritual; starting the destination from checkpoint {n}"
                        )
                    } else {
                        format!(
                            "[arx] switch: the attempted switch did not complete; recovering the previous main from checkpoint {n}"
                        )
                    };
                    if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return Err(CycleFailure::Direct);
                    }
                    let text = match fs::read_to_string(&path) {
                        Ok(text) => text,
                        Err(err) => {
                            eprintln!("error: {}: {err}", path.display());
                            adapter.kill();
                            return Err(CycleFailure::Direct);
                        }
                    };
                    let obj = match state_object::parse(&text) {
                        Ok(obj) => obj,
                        Err(err) => {
                            eprintln!("error: {err}");
                            adapter.kill();
                            return Err(CycleFailure::Direct);
                        }
                    };
                    let k = obj.header.turn + 1;
                    let t = last_handle.turn;
                    let gap = if outgoing_unavailable {
                        format!(
                            "The outgoing body became unavailable before the reset ritual could complete, so turns {k} to {t} are not in the object."
                        )
                    } else {
                        format!(
                            "The attempted model switch did not complete after the outgoing body was torn down, so turns {k} to {t} are not in the object."
                        )
                    };
                    orientation_prompt = orientation_prompt_with_object(
                        &thread,
                        n,
                        t,
                        &obj.body,
                        Some(&gap),
                        None,
                    );
                }
                Ok(None) => {
                    let notice = if outgoing_unavailable {
                        "[arx] switch: the outgoing main became unavailable during the reset ritual; starting the destination from the transcript alone"
                    } else {
                        "[arx] switch: the attempted switch did not complete; recovering the previous main from the transcript alone"
                    }
                    .to_string();
                    if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return Err(CycleFailure::Direct);
                    }
                    let t = last_handle.turn;
                    let gap = if outgoing_unavailable {
                        "The outgoing body became unavailable before the reset ritual could complete."
                    } else {
                        "The attempted model switch did not complete after the outgoing body was torn down."
                    };
                    orientation_prompt =
                        orientation_prompt_without_object(&thread, t, Some(gap));
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return Err(CycleFailure::Direct);
                }
            }
            source = if outgoing_unavailable {
                "outgoing_body_unavailable"
            } else {
                "switch_recovery"
            }
            .to_string();
        }
    }

    if let Some((from, to)) = adapter.take_pending_model_fallback() {
        *current_model = to.clone();
        if let Err(err) = log_event(log, front, &Event::Model { position: "main".to_string(), from, to }) {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
    }

    let system_prompt = match compose_system_prompt(resolved, config_dir, store) {
        Ok(system_prompt) => system_prompt,
        Err(err) => {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
    };
    let spawn_opts = SpawnOpts {
        model: current_model.to_string(),
        system_prompt,
        cwd: config_dir.to_path_buf(),
    };
    if let Some((next, model)) = destination {
        adapter.replace(next);
        *current_model = model;
    } else if let Err(err) = adapter.respawn(&resolved.sidecar, &spawn_opts) {
        eprintln!("error: {err}");
        return Err(CycleFailure::Direct);
    }
    if let Err(err) = log.append_system(&spawn_opts.system_prompt.static_block, &spawn_opts.system_prompt.dynamic) {
        eprintln!("error: {err}");
        adapter.kill();
        return Err(CycleFailure::Direct);
    }
    nudge.said = false;
    checkpoint.base = 0;
    checkpoint.last_occupancy = 0;
    reserve.armed = false;
    if let Err(err) = log_event(
        log,
        front,
        &Event::Reset {
            phase: "respawned".to_string(),
            check: Some(source),
        },
    ) {
        eprintln!("error: {err}");
        adapter.kill();
        return Err(CycleFailure::Direct);
    }

    run_turn_cycle(
        "arx",
        &orientation_prompt,
        &[],
        adapter,
        adapter_for_kill,
        adapter_for_model_fallback_kill,
        adapter_for_store_kill,
        adapter_for_hand_kill,
        adapter_for_file_kill,
        log,
        front,
        gate,
        gate_note,
        repo_tier,
        updated_input_notice,
        repo_gate_path,
        store,
        ledger,
        last_handle,
        resolved,
        roster,
        pending_model,
        current_model,
        config_dir,
        user_gate,
        nudge,
        checkpoint,
        pending_drains,
        proposals,
        admission_cache,
        admission_seq,
    )?;

    let reply_handle = log.last_handle().expect("orientation turn was just committed");
    let reply_text = match transcript::resolve(&threads_dir, &reply_handle) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("error: {err}");
            adapter.kill();
            return Err(CycleFailure::Direct);
        }
    };
    let verdict = if reply_text.trim().is_empty() { "empty" } else { "reported" };
    if let Err(err) = log_event(log, front, &Event::Orientation { verdict: verdict.to_string() }) {
        eprintln!("error: {err}");
        adapter.kill();
        return Err(CycleFailure::Direct);
    }

    Ok(())
}

fn cmd_run(path: &Path) -> ExitCode {
    let resolved = match config::load(path) {
        Ok(resolved) => resolved,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let repo_gate_label = path.display().to_string();
    let user_gate_path = config::user_gate_path();
    let user_gate_label = user_gate_path.display().to_string();
    let user_gate = match config::load_user_gate(&user_gate_path) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    let dir = config_dir(path);

    let mut gate_table = match GateTable::new(&resolved.gate, &repo_gate_label, &user_gate, &user_gate_label) {
        Ok(table) => table,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    gate_table.set_context("main", &dir);
    let gate_note = gate_table.note_handle();
    let gate_repo_tier = gate_table.repo_handle();
    let gate_updated_input_notice = gate_table.updated_input_notice_handle();

    let store_root = resolved.store_root.clone();
    let mut engine = match Engine::open(&store_root, "main") {
        Ok(e) => e,
        Err(err) => {
            eprintln!("store: {err}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(url) = &resolved.store_embedder {
        engine = engine.with_embedder(Box::new(OllamaEmbedder::new(url, "nomic-embed-text")));
    }
    let store: &mut dyn Store = &mut engine;

    let system_prompt = match compose_system_prompt(&resolved, &dir, store) {
        Ok(prompt) => prompt,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let mut log = match TranscriptLog::open(&dir.join("threads"), "main") {
        Ok(log) => log,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let said = match transcript::events(&dir.join("threads"), "main") {
        Ok(events) => events
            .iter()
            .any(|event| matches!(event, Event::Nudge { seat, .. } if seat == "main")),
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mut nudge = Nudge {
        threshold: resolved.main.nudge,
        said,
        pending: None,
    };
    let checkpoints_dir = state_object::checkpoints_dir(&dir.join("threads"), "main");
    let resumed_checkpoint = match state_object::latest(&checkpoints_dir) {
        Ok(latest) => latest.map(|(n, _)| n),
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mut checkpoint = Checkpoint {
        delta: resolved.main.checkpoint,
        base: 0,
        last_occupancy: 0,
        number: resumed_checkpoint,
        written_delta: None,
    };
    let (main_window, main_reserve) = config::main_window(&resolved.main);
    let mut reserve = Reserve {
        window: main_window,
        reserve: main_reserve,
        armed: false,
    };
    let mut pending_drains: HashMap<HandId, Vec<String>> = HashMap::new();
    let mut proposals: HashMap<HandId, Vec<Suggestion>> = HashMap::new();
    let roster_path = roster::roster_path(path);
    let (mut roster, roster_load_notice) = match Roster::load(&roster_path) {
        Ok(roster) => (roster, None),
        Err(err) => (
            Roster::default(),
            Some(format!("[arx] roster: {}: {err} — starting from an empty roster", roster_path.display())),
        ),
    };
    let migration_notice = if roster.get("judge").is_some() {
        let kept_reviewer = roster.get("reviewer").is_some();
        let previous = roster.clone();
        roster.migrate("judge", "reviewer");
        match roster.save(&roster_path) {
            Ok(()) if kept_reviewer => Some(
                "[arx] roster: removed the retired judge pin and kept the existing reviewer pin"
                    .to_string(),
            ),
            Ok(()) => Some(
                "[arx] roster: moved the retired judge pin to reviewer; the old position is no longer live"
                    .to_string(),
            ),
            Err(err) => {
                roster = previous;
                Some(format!(
                    "[arx] roster: {err} — the retired judge pin could not be migrated"
                ))
            }
        }
    } else {
        None
    };
    let (mut current_model, main_body_mismatch_notice) = main_current_model(&resolved, &roster);

    if let Err(err) = log.append_system(&system_prompt.static_block, &system_prompt.dynamic) {
        eprintln!("error: {err}");
        return ExitCode::FAILURE;
    }

    let mut adapter = match MainBody::spawn_for(
        &main_effective_binding(&resolved, &roster).1.body.to_string(),
        &resolved.sidecar,
        SpawnOpts {
            model: current_model.clone(),
            system_prompt,
            cwd: dir.clone(),
        },
    ) {
        Ok(adapter) => adapter,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let mut ledger = match SpawnLedger::open(&dir.join("threads"), "main") {
        Ok(ledger) => ledger,
        Err(err) => {
            eprintln!("error: {err}");
            adapter.kill();
            return ExitCode::FAILURE;
        }
    };

    let stdin = io::stdin();
    let stdin_is_tty = stdin.is_terminal();
    let stdout_is_tty = io::stdout().is_terminal();
    let use_panes = resolved.terminal_front == FrontKind::Panes && stdin_is_tty && stdout_is_tty;
    let mut front: Box<dyn Front> = if use_panes {
        let palette = crate::panes::palette::Palette::from_config(&resolved.terminal_palette).unwrap_or_default();
        match PanesFront::new(resolved.terminal_keys.as_deref(), palette) {
            Ok(front) => Box::new(front),
            Err(err) => {
                eprintln!("error: {err}");
                adapter.kill();
                return ExitCode::FAILURE;
            }
        }
    } else {
        Box::new(LineFront::new(io::stdout(), stdin.lock(), stdin_is_tty))
    };
    if let Some(text) = roster_load_notice {
        let _ = front.render(&TerminalMsg::Notice { text });
    }
    if let Some(text) = migration_notice {
        let _ = front.render(&TerminalMsg::Notice { text });
    }
    if let Some(text) = main_body_mismatch_notice {
        let _ = front.render(&TerminalMsg::Notice { text });
    }
    for text in dangling_pin_notices(&roster, &resolved) {
        let _ = front.render(&TerminalMsg::Notice { text });
    }
    for text in config::inert_tools_notices(effective_rows_for_notices(&resolved, &roster).iter()) {
        let _ = front.render(&TerminalMsg::Notice { text });
    }
    let mut adapter_for_kill = ManuallyDrop::new(adapter.clone());
    let mut adapter_for_model_fallback_kill = ManuallyDrop::new(adapter.clone());
    let mut adapter_for_store_kill = ManuallyDrop::new(adapter.clone());
    let mut adapter_for_hand_kill = ManuallyDrop::new(adapter.clone());
    let mut adapter_for_file_kill = ManuallyDrop::new(adapter.clone());
    let mut last_handle = transcript::Handle {
        thread: "main".to_string(),
        turn: 0,
    };
    let mut admission_cache = admission::Cache::new();
    let mut admission_seq: u64 = 0;
    let mut pending_model = None;

    macro_rules! cycle {
        ($role:expr, $text:expr) => {
            cycle!($role, $text, &[])
        };
        ($role:expr, $text:expr, $images:expr) => {
            run_turn_cycle(
                $role,
                $text,
                $images,
                &mut adapter,
                &mut adapter_for_kill,
                &mut adapter_for_model_fallback_kill,
                &mut adapter_for_store_kill,
                &mut adapter_for_hand_kill,
                &mut adapter_for_file_kill,
                &mut log,
                &mut *front,
                &mut gate_table,
                &gate_note,
                &gate_repo_tier,
                &gate_updated_input_notice,
                path,
                store,
                &mut ledger,
                &mut last_handle,
                &resolved,
                &mut roster,
                &mut pending_model,
                &mut current_model,
                &dir,
                &user_gate,
                &mut nudge,
                &mut checkpoint,
                &mut pending_drains,
                &mut proposals,
                &mut admission_cache,
                &mut admission_seq,
            )
        };
    }

    macro_rules! checkpoint_ritual {
        () => {
            run_checkpoint_ritual(
                &mut adapter,
                &mut adapter_for_kill,
                &mut adapter_for_model_fallback_kill,
                &mut adapter_for_store_kill,
                &mut adapter_for_hand_kill,
                &mut adapter_for_file_kill,
                &mut log,
                &mut *front,
                &mut gate_table,
                &gate_note,
                &gate_repo_tier,
                &gate_updated_input_notice,
                path,
                store,
                &mut ledger,
                &mut last_handle,
                &resolved,
                &mut roster,
                &mut pending_model,
                &mut current_model,
                &dir,
                &user_gate,
                &mut nudge,
                &mut checkpoint,
                &mut pending_drains,
                &mut proposals,
                &mut admission_cache,
                &mut admission_seq,
            )
        };
    }

    macro_rules! reset {
        ($fire:expr, $skip_ritual:expr) => {
            reset!(
                $fire,
                $skip_ritual, None)
        };
        ($fire:expr, $skip_ritual:expr, $destination:expr) => {
            run_reset(
                $destination,
                $fire,
                $skip_ritual,
                &mut adapter,
                &mut adapter_for_kill,
                &mut adapter_for_model_fallback_kill,
                &mut adapter_for_store_kill,
                &mut adapter_for_hand_kill,
                &mut adapter_for_file_kill,
                &mut log,
                &mut *front,
                &mut gate_table,
                &gate_note,
                &gate_repo_tier,
                &gate_updated_input_notice,
                path,
                store,
                &mut ledger,
                &mut last_handle,
                &resolved,
                &mut roster,
                &mut pending_model,
                &mut current_model,
                &dir,
                &user_gate,
                &mut nudge,
                &mut checkpoint,
                &mut reserve,
                &mut pending_drains,
                &mut proposals,
                &mut admission_cache,
                &mut admission_seq,
            )
        };
    }

    macro_rules! fail_on {
        ($result:expr) => {
            if let Err(failure) = $result {
                if matches!(failure, CycleFailure::ViaClone) {
                    std::mem::forget(adapter);
                }
                return ExitCode::FAILURE;
            }
        };
    }

    macro_rules! after_turn {
        ($result:expr) => {
            match $result {
                Err(CycleFailure::WindowExceeded) => {
                    fail_on!(reset!(Fire::Harness, true));
                }
                result => {
                    fail_on!(result);
                }
            }
        };
    }

    macro_rules! check_reserve {
        () => {
            if reserve.at_ceiling(checkpoint.last_occupancy) {
                fail_on!(reset!(Fire::Harness, false));
            } else if !reserve.armed && reserve.crossed_arm(checkpoint.last_occupancy) {
                reserve.armed = true;
                if let Err(err) = log_event(&mut log, &mut *front, &Event::Reset { phase: "armed".to_string(), check: None }) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return ExitCode::FAILURE;
                }
                let notice = format!(
                    "[arx] reset armed: window at {} of {}, reserve {}; /reset to reset now",
                    checkpoint.last_occupancy, reserve.window, reserve.reserve
                );
                if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return ExitCode::FAILURE;
                }
            }
        };
    }

    macro_rules! after_arx_cycle {
        ($text:expr) => {
            after_turn!(cycle!("arx", $text));
            check_reserve!();
        };
    }

    macro_rules! pump_deliveries {
        () => {
            loop {
                match ledger.try_next_timed() {
                    Ok(Some((delivery, timing))) => {
                        if let Some(event) = hand_nudge(&delivery, nudge.threshold) {
                            if let Err(err) = log_event(&mut log, &mut *front, &event) {
                                eprintln!("error: {err}");
                                adapter.kill();
                                return ExitCode::FAILURE;
                            }
                            if let Event::Nudge { hand: Some(id), occupancy, threshold, .. } = &event {
                                let notice = format!("nudge: hand {id} returned at {occupancy} tokens, past {threshold}");
                                if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                                    eprintln!("error: {err}");
                                    adapter.kill();
                                    return ExitCode::FAILURE;
                                }
                            }
                        }
                        let (hand, outcome) = delivery_hand_and_outcome(&delivery);
                        let (trace, bytes) = delivery_trace_and_bytes(&delivery);
                        for tool in delivery_walls(&delivery) {
                            if let Err(err) = log_event(&mut log, &mut *front, &Event::Wall { hand: hand.to_string(), tool: tool.clone() }) {
                                eprintln!("error: {err}");
                                adapter.kill();
                                return ExitCode::FAILURE;
                            }
                        }
                        if let Some((hand, tool)) = delivery_stop_wall(&delivery) {
                            if let Err(err) = log_event(&mut log, &mut *front, &Event::Wall { hand: hand.to_string(), tool: tool.to_string() }) {
                                eprintln!("error: {err}");
                                adapter.kill();
                                return ExitCode::FAILURE;
                            }
                        }
                        if let Some(size) = delivery_miss(&delivery) {
                            if let Err(err) = log_event(&mut log, &mut *front, &Event::Miss { size }) {
                                eprintln!("error: {err}");
                                adapter.kill();
                                return ExitCode::FAILURE;
                            }
                            if let Some((agent, model)) = ledger.hand_identity(hand) {
                                let notice = format!(
                                    "[arx] miss: {agent} ({model}) rejected the window; the configured window for that model may need updating"
                                );
                                if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                                    eprintln!("error: {err}");
                                    adapter.kill();
                                    return ExitCode::FAILURE;
                                }
                            }
                        }
                        if let Err(err) = log_event(
                            &mut log,
                            &mut *front,
                            &Event::Delivered {
                                hand: hand.to_string(),
                                outcome: outcome.to_string(),
                                trace,
                                bytes,
                                elapsed_ms: Some(timing.elapsed_ms),
                                lag_ms: Some(timing.lag_ms),
                            },
                        ) {
                            eprintln!("error: {err}");
                            adapter.kill();
                            return ExitCode::FAILURE;
                        }
                        let reviewer_event = reviewer_verdict_event(&ledger, &resolved, hand, &delivery);
                        let schema = reviewer_schema_outcome(&reviewer_event);
                        let text = match reviewer_event
                            .or_else(|| drain_proposal_event(hand, &delivery, &mut pending_drains, &mut proposals))
                        {
                            Some((event, formatted)) => {
                                if let Err(err) = log_event(&mut log, &mut *front, &event) {
                                    eprintln!("error: {err}");
                                    adapter.kill();
                                    return ExitCode::FAILURE;
                                }
                                formatted
                            }
                            None => format_delivery(&delivery),
                        };
                        let verdict_admission_root = admission_trace_root(&dir, &last_handle.thread);
                        let (text, verdict_admission_events) =
                            bound_verdict_delivery(&delivery, text, &resolved.admission, &verdict_admission_root, &mut admission_seq, last_handle.turn);
                        for event in &verdict_admission_events {
                            if let Err(err) = log_event(&mut log, &mut *front, event) {
                                eprintln!("error: {err}");
                                adapter.kill();
                                return ExitCode::FAILURE;
                            }
                        }
                        send_delivery_party_msg(&mut *front, &delivery, &timing, schema);
                        after_arx_cycle!(&text);
                    }
                    Ok(None) => break,
                    Err(err) => {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return ExitCode::FAILURE;
                    }
                }
            }
        };
    }

    'operator: loop {
        if let Err(err) = front.render(&TerminalMsg::Prompt) {
            eprintln!("error: {err}");
            adapter.kill();
            return ExitCode::FAILURE;
        }

        let (line, images) = loop {
            match front.read(Some(Duration::from_millis(200))) {
                Ok(Poll::Line(OperatorMsg::Line(line))) => break (line, Vec::new()),
                Ok(Poll::Line(OperatorMsg::Turn { text, images })) => break (text, images),
                Ok(Poll::Eof) => break 'operator,
                Ok(Poll::Idle) => {
                    pump_deliveries!();
                    continue;
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return ExitCode::FAILURE;
                }
            }
        };

        if line.trim() == "/reset" {
            fail_on!(reset!(Fire::Operator, false));
            pump_deliveries!();
            continue;
        }

        if let Some(outcome) = intercept_kill_line(&line, &mut ledger, &mut log, &mut *front) {
            if let Err(err) = outcome {
                eprintln!("error: {err}");
                adapter.kill();
                return ExitCode::FAILURE;
            }
            pump_deliveries!();
            continue;
        }

        if let Some(request) = parse_model_line(&line) {
            if matches!(request, ModelRequest::Open) {
                let msg = build_roster_message(&resolved, &roster);
                if let Err(err) = front.render(&msg) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return ExitCode::FAILURE;
                }
                continue;
            }

            let known_positions: Vec<String> = resolved.positions().iter().map(|row| row.role.clone()).collect();
            let known_ids: Vec<String> = resolved.models.iter().map(|entry| entry.id.clone()).collect();

            enum Action {
                SetMain(String),
                MainSelection(String, bool),
                Set(String, String),
                Clear(String),
                Error(String),
            }

            let action = match request {
                ModelRequest::Open => unreachable!("Open is handled above"),
                ModelRequest::Clear(position) => {
                    if known_positions.iter().any(|p| p == position) {
                        Action::Clear(position.to_string())
                    } else {
                        Action::Error(render_model_position_unlisted(position, &known_positions))
                    }
                }
                ModelRequest::One(token) => {
                    if known_positions.iter().any(|p| p == token) && !known_ids.iter().any(|id| id == token) {
                        Action::Error(format!("[arx] model: {token} is a position — give a model id too: /model {token} <id>"))
                    } else {
                        Action::SetMain(token.to_string())
                    }
                }
                ModelRequest::Two(position, id) => {
                    if position == "main" {
                        Action::SetMain(id.to_string())
                    } else if known_positions.iter().any(|p| p == position) {
                        Action::Set(position.to_string(), id.to_string())
                    } else {
                        Action::Error(render_model_position_unlisted(position, &known_positions))
                    }
                }
            };

            let action = match action {
                Action::SetMain(id) => Action::MainSelection(id, false),
                Action::Clear(position) if position == "main" => Action::MainSelection(resolved.main.model.clone(), true),
                other => other,
            };
            let mut switched = false;
            let text = match action {
                Action::Error(text) => text,
                Action::SetMain(_) => unreachable!(),
                Action::MainSelection(id, clear) => {
                    let result = (|| -> Result<String, String> {
                        let entry = resolved.model_entry(&id).ok_or_else(|| render_model_unlisted(&id, &known_ids))?;
                        if !matches!(entry.body, Body::ClaudeCode | Body::Codex) {
                            return Err(MODEL_SWITCH_BODY_UNSUPPORTED.to_string());
                        }
                        if entry.body == Body::ClaudeCode && config::main_window(&resolved.main).0 >= 900_000 {
                            return Err(config::ConfigError::MainWindowTooHigh.to_string());
                        }
                        let from = current_model.clone();
                        let previous_body = adapter.kind().to_string();
                        let cross_body = previous_body != entry.body.to_string();
                        let already = if clear {
                            roster.get("main").is_none()
                        } else {
                            roster.get("main") == Some(id.as_str())
                        };
                        if from == id && already {
                            return Ok(if clear {
                                format!("[arx] model: main has no pin to clear; using {id} from arx.toml")
                            } else {
                        render_model_already(&id)
                    });
                        }
                        let mut trial = roster.clone();
                                if clear {
                            trial.clear("main");
                                } else {
                            trial.pin("main", &id);
                        }
                        let persistence = trial.prepare(
                                    &roster_path).map_err(|err| err.to_string())?;
                        let mut suspended_selection = None;
                        if cross_body {
                            let prompt = compose_system_prompt(&resolved,
                                    &dir, store)?;
                            let next = MainBody::spawn_for(
                                &entry.body.to_string(),
                                    &resolved.sidecar,
                                SpawnOpts {
                                    model: id.clone(),
                                    system_prompt: prompt,
                                    cwd: dir.clone(),
                                },
                                )
                            .map_err(|err| err.to_string())?;
                            if pending_model.is_some() {
                                adapter.set_model(&from).map_err(|err| err.to_string())?;
                                suspended_selection = pending_model.take();
                            }
                                if reset!(Fire::Switch, false, Some((next, id.clone()))).is_err() {
                                let prompt = compose_system_prompt(&resolved, &dir, store)?;
                                let previous = MainBody::spawn_for(
                                    &previous_body,
                                    &resolved.sidecar,
                                    SpawnOpts {
                                        model: from.clone(),
                                        system_prompt: prompt,
                                        cwd: dir.clone(),
                                    },
                                )
                                .map_err(|err| format!("switch failed; main stopped: {err}"))?;
                                reset!(Fire::Switch, true, Some((previous, from.clone())))
                                    .map_err(|_| "switch and previous-body recovery failed; main stopped".to_string())?;
                                if let Some(selection) = suspended_selection.take() {
                                    adapter.set_model(&selection.model)
                                        .map_err(|err| format!("pending selection recovery failed; main stopped: {err}"))?;
                                    pending_model = Some(selection);
                                }
                                return Err("destination failed; recovered the previous main from arx state".into());
                            }
                        } else if id != from || pending_model.is_some() {
                            adapter.set_model(&id).map_err(|err| err.to_string())?;
                        }
                        if let Err(err) = persistence.commit() {
                            if cross_body {
                                let prompt = compose_system_prompt(&resolved, &dir, store)?;
                                let previous = MainBody::spawn_for(
                                    &previous_body,
                                    &resolved.sidecar,
                                    SpawnOpts {
                                        model: from.clone(),
                                        system_prompt: prompt,
                                        cwd: dir.clone(),
                                    },
                                )
                                .map_err(|restore| format!("roster save failed: {err}; main stopped: {restore}"))?;
                                reset!(Fire::Switch, true, Some((previous, from.clone())))
                                    .map_err(|_| format!("roster save failed: {err}; previous-body recovery failed; main stopped"))?;
                                if let Some(selection) = suspended_selection.take() {
                                    adapter.set_model(&selection.model)
                                        .map_err(|restore| format!("roster save failed: {err}; pending selection recovery failed; main stopped: {restore}"))?;
                                    pending_model = Some(selection);
                                }
                            } else {
                                let previous_selection = pending_model.as_ref().map(|selection| selection.model.as_str()).unwrap_or(&from);
                                adapter
                                    .set_model(previous_selection)
                                    .map_err(|restore| format!("roster save failed: {err}; rollback failed; main stopped: {restore}"))?;
                            }
                            return Err(format!("roster save failed: {err}; the main selection did not take"));
                        }
                        if !cross_body && previous_body == "codex" && id != from {
                            let previous_pin = match &pending_model {
                                Some(selection) => selection.previous_pin.clone(),
                                None => roster.get("main").map(str::to_string),
                            };
                            pending_model = Some(PendingMainSelection {
                                model: id.clone(), clear, path: roster_path.clone(), previous_pin,
                            });
                            roster = trial;
                            return Ok(format!("[arx] model: {id} saved; activation pending until the next turn is accepted; using {from} meanwhile"));
                        }
                        pending_model = None;
                        current_model = id.clone();
                        roster = trial;
                        log_event(
                                        &mut log,
                                        &mut *front,
                                        &Event::Model { position: "main".into(), from: from.clone(), to: id.clone(),
                            },
                                    )
                        .map_err(|err| err.to_string())?;
                        switched = from != id;
                        Ok(if clear {
                            format!("[arx] model: main cleared; falling back to {id} from arx.toml")
                        } else if switched {
                                    render_model_switched(&from, &id)
                                } else {
                                    format!("[arx] model: main pinned to {id}")
                                })
                    })();
                    match result {
                        Ok(text) => text,
                        Err(err) if err.contains("main stopped") => {
                            let _ = front.render(&TerminalMsg::Notice {
                                text: render_model_switch_failed(&err),
                            });
                            adapter.kill();
                            return ExitCode::FAILURE;
                        }
                            Err(err) => render_model_switch_failed(&err),
                    }
                }
                Action::Set(position, id) => {
                    if !known_ids.iter().any(|known| known == &id) {
                        render_model_unlisted(&id, &known_ids)
                    } else if position == "runner"
                        && resolved.model_entry(&id).is_some_and(|entry| entry.body == Body::Native)
                    {
                        render_model_switch_failed(
                            "runner requires claude-code or codex so it can implement the brief",
                        )
                    } else {
                        let (previous_id, _, _) = effective_binding(&position, &resolved, &roster);
                        let previous_pin = roster.get(&position).map(|s| s.to_string());
                        roster.pin(&position, &id);
                        let persisted =
                            save_roster_or_notice(&mut roster, &roster_path, &position, previous_pin.as_deref(), &mut *front);
                        if persisted {
                            if let Err(err) = log_event(
                                &mut log,
                                &mut *front,
                                &Event::Model { position: position.clone(), from: previous_id, to: id.clone() },
                            ) {
                                eprintln!("error: {err}");
                                adapter.kill();
                                return ExitCode::FAILURE;
                            }
                            format!("[arx] model: {position} pinned to {id}")
                        } else {
                            format!("[arx] model: {position} pin to {id} did not take; retry once the roster saves")
                        }
                    }
                }
                Action::Clear(position) => {
                    let previous = roster.get(&position).map(|s| s.to_string());
                    roster.clear(&position);
                    let (fallback_id, _, _) = effective_binding(&position, &resolved, &roster);
                    let persisted = save_roster_or_notice(&mut roster, &roster_path, &position, previous.as_deref(), &mut *front);
                    match previous {
                        Some(previous_id) if persisted => {
                            if let Err(err) = log_event(
                                &mut log,
                                &mut *front,
                                &Event::Model { position: position.clone(), from: previous_id, to: fallback_id.clone() },
                            ) {
                                eprintln!("error: {err}");
                                adapter.kill();
                                return ExitCode::FAILURE;
                            }
                            format!("[arx] model: {position} cleared; falling back to {fallback_id} from arx.toml")
                        }
                        Some(_) => format!("[arx] model: {position} clear did not take; retry once the roster saves"),
                        None => format!("[arx] model: {position} has no pin to clear; using {fallback_id} from arx.toml"),
                    }
                }
            };
            if let Err(err) = front.render(&TerminalMsg::Notice { text }) {
                eprintln!("error: {err}");
                adapter.kill();
                return ExitCode::FAILURE;
            }
            if switched {
                if let Err(err) = front.render(&build_status(&resolved, &checkpoint, &nudge, &current_model, last_handle.turn)) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return ExitCode::FAILURE;
                }
            }
            continue;
        }

        if line.trim() == "/checkpoint" {
            after_turn!(checkpoint_ritual!());
            check_reserve!();
            continue;
        }

        after_turn!(cycle!("operator", &line, &images));
        if checkpoint.due() {
            after_turn!(checkpoint_ritual!());
        }
        check_reserve!();
        pump_deliveries!();
    }

    loop {
        pump_deliveries!();
        if ledger.in_flight() == 0 {
            break;
        }
        match ledger.wait_next_timed() {
            Ok((delivery, timing)) => {
                if let Some(event) = hand_nudge(&delivery, nudge.threshold) {
                    if let Err(err) = log_event(&mut log, &mut *front, &event) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return ExitCode::FAILURE;
                    }
                    if let Event::Nudge { hand: Some(id), occupancy, threshold, .. } = &event {
                        let notice = format!("nudge: hand {id} returned at {occupancy} tokens, past {threshold}");
                        if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                            eprintln!("error: {err}");
                            adapter.kill();
                            return ExitCode::FAILURE;
                        }
                    }
                }
                let (hand, outcome) = delivery_hand_and_outcome(&delivery);
                let (trace, bytes) = delivery_trace_and_bytes(&delivery);
                for tool in delivery_walls(&delivery) {
                    if let Err(err) = log_event(&mut log, &mut *front, &Event::Wall { hand: hand.to_string(), tool: tool.clone() }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return ExitCode::FAILURE;
                    }
                }
                if let Some((hand, tool)) = delivery_stop_wall(&delivery) {
                    if let Err(err) = log_event(&mut log, &mut *front, &Event::Wall { hand: hand.to_string(), tool: tool.to_string() }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return ExitCode::FAILURE;
                    }
                }
                if let Some(size) = delivery_miss(&delivery) {
                    if let Err(err) = log_event(&mut log, &mut *front, &Event::Miss { size }) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return ExitCode::FAILURE;
                    }
                    if let Some((agent, model)) = ledger.hand_identity(hand) {
                        let notice = format!(
                            "[arx] miss: {agent} ({model}) rejected the window; the configured window for that model may need updating"
                        );
                        if let Err(err) = front.render(&TerminalMsg::Notice { text: notice }) {
                            eprintln!("error: {err}");
                            adapter.kill();
                            return ExitCode::FAILURE;
                        }
                    }
                }
                if let Err(err) = log_event(
                    &mut log,
                    &mut *front,
                    &Event::Delivered {
                        hand: hand.to_string(),
                        outcome: outcome.to_string(),
                        trace,
                        bytes,
                        elapsed_ms: Some(timing.elapsed_ms),
                        lag_ms: Some(timing.lag_ms),
                    },
                ) {
                    eprintln!("error: {err}");
                    adapter.kill();
                    return ExitCode::FAILURE;
                }
                let reviewer_event = reviewer_verdict_event(&ledger, &resolved, hand, &delivery);
                let schema = reviewer_schema_outcome(&reviewer_event);
                let text = match reviewer_event.or_else(|| drain_proposal_event(hand, &delivery, &mut pending_drains, &mut proposals)) {
                    Some((event, formatted)) => {
                        if let Err(err) = log_event(&mut log, &mut *front, &event) {
                            eprintln!("error: {err}");
                            adapter.kill();
                            return ExitCode::FAILURE;
                        }
                        formatted
                    }
                    None => format_delivery(&delivery),
                };
                let verdict_admission_root = admission_trace_root(&dir, &last_handle.thread);
                let (text, verdict_admission_events) =
                    bound_verdict_delivery(&delivery, text, &resolved.admission, &verdict_admission_root, &mut admission_seq, last_handle.turn);
                for event in &verdict_admission_events {
                    if let Err(err) = log_event(&mut log, &mut *front, event) {
                        eprintln!("error: {err}");
                        adapter.kill();
                        return ExitCode::FAILURE;
                    }
                }
                send_delivery_party_msg(&mut *front, &delivery, &timing, schema);
                after_arx_cycle!(&text);
            }
            Err(err) => {
                eprintln!("error: {err}");
                adapter.kill();
                return ExitCode::FAILURE;
            }
        }
    }

    adapter.shutdown();
    ExitCode::SUCCESS
}

fn cmd_gauge(path: &Path) -> ExitCode {
    let dir = config_dir(path);
    let resolved = match config::load(path) {
        Ok(resolved) => resolved,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    match transcript::last_usage(&dir.join("threads"), "main") {
        Ok(Some(usage)) => {
            if let Some(fields) = usage.as_object() {
                for (field, value) in fields {
                    println!("{field} {value}");
                }
            }
            let occupancy = transcript::occupancy(&usage);
            let (window, reserve) = config::main_window(&resolved.main);
            println!("occupancy {occupancy} / window {window} (arx main, configured)");
            println!("reserve {reserve} (arx main, configured)");
            println!("body auto-compact threshold {COMPACT_WINDOW} (body-reported, not arx's window)");
            ExitCode::SUCCESS
        }
        Ok(None) => {
            eprintln!("no usage recorded yet for thread 'main'");
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn event_fields(event: &Event) -> Vec<(&'static str, String)> {
    match event {
        Event::Gate { tool, verdict, .. } => {
            vec![("tool", tool.clone()), ("verdict", verdict.clone())]
        }
        Event::Spawn { hand, agent, role } => {
            vec![("hand", hand.clone()), ("agent", agent.clone()), ("role", role.clone())]
        }
        Event::Steer { hand } => vec![("hand", hand.clone())],
        Event::Tap { hand } => vec![("hand", hand.clone())],
        Event::Kill { hand } => vec![("hand", hand.clone())],
        Event::Delivered {
            hand,
            outcome,
            elapsed_ms,
            lag_ms,
            ..
        } => {
            let mut fields = vec![("hand", hand.clone()), ("outcome", outcome.clone())];
            if let Some(elapsed_ms) = elapsed_ms {
                fields.push(("elapsed_ms", elapsed_ms.to_string()));
            }
            if let Some(lag_ms) = lag_ms {
                fields.push(("lag_ms", lag_ms.to_string()));
            }
            fields
        }
        Event::Nudge {
            seat,
            hand,
            occupancy,
            threshold,
        } => {
            let mut fields = vec![("seat", seat.clone())];
            if let Some(hand) = hand {
                fields.push(("hand", hand.clone()));
            }
            fields.push(("occupancy", occupancy.to_string()));
            fields.push(("threshold", threshold.to_string()));
            fields
        }
        Event::Wall { hand, tool } => vec![("hand", hand.clone()), ("tool", tool.clone())],
        Event::Admission {
            tool,
            outcome,
            bytes,
            handle,
        } => {
            let mut fields = vec![("tool", tool.clone()), ("outcome", outcome.clone()), ("bytes", bytes.to_string())];
            if let Some(handle) = handle {
                fields.push(("handle", handle.clone()));
            }
            fields
        }
        Event::Pull { handle, path } => vec![("handle", handle.clone()), ("path", path.clone())],
        Event::Echo { handle } => vec![("handle", handle.clone())],
        Event::Rewind { tokens_cut } => vec![("tokens_cut", tokens_cut.to_string())],
        Event::Checkpoint { check, delta } => {
            vec![("check", check.clone()), ("delta", delta.to_string())]
        }
        Event::Reset { phase, check } => {
            let mut fields = vec![("phase", phase.clone())];
            if let Some(check) = check {
                fields.push(("check", check.clone()));
            }
            fields
        }
        Event::Miss { size } => vec![("size", size.to_string())],
        Event::Floor { overshoot } => vec![("overshoot", overshoot.to_string())],
        Event::Orientation { verdict } => vec![("verdict", verdict.clone())],
        Event::Schema { hand, outcome, reason } => {
            let mut fields = vec![("hand", hand.clone()), ("outcome", outcome.clone())];
            if let Some(reason) = reason {
                fields.push(("reason", reason.clone()));
            }
            fields
        }
        Event::Family { agent, family } => {
            vec![("agent", agent.clone()), ("family", family.clone())]
        }
        Event::Drain {
            hand,
            promoted,
            dropped,
            overrides,
        } => vec![
            ("hand", hand.clone()),
            ("promoted", promoted.to_string()),
            ("dropped", dropped.to_string()),
            ("overrides", overrides.to_string()),
        ],
        Event::Model { position, from, to } => {
            vec![("position", position.clone()), ("from", from.clone()), ("to", to.clone())]
        }
    }
}

fn render_event_line(event: &Event) -> String {
    event_fields(event)
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_kill_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("/kill")?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some(rest.trim())
}

fn intercept_kill_line(
    line: &str,
    ledger: &mut SpawnLedger,
    log: &mut TranscriptLog,
    front: &mut dyn Front,
) -> Option<Result<(), String>> {
    let id = parse_kill_line(line)?;
    let outcome = (|| -> Result<(), String> {
        match ledger.kill(id) {
            Ok(true) => {
                log_event(log, front, &Event::Kill { hand: id.to_string() }).map_err(|err| err.to_string())?;
            }
            Ok(false) => {
                front
                    .render(&TerminalMsg::Notice { text: format!("[arx] kill: unknown or already-settled hand '{id}'") })
                    .map_err(|err| err.to_string())?;
            }
            Err(err) => return Err(err.to_string()),
        }
        Ok(())
    })();
    Some(outcome)
}

enum ModelRequest<'a> {
    Open,
    Clear(&'a str),
    One(&'a str),
    Two(&'a str, &'a str),
}

const MODEL_SWITCH_BODY_UNSUPPORTED: &str = "[arx] model: main supports claude-code and codex; provider main is not wired";

fn parse_model_line(line: &str) -> Option<ModelRequest<'_>> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("/model")?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(ModelRequest::Open);
    }
    let mut parts = rest.split_whitespace();
    let first = parts.next()?;
    let second = parts.next();
    if first == "clear" {
        return second.map(ModelRequest::Clear);
    }
    match second {
        Some(id) => Some(ModelRequest::Two(first, id)),
        None => Some(ModelRequest::One(first)),
    }
}

fn render_model_unlisted(id: &str, models: &[String]) -> String {
    format!("[arx] model: {id} is not in [[models]]; available: {}", models.join(", "))
}

fn render_model_position_unlisted(position: &str, positions: &[String]) -> String {
    format!("[arx] model: {position} is not a known position; available: {}", positions.join(", "))
}

fn render_model_already(id: &str) -> String {
    format!("[arx] model: already {id}")
}

fn render_model_switched(from: &str, to: &str) -> String {
    format!("[arx] model: {from} -> {to}")
}

fn render_model_switch_failed(reason: &str) -> String {
    format!("[arx] model: switch failed: {reason}")
}

fn build_roster_message(resolved: &Resolved, roster: &Roster) -> TerminalMsg {
    let models: Vec<RosterModel> = resolved
        .models
        .iter()
        .map(|entry| RosterModel {
            id: entry.id.clone(),
            body: entry.body.to_string(),
            family: reviewer::family_of(entry.body, entry.provider.as_deref(), &resolved.providers),
        })
        .collect();

    let (_, main_entry, _) = main_effective_binding(resolved, roster);
    let main_family = reviewer::family_of(main_entry.body, main_entry.provider.as_deref(), &resolved.providers);

    let tabs: Vec<RosterTab> = resolved
        .positions()
        .iter()
        .map(|row| {
            let model_ids: Vec<String> = resolved
                .models
                .iter()
                .filter(|model| row.role != "runner" || model.body != Body::Native)
                .map(|model| model.id.clone())
                .collect();
            let (effective_id, entry, source) = if row.role == "main" {
                main_effective_binding(resolved, roster)
            } else {
                effective_binding(&row.role, resolved, roster)
            };
            let cursor = if source == ModelSource::Pinned {
                model_ids.iter().position(|id| id == &effective_id).map(|i| i + 1).unwrap_or(0)
            } else {
                0
            };
            RosterTab {
                position: row.role.clone(),
                fallback_id: row.model.clone(),
                effective_id,
                effective_body: entry.body.to_string(),
                pinned: source == ModelSource::Pinned,
                model_ids,
                cursor,
            }
        })
        .collect();

    TerminalMsg::Roster { models, tabs, main_family }
}

fn cmd_events(path: &Path, thread: &str) -> ExitCode {
    let dir = config_dir(path);
    if let Err(err) = config::load(path) {
        eprintln!("error: {err}");
        return ExitCode::FAILURE;
    }

    let counts = match transcript::kind_counts(&dir.join("threads"), thread) {
        Ok(counts) => counts,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    let rows = match transcript::events(&dir.join("threads"), thread) {
        Ok(rows) => rows,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };

    let width = counts.keys().map(|k| k.len()).max().unwrap_or(0).max("kind".len());
    println!("{:<width$}  count", "kind", width = width);
    for (kind, count) in &counts {
        println!("{kind:<width$}  {count}");
    }
    println!();

    for kind in events::KINDS {
        let matching: Vec<&Event> = rows.iter().filter(|event| event.kind() == kind).collect();
        if matching.is_empty() {
            continue;
        }
        println!("## {kind} ({})", matching.len());
        for event in matching {
            println!("- {}", render_event_line(event));
        }
    }

    ExitCode::SUCCESS
}

fn ember_inner(path: &Path, scope: &str, text: &str) -> Result<String, String> {
    let dir = config_dir(path);
    let resolved = config::load(path).map_err(|err| format!("error: {err}"))?;

    let log = TranscriptLog::open(&dir.join("threads"), "main").map_err(|err| format!("error: {err}"))?;
    let Some(handle) = log.last_handle() else {
        return Err("ember: thread main has no committed turn yet — run a session first".to_string());
    };
    drop(log);

    let store_root = resolved.store_root.clone();
    let mut engine = Engine::open(&store_root, "main").map_err(|err| format!("ember: {err}"))?;
    let claim_handle = ClaimHandle {
        thread: handle.thread.clone(),
        turn: handle.turn,
    };
    let claim = engine.ember(scope, text, &claim_handle).map_err(|err| format!("ember: {err}"))?;
    Ok(format!("ember {} {}/{scope}/embers/{}.md", claim.id, store_root.display(), claim.id))
}

fn cmd_ember(path: &Path, scope: &str, text: &str) -> ExitCode {
    match ember_inner(path, scope, text) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn render_table(resolved: &Resolved) -> String {
    let rows = resolved.positions();
    let headers = ["role", "source", "body", "model", "provider", "drive"];

    let cell = |row: &RoleRow, col: usize| -> String {
        match col {
            0 => row.role.clone(),
            1 => row.source.clone(),
            2 => row.body.to_string(),
            3 => row.model.clone(),
            4 => row.provider.clone().unwrap_or_else(|| "-".to_string()),
            5 => row.drive.to_string(),
            _ => unreachable!(),
        }
    };

    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in &rows {
        for col in 0..headers.len() {
            widths[col] = widths[col].max(cell(row, col).len());
        }
    }

    let mut out = String::new();
    for (col, header) in headers.iter().enumerate() {
        if col > 0 {
            out.push_str("  ");
        }
        out.push_str(&format!("{header:<width$}", width = widths[col]));
    }
    out.push('\n');

    for row in &rows {
        for col in 0..headers.len() {
            if col > 0 {
                out.push_str("  ");
            }
            out.push_str(&format!("{:<width$}", cell(row, col), width = widths[col]));
        }
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_of_a_bare_filename_is_dot() {
        assert_eq!(config_dir(Path::new("arx.toml")), Path::new("."));
    }

    #[test]
    fn config_dir_of_a_nested_path_is_its_parent() {
        assert_eq!(config_dir(Path::new("run/arx.toml")), Path::new("run"));
    }

    #[test]
    fn a_handler_gate_call_carries_the_prefixed_name_and_the_args() {
        let store = StoreCall {
            verb: "save".to_string(),
            args: serde_json::json!({"scope": "arx"}),
        };
        let call = handler_gate_call(&store.verb, &store.args);
        assert_eq!(call.tool, "mcp__arx__save");
        assert_eq!(call.input, store.args);

        let file = FileCall {
            tool: "read_file".to_string(),
            args: serde_json::json!({"path": "note.md"}),
        };
        let call = handler_gate_call(&file.tool, &file.args);
        assert_eq!(call.tool, "mcp__arx__read_file");
        assert_eq!(call.input, file.args);
    }

    #[derive(Default)]
    struct RecordingFront {
        tool_calls: Vec<(String, Option<String>)>,
        messages: Vec<TerminalMsg>,
    }

    impl Front for RecordingFront {
        fn render(&mut self, msg: &TerminalMsg) -> io::Result<()> {
            if let TerminalMsg::ToolCall { tool, subject } = msg {
                self.tool_calls.push((tool.clone(), subject.clone()));
            }
            self.messages.push(msg.clone());
            Ok(())
        }

        fn event(&mut self, _event: &Event) -> io::Result<()> {
            Ok(())
        }

        fn read(&mut self, _wait: Option<Duration>) -> io::Result<Poll> {
            Ok(Poll::Idle)
        }
    }

    fn stub_sidecar_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stub-sidecar")
    }

    fn spawn_stub_adapter() -> MainBody {
        MainBody::spawn(
            &stub_sidecar_dir(),
            SpawnOpts {
                model: "stub-model".to_string(),
                system_prompt: SystemPrompt {
                    static_block: "stub prompt".to_string(),
                    dynamic: String::new(),
                },
                cwd: tempfile::tempdir().unwrap().keep(),
            },
        )
        .expect("stub sidecar spawns")
    }

    #[allow(clippy::too_many_arguments)]
    fn run_gate_call<F: Front>(call: &GateCall, front: &mut F, adapter: &mut ManuallyDrop<MainBody>) -> GateVerdict {
        let workspace = tempfile::tempdir().unwrap();
        let mut log = TranscriptLog::open(workspace.path(), "main").unwrap();
        let repo_gate = config::GateConfig {
            allow: vec!["mcp__arx__read_file".to_string()],
            ..Default::default()
        };
        let user_gate = config::GateConfig::default();
        let mut table = GateTable::new(&repo_gate, "repo", &user_gate, "user").unwrap();
        let note_handle = table.note_handle();
        let repo_handle = table.repo_handle();

        let gate_cell: RefCell<&mut dyn Gate> = RefCell::new(&mut table);
        let front_cell: RefCell<&mut dyn Front> = RefCell::new(front);
        let log_cell = RefCell::new(&mut log);
        let updated_input_notice: RefCell<Option<String>> = RefCell::new(None);
        let repo_gate_path = workspace.path().join("arx.toml");
        let gate_err: RefCell<Option<transcript::TranscriptError>> = RefCell::new(None);
        let gate_io_err: RefCell<Option<io::Error>> = RefCell::new(None);
        let ask_err: RefCell<Option<String>> = RefCell::new(None);

        super::gate_call(
            &gate_cell,
            &note_handle,
            call,
            &log_cell,
            &front_cell,
            &updated_input_notice,
            &repo_gate_path,
            &repo_handle,
            &gate_err,
            &gate_io_err,
            &ask_err,
            adapter,
        )
    }

    #[test]
    fn an_allowed_mcp_arx_tool_call_renders_its_subject_over_the_terminal_channel() {
        let adapter = spawn_stub_adapter();
        let mut adapter_for_kill = ManuallyDrop::new(adapter.clone());

        let mut front = RecordingFront::default();
        let call = GateCall {
            tool: "mcp__arx__read_file".to_string(),
            input: serde_json::json!({"path": "src/x.rs"}),
        };
        let verdict = run_gate_call(&call, &mut front, &mut adapter_for_kill);

        assert_eq!(verdict, GateVerdict::Allow);
        assert_eq!(front.tool_calls, vec![("read_file".to_string(), Some("src/x.rs".to_string()))]);
    }

    #[test]
    fn a_bash_call_from_a_hand_renders_no_tool_call() {
        let adapter = spawn_stub_adapter();
        let mut adapter_for_kill = ManuallyDrop::new(adapter.clone());

        let mut front = RecordingFront::default();
        let call = GateCall {
            tool: "Bash".to_string(),
            input: serde_json::json!({"command": "ls"}),
        };
        run_gate_call(&call, &mut front, &mut adapter_for_kill);

        assert!(front.tool_calls.is_empty(), "tool_calls: {:?}", front.tool_calls);
    }

    fn git_repo_with_one_commit() -> PathBuf {
        let dir = tempfile::tempdir().unwrap().keep();
        let run = |args: &[&str]| {
            let status = std::process::Command::new("git").arg("-C").arg(&dir).args(args).status().unwrap();
            assert!(status.success(), "git {args:?} failed in {}", dir.display());
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "arx@localhost"]);
        run(&["config", "user.name", "arx"]);
        run(&["commit", "-q", "--allow-empty", "-m", "init"]);
        fs::create_dir_all(dir.join("store")).unwrap();
        dir
    }

    fn fake_hand_spec() -> HandSpec {
        let config_dir = git_repo_with_one_commit();
        HandSpec {
            agent: "fake".to_string(),
            kind: BodyKind::Fake,
            role: HandRole::Hand,
            model: String::new(),
            cwd: config_dir.clone(),
            threads_dir: PathBuf::from("threads"),
            thread: "main".to_string(),
            provider: None,
            write: false,
            image: ledger::DEFAULT_HAND_IMAGE.to_string(),
            env: Vec::new(),
            store_root: config_dir.join("store"),
            config_dir,
            tools: Vec::new(),
            steps: config::DEFAULT_STEPS,
            window: None,
            gate: config::GateConfig::default(),
            user_gate: config::GateConfig::default(),
            store_scope: None,
            drain_input: None,
            admission: admission::AdmissionPolicy::default(),
            role_line: None,
        }
    }

    #[test]
    fn kill_line_calls_the_ledger_kill_and_the_front_receives_a_killed_delivery() {
        let dir = tempfile::tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let hand = ledger.dispatch(fake_hand_spec(), "[delay:60000] doomed").unwrap();

        let mut log = TranscriptLog::open(&dir.path().join("threads"), "main").unwrap();
        let mut front = RecordingFront::default();

        let outcome = intercept_kill_line(&format!("/kill {hand}"), &mut ledger, &mut log, &mut front);
        assert!(matches!(outcome, Some(Ok(()))), "outcome: {outcome:?}");

        let delivery = ledger.wait_next().unwrap();
        match delivery {
            Delivery::Killed { hand: h, .. } => assert_eq!(h, hand),
            other => panic!("expected killed, got {other:?}"),
        }
    }

    #[test]
    fn kill_line_for_an_unknown_id_yields_the_error_text_and_no_ledger_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let mut log = TranscriptLog::open(&dir.path().join("threads"), "main").unwrap();
        let mut front = RecordingFront::default();

        let outcome = intercept_kill_line("/kill zz", &mut ledger, &mut log, &mut front);
        assert!(matches!(outcome, Some(Ok(()))), "outcome: {outcome:?}");
        assert!(
            front.messages.iter().any(
                |m| matches!(m, TerminalMsg::Notice { text } if text.contains("unknown or already-settled hand 'zz'"))
            ),
            "messages: {:?}",
            front.messages
        );
        assert_eq!(ledger.in_flight(), 0);
    }

    fn dispatch_family_case(image: &str, family_check: Option<(String, String)>) -> (Result<HandId, serde_json::Value>, Vec<Event>) {
        let dir = tempfile::tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let threads_dir = dir.path().join("threads");
        let mut log = TranscriptLog::open(&threads_dir, "main").unwrap();
        let log_cell: RefCell<&mut TranscriptLog> = RefCell::new(&mut log);
        let mut front = RecordingFront::default();
        let front_cell: RefCell<&mut dyn Front> = RefCell::new(&mut front);
        let event_err: RefCell<Option<transcript::TranscriptError>> = RefCell::new(None);
        let adapter = spawn_stub_adapter();
        let mut adapter_for_kill = ManuallyDrop::new(adapter.clone());

        let mut spec = fake_hand_spec();
        spec.role = HandRole::Reviewer;
        spec.image = image.to_string();

        let result = dispatch_and_record(
            &mut ledger,
            &event_err,
            &mut adapter_for_kill,
            spec,
            family_check,
            "review the diff",
            &log_cell,
            &front_cell,
        );
        drop(log_cell);
        drop(front_cell);
        let events = transcript::events(&threads_dir, "main").unwrap();
        (result, events)
    }

    #[test]
    fn same_family_reviewer_whose_preparation_fails_emits_neither_family_nor_spawn_event() {
        let (result, events) =
            dispatch_family_case("arx-r18-missing-image:does-not-exist", Some(("claude".to_string(), "claude".to_string())));

        assert!(result.is_err(), "expected dispatch preparation to fail, got {result:?}");
        assert!(events.is_empty(), "same-family reviewer prep failure must emit no family or spawn event, got {events:?}");
    }

    #[test]
    fn successful_same_family_dispatch_records_exactly_one_family_event_before_the_spawn_event() {
        let (result, events) =
            dispatch_family_case(ledger::DEFAULT_HAND_IMAGE, Some(("claude".to_string(), "claude".to_string())));

        assert!(result.is_ok(), "expected dispatch to succeed, got {result:?}");
        let family_count = events.iter().filter(|e| matches!(e, Event::Family { .. })).count();
        assert_eq!(family_count, 1, "events: {events:?}");

        let family_index = events.iter().position(|e| matches!(e, Event::Family { .. })).expect("family event recorded");
        let spawn_index = events.iter().position(|e| matches!(e, Event::Spawn { .. })).expect("spawn event recorded");
        assert!(family_index < spawn_index, "family event must land before the spawn event: {events:?}");
    }

    #[test]
    fn successful_different_family_dispatch_emits_no_collision_warning() {
        let (result, events) =
            dispatch_family_case(ledger::DEFAULT_HAND_IMAGE, Some(("claude".to_string(), "gpt".to_string())));

        assert!(result.is_ok(), "expected dispatch to succeed, got {result:?}");
        assert!(
            !events.iter().any(|e| matches!(e, Event::Family { .. })),
            "different-family dispatch must not emit a collision warning: {events:?}"
        );
        assert_eq!(events.iter().filter(|e| matches!(e, Event::Spawn { .. })).count(), 1, "events: {events:?}");
    }

    #[test]
    fn failed_family_event_write_cancels_and_sweeps_the_already_dispatched_hand() {
        let dir = tempfile::tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let threads_dir = dir.path().join("threads");
        fs::create_dir_all(&threads_dir).unwrap();
        std::os::unix::fs::symlink("/dev/full", threads_dir.join("main.jsonl")).unwrap();
        let mut log = TranscriptLog::open(&threads_dir, "main").unwrap();
        let log_cell: RefCell<&mut TranscriptLog> = RefCell::new(&mut log);
        let mut front = RecordingFront::default();
        let front_cell: RefCell<&mut dyn Front> = RefCell::new(&mut front);
        let event_err: RefCell<Option<transcript::TranscriptError>> = RefCell::new(None);
        let adapter = spawn_stub_adapter();
        let mut adapter_for_kill = ManuallyDrop::new(adapter.clone());

        let mut spec = fake_hand_spec();
        spec.role = HandRole::Reviewer;

        let result = dispatch_and_record(
            &mut ledger,
            &event_err,
            &mut adapter_for_kill,
            spec,
            Some(("claude".to_string(), "claude".to_string())),
            "[delay:5000] doomed",
            &log_cell,
            &front_cell,
        );

        assert!(result.is_err(), "expected the family event write to fail, got {result:?}");
        assert!(event_err.borrow().is_some(), "expected a transcript error to be recorded");
        assert_eq!(
            ledger.in_flight(),
            0,
            "a failed family-event write must not leave the dispatched hand's resources running"
        );
    }

    struct NullStore;

    impl Store for NullStore {
        fn save(&mut self, _intent: &SaveIntent, _handle: &ClaimHandle) -> Result<Vec<arx_engine::Claim>, StoreError> {
            Ok(Vec::new())
        }

        fn recall(&mut self, _scope: &str, _query: &str, _limit: usize) -> Result<Vec<arx_engine::Claim>, StoreError> {
            Ok(Vec::new())
        }

        fn decisions_query(&mut self, _scope: &str, _query: &str, _limit: usize) -> Result<Vec<arx_engine::Claim>, StoreError> {
            Ok(Vec::new())
        }

        fn decision(&mut self, _scope: &str, _id: &str) -> Result<Option<arx_engine::Claim>, StoreError> {
            Ok(None)
        }

        fn drain(&mut self) -> Result<arx_engine::DrainReport, StoreError> {
            Ok(arx_engine::DrainReport { promoted: 0, dropped: 0 })
        }

        fn grounding(&mut self, _scope: &str) -> Result<Vec<arx_engine::Claim>, StoreError> {
            Ok(Vec::new())
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[test]
    fn a_turn_cycle_renders_exactly_one_usage_before_that_cycle_s_status() {
        let workspace = tempfile::tempdir().unwrap();
        let dir = workspace.path().to_path_buf();

        let mut adapter = spawn_stub_adapter();
        let mut adapter_for_kill = ManuallyDrop::new(adapter.clone());
        let mut adapter_for_model_fallback_kill = ManuallyDrop::new(adapter.clone());
        let mut adapter_for_store_kill = ManuallyDrop::new(adapter.clone());
        let mut adapter_for_hand_kill = ManuallyDrop::new(adapter.clone());
        let mut adapter_for_file_kill = ManuallyDrop::new(adapter.clone());

        let mut log = TranscriptLog::open(&dir.join("threads"), "main").unwrap();
        let mut front = RecordingFront::default();

        let repo_gate = config::GateConfig::default();
        let user_gate = config::GateConfig::default();
        let mut gate_table = GateTable::new(&repo_gate, "repo", &user_gate, "user").unwrap();
        gate_table.set_context("main", &dir);
        let gate_note = gate_table.note_handle();
        let gate_repo_tier = gate_table.repo_handle();
        let gate_updated_input_notice = gate_table.updated_input_notice_handle();
        let repo_gate_path = dir.join("arx.toml");

        let mut store = NullStore;
        let mut ledger = SpawnLedger::open(&dir.join("threads"), "main").unwrap();
        let mut last_handle = transcript::Handle { thread: "main".to_string(), turn: 0 };
        let resolved = bare_resolved(Vec::new());
        let mut current_model = "stub-model".to_string();
        let mut nudge = Nudge { threshold: 120_000, said: false, pending: None };
        let mut checkpoint = Checkpoint { delta: None, base: 0, last_occupancy: 0, number: None, written_delta: None };
        let mut pending_drains: HashMap<HandId, Vec<String>> = HashMap::new();
        let mut proposals: HashMap<HandId, Vec<Suggestion>> = HashMap::new();
        let mut admission_cache = admission::Cache::new();
        let mut admission_seq: u64 = 0;

        let result = run_turn_cycle(
            "operator",
            "hello",
            &[],
            &mut adapter,
            &mut adapter_for_kill,
            &mut adapter_for_model_fallback_kill,
            &mut adapter_for_store_kill,
            &mut adapter_for_hand_kill,
            &mut adapter_for_file_kill,
            &mut log,
            &mut front,
            &mut gate_table,
            &gate_note,
            &gate_repo_tier,
            &gate_updated_input_notice,
            &repo_gate_path,
            &mut store,
            &mut ledger,
            &mut last_handle,
            &resolved,
            &mut Roster::default(),
            &mut None,
            &mut current_model,
            &dir,
            &user_gate,
            &mut nudge,
            &mut checkpoint,
            &mut pending_drains,
            &mut proposals,
            &mut admission_cache,
            &mut admission_seq,
        );
        assert!(result.is_ok(), "cycle failed");

        let usage_positions: Vec<usize> =
            front.messages.iter().enumerate().filter(|(_, m)| matches!(m, TerminalMsg::Usage { .. })).map(|(i, _)| i).collect();
        assert_eq!(usage_positions.len(), 1, "messages: {:?}", front.messages);

        let status_positions: Vec<usize> =
            front.messages.iter().enumerate().filter(|(_, m)| matches!(m, TerminalMsg::Status { .. })).map(|(i, _)| i).collect();
        assert_eq!(status_positions.len(), 1, "messages: {:?}", front.messages);
        assert!(usage_positions[0] < status_positions[0], "Usage must render before that cycle's Status");

        match &front.messages[usage_positions[0]] {
            TerminalMsg::Usage { input, output, cache_creation, cache_read } => {
                assert_eq!(*input, 7);
                assert_eq!(*output, 5);
                assert_eq!(*cache_creation, 0);
                assert_eq!(*cache_read, 40);
            }
            other => panic!("expected Usage, got {other:?}"),
        }
    }

    /// A hand-written sidecar (never the shared stub, which does not understand
    /// `interrupt`) that answers `prompt` with one delta then holds until it sees
    /// `{"op":"interrupt"}`, at which point it ends the turn with `stopped: true`. Every
    /// `interrupt` op it receives is appended to `ops_log`, so the test can prove the
    /// adapter sent exactly one.
    fn spawn_control_probe_adapter() -> (MainBody, PathBuf) {
        let sidecar_dir = tempfile::tempdir().unwrap().keep();
        let ops_log = sidecar_dir.join("received-interrupts.log");
        let ops_log_json = serde_json::to_string(&ops_log.display().to_string()).unwrap();
        let script = format!(
            r#"
import readline from 'node:readline';
import {{ appendFileSync }} from 'node:fs';

function writeEvent(ev) {{
  process.stdout.write(JSON.stringify(ev) + '\n');
}}

const rl = readline.createInterface({{ input: process.stdin, terminal: false }});

rl.on('line', (line) => {{
  if (!line.trim()) return;
  const op = JSON.parse(line);
  if (op.op === 'init') {{
    writeEvent({{ ev: 'ready' }});
  }} else if (op.op === 'prompt') {{
    writeEvent({{ ev: 'delta', text: 'partial' }});
  }} else if (op.op === 'interrupt') {{
    appendFileSync({ops_log_json}, 'interrupt\n');
    writeEvent({{ ev: 'turn', text: 'partial' }});
    writeEvent({{ ev: 'usage', usage: {{ input_tokens: 1, output_tokens: 1 }} }});
    writeEvent({{ ev: 'turn_end', stopped: true }});
  }} else if (op.op === 'shutdown') {{
    process.exit(0);
  }}
}});

rl.on('close', () => process.exit(0));
"#,
        );
        fs::write(sidecar_dir.join("claude-main.mjs"), script).unwrap();

        let adapter = MainBody::spawn(
            &sidecar_dir,
            SpawnOpts {
                model: "stub-model".to_string(),
                system_prompt: SystemPrompt {
                    static_block: "stub prompt".to_string(),
                    dynamic: String::new(),
                },
                cwd: tempfile::tempdir().unwrap().keep(),
            },
        )
        .expect("control-probe sidecar spawns");
        (adapter, ops_log)
    }

    /// A sidecar that fires `count` deltas back to back with no delay, then hangs until
    /// it sees `{"op":"interrupt"}`, at which point it ends the turn with `stopped: true`.
    /// Proves the control lane is polled even when messages arrive faster than the
    /// receive loop's own wait, not only when that wait times out.
    fn spawn_burst_then_hang_adapter(count: u32) -> MainBody {
        let sidecar_dir = tempfile::tempdir().unwrap().keep();
        let mut deltas = String::new();
        for i in 0..count {
            deltas.push_str(&format!("writeEvent({{ ev: 'delta', text: 'chunk{i}' }});\n"));
        }
        let script = format!(
            r#"
import readline from 'node:readline';

function writeEvent(ev) {{
  process.stdout.write(JSON.stringify(ev) + '\n');
}}

const rl = readline.createInterface({{ input: process.stdin, terminal: false }});

rl.on('line', (line) => {{
  if (!line.trim()) return;
  const op = JSON.parse(line);
  if (op.op === 'init') {{
    writeEvent({{ ev: 'ready' }});
  }} else if (op.op === 'prompt') {{
    {deltas}
  }} else if (op.op === 'interrupt') {{
    writeEvent({{ ev: 'turn', text: 'burst then stopped' }});
    writeEvent({{ ev: 'turn_end', stopped: true }});
  }} else if (op.op === 'shutdown') {{
    process.exit(0);
  }}
}});

rl.on('close', () => process.exit(0));
"#,
        );
        fs::write(sidecar_dir.join("claude-main.mjs"), script).unwrap();
        MainBody::spawn(
            &sidecar_dir,
            SpawnOpts {
                model: "stub-model".to_string(),
                system_prompt: SystemPrompt {
                    static_block: "stub prompt".to_string(),
                    dynamic: String::new(),
                },
                cwd: tempfile::tempdir().unwrap().keep(),
            },
        )
        .expect("burst sidecar spawns")
    }

    #[test]
    fn on_control_is_polled_between_every_message_not_only_on_a_timeout() {
        let mut adapter = spawn_burst_then_hang_adapter(5);
        let mut calls = 0u32;
        let mut on_control = || {
            calls += 1;
            if calls == 3 { Some(main_body::Control::Stop) } else { None }
        };

        let outcome = adapter
            .run_turn_controlled(
                "go",
                &[],
                |_| {},
                |_, _| {},
                |_| serde_json::json!({"error": "no store"}),
                |_| serde_json::json!({"error": "no hands"}),
                |_| serde_json::json!({"error": "no files"}),
                &mut on_control,
            )
            .expect("the burst still completes once stopped");

        assert!(outcome.stopped);
        assert!(calls >= 3, "on_control must have been reached without waiting for a timeout: calls={calls}");
    }

    #[test]
    fn a_stray_stop_left_in_the_control_channel_from_a_previous_turn_is_drained_and_never_interrupts_the_next_one() {
        let (mut adapter, prompt_log) = spawn_prompt_capture_adapter();
        let pending = std::rc::Rc::new(std::cell::RefCell::new(vec![main_body::Control::Stop]));
        let pending_for_closure = pending.clone();
        let mut on_control = move || pending_for_closure.borrow_mut().pop();

        let outcome = adapter
            .run_turn_controlled(
                "hello",
                &[],
                |_| {},
                |_, _| {},
                |_| serde_json::json!({"error": "no store"}),
                |_| serde_json::json!({"error": "no hands"}),
                |_| serde_json::json!({"error": "no files"}),
                &mut on_control,
            )
            .expect("turn completes normally");

        assert!(!outcome.stopped, "the stale signal must not stop a turn it was never meant for");
        assert_eq!(outcome.text, "ack");
        let received = fs::read_to_string(&prompt_log).unwrap();
        assert!(!received.contains("interrupt"), "no interrupt op should ever have been sent: {received:?}");
    }

    #[test]
    fn run_turn_controlled_sends_exactly_one_interrupt_and_returns_stopped_true() {
        let (mut adapter, ops_log) = spawn_control_probe_adapter();

        let mut calls = 0u32;
        let mut on_control = || {
            calls += 1;
            if calls >= 2 {
                Some(main_body::Control::Stop)
            } else {
                None
            }
        };

        let outcome = adapter
            .run_turn_controlled(
                "hang until stopped",
                &[],
                |_| {},
                |_, _| {},
                |_| serde_json::json!({"error": "no store"}),
                |_| serde_json::json!({"error": "no hands"}),
                |_| serde_json::json!({"error": "no files"}),
                &mut on_control,
            )
            .expect("the interrupted turn still completes");

        assert_eq!(outcome.text, "partial");
        assert!(outcome.stopped);

        let received = fs::read_to_string(&ops_log).unwrap_or_default();
        assert_eq!(received.lines().count(), 1, "expected exactly one interrupt op, got: {received:?}");
    }

    /// A hand-written sidecar that answers `init` with `ready`, then dumps the raw
    /// `prompt` op line it received to `prompt_log` and ends the turn at once.
    fn spawn_prompt_capture_adapter() -> (MainBody, PathBuf) {
        let sidecar_dir = tempfile::tempdir().unwrap().keep();
        let prompt_log = sidecar_dir.join("received-prompt.log");
        let prompt_log_json = serde_json::to_string(&prompt_log.display().to_string()).unwrap();
        let script = format!(
            r#"
import readline from 'node:readline';
import {{ appendFileSync }} from 'node:fs';

function writeEvent(ev) {{
  process.stdout.write(JSON.stringify(ev) + '\n');
}}

const rl = readline.createInterface({{ input: process.stdin, terminal: false }});

rl.on('line', (line) => {{
  if (!line.trim()) return;
  const op = JSON.parse(line);
  if (op.op === 'init') {{
    writeEvent({{ ev: 'ready' }});
  }} else if (op.op === 'prompt') {{
    appendFileSync({prompt_log_json}, line.trim() + '\n');
    writeEvent({{ ev: 'turn', text: 'ack' }});
    writeEvent({{ ev: 'turn_end' }});
  }} else if (op.op === 'interrupt') {{
    appendFileSync({prompt_log_json}, 'interrupt\n');
  }} else if (op.op === 'shutdown') {{
    process.exit(0);
  }}
}});

rl.on('close', () => process.exit(0));
"#,
        );
        fs::write(sidecar_dir.join("claude-main.mjs"), script).unwrap();

        let adapter = MainBody::spawn(
            &sidecar_dir,
            SpawnOpts {
                model: "stub-model".to_string(),
                system_prompt: SystemPrompt {
                    static_block: "stub prompt".to_string(),
                    dynamic: String::new(),
                },
                cwd: tempfile::tempdir().unwrap().keep(),
            },
        )
        .expect("prompt-capture sidecar spawns");
        (adapter, prompt_log)
    }

    fn no_control() -> Option<main_body::Control> {
        None
    }

    #[test]
    fn a_turn_with_one_image_writes_a_prompt_op_whose_images_carry_the_path_and_media_type() {
        let (mut adapter, prompt_log) = spawn_prompt_capture_adapter();
        let image = main_body::Attachment {
            n: 1,
            path: PathBuf::from("threads/main.images/1.png"),
            media_type: "image/png".to_string(),
            bytes: 3,
        };

        adapter
            .run_turn_controlled(
                "what is in this image [image 1]",
                &[image],
                |_| {},
                |_, _| {},
                |_| serde_json::json!({"error": "no store"}),
                |_| serde_json::json!({"error": "no hands"}),
                |_| serde_json::json!({"error": "no files"}),
                &mut no_control,
            )
            .expect("turn completes");

        let received = fs::read_to_string(&prompt_log).unwrap();
        let op: serde_json::Value = serde_json::from_str(received.trim()).unwrap();
        assert_eq!(op["text"], "what is in this image [image 1]");
        assert_eq!(op["images"][0]["path"], "threads/main.images/1.png");
        assert_eq!(op["images"][0]["media_type"], "image/png");
    }

    #[test]
    fn a_line_with_no_images_writes_the_op_it_writes_today_byte_identical() {
        let (mut adapter, prompt_log) = spawn_prompt_capture_adapter();

        adapter
            .run_turn(
                "hello",
                |_| {},
                |_, _| {},
                |_| serde_json::json!({"error": "no store"}),
                |_| serde_json::json!({"error": "no hands"}),
                |_| serde_json::json!({"error": "no files"}),
            )
            .expect("turn completes");

        let received = fs::read_to_string(&prompt_log).unwrap();
        assert_eq!(received.trim(), r#"{"op":"prompt","text":"hello"}"#);
    }

    fn file_workspace() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn test_admission_ctx<'a>(workspace: &Path, policy: &'a admission::AdmissionPolicy, cache: &'a mut admission::Cache, seq: &'a mut u64) -> AdmissionCtx<'a> {
        AdmissionCtx {
            policy,
            cache,
            root: admission::TraceRoot {
                host_dir: workspace.join("threads/main.traces"),
                handle_prefix: "threads/main.traces".to_string(),
                fallback_dir: std::env::temp_dir().join("arx-traces").join("main-test"),
            },
            threads_dir: workspace.join("threads"),
            turn: 1,
            seq,
        }
    }

    #[test]
    fn read_file_is_bounded_to_the_workspace() {
        let workspace = file_workspace();
        fs::write(workspace.path().join("secret.txt"), "shh").unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "shh").unwrap();
        let rel = outside.path().strip_prefix(workspace.path().parent().unwrap()).map(|p| p.to_path_buf());
        let path = match rel {
            Ok(rel) => Path::new("..").join(rel).join("secret.txt").display().to_string(),
            Err(_) => outside.path().join("secret.txt").display().to_string(),
        };
        let call = FileCall {
            tool: "read_file".to_string(),
            args: serde_json::json!({"path": path}),
        };
        let policy = admission::AdmissionPolicy::default();
        let mut cache = admission::Cache::new();
        let mut seq = 0u64;
        let mut ctx = test_admission_ctx(workspace.path(), &policy, &mut cache, &mut seq);
        let (result, row, _events) = file_call(workspace.path(), &call, &mut ctx);
        assert_eq!(result, serde_json::json!({"error": "read_file error: path escapes the workspace"}));
        assert!(row.ends_with("read_file error: path escapes the workspace"), "row: {row}");
    }

    #[test]
    fn read_file_returns_a_small_file_whole() {
        let workspace = file_workspace();
        fs::write(workspace.path().join("small.txt"), "hello there").unwrap();
        let call = FileCall {
            tool: "read_file".to_string(),
            args: serde_json::json!({"path": "small.txt"}),
        };
        let policy = admission::AdmissionPolicy::default();
        let mut cache = admission::Cache::new();
        let mut seq = 0u64;
        let mut ctx = test_admission_ctx(workspace.path(), &policy, &mut cache, &mut seq);
        let (result, row, events) = file_call(workspace.path(), &call, &mut ctx);
        assert_eq!(result, serde_json::json!({"text": "hello there"}));
        assert_eq!(row, "small.txt: read 11 bytes, inline");
        assert_eq!(events, vec![Event::Admission { tool: "read_file".to_string(), outcome: "inline".to_string(), bytes: 11, handle: None }]);
    }

    #[test]
    fn read_file_over_the_threshold_externalizes_under_the_thread() {
        let workspace = file_workspace();
        let content = "x".repeat(16_484);
        fs::write(workspace.path().join("big.txt"), &content).unwrap();
        let call = FileCall {
            tool: "read_file".to_string(),
            args: serde_json::json!({"path": "big.txt"}),
        };
        let policy = admission::AdmissionPolicy::default();
        let mut cache = admission::Cache::new();
        let mut seq = 0u64;
        let mut ctx = test_admission_ctx(workspace.path(), &policy, &mut cache, &mut seq);
        let (result, row, events) = file_call(workspace.path(), &call, &mut ctx);
        let text = result["text"].as_str().unwrap();
        let handle = "threads/main.traces/1-1.md";
        assert!(
            text.starts_with(&format!("[arx] externalized read_file result: {handle} (16484 bytes, 1 lines; body from line 12)")),
            "text head: {}",
            &text[..text.len().min(160)]
        );
        assert_eq!(row, "big.txt: read 16484 bytes, externalized");
        assert!(matches!(&events[0], Event::Admission { outcome, handle: Some(h), .. } if outcome == "externalized" && h == handle));

        let trace_path = workspace.path().join("threads/main.traces/1-1.md");
        let trace_contents = fs::read_to_string(&trace_path).unwrap();
        assert!(trace_contents.starts_with("---\nproducer: externalized tool result\n"), "trace: {trace_contents}");
        assert!(trace_contents.contains("tool: read_file"));
        assert!(trace_contents.contains("source: big.txt"));
    }

    #[test]
    fn read_file_slices_by_offset_and_limit() {
        let workspace = file_workspace();
        fs::write(workspace.path().join("lines.txt"), "one\ntwo\nthree\nfour\n").unwrap();
        let call = FileCall {
            tool: "read_file".to_string(),
            args: serde_json::json!({"path": "lines.txt", "offset": 2, "limit": 2}),
        };
        let policy = admission::AdmissionPolicy::default();
        let mut cache = admission::Cache::new();
        let mut seq = 0u64;
        let mut ctx = test_admission_ctx(workspace.path(), &policy, &mut cache, &mut seq);
        let (result, row, events) = file_call(workspace.path(), &call, &mut ctx);
        assert_eq!(result, serde_json::json!({"text": "two\nthree\n"}));
        assert_eq!(row, "lines.txt: read 19 bytes, inline");
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn read_file_reads_the_tmp_fallback_root() {
        let workspace = file_workspace();
        let policy = admission::AdmissionPolicy::default();
        let mut cache = admission::Cache::new();
        let mut seq = 0u64;

        let host_dir = workspace.path().join("threads/main.traces");
        fs::create_dir_all(workspace.path().join("threads")).unwrap();
        fs::write(&host_dir, b"not a directory").unwrap();
        let fallback_dir = std::env::temp_dir().join("arx-traces").join(format!("main-fallback-read-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&fallback_dir);

        let content = "y".repeat(16_484);
        fs::write(workspace.path().join("big.txt"), &content).unwrap();

        {
            let mut ctx = AdmissionCtx {
                policy: &policy,
                cache: &mut cache,
                root: admission::TraceRoot {
                    host_dir: host_dir.clone(),
                    handle_prefix: "threads/main.traces".to_string(),
                    fallback_dir: fallback_dir.clone(),
                },
                threads_dir: workspace.path().join("threads"),
                turn: 1,
                seq: &mut seq,
            };
            let call = FileCall {
                tool: "read_file".to_string(),
                args: serde_json::json!({"path": "big.txt"}),
            };
            let (result, _row, _events) = file_call(workspace.path(), &call, &mut ctx);
            let text = result["text"].as_str().unwrap();
            assert!(text.contains(&fallback_dir.display().to_string()), "text: {text}");

            let edit_call = FileCall {
                tool: "edit_file".to_string(),
                args: serde_json::json!({"path": fallback_dir.join("1-1.md").display().to_string(), "old": "y", "new": "z"}),
            };
            let (edit_result, _row, _events) = file_call(workspace.path(), &edit_call, &mut ctx);
            assert_eq!(edit_result, serde_json::json!({"error": "edit_file error: path escapes the workspace"}));
        }
        assert!(fallback_dir.join("1-1.md").exists());
        let _ = fs::remove_dir_all(&fallback_dir);
    }

    #[test]
    fn edit_file_replaces_exactly_one_match() {
        let workspace = file_workspace();
        fs::write(workspace.path().join("note.md"), "alpha beta alpha").unwrap();
        let call = FileCall {
            tool: "edit_file".to_string(),
            args: serde_json::json!({"path": "note.md", "old": "beta", "new": "gamma"}),
        };
        let policy = admission::AdmissionPolicy::default();
        let mut cache = admission::Cache::new();
        let mut seq = 0u64;
        let mut ctx = test_admission_ctx(workspace.path(), &policy, &mut cache, &mut seq);
        let (result, row, _events) = file_call(workspace.path(), &call, &mut ctx);
        assert_eq!(result, serde_json::json!({"text": "edited note.md"}));
        assert_eq!(row, "note.md: edit\n--- old\nbeta\n+++ new\ngamma");
        assert_eq!(fs::read_to_string(workspace.path().join("note.md")).unwrap(), "alpha gamma alpha");
    }

    #[test]
    fn edit_file_refuses_zero_or_many_matches() {
        let workspace = file_workspace();
        fs::write(workspace.path().join("note.md"), "alpha beta alpha").unwrap();
        let policy = admission::AdmissionPolicy::default();
        let mut cache = admission::Cache::new();
        let mut seq = 0u64;
        let mut ctx = test_admission_ctx(workspace.path(), &policy, &mut cache, &mut seq);
        let zero = FileCall {
            tool: "edit_file".to_string(),
            args: serde_json::json!({"path": "note.md", "old": "nope", "new": "x"}),
        };
        let (result, _row, _events) = file_call(workspace.path(), &zero, &mut ctx);
        assert_eq!(result, serde_json::json!({"error": "edit_file error: old text matches 0 times"}));

        let many = FileCall {
            tool: "edit_file".to_string(),
            args: serde_json::json!({"path": "note.md", "old": "alpha", "new": "x"}),
        };
        let (result, _row, _events) = file_call(workspace.path(), &many, &mut ctx);
        assert_eq!(result, serde_json::json!({"error": "edit_file error: old text matches 2 times"}));
        assert_eq!(fs::read_to_string(workspace.path().join("note.md")).unwrap(), "alpha beta alpha");
    }

    #[test]
    fn edit_file_refuses_arx_state() {
        let workspace = file_workspace();
        fs::create_dir_all(workspace.path().join("store")).unwrap();
        fs::write(workspace.path().join("store/x.md"), "keep").unwrap();
        let call = FileCall {
            tool: "edit_file".to_string(),
            args: serde_json::json!({"path": "store/x.md", "old": "keep", "new": "changed"}),
        };
        let policy = admission::AdmissionPolicy::default();
        let mut cache = admission::Cache::new();
        let mut seq = 0u64;
        let mut ctx = test_admission_ctx(workspace.path(), &policy, &mut cache, &mut seq);
        let (result, row, _events) = file_call(workspace.path(), &call, &mut ctx);
        assert_eq!(result, serde_json::json!({"error": "edit_file error: store/x.md is arx's own state"}));
        assert!(row.ends_with("edit_file error: store/x.md is arx's own state"), "row: {row}");
        assert_eq!(fs::read_to_string(workspace.path().join("store/x.md")).unwrap(), "keep");
    }

    fn run_list_files(workspace: &Path, path: &str, depth: u64) -> serde_json::Value {
        let call = FileCall {
            tool: "list_files".to_string(),
            args: serde_json::json!({"path": path, "depth": depth}),
        };
        let policy = admission::AdmissionPolicy::default();
        let mut cache = admission::Cache::new();
        let mut seq = 0u64;
        let mut ctx = test_admission_ctx(workspace, &policy, &mut cache, &mut seq);
        let (result, _row, _events) = file_call(workspace, &call, &mut ctx);
        result
    }

    #[test]
    fn list_files_does_not_expose_entries_through_an_escaping_directory_symlink() {
        let workspace = file_workspace();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "shh").unwrap();
        std::os::unix::fs::symlink(outside.path(), workspace.path().join("escape")).unwrap();
        let result = run_list_files(workspace.path(), ".", 4);
        let text = result["text"].as_str().unwrap();
        assert!(!text.contains("secret.txt"), "listing must not expose the escaped directory's entries: {text}");
    }

    #[test]
    fn list_files_keeps_ordinary_nested_directories_under_the_root() {
        let workspace = file_workspace();
        fs::create_dir_all(workspace.path().join("a/b")).unwrap();
        fs::write(workspace.path().join("a/b/c.txt"), "hi").unwrap();
        let result = run_list_files(workspace.path(), ".", 4);
        let text = result["text"].as_str().unwrap();
        assert!(text.contains("a/"), "text: {text}");
        assert!(text.contains("a/b/"), "text: {text}");
        assert!(text.contains("a/b/c.txt"), "text: {text}");
        let canonical_root = workspace.path().canonicalize().unwrap();
        for line in text.lines() {
            let name = line.trim_end_matches('/');
            let canonical = canonical_root.join(name).canonicalize().unwrap();
            assert!(canonical.starts_with(&canonical_root), "entry escaped root: {line}");
        }
    }

    #[test]
    fn list_files_dangling_symlink_terminates_cleanly() {
        let workspace = file_workspace();
        std::os::unix::fs::symlink(workspace.path().join("does-not-exist"), workspace.path().join("dangling")).unwrap();
        let result = run_list_files(workspace.path(), ".", 4);
        assert!(result.get("error").is_none(), "result: {result}");
        let text = result["text"].as_str().unwrap();
        assert!(text.contains("dangling"), "text: {text}");
    }

    #[test]
    fn list_files_symlink_loop_terminates_within_a_bound() {
        let workspace = file_workspace();
        fs::create_dir_all(workspace.path().join("loop")).unwrap();
        std::os::unix::fs::symlink(workspace.path().join("loop"), workspace.path().join("loop/self")).unwrap();
        let workspace_path = workspace.path().to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = run_list_files(&workspace_path, ".", 8);
            let _ = tx.send(result);
        });
        let result = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("list_files must terminate on a symlink loop, not hang");
        assert!(result.get("error").is_none(), "result: {result}");
    }

    #[test]
    fn every_kind_round_trips_with_kind_first() {
        assert_eq!(events::every_kind().len(), events::KINDS.len());
        for event in events::every_kind() {
            let json = serde_json::to_string(&event).unwrap();
            assert!(json.starts_with("{\"kind\":"), "json: {json}");
            let parsed: Event = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, event);
        }
    }

    #[test]
    fn gate_event_serializes_to_the_locked_bytes() {
        let event = Event::Gate {
            tool: "save".to_string(),
            verdict: "allow".to_string(),
            by: None,
            reason: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "{\"kind\":\"gate\",\"tool\":\"save\",\"verdict\":\"allow\"}");
    }

    #[test]
    fn hand_nudge_fires_at_or_above_threshold_and_never_below() {
        let at_threshold = Delivery::Verdict {
            hand: "h9".to_string(),
            text: "done".to_string(),
            trace: None,
            bytes: None,
            usage: Some(serde_json::json!({
                "input_tokens": 30,
                "cache_creation_input_tokens": 0,
                "cache_read_input_tokens": 10
            })),
            walls: Vec::new(),
            miss: None,
            diff: None,
        };
        let event = hand_nudge(&at_threshold, 40).expect("occupancy at the threshold nudges");
        assert_eq!(
            event,
            Event::Nudge {
                seat: "hand".to_string(),
                hand: Some("h9".to_string()),
                occupancy: 40,
                threshold: 40,
            }
        );

        let below_threshold = Delivery::Verdict {
            hand: "h9".to_string(),
            text: "done".to_string(),
            trace: None,
            bytes: None,
            usage: Some(serde_json::json!({"input_tokens": 39})),
            walls: Vec::new(),
            miss: None,
            diff: None,
        };
        assert_eq!(hand_nudge(&below_threshold, 40), None);

        let no_usage = Delivery::Verdict {
            hand: "h9".to_string(),
            text: "done".to_string(),
            trace: None,
            bytes: None,
            usage: None,
            walls: Vec::new(),
            miss: None,
            diff: None,
        };
        assert_eq!(hand_nudge(&no_usage, 40), None);

        let stop = Delivery::Stop {
            hand: "h9".to_string(),
            question: "which db?".to_string(),
            wall: None,
        };
        assert_eq!(hand_nudge(&stop, 40), None);
    }

    fn verdict_delivery(hand: &str, text: &str) -> Delivery {
        Delivery::Verdict {
            hand: hand.to_string(),
            text: text.to_string(),
            trace: Some(format!("threads/main.{hand}")),
            bytes: None,
            usage: None,
            walls: Vec::new(),
            miss: None,
            diff: None,
        }
    }

    fn verdict_test_root(workspace: &Path) -> admission::TraceRoot {
        admission::TraceRoot {
            host_dir: workspace.join("threads/main.traces"),
            handle_prefix: "threads/main.traces".to_string(),
            fallback_dir: workspace.join("fallback-main"),
        }
    }

    #[test]
    fn format_delivery_does_not_bound_an_oversized_verdict_by_itself() {
        let huge = "z".repeat(20_000);
        let delivery = verdict_delivery("h1", &huge);
        let out = format_delivery(&delivery);
        assert_eq!(out, format!("[hand h1 verdict] {huge} (trace: threads/main.h1)"), "format_delivery stays a pure passthrough today");
        assert!(out.len() > 20_000, "regression baseline: the raw formatter lands the whole oversized verdict unbounded");
    }

    #[test]
    fn bound_verdict_delivery_leaves_a_short_verdict_byte_identical() {
        let delivery = verdict_delivery("h1", "stub verdict: Trace a bug");
        let formatted = format_delivery(&delivery);
        let workspace = tempfile::tempdir().unwrap();
        let policy = admission::AdmissionPolicy::default();
        let root = verdict_test_root(workspace.path());
        let mut seq = 0u64;
        let (bounded, events) = bound_verdict_delivery(&delivery, formatted.clone(), &policy, &root, &mut seq, 1);
        assert_eq!(bounded, "[hand h1 verdict] stub verdict: Trace a bug (trace: threads/main.h1)");
        assert_eq!(bounded, formatted, "under threshold, bounding must be a no-op");
        assert!(!root.host_dir.exists(), "a short verdict never mints a trace file");
        assert_eq!(
            events,
            vec![Event::Admission {
                tool: "hand_verdict".to_string(),
                outcome: "inline".to_string(),
                bytes: formatted.len() as u64,
                handle: None,
            }],
            "an inline verdict still records its admission outcome for D09's calibration record"
        );
    }

    #[test]
    fn bound_verdict_delivery_externalizes_an_oversized_verdict_with_a_retrievable_handle() {
        let prose = "z".repeat(20_000);
        let fence = "```yaml\nverdict: FAIL\ngate: ran\nmust_fix:\n  - fix the thing\n```\n";
        let raw = format!("{prose}\n{fence}");
        let delivery = verdict_delivery("h1", &raw);
        let formatted = format_delivery(&delivery);
        assert!(formatted.len() > 20_000, "sanity: the pre-gate formatted line is still oversized");

        let workspace = tempfile::tempdir().unwrap();
        let policy = admission::AdmissionPolicy::default();
        let root = verdict_test_root(workspace.path());
        let mut seq = 0u64;
        let (bounded, events) = bound_verdict_delivery(&delivery, formatted.clone(), &policy, &root, &mut seq, 3);

        assert!(bounded.len() < 2000, "main must see a bounded preview, not the whole verdict: {bounded}");
        assert!(bounded.contains("verdict: FAIL"), "preview must keep the structured head: {bounded}");
        assert!(bounded.contains("gate: ran"), "preview must keep the structured head: {bounded}");
        assert!(bounded.contains("must_fix:"), "preview must keep the structured head: {bounded}");

        let handle = bounded
            .split_whitespace()
            .find(|token| token.starts_with("threads/main.traces/"))
            .expect("preview names a handle");
        let file_path = workspace.path().join(handle);
        let contents = fs::read_to_string(&file_path).unwrap();
        assert!(contents.ends_with(&formatted), "the full verdict, including the [hand ...] wrapper, must stay retrievable through the handle");
        assert!(contents.contains(&raw), "the raw verdict text must survive whole");
        assert_eq!(
            events,
            vec![Event::Admission {
                tool: "hand_verdict".to_string(),
                outcome: "externalized".to_string(),
                bytes: formatted.len() as u64,
                handle: Some(handle.to_string()),
            }],
            "the externalization must be recorded, matching D09's per-tool pull-rate instrumentation"
        );
    }

    #[test]
    fn bound_verdict_delivery_bounds_a_failed_hands_verdict_too() {
        let stderr_tail = "e".repeat(20_000);
        let raw = format!("claude -p failed — exit status: 1; stderr tail: {stderr_tail}");
        let delivery = verdict_delivery("h2", &raw);
        let formatted = format_delivery(&delivery);

        let workspace = tempfile::tempdir().unwrap();
        let policy = admission::AdmissionPolicy::default();
        let root = verdict_test_root(workspace.path());
        let mut seq = 0u64;
        let (bounded, events) = bound_verdict_delivery(&delivery, formatted, &policy, &root, &mut seq, 1);

        assert!(bounded.len() < 2000, "a failed hand's verdict must be bounded too: {bounded}");
        assert!(bounded.contains("externalized hand h2 verdict"), "preview: {bounded}");
        assert_eq!(events.len(), 1, "the failure path admits through the same event-emitting ladder: {events:?}");
    }

    #[test]
    fn bound_verdict_delivery_leaves_stop_and_killed_deliveries_untouched() {
        let huge_question = "why?".repeat(10_000);
        let stop = Delivery::Stop {
            hand: "h3".to_string(),
            question: huge_question.clone(),
            wall: None,
        };
        let killed = Delivery::Killed {
            hand: "h4".to_string(),
            trace: Some("threads/main.h4".to_string()),
            bytes: None,
        };
        let workspace = tempfile::tempdir().unwrap();
        let policy = admission::AdmissionPolicy::default();
        let root = verdict_test_root(workspace.path());
        let mut seq = 0u64;

        let stop_text = format_delivery(&stop);
        let (bounded_stop, stop_events) = bound_verdict_delivery(&stop, stop_text.clone(), &policy, &root, &mut seq, 1);
        assert_eq!(bounded_stop, stop_text, "this card scopes bounding to verdicts, not stops");
        assert!(stop_events.is_empty());

        let killed_text = format_delivery(&killed);
        let (bounded_killed, killed_events) = bound_verdict_delivery(&killed, killed_text.clone(), &policy, &root, &mut seq, 1);
        assert_eq!(bounded_killed, killed_text);
        assert!(killed_events.is_empty());
        assert!(!root.host_dir.exists());
    }

    #[test]
    fn timing_is_elapsed_to_the_send_and_lag_to_the_pull() {
        let started = Instant::now();
        let stamp = started + std::time::Duration::from_millis(120);
        let now = stamp + std::time::Duration::from_millis(50);
        let timing = ledger::Timing::from_instants(started, stamp, now);
        assert_eq!(timing.elapsed_ms, 120);
        assert_eq!(timing.lag_ms, 50);
    }

    #[test]
    fn a_delivered_line_prints_its_timings_after_outcome() {
        let event = Event::Delivered {
            hand: "h1".to_string(),
            outcome: "verdict".to_string(),
            trace: None,
            bytes: None,
            elapsed_ms: Some(120),
            lag_ms: Some(50),
        };
        assert_eq!(
            event_fields(&event),
            vec![
                ("hand", "h1".to_string()),
                ("outcome", "verdict".to_string()),
                ("elapsed_ms", "120".to_string()),
                ("lag_ms", "50".to_string()),
            ]
        );

        let bare = Event::Delivered {
            hand: "h1".to_string(),
            outcome: "killed".to_string(),
            trace: None,
            bytes: None,
            elapsed_ms: None,
            lag_ms: None,
        };
        assert_eq!(
            event_fields(&bare),
            vec![("hand", "h1".to_string()), ("outcome", "killed".to_string())]
        );
    }

    fn gate_call(tool: &str, field: &str, value: &str) -> GateCall {
        GateCall {
            tool: tool.to_string(),
            input: serde_json::json!({ field: value }),
        }
    }

    #[test]
    fn rule_parse_round_trips_the_locked_forms() {
        let bare = gate::Rule::parse("Bash").unwrap();
        assert_eq!(bare.tool, "Bash");
        assert_eq!(bare.pattern, None);

        let wildcard = gate::Rule::parse("Bash(git push*)").unwrap();
        assert_eq!(wildcard.tool, "Bash");
        assert_eq!(wildcard.pattern.as_deref(), Some("git push*"));

        let prefix = gate::Rule::parse("Bash(git push:*)").unwrap();
        assert_eq!(prefix.pattern.as_deref(), Some("git push*"));

        let path = gate::Rule::parse("Read(/etc/*)").unwrap();
        assert_eq!(path.tool, "Read");
        assert_eq!(path.pattern.as_deref(), Some("/etc/*"));
    }

    #[test]
    fn rule_parse_errors_on_malformed_syntax() {
        assert!(gate::Rule::parse("(x)").is_err());
        assert!(gate::Rule::parse("Bash(").is_err());
    }

    #[test]
    fn parse_tier_rules_refuses_a_pattern_on_a_tool_without_a_subject() {
        let err = gate::parse_tier_rules(&["mcp__arx__save(x)".to_string()], "./arx.toml").unwrap_err();
        assert!(err.contains("mcp__arx__save"));
        assert!(err.contains("bare tool name"));
    }

    #[test]
    fn subject_reads_the_locked_field_per_tool_and_none_off_the_table() {
        assert_eq!(gate::subject(&gate_call("Bash", "command", "ls")).as_deref(), Some("ls"));
        assert_eq!(
            gate::subject(&gate_call("Read", "file_path", "/etc/passwd")).as_deref(),
            Some("/etc/passwd")
        );
        assert_eq!(gate::subject(&gate_call("Edit", "file_path", "a.rs")).as_deref(), Some("a.rs"));
        assert_eq!(gate::subject(&gate_call("Write", "file_path", "a.rs")).as_deref(), Some("a.rs"));
        assert_eq!(
            gate::subject(&gate_call("MultiEdit", "file_path", "a.rs")).as_deref(),
            Some("a.rs")
        );
        assert_eq!(
            gate::subject(&gate_call("NotebookEdit", "file_path", "a.ipynb")).as_deref(),
            Some("a.ipynb")
        );
        assert_eq!(gate::subject(&gate_call("WebFetch", "url", "https://x")).as_deref(), Some("https://x"));
        assert_eq!(gate::subject(&gate_call("WebSearch", "query", "rust")).as_deref(), Some("rust"));
        assert_eq!(gate::subject(&gate_call("Grep", "pattern", "fn main")).as_deref(), Some("fn main"));
        assert_eq!(gate::subject(&gate_call("Glob", "pattern", "**/*.rs")).as_deref(), Some("**/*.rs"));
        assert_eq!(
            gate::subject(&GateCall {
                tool: "mcp__arx__save".to_string(),
                input: serde_json::json!({}),
            }),
            None
        );
    }

    #[test]
    fn precedence_repo_wins_in_full_before_the_user_tier_is_consulted() {
        let repo = config::GateConfig {
            allow: vec!["Bash(git *)".to_string()],
            ..Default::default()
        };
        let user = config::GateConfig {
            deny: vec!["Bash(git push*)".to_string()],
            ..Default::default()
        };
        let mut table = GateTable::new(&repo, "./arx.toml", &user, "user.toml").unwrap();
        let decision = table.decide_with(&gate_call("Bash", "command", "git push"));
        assert_eq!(decision.verdict, GateVerdict::Allow);
    }

    #[test]
    fn repo_deny_beats_repo_allow_on_the_same_call() {
        let repo = config::GateConfig {
            deny: vec!["Bash(git push*)".to_string()],
            allow: vec!["Bash(git *)".to_string()],
            ..Default::default()
        };
        let user = config::GateConfig::default();
        let mut table = GateTable::new(&repo, "./arx.toml", &user, "user.toml").unwrap();
        let decision = table.decide_with(&gate_call("Bash", "command", "git push"));
        assert!(matches!(decision.verdict, GateVerdict::Deny { .. }));
    }

    #[test]
    fn append_rule_preserves_comments_and_other_tables_and_appends_to_existing_arrays() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arx.toml");
        let original = "# a leading comment\n[main]\nbody = \"claude-code\"\nmodel = \"claude-sonnet-5\"\n";
        fs::write(&path, original).unwrap();

        gate::append_rule(&path, "allow", "Bash(git push origin main)").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(original), "text: {text}");
        assert!(text.contains("[gate]"), "text: {text}");
        assert!(text.contains("Bash(git push origin main)"), "text: {text}");

        gate::append_rule(&path, "allow", "Read(/etc/*)").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(original), "text: {text}");
        assert!(text.contains("Bash(git push origin main)"), "text: {text}");
        assert!(text.contains("Read(/etc/*)"), "text: {text}");
    }

    #[test]
    fn empty_tables_decide_allow_with_no_by() {
        let empty = config::GateConfig::default();
        let mut table = GateTable::new(&empty, "./arx.toml", &empty, "user.toml").unwrap();
        let decision = table.decide_with(&gate_call("Bash", "command", "ls"));
        assert_eq!(decision.verdict, GateVerdict::Allow);
        assert_eq!(decision.by, None);
        assert_eq!(decision.reason, None);
    }

    fn hook_fixture(name: &str) -> String {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/hooks")
            .join(name)
            .display()
            .to_string()
    }

    #[test]
    fn run_hook_denies_via_hook_specific_output_json() {
        let hook = hook_fixture("deny-rm.sh");
        let payload = gate::hook_payload("main", std::path::Path::new("/repo"), &gate_call("Bash", "command", "rm -rf x"));
        let answer = gate::run_hook(&hook, &payload, std::time::Duration::from_secs(5));
        assert_eq!(answer, gate::HookAnswer::Deny("rm is blocked".to_string()));
    }

    #[test]
    fn run_hook_fails_loudly_rather_than_allow_when_a_hook_answers_ask() {
        let hook = hook_fixture("ask.sh");
        let payload = gate::hook_payload("main", std::path::Path::new("/repo"), &gate_call("Bash", "command", "rm -rf x"));
        let answer = gate::run_hook(&hook, &payload, std::time::Duration::from_secs(5));
        match answer {
            gate::HookAnswer::Failed(reason) => {
                assert!(reason.contains("ask"), "reason: {reason}");
                assert!(reason.contains("needs a human"), "reason: {reason}");
            }
            other => panic!("expected a loud failure naming ask, got {other:?}"),
        }
    }

    #[test]
    fn run_hook_is_no_opinion_when_deny_rm_sees_no_match() {
        let hook = hook_fixture("deny-rm.sh");
        let payload = gate::hook_payload("main", std::path::Path::new("/repo"), &gate_call("Bash", "command", "ls"));
        let answer = gate::run_hook(&hook, &payload, std::time::Duration::from_secs(5));
        assert_eq!(answer, gate::HookAnswer::NoOpinion);
    }

    #[test]
    fn run_hook_is_no_opinion_for_approve_since_allow_never_widens() {
        let hook = hook_fixture("approve.sh");
        let payload = gate::hook_payload("main", std::path::Path::new("/repo"), &gate_call("Bash", "command", "ls"));
        let answer = gate::run_hook(&hook, &payload, std::time::Duration::from_secs(5));
        assert_eq!(answer, gate::HookAnswer::NoOpinion);
    }

    #[test]
    fn run_hook_denies_on_exit_2() {
        let hook = hook_fixture("exit2.sh");
        let payload = gate::hook_payload("main", std::path::Path::new("/repo"), &gate_call("Bash", "command", "ls"));
        let answer = gate::run_hook(&hook, &payload, std::time::Duration::from_secs(5));
        assert_eq!(answer, gate::HookAnswer::Deny("blocked by exit2".to_string()));
    }

    #[test]
    fn run_hook_denies_on_timeout_in_about_five_seconds() {
        let hook = hook_fixture("slow.sh");
        let payload = gate::hook_payload("main", std::path::Path::new("/repo"), &gate_call("Bash", "command", "ls"));
        let started = std::time::Instant::now();
        let answer = gate::run_hook(&hook, &payload, std::time::Duration::from_secs(5));
        let elapsed = started.elapsed();
        match answer {
            gate::HookAnswer::Failed(reason) => {
                assert!(reason.contains("timeout"), "reason: {reason}")
            }
            other => panic!("expected a timeout failure, got {other:?}"),
        }
        assert!(elapsed >= std::time::Duration::from_secs(5), "elapsed: {elapsed:?}");
        assert!(elapsed < std::time::Duration::from_secs(10), "elapsed: {elapsed:?}");
    }

    #[test]
    fn run_hook_denies_naming_the_parse_failure_on_garbage_stdout() {
        let hook = hook_fixture("garbage.sh");
        let payload = gate::hook_payload("main", std::path::Path::new("/repo"), &gate_call("Bash", "command", "ls"));
        let answer = gate::run_hook(&hook, &payload, std::time::Duration::from_secs(5));
        match answer {
            gate::HookAnswer::Failed(reason) => {
                assert!(reason.contains("not JSON"), "reason: {reason}")
            }
            other => panic!("expected a failure naming the parse failure, got {other:?}"),
        }
    }

    #[test]
    fn run_hook_denies_naming_a_missing_command() {
        let hook = "/no/such/hook/binary".to_string();
        let payload = gate::hook_payload("main", std::path::Path::new("/repo"), &gate_call("Bash", "command", "ls"));
        let answer = gate::run_hook(&hook, &payload, std::time::Duration::from_secs(5));
        match answer {
            gate::HookAnswer::Failed(reason) => assert!(reason.contains(&hook), "reason: {reason}"),
            other => panic!("expected a failure naming the command, got {other:?}"),
        }
    }

    #[test]
    fn table_with_a_hook_denies_rm_and_falls_through_to_default_allow_on_ls() {
        let hook = hook_fixture("deny-rm.sh");
        let repo = config::GateConfig {
            hooks: vec![hook],
            ..Default::default()
        };
        let user = config::GateConfig::default();
        let mut table = GateTable::new(&repo, "./arx.toml", &user, "user.toml").unwrap();
        table.set_context("main", std::path::Path::new("/repo"));

        let deny_decision = table.decide_with(&gate_call("Bash", "command", "rm -rf x"));
        assert!(matches!(deny_decision.verdict, GateVerdict::Deny { .. }));
        assert_eq!(deny_decision.by, Some("hook".to_string()));

        let allow_decision = table.decide_with(&gate_call("Bash", "command", "ls"));
        assert_eq!(allow_decision.verdict, GateVerdict::Allow);
        assert_eq!(allow_decision.by, None);
    }

    #[test]
    fn table_with_a_hook_answering_ask_denies_the_call_rather_than_allow_it() {
        let hook = hook_fixture("ask.sh");
        let repo = config::GateConfig {
            hooks: vec![hook],
            ..Default::default()
        };
        let user = config::GateConfig::default();
        let mut table = GateTable::new(&repo, "./arx.toml", &user, "user.toml").unwrap();
        table.set_context("main", std::path::Path::new("/repo"));

        let decision = table.decide_with(&gate_call("Bash", "command", "rm -rf x"));
        assert!(matches!(decision.verdict, GateVerdict::Deny { .. }), "verdict: {:?}", decision.verdict);
        assert_eq!(decision.by, Some("hook".to_string()));
        let reason = decision.reason.expect("a denial carries a reason");
        assert!(reason.contains("ask"), "reason: {reason}");
    }

    fn ember_test_workspace() -> tempfile::TempDir {
        let workspace = tempfile::tempdir().unwrap();
        fs::create_dir_all(workspace.path().join("agents")).unwrap();
        fs::write(workspace.path().join("agents/runner.md"), "---\ntools: [Read, Bash]\nwrite: true\n---\nrunner\n").unwrap();
        fs::write(workspace.path().join("agents/reviewer.md"), "---\n{}\n---\nreviewer\n").unwrap();
        fs::write(workspace.path().join("agents/hand-ex.md"), "---\n{}\n---\nhand\n").unwrap();
        fs::write(
            workspace.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\n",
        )
        .unwrap();
        workspace
    }

    #[test]
    fn ember_refuses_without_a_committed_turn() {
        let workspace = ember_test_workspace();
        let config_path = workspace.path().join("arx.toml");

        let err = ember_inner(&config_path, "arx", "keep replies short").unwrap_err();
        assert_eq!(err, "ember: thread main has no committed turn yet — run a session first");
    }

    #[test]
    fn ember_refuses_an_unregistered_scope() {
        let workspace = ember_test_workspace();
        let config_path = workspace.path().join("arx.toml");

        let mut log = TranscriptLog::open(&workspace.path().join("threads"), "main").unwrap();
        log.append_turn_as("main", "hello").unwrap();
        drop(log);

        let err = ember_inner(&config_path, "nowhere", "keep replies short").unwrap_err();
        assert!(err.contains("no directory under the store root"), "err: {err}");
    }

    #[test]
    fn authoring_prompt_lists_hands_or_none() {
        let empty = checkpoint_authoring_prompt(1, &format_hands_block(&[]), None);
        assert!(empty.contains("[arx] checkpoint 1: author the state object"), "prompt: {empty}");
        assert!(empty.contains("## Hands in flight\n- none\n"), "prompt: {empty}");

        let hands = vec![("h1".to_string(), "fake".to_string()), ("h2".to_string(), "hand-ex".to_string())];
        let listed = checkpoint_authoring_prompt(2, &format_hands_block(&hands), None);
        assert!(listed.contains("## Hands in flight\n- h1 fake\n- h2 hand-ex\n"), "prompt: {listed}");

        let repaired = checkpoint_authoring_prompt(1, &format_hands_block(&[]), Some("The mechanical check failed on your last attempt: FileHash file: arx.toml @ abc (expected abc, found def) Fix every one."));
        assert!(
            repaired.contains("Reply with the object body only, in this shape and nothing else:\nThe mechanical check failed"),
            "prompt: {repaired}"
        );
        assert!(repaired.contains("Fix every one.\n## Position"), "prompt: {repaired}");
    }

    #[test]
    fn truth_check_reply_ok_keeps_the_authored_body() {
        let authored = "## Position\nsomething\n";
        assert_eq!(resolve_truth_check(authored, "ok"), authored);
        assert_eq!(resolve_truth_check(authored, "  OK\n"), authored);
        assert_eq!(resolve_truth_check(authored, "## Position\ncorrected\n"), "## Position\ncorrected\n");
    }

    #[test]
    fn clock_fires_on_the_delta_and_rebases() {
        let mut checkpoint = Checkpoint {
            delta: Some(1000),
            base: 0,
            last_occupancy: 999,
            number: None,
            written_delta: None,
        };
        assert!(!checkpoint.due());

        checkpoint.last_occupancy = 1000;
        assert!(checkpoint.due());

        checkpoint.base = checkpoint.last_occupancy;
        assert!(!checkpoint.due());

        checkpoint.last_occupancy = 1999;
        assert!(!checkpoint.due());
        checkpoint.last_occupancy = 2000;
        assert!(checkpoint.due());

        let off = Checkpoint {
            delta: None,
            base: 0,
            last_occupancy: u64::MAX,
            number: None,
            written_delta: None,
        };
        assert!(!off.due());
    }

    #[test]
    fn status_carries_no_checkpoint_before_any_is_written_or_resumed() {
        let resolved = bare_resolved(Vec::new());
        let checkpoint = Checkpoint {
            delta: None,
            base: 0,
            last_occupancy: 4200,
            number: None,
            written_delta: None,
        };
        let nudge = Nudge {
            threshold: 120_000,
            said: false,
            pending: None,
        };

        let status = build_status(&resolved, &checkpoint, &nudge, "claude-sonnet-5", 3);
        match status {
            TerminalMsg::Status { model, occupancy, nudge, checkpoint, turn, .. } => {
                assert_eq!(model, "claude-sonnet-5");
                assert_eq!(occupancy, 4200);
                assert_eq!(nudge, 120_000);
                assert_eq!(checkpoint, None);
                assert_eq!(turn, 3);
            }
            _ => panic!("expected Status"),
        }
    }

    #[test]
    fn status_carries_the_checkpoint_number_hydrated_from_a_resumed_session() {
        let resolved = bare_resolved(Vec::new());
        let checkpoint = Checkpoint {
            delta: None,
            base: 0,
            last_occupancy: 0,
            number: Some(3),
            written_delta: None,
        };
        let nudge = Nudge {
            threshold: 120_000,
            said: false,
            pending: None,
        };

        let status = build_status(&resolved, &checkpoint, &nudge, "claude-sonnet-5", 0);
        match status {
            TerminalMsg::Status { checkpoint, .. } => assert_eq!(checkpoint, Some((3, 0))),
            _ => panic!("expected Status"),
        }
    }

    #[test]
    fn an_unknown_native_tool_name_is_refused_at_resolve() {
        let key_env = "ARX_TEST_UNKNOWN_NATIVE_TOOL_KEY";
        unsafe {
            std::env::set_var(key_env, "test-key");
        }

        let stub_row = |role: &str| RoleRow {
            role: role.to_string(),
            source: format!("agents/{role}.md"),
            body: Body::ClaudeCode,
            model: "claude-sonnet-5".to_string(),
            provider: None,
            drive: "claude -p",
            nudge: 120_000,
            checkpoint: None,
            steps: config::DEFAULT_STEPS,
            window: None,
            reserve: None,
            write: false,
            env: Vec::new(),
            image: Some("alpine:3.20".to_string()),
            tools: Vec::new(),
            role_line: None,
        };

        let hand_row = RoleRow {
            role: "hand-bad".to_string(),
            source: "agents/hand-bad.md".to_string(),
            body: Body::Native,
            model: "fake-model".to_string(),
            provider: Some("fake".to_string()),
            drive: "native loop",
            nudge: 120_000,
            checkpoint: None,
            steps: config::DEFAULT_STEPS,
            window: None,
            reserve: None,
            write: false,
            env: Vec::new(),
            image: None,
            tools: vec!["bogus".to_string()],
            role_line: None,
        };

        let mut providers = std::collections::BTreeMap::new();
        providers.insert(
            "fake".to_string(),
            config::ProviderConfig {
                kind: crate::provider::ProviderKind::Anthropic,
                base_url: "http://127.0.0.1:0".to_string(),
                api_key_env: key_env.to_string(),
            },
        );

        let mut resolved = Resolved {
            main: stub_row("main"),
            runner: stub_row("runner"),
            reviewer: stub_row("reviewer"),
            hands: vec![hand_row],
            drain: None,
            models: Vec::new(),
            instructions: None,
            repo_instructions: None,
            sidecar: PathBuf::from("stub-sidecar"),
            checkpoint: None,
            store_scope: None,
            store_embedder: None,
            store_root: PathBuf::from("store"),
            providers,
            gate: config::GateConfig::default(),
            admission: crate::admission::AdmissionPolicy::default(),
            terminal_front: FrontKind::Line,
            terminal_keys: None,
            terminal_palette: None,
        };
        refresh_models(&mut resolved);

        let err = resolve_hand_spec(
            "hand-bad",
            &resolved,
            &Roster::default(),
            Path::new("."),
            &config::GateConfig::default(),
        )
        .unwrap_err();
        assert!(err.contains("hand-bad lists an unknown native tool bogus"), "err: {err}");

        unsafe {
            std::env::remove_var(key_env);
        }
    }

    fn model_entry_for_row(row: &RoleRow) -> ModelEntry {
        ModelEntry {
            id: row.model.clone(),
            body: row.body,
            image: row.image.clone(),
            provider: row.provider.clone(),
        }
    }

    fn refresh_models(resolved: &mut Resolved) {
        let mut seen = std::collections::HashSet::new();
        let mut models = Vec::new();
        for row in resolved.positions() {
            if seen.insert(row.model.clone()) {
                models.push(model_entry_for_row(row));
            }
        }
        resolved.models = models;
    }

    fn bare_role_row(role: &str) -> RoleRow {
        RoleRow {
            role: role.to_string(),
            source: format!("agents/{role}.md"),
            body: Body::ClaudeCode,
            model: "claude-sonnet-5".to_string(),
            provider: None,
            drive: "claude -p",
            nudge: 120_000,
            checkpoint: None,
            steps: config::DEFAULT_STEPS,
            window: None,
            reserve: None,
            write: false,
            env: Vec::new(),
            image: None,
            tools: Vec::new(),
            role_line: None,
        }
    }

    fn bare_resolved(hands: Vec<RoleRow>) -> Resolved {
        let mut resolved = Resolved {
            main: bare_role_row("main"),
            runner: {
                let mut row = bare_role_row("runner");
                row.write = true;
                row
            },
            reviewer: bare_role_row("reviewer"),
            hands,
            drain: None,
            models: Vec::new(),
            instructions: None,
            repo_instructions: None,
            sidecar: PathBuf::from("stub-sidecar"),
            checkpoint: None,
            store_scope: None,
            store_embedder: None,
            store_root: PathBuf::from("store"),
            providers: std::collections::BTreeMap::new(),
            gate: config::GateConfig::default(),
            admission: crate::admission::AdmissionPolicy::default(),
            terminal_front: FrontKind::Line,
            terminal_keys: None,
            terminal_palette: None,
        };
        refresh_models(&mut resolved);
        resolved
    }

    #[test]
    fn roster_picker_excludes_native_models_only_from_runner() {
        let mut resolved = bare_resolved(Vec::new());
        resolved.models.push(ModelEntry {
            id: "deepseek".to_string(),
            body: Body::Native,
            image: None,
            provider: Some("deepseek".to_string()),
        });

        let TerminalMsg::Roster { tabs, .. } = build_roster_message(&resolved, &Roster::default()) else {
            panic!("expected roster message");
        };
        let runner = tabs.iter().find(|tab| tab.position == "runner").unwrap();
        let reviewer = tabs.iter().find(|tab| tab.position == "reviewer").unwrap();

        assert!(!runner.model_ids.iter().any(|id| id == "deepseek"));
        assert!(reviewer.model_ids.iter().any(|id| id == "deepseek"));
    }

    #[test]
    fn static_block_is_the_instructions_file_byte_for_byte() {
        let dir = tempfile::tempdir().unwrap();
        let instructions_path = dir.path().join("prompts/main.md");
        fs::create_dir_all(instructions_path.parent().unwrap()).unwrap();
        fs::write(&instructions_path, "the standing prompt\nwith two lines\n").unwrap();

        let mut resolved = bare_resolved(Vec::new());
        resolved.instructions = Some(instructions_path);

        let mut engine = Engine::open(&dir.path().join("store"), "test").unwrap();
        let store: &mut dyn Store = &mut engine;
        let prompt = compose_system_prompt(&resolved, dir.path(), store).unwrap();
        assert_eq!(prompt.static_block, "the standing prompt\nwith two lines\n");
    }

    #[test]
    fn dynamic_block_renders_parts_in_order_and_omits_empty_ones() {
        let dir = tempfile::tempdir().unwrap();
        let store_root = dir.path().join("store");
        fs::create_dir_all(store_root.join("testscope")).unwrap();

        let mut hand_a = bare_role_row("hand-cc");
        hand_a.tools = Vec::new();
        hand_a.role_line = Some("Read-only doer for deep trace and live repro.".to_string());

        let mut hand_b = bare_role_row("hand-cw");
        hand_b.model = "gpt-5.6-sol".to_string();
        hand_b.body = Body::Codex;
        hand_b.tools = vec!["Bash".to_string(), "Read".to_string()];
        hand_b.role_line = None;

        let mut resolved = bare_resolved(vec![hand_a, hand_b]);
        resolved.store_scope = Some("testscope".to_string());

        let mut engine = Engine::open(&store_root, "test").unwrap();
        let store: &mut dyn Store = &mut engine;
        let dynamic = compose_dynamic_block(&resolved, dir.path(), store).unwrap();

        let absolute_dir = std::env::current_dir().unwrap().join(dir.path());
        let expected = format!(
            "Workspace {}. Store scope testscope.\n\nHands available:\n\
             - runner: claude-sonnet-5, claude-code, tools none\n\
             - hand-cc: claude-sonnet-5, claude-code, tools none. Read-only doer for deep trace and live repro.\n\
             - hand-cw: gpt-5.6-sol, codex, tools Bash, Read\n\
             - reviewer: claude-sonnet-5, claude-code, read-only; returns a JSON verdict under arx's schema (verdict PASS|FAIL, must_fix)",
            absolute_dir.display()
        );
        assert_eq!(dynamic, expected);
    }

    static CODEX_ENV_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serialize_on_codex_env() -> std::sync::MutexGuard<'static, ()> {
        CODEX_ENV_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn sample_codex_spec() -> HandSpec {
        HandSpec {
            agent: "hand-cx".to_string(),
            kind: BodyKind::Codex,
            role: HandRole::Hand,
            model: "gpt-5.6-sol".to_string(),
            cwd: PathBuf::from("/tmp/arx-main-codex-test"),
            threads_dir: PathBuf::from("/tmp/arx-main-codex-test/threads"),
            thread: "main".to_string(),
            provider: None,
            write: false,
            image: ledger::DEFAULT_HAND_IMAGE.to_string(),
            env: Vec::new(),
            store_root: PathBuf::from("/tmp/arx-main-codex-test/store"),
            config_dir: PathBuf::from("/tmp/arx-main-codex-test"),
            tools: Vec::new(),
            steps: config::DEFAULT_STEPS,
            window: None,
            gate: config::GateConfig::default(),
            user_gate: config::GateConfig::default(),
            store_scope: None,
            drain_input: None,
            admission: crate::admission::AdmissionPolicy::default(),
            role_line: None,
        }
    }

    #[test]
    fn codex_argv_places_the_read_only_sandbox_clamp_right_after_skip_git_repo_check() {
        let spec = sample_codex_spec();
        assert!(!spec.write);
        let argv = codex_hand::build_argv(&spec);
        let skip_pos = argv.iter().position(|arg| arg == "--skip-git-repo-check").unwrap();
        assert_eq!(argv[skip_pos + 1], "--sandbox", "argv: {argv:?}");
        assert_eq!(argv[skip_pos + 2], "read-only", "argv: {argv:?}");
    }

    #[test]
    fn codex_argv_places_the_danger_full_access_sandbox_clamp_right_after_skip_git_repo_check_for_a_write_hand() {
        let mut spec = sample_codex_spec();
        spec.write = true;
        let argv = codex_hand::build_argv(&spec);
        let skip_pos = argv.iter().position(|arg| arg == "--skip-git-repo-check").unwrap();
        assert_eq!(argv[skip_pos + 1], "--sandbox", "argv: {argv:?}");
        assert_eq!(argv[skip_pos + 2], "danger-full-access", "argv: {argv:?}");
    }

    #[test]
    fn codex_credential_source_prefers_codex_home_then_falls_back_to_home() {
        let _guard = serialize_on_codex_env();
        let previous_codex_home = std::env::var("CODEX_HOME").ok();

        unsafe {
            std::env::set_var("CODEX_HOME", "/tmp/arx-test-codex-home");
        }
        assert_eq!(container_hand::codex_credential_source(), PathBuf::from("/tmp/arx-test-codex-home/auth.json"));

        unsafe {
            std::env::remove_var("CODEX_HOME");
        }
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(container_hand::codex_credential_source(), Path::new(&home).join(".codex").join("auth.json"));

        unsafe {
            match previous_codex_home {
                Some(value) => std::env::set_var("CODEX_HOME", value),
                None => std::env::remove_var("CODEX_HOME"),
            }
        }
    }

    #[test]
    fn inject_codex_credential_validates_tokens_or_api_key_and_refuses_otherwise() {
        let _guard = serialize_on_codex_env();
        let previous_codex_home = std::env::var("CODEX_HOME").ok();

        let source_dir = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("CODEX_HOME", source_dir.path());
        }
        let write_auth = |body: &str| fs::write(source_dir.path().join("auth.json"), body).unwrap();

        write_auth(r#"{"auth_mode":"chatgpt","tokens":{"access_token":"a"}}"#);
        let dest = tempfile::tempdir().unwrap();
        assert!(container_hand::inject_codex_credential(dest.path()).is_ok());
        assert!(dest.path().join(".codex").join("auth.json").is_file());

        write_auth(r#"{"OPENAI_API_KEY":"sk-test"}"#);
        let dest = tempfile::tempdir().unwrap();
        assert!(container_hand::inject_codex_credential(dest.path()).is_ok());

        write_auth(r#"{"OPENAI_API_KEY":null}"#);
        let dest = tempfile::tempdir().unwrap();
        let err = container_hand::inject_codex_credential(dest.path()).unwrap_err();
        assert!(err.contains("no Codex credential at"), "err: {err}");

        write_auth("not json");
        let dest = tempfile::tempdir().unwrap();
        let err = container_hand::inject_codex_credential(dest.path()).unwrap_err();
        assert!(err.contains("no Codex credential at"), "err: {err}");

        unsafe {
            match previous_codex_home {
                Some(value) => std::env::set_var("CODEX_HOME", value),
                None => std::env::remove_var("CODEX_HOME"),
            }
        }
    }

    #[test]
    fn find_rollout_file_picks_the_newest_of_two_rollouts() {
        let home = tempfile::tempdir().unwrap();
        let sessions = home.path().join("sessions").join("2026").join("09").join("07");
        fs::create_dir_all(&sessions).unwrap();
        let older = sessions.join("rollout-2026-09-07T00-00-00-aaa.jsonl");
        let newer = sessions.join("rollout-2026-09-07T00-05-00-bbb.jsonl");
        fs::write(&older, "older").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&newer, "newer").unwrap();

        let picked = codex_hand::find_rollout_file(home.path()).unwrap();
        assert_eq!(picked, newer);
    }

    #[test]
    fn forwarded_env_never_carries_home_or_codex_home() {
        let _guard = serialize_on_codex_env();
        let previous_codex_home = std::env::var("CODEX_HOME").ok();

        unsafe {
            std::env::set_var("CODEX_HOME", "/tmp/arx-test-codex-home-should-not-forward");
            std::env::set_var("ARX_TEST_ENV_FORWARD_PROBE", "carried");
        }

        let names = vec![
            "HOME".to_string(),
            "CODEX_HOME".to_string(),
            "ARX_TEST_ENV_FORWARD_PROBE".to_string(),
        ];
        let forwarded = container_hand::forwarded_env(&names);

        assert!(!forwarded.iter().any(|(name, _)| name == "HOME"), "forwarded: {forwarded:?}");
        assert!(!forwarded.iter().any(|(name, _)| name == "CODEX_HOME"), "forwarded: {forwarded:?}");
        assert!(
            forwarded.iter().any(|(name, value)| name == "ARX_TEST_ENV_FORWARD_PROBE" && value == "carried"),
            "forwarded: {forwarded:?}"
        );

        unsafe {
            std::env::remove_var("ARX_TEST_ENV_FORWARD_PROBE");
            match previous_codex_home {
                Some(value) => std::env::set_var("CODEX_HOME", value),
                None => std::env::remove_var("CODEX_HOME"),
            }
        }
    }

    #[test]
    fn credential_source_dir_resolves_beneath_the_toplevel_for_masking() {
        let _guard = serialize_on_codex_env();
        let previous_codex_home = std::env::var("CODEX_HOME").ok();

        let toplevel = PathBuf::from("/tmp/arx-test-toplevel-for-mask");
        let codex_home = toplevel.join(".codex");
        unsafe {
            std::env::set_var("CODEX_HOME", codex_home.to_str().unwrap());
        }

        let resolved = container_hand::credential_source_dir(BodyKind::Codex).unwrap();
        assert_eq!(resolved, codex_home);
        let relative = resolved.strip_prefix(&toplevel).unwrap();
        assert_eq!(relative, Path::new(".codex"));

        assert!(container_hand::credential_source_dir(BodyKind::Native).is_none());
        assert!(container_hand::credential_source_dir(BodyKind::Fake).is_none());

        unsafe {
            match previous_codex_home {
                Some(value) => std::env::set_var("CODEX_HOME", value),
                None => std::env::remove_var("CODEX_HOME"),
            }
        }
    }

    #[test]
    fn resolve_absolute_joins_a_relative_path_onto_the_given_base_and_canonicalizes() {
        let base = tempfile::tempdir().unwrap();
        let nested = base.path().join("nested");
        fs::create_dir_all(&nested).unwrap();

        let resolved = container_hand::resolve_absolute(base.path(), PathBuf::from("nested"));
        assert!(resolved.is_absolute());
        assert_eq!(resolved, nested.canonicalize().unwrap());

        let already_absolute = container_hand::resolve_absolute(base.path(), nested.clone());
        assert_eq!(already_absolute, nested.canonicalize().unwrap());
    }

    #[test]
    fn reviewer_schema_constant_is_valid_json() {
        assert!(serde_json::from_str::<serde_json::Value>(reviewer::REVIEWER_SCHEMA).is_ok());
    }

    #[test]
    fn validate_verdict_accepts_a_conformant_object() {
        let text = r#"{"verdict":"FAIL","summary":"needs work","must_fix":["fix c"]}"#;
        assert!(reviewer::validate_verdict(text).is_ok());
    }

    #[test]
    fn validate_verdict_rejects_an_extra_top_level_field() {
        let text = r#"{"verdict":"PASS","summary":"s","must_fix":[],"extra":true}"#;
        let err = reviewer::validate_verdict(text).unwrap_err();
        assert_eq!(err, "unexpected field extra");
    }

    #[test]
    fn validate_verdict_rejects_a_missing_field() {
        let text = r#"{"verdict":"PASS","must_fix":[]}"#;
        let err = reviewer::validate_verdict(text).unwrap_err();
        assert_eq!(err, "missing field summary");
    }

    #[test]
    fn validate_verdict_rejects_a_bad_verdict_value() {
        let text = r#"{"verdict":"maybe","summary":"s","must_fix":[]}"#;
        let err = reviewer::validate_verdict(text).unwrap_err();
        assert_eq!(err, "verdict not one of PASS|FAIL");
    }

    #[test]
    fn validate_verdict_rejects_a_non_string_must_fix() {
        let text = r#"{"verdict":"FAIL","summary":"s","must_fix":[1]}"#;
        let err = reviewer::validate_verdict(text).unwrap_err();
        assert_eq!(err, "must_fix[0] not a string");
    }

    #[test]
    fn validate_verdict_rejects_pass_with_must_fix() {
        let text = r#"{"verdict":"PASS","summary":"s","must_fix":["still broken"]}"#;
        let err = reviewer::validate_verdict(text).unwrap_err();
        assert_eq!(err, "PASS verdict has non-empty must_fix");
    }

    #[test]
    fn validate_verdict_rejects_fail_without_must_fix() {
        let text = r#"{"verdict":"FAIL","summary":"broken","must_fix":[]}"#;
        let err = reviewer::validate_verdict(text).unwrap_err();
        assert_eq!(err, "FAIL verdict has empty must_fix");
    }

    #[test]
    fn validate_verdict_rejects_non_json() {
        let err = reviewer::validate_verdict("not json at all").unwrap_err();
        assert!(err.starts_with("not JSON: "), "err: {err}");
    }

    #[test]
    fn validate_verdict_rejects_a_non_object() {
        let err = reviewer::validate_verdict("[1,2,3]").unwrap_err();
        assert_eq!(err, "not an object");
    }

    #[test]
    fn family_of_claude_code_is_claude() {
        let row = bare_role_row("reviewer");
        let providers = std::collections::BTreeMap::new();
        assert_eq!(reviewer::family(&row, &providers), "claude");
    }

    #[test]
    fn family_of_codex_is_gpt() {
        let mut row = bare_role_row("reviewer");
        row.body = Body::Codex;
        let providers = std::collections::BTreeMap::new();
        assert_eq!(reviewer::family(&row, &providers), "gpt");
    }

    #[test]
    fn family_of_native_anthropic_is_claude() {
        let mut row = bare_role_row("reviewer");
        row.body = Body::Native;
        row.provider = Some("anthro".to_string());
        let mut providers = std::collections::BTreeMap::new();
        providers.insert(
            "anthro".to_string(),
            config::ProviderConfig {
                kind: crate::provider::ProviderKind::Anthropic,
                base_url: "http://127.0.0.1:0".to_string(),
                api_key_env: "ARX_TEST_FAMILY_KEY".to_string(),
            },
        );
        assert_eq!(reviewer::family(&row, &providers), "claude");
    }

    #[test]
    fn family_of_native_openai_compatible_is_the_provider_key() {
        let mut row = bare_role_row("reviewer");
        row.body = Body::Native;
        row.provider = Some("deepseek".to_string());
        let mut providers = std::collections::BTreeMap::new();
        providers.insert(
            "deepseek".to_string(),
            config::ProviderConfig {
                kind: crate::provider::ProviderKind::OpenAiCompatible,
                base_url: "http://127.0.0.1:0".to_string(),
                api_key_env: "ARX_TEST_FAMILY_KEY2".to_string(),
            },
        );
        assert_eq!(reviewer::family(&row, &providers), "deepseek");
    }

    #[test]
    fn render_hand_roster_output_ends_with_the_reviewer_line() {
        let mut runner = bare_role_row("runner");
        runner.write = true;
        let hand = bare_role_row("hand-cc");
        let reviewer_row = bare_role_row("reviewer");
        let section = render_hand_roster(&runner, &reviewer_row, &[hand]).unwrap();
        assert!(section.contains("- runner: claude-sonnet-5, claude-code, tools none"));
        assert!(
            section.ends_with(
                "- reviewer: claude-sonnet-5, claude-code, read-only; returns a JSON verdict under arx's schema (verdict PASS|FAIL, must_fix)"
            ),
            "section: {section}"
        );
    }

    #[test]
    fn a_reviewer_definition_with_write_true_still_resolves_read_only() {
        let mut reviewer_row = bare_role_row("reviewer");
        reviewer_row.model = "reviewer-model".to_string();
        reviewer_row.write = true;
        reviewer_row.image = Some("alpine:3.20".to_string());
        let mut resolved = bare_resolved(Vec::new());
        resolved.reviewer = reviewer_row;
        refresh_models(&mut resolved);

        let spec = resolve_hand_spec("reviewer", &resolved, &Roster::default(), Path::new("."), &config::GateConfig::default()).unwrap();
        assert!(!spec.write, "a reviewer must never resolve to a write-enabled hand spec");
    }

    #[test]
    fn runner_resolves_with_its_write_tools() {
        let mut runner = bare_role_row("runner");
        runner.model = "runner-model".into();
        runner.write = true;
        runner.tools = vec!["Read".into(), "Bash".into()];
        runner.image = Some("arx-runner:test".into());
        let mut resolved = bare_resolved(Vec::new());
        resolved.runner = runner;
        refresh_models(&mut resolved);

        let spec = resolve_hand_spec("runner", &resolved, &Roster::default(), Path::new("."), &config::GateConfig::default()).unwrap();
        assert_eq!(spec.role, HandRole::Runner);
        assert!(spec.write);
        assert_eq!(spec.tools, vec!["Read", "Bash"]);
    }

    #[test]
    fn a_hand_definition_with_write_true_still_resolves_write_enabled() {
        let hand_row = {
            let mut row = bare_role_row("hand-cw");
            row.model = "hand-cw-model".to_string();
            row.write = true;
            row.image = Some("alpine:3.20".to_string());
            row
        };
        let resolved = bare_resolved(vec![hand_row]);

        let spec =
            resolve_hand_spec("hand-cw", &resolved, &Roster::default(), Path::new("."), &config::GateConfig::default()).unwrap();
        assert!(spec.write, "a non-reviewer hand's write: true must still resolve write-enabled");
    }

    fn snapshot_ids() -> Vec<String> {
        vec!["cand-a".to_string(), "cand-b".to_string()]
    }

    #[test]
    fn validate_proposal_accepts_a_covering_list() {
        let text = serde_json::json!({
            "suggestions": [
                {"id": "cand-a", "action": "promote", "supersedes": null, "reason": "looks right"},
                {"id": "cand-b", "action": "drop", "supersedes": null, "reason": "duplicate"}
            ]
        })
        .to_string();
        let suggestions = drain::validate_proposal(&text, &snapshot_ids()).unwrap();
        assert_eq!(suggestions.len(), 2);
    }

    #[test]
    fn validate_proposal_refuses_a_missing_id() {
        let text = serde_json::json!({
            "suggestions": [
                {"id": "cand-a", "action": "promote", "supersedes": null, "reason": "looks right"}
            ]
        })
        .to_string();
        let err = drain::validate_proposal(&text, &snapshot_ids()).unwrap_err();
        assert!(err.contains("cand-b"), "err: {err}");
    }

    #[test]
    fn validate_proposal_refuses_an_extra_id() {
        let text = serde_json::json!({
            "suggestions": [
                {"id": "cand-a", "action": "promote", "supersedes": null, "reason": "looks right"},
                {"id": "cand-b", "action": "promote", "supersedes": null, "reason": "looks right too"},
                {"id": "cand-c", "action": "promote", "supersedes": null, "reason": "not in the snapshot"}
            ]
        })
        .to_string();
        let err = drain::validate_proposal(&text, &snapshot_ids()).unwrap_err();
        assert!(err.contains("cand-c"), "err: {err}");
    }

    #[test]
    fn validate_proposal_refuses_a_drop_with_supersedes() {
        let text = serde_json::json!({
            "suggestions": [
                {"id": "cand-a", "action": "drop", "supersedes": "cand-x", "reason": "bad shape"},
                {"id": "cand-b", "action": "promote", "supersedes": null, "reason": "fine"}
            ]
        })
        .to_string();
        let err = drain::validate_proposal(&text, &snapshot_ids()).unwrap_err();
        assert!(err.contains("supersedes"), "err: {err}");
    }

    #[test]
    fn validate_proposal_refuses_non_json_naming_the_fault() {
        let err = drain::validate_proposal("not json at all", &snapshot_ids()).unwrap_err();
        assert!(err.contains("not JSON matching the drain schema"), "err: {err}");
    }

    #[test]
    fn validate_proposal_refuses_a_reply_that_omits_supersedes() {
        let text = serde_json::json!({
            "suggestions": [
                {"id": "cand-a", "action": "promote", "reason": "looks right"},
                {"id": "cand-b", "action": "promote", "supersedes": null, "reason": "fine"}
            ]
        })
        .to_string();
        let err = drain::validate_proposal(&text, &snapshot_ids()).unwrap_err();
        assert!(err.contains("not JSON matching the drain schema"), "err: {err}");
    }

    #[test]
    fn render_proposal_prints_the_three_line_shapes() {
        let suggestions = vec![
            Suggestion {
                id: "cand-a".to_string(),
                action: SuggestionAction::Promote,
                supersedes: None,
                reason: "looks right".to_string(),
            },
            Suggestion {
                id: "cand-b".to_string(),
                action: SuggestionAction::Promote,
                supersedes: Some("cand-old".to_string()),
                reason: "newer answer".to_string(),
            },
            Suggestion {
                id: "cand-c".to_string(),
                action: SuggestionAction::Drop,
                supersedes: None,
                reason: "README already says it".to_string(),
            },
        ];
        let rendered = drain::render_proposal("h6", "threads/main.h6", &suggestions);
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(lines[0], "[hand h6 drain proposal] 3 candidates (trace: threads/main.h6)");
        assert_eq!(lines[1], "- cand-a promote — looks right");
        assert_eq!(lines[2], "- cand-b promote superseding cand-old — newer answer");
        assert_eq!(lines[3], "- cand-c drop — README already says it");
    }

    #[test]
    fn context_window_messages_are_classified_and_others_are_not() {
        assert!(main_body::is_context_window_message("prompt is too long: 1000123 tokens > 1000000 maximum"));
        assert!(main_body::is_context_window_message("Tokens > limit"));
        assert!(main_body::is_context_window_message("the context window is full"));
        assert!(main_body::is_context_window_message("context_length_exceeded"));
        assert!(main_body::is_context_window_message("too many tokens for this request"));
        assert!(!main_body::is_context_window_message("rate limited"));
        assert!(!main_body::is_context_window_message("network error"));
        assert!(!main_body::is_context_window_message(""));
    }

    fn write_store_root_fixture(dir: &Path, store_toml: &str) {
        fs::create_dir_all(dir.join("agents")).unwrap();
        fs::write(dir.join("agents/runner.md"), "---\ntools: [Read, Bash]\nwrite: true\n---\nrunner\n").unwrap();
        fs::write(dir.join("agents/reviewer.md"), "---\n{}\n---\nreviewer\n").unwrap();
        fs::write(dir.join("agents/hand-ex.md"), "---\n{}\n---\nhand\n").unwrap();
        fs::write(
            dir.join("arx.toml"),
            format!(
                "sidecar = \".\"\n\n[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"stub-model\"\nbody = \"claude-code\"\n\n[[models]]\nid = \"gpt-5.4-codex\"\nbody = \"codex\"\n\n[roles]\nmain = \"stub-model\"\nrunner = \"gpt-5.4-codex\"\nreviewer = \"gpt-5.4-codex\"\nhand-ex = \"stub-model\"\n\n{store_toml}"
            ),
        )
        .unwrap();
    }

    #[test]
    fn store_root_defaults_to_store_under_the_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        write_store_root_fixture(dir.path(), "[store]\nscope = \"arx\"\n");

        let resolved = config::load(&dir.path().join("arx.toml")).unwrap();
        assert!(resolved.store_root.is_absolute());
        assert_eq!(resolved.store_root, dir.path().join("store"));
    }

    #[test]
    fn store_root_expands_tilde_and_takes_absolute_and_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        write_store_root_fixture(dir.path(), "[store]\nscope = \"arx\"\nroot = \"~/x/store\"\n");
        let resolved = config::load(&dir.path().join("arx.toml")).unwrap();
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        assert_eq!(resolved.store_root, home.join("x/store"));

        write_store_root_fixture(dir.path(), "[store]\nscope = \"arx\"\nroot = \"/abs/store\"\n");
        let resolved = config::load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.store_root, PathBuf::from("/abs/store"));

        write_store_root_fixture(dir.path(), "[store]\nscope = \"arx\"\nroot = \"elsewhere/store\"\n");
        let resolved = config::load(&dir.path().join("arx.toml")).unwrap();
        assert!(resolved.store_root.is_absolute());
        assert_eq!(resolved.store_root, dir.path().join("elsewhere/store"));
    }

    #[test]
    fn empty_store_root_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_store_root_fixture(dir.path(), "[store]\nscope = \"arx\"\nroot = \"  \"\n");
        let err = config::load(&dir.path().join("arx.toml")).unwrap_err();
        assert_eq!(err.to_string(), "store.root must not be empty");
    }

    #[test]
    fn roster_round_trips_pin_and_clear_through_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roster.toml");

        let mut roster = Roster::default();
        assert_eq!(roster.get("hand-ex"), None);
        roster.pin("hand-ex", "gpt-5.6-sol");
        roster.save(&path).unwrap();

        let reloaded = Roster::load(&path).unwrap();
        assert_eq!(reloaded.get("hand-ex"), Some("gpt-5.6-sol"));
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("[pins]"), "roster file: {contents}");
        assert!(contents.contains("hand-ex"), "roster file: {contents}");

        let mut mutated = reloaded;
        assert!(mutated.clear("hand-ex"));
        assert!(!mutated.clear("hand-ex"), "a second clear on an absent pin reports nothing to clear");
        mutated.save(&path).unwrap();
        let cleared = Roster::load(&path).unwrap();
        assert_eq!(cleared.get("hand-ex"), None);
    }

    #[test]
    fn retired_judge_pin_migrates_without_overwriting_a_reviewer_pin() {
        let mut roster = Roster::default();
        roster.pin("judge", "claude-sonnet-5");
        assert!(roster.migrate("judge", "reviewer"));
        assert_eq!(roster.get("judge"), None);
        assert_eq!(roster.get("reviewer"), Some("claude-sonnet-5"));

        roster.pin("judge", "old-model");
        roster.pin("reviewer", "new-model");
        assert!(roster.migrate("judge", "reviewer"));
        assert_eq!(roster.get("judge"), None);
        assert_eq!(roster.get("reviewer"), Some("new-model"));
    }

    #[test]
    fn roster_save_leaves_no_temp_file_and_survives_being_called_twice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roster.toml");

        let mut roster = Roster::default();
        roster.pin("main", "claude-opus-5");
        roster.save(&path).unwrap();
        roster.pin("main", "claude-sonnet-5");
        roster.save(&path).unwrap();

        let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().filter_map(|e| e.ok()).collect();
        assert_eq!(entries.len(), 1, "no sibling temp file must survive a save: {entries:?}");
        assert_eq!(entries[0].path(), path);

        let reloaded = Roster::load(&path).unwrap();
        assert_eq!(reloaded.get("main"), Some("claude-sonnet-5"));
    }

    #[test]
    fn save_roster_or_notice_rolls_back_and_notifies_on_a_failed_save() {
        let dir = tempfile::tempdir().unwrap();
        let blocking_file = dir.path().join("blocked");
        fs::write(&blocking_file, "x").unwrap();
        let roster_path = blocking_file.join("roster.toml");

        let mut roster = Roster::default();
        roster.pin("main", "claude-opus-5");
        let mut front = RecordingFront::default();

        let persisted = save_roster_or_notice(&mut roster, &roster_path, "main", None, &mut front);
        assert!(!persisted);
        assert_eq!(roster.get("main"), None, "a failed save must roll the pin back to what it was before");
        let notice = front.messages.iter().find_map(|msg| match msg {
            TerminalMsg::Notice { text } => Some(text.clone()),
            _ => None,
        });
        let notice = notice.expect("a failed save must render a notice");
        assert!(notice.contains("main"), "notice: {notice}");
        assert!(notice.contains("did not take"), "notice: {notice}");
    }

    #[test]
    fn save_roster_or_notice_restores_the_previous_pin_not_just_a_clear_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let blocking_file = dir.path().join("blocked");
        fs::write(&blocking_file, "x").unwrap();
        let roster_path = blocking_file.join("roster.toml");

        let mut roster = Roster::default();
        roster.pin("main", "claude-sonnet-5");
        let mut front = RecordingFront::default();

        let persisted = save_roster_or_notice(&mut roster, &roster_path, "main", Some("claude-sonnet-5"), &mut front);
        assert!(!persisted);
        assert_eq!(roster.get("main"), Some("claude-sonnet-5"), "rollback must restore the exact previous pin");
    }

    #[test]
    fn roster_load_of_a_missing_file_is_an_empty_roster_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.toml");
        let roster = Roster::load(&path).unwrap();
        assert_eq!(roster.get("main"), None);
    }

    #[test]
    fn roster_load_of_a_malformed_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roster.toml");
        fs::write(&path, "not = [valid\n").unwrap();
        assert!(Roster::load(&path).is_err());
    }

    static ARX_ROSTER_DIR_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serialize_on_roster_dir_env() -> std::sync::MutexGuard<'static, ()> {
        ARX_ROSTER_DIR_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn roster_path_honours_the_arx_roster_dir_override() {
        let _guard = serialize_on_roster_dir_env();
        let dir = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("ARX_ROSTER_DIR", dir.path());
        }
        let config_path = dir.path().join("project").join("arx.toml");
        let path = roster::roster_path(&config_path);
        unsafe {
            std::env::remove_var("ARX_ROSTER_DIR");
        }
        assert_eq!(path.parent(), Some(dir.path()));
        let name = path.file_name().and_then(|n| n.to_str()).unwrap();
        assert!(name.starts_with("project-"), "name: {name}");
        let hex = name.strip_prefix("project-").unwrap().strip_suffix(".toml").unwrap();
        assert_eq!(hex.len(), 16, "hex: {hex}");
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn effective_rows_for_notices_fires_once_a_pin_moves_a_position_onto_a_body_that_ignores_its_tools() {
        let mut hand_row = bare_role_row("hand-ex");
        hand_row.model = "claude-sonnet-5".to_string();
        hand_row.tools = vec!["Bash".to_string()];
        let mut resolved = bare_resolved(vec![hand_row]);
        resolved.models.push(ModelEntry {
            id: "gpt-5.6-sol".to_string(),
            body: Body::Codex,
            image: None,
            provider: None,
        });

        let unpinned = Roster::default();
        assert!(config::inert_tools_notices(effective_rows_for_notices(&resolved, &unpinned).iter()).is_empty());

        let mut pinned = Roster::default();
        pinned.pin("hand-ex", "gpt-5.6-sol");
        let notices = config::inert_tools_notices(effective_rows_for_notices(&resolved, &pinned).iter());
        assert_eq!(
            notices,
            vec!["[arx] tools: hand-ex runs gpt-5.6-sol on the codex body, which negates shell unless Bash is listed, and always negates image and web since arx names no tool for them — the rest of codex's built-ins stay on".to_string()]
        );
    }

    #[test]
    fn effective_rows_for_notices_falls_silent_once_a_pin_moves_a_position_off_a_mismatching_default() {
        let mut hand_row = bare_role_row("hand-do");
        hand_row.body = Body::Codex;
        hand_row.model = "gpt-5.6-sol".to_string();
        hand_row.tools = vec!["Bash".to_string()];
        let mut resolved = bare_resolved(vec![hand_row]);
        resolved.models.push(ModelEntry {
            id: "claude-sonnet-5".to_string(),
            body: Body::ClaudeCode,
            image: None,
            provider: None,
        });

        let unpinned = Roster::default();
        assert_eq!(
            config::inert_tools_notices(effective_rows_for_notices(&resolved, &unpinned).iter()),
            vec!["[arx] tools: hand-do runs gpt-5.6-sol on the codex body, which negates shell unless Bash is listed, and always negates image and web since arx names no tool for them — the rest of codex's built-ins stay on".to_string()]
        );

        let mut pinned = Roster::default();
        pinned.pin("hand-do", "claude-sonnet-5");
        assert!(config::inert_tools_notices(effective_rows_for_notices(&resolved, &pinned).iter()).is_empty());
    }

    #[test]
    fn roster_path_is_stable_for_the_same_config_path() {
        let _guard = serialize_on_roster_dir_env();
        let dir = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("ARX_ROSTER_DIR", dir.path());
        }
        let config_path = dir.path().join("a").join("arx.toml");
        let first = roster::roster_path(&config_path);
        let second = roster::roster_path(&config_path);
        unsafe {
            std::env::remove_var("ARX_ROSTER_DIR");
        }
        assert_eq!(first, second);
    }

    #[test]
    fn effective_binding_prefers_the_pin_over_the_roles_default() {
        let mut hand_row = bare_role_row("hand-ex");
        hand_row.model = "claude-sonnet-5".to_string();
        let resolved = bare_resolved(vec![hand_row]);

        let mut roster = Roster::default();
        roster.pin("hand-ex", "claude-sonnet-5");
        let (id, entry, source) = effective_binding("hand-ex", &resolved, &roster);
        assert_eq!(id, "claude-sonnet-5");
        assert_eq!(entry.id, "claude-sonnet-5");
        assert_eq!(source, ModelSource::Pinned);
    }

    #[test]
    fn effective_binding_falls_back_to_the_roles_default_when_the_pin_is_dangling() {
        let mut hand_row = bare_role_row("hand-ex");
        hand_row.model = "claude-sonnet-5".to_string();
        let resolved = bare_resolved(vec![hand_row]);

        let mut roster = Roster::default();
        roster.pin("hand-ex", "no-such-model");
        let (id, entry, source) = effective_binding("hand-ex", &resolved, &roster);
        assert_eq!(id, "claude-sonnet-5");
        assert_eq!(entry.id, "claude-sonnet-5");
        assert_eq!(source, ModelSource::FromFile);
    }

    #[test]
    fn effective_binding_is_from_file_with_no_pin_at_all() {
        let mut hand_row = bare_role_row("hand-ex");
        hand_row.model = "claude-sonnet-5".to_string();
        let resolved = bare_resolved(vec![hand_row]);

        let (id, _, source) = effective_binding("hand-ex", &resolved, &Roster::default());
        assert_eq!(id, "claude-sonnet-5");
        assert_eq!(source, ModelSource::FromFile);
    }

    #[test]
    fn dangling_pin_notices_names_the_position_the_dangling_id_and_the_fallback() {
        let mut hand_row = bare_role_row("hand-ex");
        hand_row.model = "claude-sonnet-5".to_string();
        let resolved = bare_resolved(vec![hand_row]);

        let mut roster = Roster::default();
        roster.pin("hand-ex", "no-such-model");
        let notices = dangling_pin_notices(&roster, &resolved);
        assert_eq!(notices.len(), 1, "notices: {notices:?}");
        assert_eq!(
            notices[0],
            "[arx] roster: hand-ex pinned to no-such-model, which is not in [[models]]; using claude-sonnet-5 from arx.toml"
        );
    }

    #[test]
    fn dangling_pin_notices_ignores_a_pin_naming_an_unknown_position_and_a_healthy_pin() {
        let mut hand_row = bare_role_row("hand-ex");
        hand_row.model = "claude-sonnet-5".to_string();
        let resolved = bare_resolved(vec![hand_row]);

        let mut roster = Roster::default();
        roster.pin("hand-ghost", "no-such-model");
        roster.pin("hand-ex", "claude-sonnet-5");
        assert!(dangling_pin_notices(&roster, &resolved).is_empty());
    }

    #[test]
    fn a_model_event_committed_before_position_existed_still_deserializes_as_main() {
        let legacy = r#"{"kind":"model","from":"claude-sonnet-5","to":"claude-opus-5"}"#;
        let event: Event = serde_json::from_str(legacy).unwrap();
        match event {
            Event::Model { position, from, to } => {
                assert_eq!(position, "main");
                assert_eq!(from, "claude-sonnet-5");
                assert_eq!(to, "claude-opus-5");
            }
            other => panic!("expected Event::Model, got {other:?}"),
        }
    }

    #[test]
    fn main_current_model_accepts_a_codex_pin() {
        let resolved = bare_resolved(Vec::new());
        let mut models = resolved.models.clone();
        models.push(ModelEntry {
            id: "gpt-5.6-sol".to_string(),
            body: Body::Codex,
            image: Some("arx-hand-codex:0.152.1".to_string()),
            provider: None,
        });
        let mut resolved = resolved;
        resolved.models = models;

        let mut roster = Roster::default();
        roster.pin("main", "gpt-5.6-sol");

        let (id, notice) = main_current_model(&resolved, &roster);
        assert_eq!(id, "gpt-5.6-sol");
        assert!(notice.is_none());
    }

    #[test]
    fn main_current_model_is_silent_and_unchanged_with_no_pin() {
        let resolved = bare_resolved(Vec::new());
        let (id, notice) = main_current_model(&resolved, &Roster::default());
        assert_eq!(id, "claude-sonnet-5");
        assert!(notice.is_none());
    }
}
