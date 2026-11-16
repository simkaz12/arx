use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::admission::AdmissionPolicy;
use crate::provider::ProviderKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Body {
    Native,
    ClaudeCode,
    Codex,
}

impl Body {
    fn wire(&self) -> &'static str {
        match self {
            Body::Native => "native",
            Body::ClaudeCode => "claude-code",
            Body::Codex => "codex",
        }
    }
}

impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrontKind {
    #[default]
    Line,
    Panes,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalConfig {
    #[serde(default)]
    pub front: FrontKind,
    pub keys: Option<PathBuf>,
    pub palette: Option<PaletteConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteConfig {
    pub operator: Option<String>,
    pub rule: Option<String>,
    pub gutter: Option<String>,
    pub tool: Option<String>,
}

/// A registry entry — one model id, the body that runs it, and that body's own
/// extras. `[[models]]` in `arx.toml` is the sole availables list for every
/// position; a position names an id here and the body follows.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub id: String,
    pub body: Body,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleConfig {
    pub instructions: Option<PathBuf>,
    pub repo_instructions: Option<PathBuf>,
    pub nudge: Option<u64>,
    pub checkpoint: Option<u64>,
    pub steps: Option<u32>,
    pub window: Option<u64>,
    pub reserve: Option<u64>,
    #[serde(default)]
    pub write: bool,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MainConfig {
    body: Body,
    instructions: Option<PathBuf>,
    repo_instructions: Option<PathBuf>,
    nudge: Option<u64>,
    checkpoint: Option<u64>,
    steps: Option<u32>,
    window: Option<u64>,
    reserve: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreConfig {
    scope: String,
    #[serde(default)]
    embedder: Option<String>,
    #[serde(default)]
    root: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key_env: String,
}

pub use crate::gate::GateConfig;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartupConfig {
    agents: Option<PathBuf>,
    sidecar: Option<PathBuf>,
    main: MainConfig,
    #[serde(default)]
    models: Vec<ModelEntry>,
    #[serde(default)]
    roles: BTreeMap<String, String>,
    store: Option<StoreConfig>,
    providers: Option<BTreeMap<String, ProviderConfig>>,
    #[serde(default)]
    gate: GateConfig,
    admission: Option<AdmissionConfig>,
    #[serde(default)]
    terminal: TerminalConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmissionConfig {
    threshold_tokens: Option<u64>,
    overrides: Option<BTreeMap<String, u64>>,
}

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    TomlParse {
        path: PathBuf,
        source: toml::de::Error,
    },
    DriveSurfaceConfigured {
        key: String,
        table: String,
    },
    EmptyModelRegistry,
    EmptyModelId,
    NonCanonicalModelId {
        id: String,
    },
    DuplicateModelId {
        id: String,
    },
    ProviderOnNonNative {
        id: String,
        body: Body,
    },
    NativeMissingProvider {
        id: String,
    },
    UnknownProvider {
        id: String,
        provider: String,
    },
    EmptyProviderField {
        provider: String,
        field: &'static str,
    },
    UnknownRoleKey {
        key: String,
    },
    MissingRoleEntry {
        position: String,
    },
    UnknownModel {
        role: String,
        id: String,
        known: Vec<String>,
    },
    MainRoleBodyMismatch {
        id: String,
        main_body: Body,
        entry_body: Body,
    },
    FrontmatterFieldMoved {
        path: PathBuf,
        key: String,
    },
    NonPositiveNudge,
    NonPositiveCheckpoint,
    NonPositiveSteps {
        role: String,
    },
    NonPositiveWindow {
        role: String,
    },
    NonPositiveReserve,
    ReserveNotBelowWindow,
    MainWindowTooHigh,
    EmptyStoreEmbedder,
    EmptyStoreRoot,
    StoreRootHome,
    AgentsDirUnreadable {
        path: PathBuf,
    },
    NoRunnerDefinition,
    NoReviewerDefinition,
    RunnerBodyUnsupported {
        id: String,
        body: Body,
    },
    NoHandDefinition,
    MainOverride,
    FrontmatterMissing {
        path: PathBuf,
    },
    FrontmatterParse {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    GateRule {
        message: String,
    },
    ZeroAdmissionThreshold,
    InvalidPalette {
        message: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            ConfigError::TomlParse { path, source } => write!(f, "{}: {source}", path.display()),
            ConfigError::DriveSurfaceConfigured { key, table } => write!(
                f,
                "'{key}' in [{table}]: the drive surface is never configured — it follows from the role (0bd4df2f)"
            ),
            ConfigError::EmptyModelRegistry => write!(f, "[[models]]: at least one entry is required"),
            ConfigError::EmptyModelId => write!(f, "[[models]]: entry id must not be empty"),
            ConfigError::NonCanonicalModelId { id } => {
                write!(f, "[[models]]: id '{id}' has leading or trailing whitespace")
            }
            ConfigError::DuplicateModelId { id } => write!(f, "[[models]]: duplicate id '{id}'"),
            ConfigError::ProviderOnNonNative { id, body } => write!(
                f,
                "model '{id}': provider is set but body is '{body}' — a body is not a provider; provider applies only to body = native (975f65cd)"
            ),
            ConfigError::NativeMissingProvider { id } => write!(f, "model '{id}': body 'native' requires a provider"),
            ConfigError::UnknownProvider { id, provider } => write!(
                f,
                "model '{id}' names provider '{provider}' which has no [providers.{provider}] table"
            ),
            ConfigError::EmptyProviderField { provider, field } => {
                write!(f, "[providers.{provider}]: {field} is empty")
            }
            ConfigError::UnknownRoleKey { key } => {
                write!(f, "[roles].{key} names no agents/{key}.md — remove the row or add the file")
            }
            ConfigError::MissingRoleEntry { position } => {
                write!(f, "[roles] has no entry for '{position}'")
            }
            ConfigError::UnknownModel { role, id, known } => write!(
                f,
                "[roles].{role} names model id '{id}' which is not in [[models]]; known ids: {}",
                known.join(", ")
            ),
            ConfigError::MainRoleBodyMismatch { id, main_body, entry_body } => write!(
                f,
                "[roles].main names '{id}' whose body '{entry_body}' does not match [main].body '{main_body}'"
            ),
            ConfigError::FrontmatterFieldMoved { path, key } => write!(
                f,
                "{}: '{key}' belongs to [[models]] and [roles] in arx.toml now, not agent frontmatter",
                path.display()
            ),
            ConfigError::NonPositiveNudge => write!(f, "main.nudge must be positive"),
            ConfigError::NonPositiveCheckpoint => write!(f, "main.checkpoint must be positive"),
            ConfigError::NonPositiveSteps { role } => write!(f, "{role}.steps must be positive"),
            ConfigError::NonPositiveWindow { role } => write!(f, "{role}.window must be positive"),
            ConfigError::NonPositiveReserve => write!(f, "main: reserve must be at least 1 token"),
            ConfigError::ReserveNotBelowWindow => write!(f, "main: reserve must be below window"),
            ConfigError::MainWindowTooHigh => write!(
                f,
                "main: window must stay below 900000; the body compacts near 967k of the 1M compaction window arx sets"
            ),
            ConfigError::EmptyStoreEmbedder => write!(f, "store.embedder must not be empty"),
            ConfigError::EmptyStoreRoot => write!(f, "store.root must not be empty"),
            ConfigError::StoreRootHome => write!(f, "store.root: cannot expand ~, HOME is not set"),
            ConfigError::AgentsDirUnreadable { path } => {
                write!(f, "agents dir not found: {}", path.display())
            }
            ConfigError::NoRunnerDefinition => write!(
                f,
                "agents dir names no 'runner' definition — the runner role is unfilled"
            ),
            ConfigError::NoReviewerDefinition => write!(
                f,
                "agents dir names no 'reviewer' definition — the reviewer role is unfilled (0bd4df2f)"
            ),
            ConfigError::RunnerBodyUnsupported { id, body } => write!(
                f,
                "[roles].runner names '{id}' with body '{body}'; runner requires claude-code or codex so it can implement the brief"
            ),
            ConfigError::NoHandDefinition => write!(
                f,
                "agents dir names no hand definition"
            ),
            ConfigError::MainOverride => write!(
                f,
                "agents/main.md: main's block is startup config, fixed per run — no per-dispatch body override (0bd4df2f)"
            ),
            ConfigError::FrontmatterMissing { path } => {
                write!(f, "{}: no frontmatter", path.display())
            }
            ConfigError::FrontmatterParse { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
            ConfigError::GateRule { message } => write!(f, "{message}"),
            ConfigError::ZeroAdmissionThreshold => write!(f, "admission: threshold must be at least 1 token"),
            ConfigError::InvalidPalette { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoleKind {
    Main,
    Dispatch,
}

fn drive_surface(kind: RoleKind, body: Body) -> &'static str {
    match (kind, body) {
        (RoleKind::Main, Body::ClaudeCode) => "agent-sdk",
        (RoleKind::Main, Body::Codex) => "app-server",
        (RoleKind::Main, Body::Native) => "native loop",
        (RoleKind::Dispatch, Body::ClaudeCode) => "claude -p",
        (RoleKind::Dispatch, Body::Codex) => "codex exec",
        (RoleKind::Dispatch, Body::Native) => "native loop",
    }
}

#[derive(Debug, Clone)]
pub struct RoleRow {
    pub role: String,
    pub source: String,
    pub body: Body,
    pub model: String,
    pub provider: Option<String>,
    pub drive: &'static str,
    pub nudge: u64,
    pub checkpoint: Option<u64>,
    pub steps: u32,
    pub window: Option<u64>,
    pub reserve: Option<u64>,
    pub write: bool,
    pub env: Vec<String>,
    pub image: Option<String>,
    pub tools: Vec<String>,
    pub role_line: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Resolved {
    pub main: RoleRow,
    pub runner: RoleRow,
    pub reviewer: RoleRow,
    pub hands: Vec<RoleRow>,
    pub drain: Option<RoleRow>,
    pub models: Vec<ModelEntry>,
    pub instructions: Option<PathBuf>,
    pub repo_instructions: Option<PathBuf>,
    pub sidecar: PathBuf,
    pub checkpoint: Option<u64>,
    pub store_scope: Option<String>,
    pub store_embedder: Option<String>,
    pub store_root: PathBuf,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub gate: GateConfig,
    pub admission: AdmissionPolicy,
    pub terminal_front: FrontKind,
    pub terminal_keys: Option<PathBuf>,
    pub terminal_palette: Option<PaletteConfig>,
}

impl Resolved {
    /// Every position in the roster, `main` first, then `runner`, `reviewer`, then the hands
    /// in the order `[[models]]`/`[roles]` resolved them (already alphabetical),
    /// then `drain` if configured. This is the canonical enumeration for the
    /// `/model` roster line and the tabbed modal — a fixed, four-row order for
    /// the shipped `arx.toml`.
    pub fn positions(&self) -> Vec<&RoleRow> {
        let mut rows = vec![&self.main, &self.runner, &self.reviewer];
        rows.extend(self.hands.iter());
        rows.extend(self.drain.iter());
        rows
    }

    pub fn position(&self, name: &str) -> Option<&RoleRow> {
        self.positions().into_iter().find(|row| row.role == name)
    }

    pub fn model_entry(&self, id: &str) -> Option<&ModelEntry> {
        self.models.iter().find(|entry| entry.id == id)
    }
}

pub fn inert_tools_notices<'a>(rows: impl IntoIterator<Item = &'a RoleRow>) -> Vec<String> {
    let mut notices = Vec::new();
    for row in rows {
        match row.body {
            Body::Codex => notices.push(format!(
                "[arx] tools: {} runs {} on the {} body, which negates shell unless Bash is listed, and always negates image and web since arx names no tool for them — the rest of codex's built-ins stay on",
                row.role, row.model, row.body
            )),
            Body::Native => {
                if row.tools.is_empty() {
                    continue;
                }
                if let Some(tool) = row.tools.iter().find(|tool| tool.as_str() != "read_file" && tool.as_str() != "recall") {
                    notices.push(format!("[arx] tools: {} lists {tool}, which the native body does not have", row.role));
                }
            }
            Body::ClaudeCode => {}
        }
    }
    notices
}

const DEFAULT_NUDGE: u64 = 120_000;
pub(crate) const DEFAULT_STEPS: u32 = 16;
const DEFAULT_MAIN_WINDOW: u64 = 800_000;
const DEFAULT_MAIN_RESERVE: u64 = 16_000;
pub(crate) const DEFAULT_STORE_DIRNAME: &str = "store";

fn check_numeric_fields(
    role: &str,
    nudge: Option<u64>,
    checkpoint: Option<u64>,
    steps: Option<u32>,
    window: Option<u64>,
) -> Result<(), ConfigError> {
    if nudge == Some(0) {
        return Err(ConfigError::NonPositiveNudge);
    }
    if checkpoint == Some(0) {
        return Err(ConfigError::NonPositiveCheckpoint);
    }
    if steps == Some(0) {
        return Err(ConfigError::NonPositiveSteps { role: role.to_string() });
    }
    if window == Some(0) {
        return Err(ConfigError::NonPositiveWindow { role: role.to_string() });
    }
    Ok(())
}

fn check_role(role: &str, config: &RoleConfig) -> Result<(), ConfigError> {
    check_numeric_fields(role, config.nudge, config.checkpoint, config.steps, config.window)
}

fn check_main(config: &MainConfig) -> Result<(), ConfigError> {
    check_numeric_fields("main", config.nudge, config.checkpoint, config.steps, config.window)?;
    if config.reserve == Some(0) {
        return Err(ConfigError::NonPositiveReserve);
    }
    let window_eff = config.window.unwrap_or(DEFAULT_MAIN_WINDOW);
    let reserve_eff = config.reserve.unwrap_or(DEFAULT_MAIN_RESERVE);
    if reserve_eff >= window_eff {
        return Err(ConfigError::ReserveNotBelowWindow);
    }
    Ok(())
}

fn provider_empty(provider: &Option<String>) -> bool {
    provider.as_deref().map(str::is_empty).unwrap_or(true)
}

fn validate_registry(models: &[ModelEntry], providers: &BTreeMap<String, ProviderConfig>) -> Result<(), ConfigError> {
    if models.is_empty() {
        return Err(ConfigError::EmptyModelRegistry);
    }
    let mut seen = std::collections::HashSet::new();
    for entry in models {
        if entry.id.is_empty() {
            return Err(ConfigError::EmptyModelId);
        }
        if entry.id.trim() != entry.id {
            return Err(ConfigError::NonCanonicalModelId { id: entry.id.clone() });
        }
        if !seen.insert(entry.id.as_str()) {
            return Err(ConfigError::DuplicateModelId { id: entry.id.clone() });
        }
    }
    for entry in models {
        match entry.body {
            Body::Native => {
                if provider_empty(&entry.provider) {
                    return Err(ConfigError::NativeMissingProvider { id: entry.id.clone() });
                }
                let name = entry.provider.as_deref().unwrap_or_default();
                if !providers.contains_key(name) {
                    return Err(ConfigError::UnknownProvider {
                        id: entry.id.clone(),
                        provider: name.to_string(),
                    });
                }
            }
            _ => {
                if entry.provider.is_some() {
                    return Err(ConfigError::ProviderOnNonNative {
                        id: entry.id.clone(),
                        body: entry.body,
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_roles(
    roles: &BTreeMap<String, String>,
    known_positions: &[String],
    models: &[ModelEntry],
) -> Result<(), ConfigError> {
    for key in roles.keys() {
        if key != "main" && !known_positions.contains(key) {
            return Err(ConfigError::UnknownRoleKey { key: key.clone() });
        }
    }
    let mut required: Vec<String> = vec!["main".to_string()];
    required.extend(known_positions.iter().cloned());
    for position in &required {
        if !roles.contains_key(position) {
            return Err(ConfigError::MissingRoleEntry {
                position: position.clone(),
            });
        }
    }
    let mut known_ids: Vec<String> = models.iter().map(|entry| entry.id.clone()).collect();
    known_ids.sort();
    for (role, id) in roles {
        if !models.iter().any(|entry| &entry.id == id) {
            return Err(ConfigError::UnknownModel {
                role: role.clone(),
                id: id.clone(),
                known: known_ids.clone(),
            });
        }
    }
    Ok(())
}

pub fn main_window(row: &RoleRow) -> (u64, u64) {
    (row.window.unwrap_or(DEFAULT_MAIN_WINDOW), row.reserve.unwrap_or(DEFAULT_MAIN_RESERVE))
}

fn find_drive_key(value: &toml::Value, path: &str) -> Option<(String, String)> {
    match value {
        toml::Value::Table(table) => {
            for key in table.keys() {
                if key == "drive" || key == "drive_surface" {
                    return Some((key.clone(), path.to_string()));
                }
            }
            for (key, child) in table {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if let Some(found) = find_drive_key(child, &child_path) {
                    return Some(found);
                }
            }
            None
        }
        toml::Value::Array(items) => {
            for item in items {
                if let Some(found) = find_drive_key(item, path) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn split_frontmatter(text: &str) -> Option<&str> {
    let mut idx = 0usize;
    let mut lines = text.split_inclusive('\n');
    let first_line = lines.next()?;
    if first_line.trim_end_matches(['\n', '\r']) != "---" {
        return None;
    }
    idx += first_line.len();

    let mut cursor = idx;
    for line in text[idx..].split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']) == "---" {
            return Some(&text[idx..cursor]);
        }
        cursor += line.len();
    }
    None
}

fn role_line(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let mut lines = text.split_inclusive('\n');
    let first_line = lines.next()?;
    if first_line.trim_end_matches(['\n', '\r']) != "---" {
        return None;
    }
    let mut cursor = first_line.len();
    let mut body_start = None;
    for line in text[cursor..].split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']) == "---" {
            body_start = Some(cursor + line.len());
            break;
        }
        cursor += line.len();
    }
    let body_start = body_start?;
    text[body_start..]
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn frontmatter_has_key(yaml: &str, key: &str) -> bool {
    matches!(
        serde_yaml::from_str::<serde_yaml::Value>(yaml),
        Ok(serde_yaml::Value::Mapping(map)) if map.contains_key(serde_yaml::Value::String(key.to_string()))
    )
}

const RETIRED_FRONTMATTER_KEYS: [&str; 4] = ["body", "model", "provider", "image"];

fn parse_frontmatter(path: &Path) -> Result<RoleConfig, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let yaml = split_frontmatter(&text).ok_or_else(|| ConfigError::FrontmatterMissing {
        path: path.to_path_buf(),
    })?;
    match serde_yaml::from_str::<RoleConfig>(yaml) {
        Ok(config) => Ok(config),
        Err(source) => {
            for key in RETIRED_FRONTMATTER_KEYS {
                if frontmatter_has_key(yaml, key) {
                    return Err(ConfigError::FrontmatterFieldMoved {
                        path: path.to_path_buf(),
                        key: key.to_string(),
                    });
                }
            }
            Err(ConfigError::FrontmatterParse {
                path: path.to_path_buf(),
                source,
            })
        }
    }
}

pub fn resolve_store_root(config_dir: &Path, root: Option<&Path>) -> Result<PathBuf, ConfigError> {
    let joined = match root {
        None => config_dir.join(DEFAULT_STORE_DIRNAME),
        Some(value) => {
            let text = value.to_string_lossy();
            if text.trim().is_empty() {
                return Err(ConfigError::EmptyStoreRoot);
            }
            if text == "~" || text.starts_with("~/") {
                let home = std::env::var_os("HOME").ok_or(ConfigError::StoreRootHome)?;
                let rest = text.trim_start_matches('~').trim_start_matches('/');
                PathBuf::from(home).join(rest)
            } else {
                config_dir.join(value)
            }
        }
    };
    if joined.is_relative() {
        if let Ok(cwd) = std::env::current_dir() {
            return Ok(cwd.join(joined));
        }
    }
    Ok(joined)
}

fn row_from_registry(
    role: &str,
    source: String,
    config: &RoleConfig,
    entry: &ModelEntry,
    role_line: Option<String>,
) -> RoleRow {
    RoleRow {
        role: role.to_string(),
        source,
        body: entry.body,
        model: entry.id.clone(),
        provider: entry.provider.clone(),
        drive: drive_surface(RoleKind::Dispatch, entry.body),
        nudge: config.nudge.unwrap_or(DEFAULT_NUDGE),
        checkpoint: config.checkpoint,
        steps: config.steps.unwrap_or(DEFAULT_STEPS),
        window: config.window,
        reserve: config.reserve,
        write: config.write,
        env: config.env.clone(),
        image: entry.image.clone(),
        tools: config.tools.clone(),
        role_line,
    }
}

pub fn load(path: &Path) -> Result<Resolved, ConfigError> {
    let resolved = load_unbound(path)?;
    let roster = crate::roster::Roster::load(&crate::roster::roster_path(path)).unwrap_or_default();
    let body = roster
        .get("main")
        .and_then(|id| resolved.model_entry(id))
        .filter(|entry| matches!(entry.body, Body::ClaudeCode | Body::Codex))
        .map(|entry| entry.body)
        .unwrap_or(resolved.main.body);
    if body == Body::ClaudeCode && main_window(&resolved.main).0 >= 900_000 {
        return Err(ConfigError::MainWindowTooHigh);
    }
    Ok(resolved)
}

fn load_unbound(path: &Path) -> Result<Resolved, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let raw: toml::Table = text.parse().map_err(|source| ConfigError::TomlParse {
        path: path.to_path_buf(),
        source,
    })?;
    if let Some((key, table)) = find_drive_key(&toml::Value::Table(raw), "") {
        return Err(ConfigError::DriveSurfaceConfigured { key, table });
    }

    let startup: StartupConfig = toml::from_str(&text).map_err(|source| ConfigError::TomlParse {
        path: path.to_path_buf(),
        source,
    })?;
    check_main(&startup.main)?;

    let providers = startup.providers.clone().unwrap_or_default();
    for (name, provider) in &providers {
        if provider.base_url.is_empty() {
            return Err(ConfigError::EmptyProviderField {
                provider: name.clone(),
                field: "base_url",
            });
        }
        if provider.api_key_env.is_empty() {
            return Err(ConfigError::EmptyProviderField {
                provider: name.clone(),
                field: "api_key_env",
            });
        }
    }

    validate_registry(&startup.models, &providers)?;

    let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let agents_dir = config_dir.join(startup.agents.clone().unwrap_or_else(|| PathBuf::from("agents")));
    let sidecar_dir = config_dir.join(startup.sidecar.clone().unwrap_or_else(|| PathBuf::from("sidecar")));
    let instructions = startup.main.instructions.clone().map(|p| config_dir.join(p));
    let repo_instructions = startup.main.repo_instructions.clone().map(|p| config_dir.join(p));

    let entries = fs::read_dir(&agents_dir).map_err(|_| ConfigError::AgentsDirUnreadable {
        path: agents_dir.clone(),
    })?;

    let mut md_files: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| ConfigError::AgentsDirUnreadable {
            path: agents_dir.clone(),
        })?;
        let entry_path = entry.path();
        if entry_path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            md_files.push(entry_path);
        }
    }

    if md_files
        .iter()
        .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("main.md"))
    {
        return Err(ConfigError::MainOverride);
    }

    let reviewer_path = md_files
        .iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some("reviewer.md"))
        .cloned()
        .ok_or(ConfigError::NoReviewerDefinition)?;

    let runner_path = md_files
        .iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some("runner.md"))
        .cloned()
        .ok_or(ConfigError::NoRunnerDefinition)?;

    let drain_path = md_files
        .iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some("drain.md"))
        .cloned();

    let mut hand_paths: Vec<PathBuf> = md_files
        .into_iter()
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str());
            name != Some("runner.md") && name != Some("reviewer.md") && name != Some("drain.md")
        })
        .collect();
    hand_paths.sort();

    if hand_paths.is_empty() {
        return Err(ConfigError::NoHandDefinition);
    }

    let mut known_positions: Vec<String> = vec!["runner".to_string(), "reviewer".to_string()];
    known_positions.extend(hand_paths.iter().map(|p| p.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string()));
    if drain_path.is_some() {
        known_positions.push("drain".to_string());
    }
    validate_roles(&startup.roles, &known_positions, &startup.models)?;

    let registry_lookup = |id: &str| startup.models.iter().find(|entry| entry.id == id);

    let runner_config = parse_frontmatter(&runner_path)?;
    check_role("runner", &runner_config)?;
    let runner_id = startup.roles.get("runner").expect("validate_roles required 'runner'");
    let runner_entry = registry_lookup(runner_id).expect("validate_roles checked the id resolves");
    if runner_entry.body == Body::Native {
        return Err(ConfigError::RunnerBodyUnsupported {
            id: runner_id.clone(),
            body: runner_entry.body,
        });
    }
    let runner = row_from_registry(
        "runner",
        runner_path.display().to_string(),
        &runner_config,
        runner_entry,
        role_line(&runner_path),
    );

    let reviewer_config = parse_frontmatter(&reviewer_path)?;
    check_role("reviewer", &reviewer_config)?;
    let reviewer_id = startup.roles.get("reviewer").expect("validate_roles required 'reviewer'");
    let reviewer_entry = registry_lookup(reviewer_id).expect("validate_roles checked the id resolves");
    let reviewer = row_from_registry(
        "reviewer",
        reviewer_path.display().to_string(),
        &reviewer_config,
        reviewer_entry,
        role_line(&reviewer_path),
    );

    let mut hands = Vec::new();
    for hand_path in &hand_paths {
        let name = hand_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let hand_config = parse_frontmatter(hand_path)?;
        check_role(&name, &hand_config)?;
        let hand_id = startup.roles.get(&name).expect("validate_roles required every hand position");
        let hand_entry = registry_lookup(hand_id).expect("validate_roles checked the id resolves");
        hands.push(row_from_registry(
            &name,
            hand_path.display().to_string(),
            &hand_config,
            hand_entry,
            role_line(hand_path),
        ));
    }

    let main_id = startup.roles.get("main").expect("validate_roles required 'main'");
    let main_entry = registry_lookup(main_id).expect("validate_roles checked the id resolves");
    let main = RoleRow {
        role: "main".to_string(),
        source: path.display().to_string(),
        body: main_entry.body,
        model: main_entry.id.clone(),
        provider: main_entry.provider.clone(),
        drive: drive_surface(RoleKind::Main, main_entry.body),
        nudge: startup.main.nudge.unwrap_or(DEFAULT_NUDGE),
        checkpoint: startup.main.checkpoint,
        steps: startup.main.steps.unwrap_or(DEFAULT_STEPS),
        window: startup.main.window,
        reserve: startup.main.reserve,
        write: false,
        env: Vec::new(),
        image: main_entry.image.clone(),
        tools: Vec::new(),
        role_line: None,
    };

    let drain = match &drain_path {
        Some(drain_path) => {
            let drain_config = parse_frontmatter(drain_path)?;
            check_role("drain", &drain_config)?;
            let drain_id = startup.roles.get("drain").expect("validate_roles required 'drain'");
            let drain_entry = registry_lookup(drain_id).expect("validate_roles checked the id resolves");
            Some(row_from_registry(
                "drain",
                drain_path.display().to_string(),
                &drain_config,
                drain_entry,
                role_line(drain_path),
            ))
        }
        None => None,
    };

    let admission = match &startup.admission {
        Some(cfg) => {
            if cfg.threshold_tokens == Some(0) {
                return Err(ConfigError::ZeroAdmissionThreshold);
            }
            if cfg.overrides.as_ref().is_some_and(|overrides| overrides.values().any(|value| *value == 0)) {
                return Err(ConfigError::ZeroAdmissionThreshold);
            }
            AdmissionPolicy {
                threshold_tokens: cfg.threshold_tokens.unwrap_or(AdmissionPolicy::default().threshold_tokens),
                overrides: cfg.overrides.clone().unwrap_or_default(),
            }
        }
        None => AdmissionPolicy::default(),
    };

    let store_scope = startup.store.as_ref().map(|store| store.scope.clone());
    let store_embedder = startup.store.as_ref().and_then(|store| store.embedder.clone());
    if let Some(embedder) = &store_embedder {
        if embedder.trim().is_empty() {
            return Err(ConfigError::EmptyStoreEmbedder);
        }
    }
    let store_root = resolve_store_root(
        config_dir,
        startup.store.as_ref().and_then(|store| store.root.as_deref()),
    )?;

    let terminal_keys = startup.terminal.keys.clone().map(|p| config_dir.join(p));
    if let Err(message) = crate::panes::palette::Palette::from_config(&startup.terminal.palette) {
        return Err(ConfigError::InvalidPalette { message });
    }

    Ok(Resolved {
        checkpoint: main.checkpoint,
        main,
        runner,
        reviewer,
        hands,
        drain,
        models: startup.models.clone(),
        instructions,
        repo_instructions,
        sidecar: sidecar_dir,
        store_scope,
        store_embedder,
        store_root,
        providers,
        gate: startup.gate.clone(),
        admission,
        terminal_front: startup.terminal.front,
        terminal_keys,
        terminal_palette: startup.terminal.palette.clone(),
    })
}

pub fn user_gate_path() -> PathBuf {
    if let Ok(path) = std::env::var("ARX_USER_GATE") {
        return PathBuf::from(path);
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config")
        });
    base.join("arx").join("gate.toml")
}

pub fn load_user_gate(path: &Path) -> Result<GateConfig, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(GateConfig::default()),
        Err(source) => {
            return Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct UserGateFile {
        #[serde(default)]
        gate: GateConfig,
    }
    let parsed: UserGateFile = toml::from_str(&text).map_err(|source| ConfigError::TomlParse {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(parsed.gate)
}

pub fn check_gate(cfg: &GateConfig, label: &str) -> Result<(), ConfigError> {
    crate::gate::reject_ask_rules(cfg, label).map_err(|message| ConfigError::GateRule { message })?;
    for rules in [&cfg.deny, &cfg.allow] {
        crate::gate::parse_tier_rules(rules, label).map_err(|message| ConfigError::GateRule { message })?;
    }
    for hook in &cfg.hooks {
        if hook.trim().is_empty() {
            return Err(ConfigError::GateRule {
                message: format!("{label}: empty hook command"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_REGISTRY_AND_ROLES: &str =
        "\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\n";

    fn minimal_main(extra_main: &str) -> String {
        format!("[main]\nbody = \"claude-code\"\n{extra_main}{MINIMAL_REGISTRY_AND_ROLES}")
    }

    #[test]
    fn frontmatter_split_valid() {
        let text = "---\ntools: [Read]\n---\nsome prose after\n";
        let yaml = split_frontmatter(text).expect("frontmatter block");
        assert_eq!(yaml, "tools: [Read]\n");
    }

    #[test]
    fn frontmatter_split_missing() {
        let text = "no frontmatter here\ntools: [Read]\n";
        assert_eq!(split_frontmatter(text), None);
    }

    #[test]
    fn frontmatter_split_unterminated() {
        let text = "---\ntools: [Read]\n";
        assert_eq!(split_frontmatter(text), None);
    }

    #[test]
    fn registry_provider_on_non_native_body_is_rejected() {
        let models = vec![ModelEntry {
            id: "gpt-5.6-sol".to_string(),
            body: Body::Codex,
            image: None,
            provider: Some("anthropic".to_string()),
        }];
        let err = validate_registry(&models, &BTreeMap::new()).unwrap_err();
        assert!(matches!(err, ConfigError::ProviderOnNonNative { .. }));
        assert!(err.to_string().contains("a body is not a provider"));
    }

    #[test]
    fn registry_native_without_provider_is_rejected() {
        let models = vec![ModelEntry {
            id: "deepseek-v4-flash".to_string(),
            body: Body::Native,
            image: None,
            provider: None,
        }];
        let err = validate_registry(&models, &BTreeMap::new()).unwrap_err();
        assert!(matches!(err, ConfigError::NativeMissingProvider { .. }));
        assert!(err.to_string().contains("requires a provider"));

        let models_empty = vec![ModelEntry {
            id: "deepseek-v4-flash".to_string(),
            body: Body::Native,
            image: None,
            provider: Some(String::new()),
        }];
        let err = validate_registry(&models_empty, &BTreeMap::new()).unwrap_err();
        assert!(matches!(err, ConfigError::NativeMissingProvider { .. }));
    }

    #[test]
    fn drive_key_walk_catches_nested_key() {
        let toml_text = "[main]\nbody = \"claude-code\"\nmodel = \"claude-fable-5\"\ndrive_surface = \"agent-sdk\"\n";
        let table: toml::Table = toml_text.parse().unwrap();
        let (key, table) = find_drive_key(&toml::Value::Table(table), "").expect("drive key found");
        assert_eq!(key, "drive_surface");
        assert_eq!(table, "main");
    }

    #[test]
    fn drive_key_walk_catches_key_nested_in_array_of_tables() {
        let toml_text = "[main]\nbody = \"claude-code\"\nmodel = \"claude-fable-5\"\n\n[[roles]]\nname = \"extra\"\ndrive = \"native loop\"\n";
        let table: toml::Table = toml_text.parse().unwrap();
        let (key, table) = find_drive_key(&toml::Value::Table(table), "").expect("drive key found");
        assert_eq!(key, "drive");
        assert_eq!(table, "roles");
    }

    #[test]
    fn drive_key_walk_catches_key_nested_in_the_models_registry() {
        let toml_text =
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\ndrive = \"claude -p\"\n";
        let table: toml::Table = toml_text.parse().unwrap();
        let (key, table) = find_drive_key(&toml::Value::Table(table), "").expect("drive key found");
        assert_eq!(key, "drive");
        assert_eq!(table, "models");
    }

    fn write_minimal_agents(dir: &Path) {
        fs::create_dir_all(dir.join("agents")).unwrap();
        fs::write(dir.join("agents/runner.md"), "---\ntools: [Read, Bash]\nwrite: true\n---\nrunner\n").unwrap();
        fs::write(dir.join("agents/reviewer.md"), "---\ntools: []\n---\nreviewer\n").unwrap();
        fs::write(dir.join("agents/hand-ex.md"), "---\ntools: []\n---\nhand\n").unwrap();
    }

    #[test]
    fn a_hand_file_still_carrying_model_is_refused_with_the_moved_message() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("agents/hand-ex.md"),
            "---\nmodel: claude-sonnet-5\n---\nhand\n",
        )
        .unwrap();
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::FrontmatterFieldMoved { .. }));
        assert!(err.to_string().contains("'model' belongs to [[models]] and [roles]"), "err: {err}");
    }

    #[test]
    fn a_hand_file_still_carrying_body_is_refused_with_the_moved_message() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("agents/hand-ex.md"),
            "---\nbody: claude-code\n---\nhand\n",
        )
        .unwrap();
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::FrontmatterFieldMoved { .. }));
        assert!(err.to_string().contains("'body' belongs to [[models]] and [roles]"), "err: {err}");
    }

    #[test]
    fn a_hand_file_still_carrying_provider_is_refused_with_the_moved_message() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("agents/hand-ex.md"),
            "---\nprovider: anthropic\n---\nhand\n",
        )
        .unwrap();
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::FrontmatterFieldMoved { .. }));
        assert!(err.to_string().contains("'provider' belongs to [[models]] and [roles]"), "err: {err}");
    }

    #[test]
    fn a_hand_file_still_carrying_image_is_refused_with_the_moved_message() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("agents/hand-ex.md"),
            "---\nimage: custom:latest\n---\nhand\n",
        )
        .unwrap();
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::FrontmatterFieldMoved { .. }));
        assert!(err.to_string().contains("'image' belongs to [[models]] and [roles]"), "err: {err}");
    }

    #[test]
    fn roles_value_naming_an_unknown_id_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"nonesuch\"\n",
        )
        .unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::UnknownModel { .. }), "err: {err}");
        assert!(err.to_string().contains("hand-ex"), "err: {err}");
        assert!(err.to_string().contains("nonesuch"), "err: {err}");
        assert!(err.to_string().contains("claude-sonnet-5"), "err: {err}");
    }

    #[test]
    fn two_registry_entries_sharing_an_id_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"codex\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\n",
        )
        .unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::DuplicateModelId { .. }));
        assert_eq!(err.to_string(), "[[models]]: duplicate id 'claude-sonnet-5'");
    }

    #[test]
    fn a_models_entry_with_codex_body_and_a_provider_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[[models]]\nid = \"gpt-5.6-sol\"\nbody = \"codex\"\nprovider = \"anthropic\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"gpt-5.6-sol\"\nreviewer = \"gpt-5.6-sol\"\nhand-ex = \"claude-sonnet-5\"\n",
        )
        .unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::ProviderOnNonNative { .. }));
    }

    #[test]
    fn a_models_entry_with_native_body_and_no_provider_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[[models]]\nid = \"deepseek-v4-flash\"\nbody = \"native\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\n",
        )
        .unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::NativeMissingProvider { .. }));
    }

    #[test]
    fn an_unmatched_roles_key_is_refused_naming_the_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\nhand-ghost = \"claude-sonnet-5\"\n",
        )
        .unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::UnknownRoleKey { .. }));
        assert!(err.to_string().contains("agents/hand-ghost.md"), "err: {err}");
    }

    #[test]
    fn a_missing_required_roles_entry_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\n",
        )
        .unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::MissingRoleEntry { .. }));
        assert_eq!(err.to_string(), "[roles] has no entry for 'hand-ex'");
    }

    #[test]
    fn registry_main_body_overrides_the_legacy_body_selector() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[[models]]\nid = \"gpt-5.6-sol\"\nbody = \"codex\"\n\n[roles]\nmain = \"gpt-5.6-sol\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\n",
        )
        .unwrap();

        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.main.body, Body::Codex);
        assert_eq!(resolved.main.model, "gpt-5.6-sol");
    }

    #[test]
    fn an_empty_registry_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\n",
        )
        .unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::EmptyModelRegistry));
    }

    #[test]
    fn sidecar_and_instructions_resolve_relative_to_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("prompt.md"), "you are main\n").unwrap();
        fs::write(dir.path().join("REPO.md"), "repo instructions\n").unwrap();
        fs::write(
            dir.path().join("arx.toml"),
            format!(
                "sidecar = \"custom-sidecar\"\n\n{}",
                minimal_main("instructions = \"prompt.md\"\nrepo_instructions = \"REPO.md\"\n")
            ),
        )
        .unwrap();

        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.sidecar, dir.path().join("custom-sidecar"));
        assert_eq!(resolved.instructions, Some(dir.path().join("prompt.md")));
        assert_eq!(resolved.repo_instructions, Some(dir.path().join("REPO.md")));
    }

    #[test]
    fn sidecar_and_instructions_default_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();

        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.sidecar, dir.path().join("sidecar"));
        assert_eq!(resolved.instructions, None);
        assert_eq!(resolved.repo_instructions, None);
    }

    #[test]
    fn terminal_section_defaults_to_the_line_front_with_no_keys_override() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();

        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.terminal_front, FrontKind::Line);
        assert_eq!(resolved.terminal_keys, None);
    }

    #[test]
    fn terminal_section_parses_panes_and_resolves_keys_against_the_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            format!("{}\n[terminal]\nfront = \"panes\"\nkeys = \"keys.toml\"\n", minimal_main("")),
        )
        .unwrap();

        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.terminal_front, FrontKind::Panes);
        assert_eq!(resolved.terminal_keys, Some(dir.path().join("keys.toml")));
    }

    #[test]
    fn terminal_palette_defaults_to_none_and_a_good_override_resolves() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();
        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert!(resolved.terminal_palette.is_none());

        fs::write(
            dir.path().join("arx.toml"),
            format!("{}\n[terminal.palette]\ntool = \"#b0503c\"\n", minimal_main("")),
        )
        .unwrap();
        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.terminal_palette.unwrap().tool.as_deref(), Some("#b0503c"));
    }

    #[test]
    fn terminal_palette_refuses_a_color_the_config_load_cannot_resolve() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            format!("{}\n[terminal.palette]\nrule = \"olive\"\n", minimal_main("")),
        )
        .unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPalette { .. }), "err: {err}");
    }

    #[test]
    fn terminal_section_refuses_an_unknown_field() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(
            dir.path().join("arx.toml"),
            format!("{}\n[terminal]\nnope = true\n", minimal_main("")),
        )
        .unwrap();

        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::TomlParse { .. }));
    }

    #[test]
    fn main_nudge_defaults_to_120000_and_reads_a_set_key() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();
        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.main.nudge, 120_000);

        fs::write(dir.path().join("arx.toml"), minimal_main("nudge = 40000\n")).unwrap();
        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.main.nudge, 40_000);
    }

    #[test]
    fn main_nudge_of_zero_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("nudge = 0\n")).unwrap();
        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::NonPositiveNudge));
        assert_eq!(err.to_string(), "main.nudge must be positive");
    }

    #[test]
    fn main_checkpoint_parses() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();
        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.main.checkpoint, None);
        assert_eq!(resolved.checkpoint, None);

        fs::write(dir.path().join("arx.toml"), minimal_main("checkpoint = 50000\n")).unwrap();
        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.main.checkpoint, Some(50_000));
        assert_eq!(resolved.checkpoint, Some(50_000));
    }

    #[test]
    fn main_checkpoint_zero_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("checkpoint = 0\n")).unwrap();
        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::NonPositiveCheckpoint));
        assert_eq!(err.to_string(), "main.checkpoint must be positive");
    }

    #[test]
    fn steps_parses_and_defaults_to_sixteen() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();
        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.main.steps, 16);

        fs::write(dir.path().join("arx.toml"), minimal_main("steps = 5\n")).unwrap();
        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.main.steps, 5);
    }

    #[test]
    fn steps_zero_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("steps = 0\n")).unwrap();
        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::NonPositiveSteps { .. }));
        assert_eq!(err.to_string(), "main.steps must be positive");
    }

    #[test]
    fn window_parses_and_defaults_to_none() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("")).unwrap();
        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.main.window, None);

        fs::write(dir.path().join("arx.toml"), minimal_main("window = 200000\n")).unwrap();
        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        assert_eq!(resolved.main.window, Some(200_000));
    }

    #[test]
    fn window_zero_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_agents(dir.path());
        fs::write(dir.path().join("arx.toml"), minimal_main("window = 0\n")).unwrap();
        let err = load(&dir.path().join("arx.toml")).unwrap_err();
        assert!(matches!(err, ConfigError::NonPositiveWindow { .. }));
        assert_eq!(err.to_string(), "main.window must be positive");
    }

    #[test]
    fn gate_table_parses_all_four_arrays() {
        let text = format!(
            "{}\n[gate]\ndeny = [\"Bash(rm -rf*)\"]\nask = [\"Bash(git push*)\"]\nallow = [\"Read\"]\nhooks = [\"/bin/true\"]\n",
            minimal_main("")
        );
        let startup: StartupConfig = toml::from_str(&text).unwrap();
        assert_eq!(startup.gate.deny, vec!["Bash(rm -rf*)".to_string()]);
        assert_eq!(startup.gate.ask, vec!["Bash(git push*)".to_string()]);
        assert_eq!(startup.gate.allow, vec!["Read".to_string()]);
        assert_eq!(startup.gate.hooks, vec!["/bin/true".to_string()]);
    }

    #[test]
    fn gate_table_defaults_to_empty_when_absent() {
        let text = minimal_main("");
        let startup: StartupConfig = toml::from_str(&text).unwrap();
        assert_eq!(startup.gate.deny, Vec::<String>::new());
        assert_eq!(startup.gate.ask, Vec::<String>::new());
        assert_eq!(startup.gate.allow, Vec::<String>::new());
        assert_eq!(startup.gate.hooks, Vec::<String>::new());
    }

    #[test]
    fn gate_table_unknown_key_is_refused() {
        let text = format!("{}\n[gate]\nnope = true\n", minimal_main(""));
        let result: Result<StartupConfig, _> = toml::from_str(&text);
        assert!(result.is_err());
    }

    #[test]
    fn check_gate_refuses_a_pattern_on_a_tool_without_a_subject() {
        let cfg = GateConfig {
            deny: vec!["mcp__arx__save(x)".to_string()],
            ..Default::default()
        };
        let err = check_gate(&cfg, "./arx.toml").unwrap_err();
        assert!(err.to_string().contains("mcp__arx__save"));
        assert!(err.to_string().contains("bare tool name"));
    }

    #[test]
    fn check_gate_accepts_the_colon_star_prefix_form() {
        let cfg = GateConfig {
            deny: vec!["Bash(git push:*)".to_string()],
            ..Default::default()
        };
        assert!(check_gate(&cfg, "./arx.toml").is_ok());
    }

    #[test]
    fn check_gate_refuses_a_rule_naming_ask() {
        let cfg = GateConfig {
            ask: vec!["Bash(git push*)".to_string()],
            ..Default::default()
        };
        let err = check_gate(&cfg, "./arx.toml").unwrap_err();
        assert!(err.to_string().contains("./arx.toml"));
        assert!(err.to_string().contains("Bash(git push*)"));
    }

    #[test]
    fn check_gate_refuses_an_empty_hook_command() {
        let cfg = GateConfig {
            hooks: vec!["".to_string()],
            ..Default::default()
        };
        let err = check_gate(&cfg, "./arx.toml").unwrap_err();
        assert!(err.to_string().contains("empty hook command"));
    }

    #[test]
    fn frontmatter_write_env_and_image_parse() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hand-wr.md");
        fs::write(&path, "---\nwrite: true\nenv: [ARX_ONE, ARX_TWO]\n---\nhand\n").unwrap();
        let config = parse_frontmatter(&path).unwrap();
        assert!(config.write);
        assert_eq!(config.env, vec!["ARX_ONE".to_string(), "ARX_TWO".to_string()]);
    }

    #[test]
    fn frontmatter_defaults_are_not_write_enabled_with_an_empty_allowlist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hand-ex.md");
        fs::write(&path, "---\ntools: []\n---\nhand\n").unwrap();
        let config = parse_frontmatter(&path).unwrap();
        assert!(!config.write);
        assert!(config.env.is_empty());
    }

    #[test]
    fn frontmatter_tools_parse() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hand-ex.md");
        fs::write(&path, "---\ntools: [Read, Glob, Bash]\n---\nhand\n").unwrap();
        let config = parse_frontmatter(&path).unwrap();
        assert_eq!(config.tools, vec!["Read".to_string(), "Glob".to_string(), "Bash".to_string()]);
    }

    #[test]
    fn frontmatter_tools_default_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hand-ex.md");
        fs::write(&path, "---\n{}\n---\nhand\n").unwrap();
        let config = parse_frontmatter(&path).unwrap();
        assert!(config.tools.is_empty());
    }

    #[test]
    fn positions_orders_main_runner_reviewer_hands_then_drain() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("agents")).unwrap();
        fs::write(dir.path().join("agents/runner.md"), "---\ntools: [Read, Bash]\nwrite: true\n---\nrunner\n").unwrap();
        fs::write(dir.path().join("agents/reviewer.md"), "---\ntools: []\n---\nreviewer\n").unwrap();
        fs::write(dir.path().join("agents/hand-ex.md"), "---\ntools: []\n---\nhand\n").unwrap();
        fs::write(dir.path().join("agents/drain.md"), "---\ntools: []\n---\ndrain\n").unwrap();
        fs::write(
            dir.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\ndrain = \"claude-sonnet-5\"\n",
        )
        .unwrap();

        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        let names: Vec<&str> = resolved.positions().iter().map(|row| row.role.as_str()).collect();
        assert_eq!(names, vec!["main", "runner", "reviewer", "hand-ex", "drain"]);
    }

    #[test]
    fn positions_orders_main_runner_reviewer_then_hands_alphabetically_then_drain() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("agents")).unwrap();
        fs::write(dir.path().join("agents/runner.md"), "---\ntools: [Read, Bash]\nwrite: true\n---\nrunner\n").unwrap();
        fs::write(dir.path().join("agents/reviewer.md"), "---\ntools: []\n---\nreviewer\n").unwrap();
        fs::write(dir.path().join("agents/hand-ex.md"), "---\ntools: []\n---\nhand\n").unwrap();
        fs::write(dir.path().join("agents/bolt.md"), "---\ntools: []\n---\nbolt\n").unwrap();
        fs::write(dir.path().join("agents/hand-do.md"), "---\ntools: []\nwrite: true\n---\nhand-do\n").unwrap();
        fs::write(dir.path().join("agents/drain.md"), "---\ntools: []\n---\ndrain\n").unwrap();
        fs::write(
            dir.path().join("arx.toml"),
            "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\nbolt = \"claude-sonnet-5\"\nhand-do = \"claude-sonnet-5\"\ndrain = \"claude-sonnet-5\"\n",
        )
        .unwrap();

        let resolved = load(&dir.path().join("arx.toml")).unwrap();
        let names: Vec<&str> = resolved.positions().iter().map(|row| row.role.as_str()).collect();
        assert_eq!(names, vec!["main", "runner", "reviewer", "bolt", "hand-do", "hand-ex", "drain"]);
    }

    fn bare_row(role: &str, body: Body, model: &str, tools: Vec<String>) -> RoleRow {
        RoleRow {
            role: role.to_string(),
            source: String::new(),
            body,
            model: model.to_string(),
            provider: None,
            drive: "test",
            nudge: 1,
            checkpoint: None,
            steps: 1,
            window: None,
            reserve: None,
            write: false,
            env: Vec::new(),
            image: None,
            tools,
            role_line: None,
        }
    }

    #[test]
    fn inert_tools_notice_fires_once_for_a_codex_position_with_a_tools_list() {
        let row = bare_row("bolt", Body::Codex, "gpt-5.6-luna", vec!["Read".to_string(), "Bash".to_string()]);
        let notices = inert_tools_notices([&row]);
        assert_eq!(notices, vec!["[arx] tools: bolt runs gpt-5.6-luna on the codex body, which negates shell unless Bash is listed, and always negates image and web since arx names no tool for them — the rest of codex's built-ins stay on"]);
    }

    #[test]
    fn inert_tools_notice_fires_for_a_codex_position_with_an_empty_tools_list() {
        let row = bare_row("bolt", Body::Codex, "gpt-5.6-luna", Vec::new());
        let notices = inert_tools_notices([&row]);
        assert_eq!(notices, vec!["[arx] tools: bolt runs gpt-5.6-luna on the codex body, which negates shell unless Bash is listed, and always negates image and web since arx names no tool for them — the rest of codex's built-ins stay on"]);
    }

    #[test]
    fn inert_tools_notice_is_silent_for_a_claude_code_position_with_the_same_tools_list() {
        let row = bare_row("hand-ex", Body::ClaudeCode, "claude-sonnet-5", vec!["Read".to_string(), "Bash".to_string()]);
        assert!(inert_tools_notices([&row]).is_empty());
    }

    #[test]
    fn inert_tools_notice_names_the_offending_tool_for_a_native_position() {
        let row = bare_row("hand-native", Body::Native, "claude-haiku-4-5", vec!["Bash".to_string()]);
        let notices = inert_tools_notices([&row]);
        assert_eq!(notices, vec!["[arx] tools: hand-native lists Bash, which the native body does not have"]);
    }

    #[test]
    fn inert_tools_notice_is_silent_for_a_native_position_naming_only_allowed_tools() {
        let row = bare_row("hand-native", Body::Native, "claude-haiku-4-5", vec!["read_file".to_string(), "recall".to_string()]);
        assert!(inert_tools_notices([&row]).is_empty());
    }
}
