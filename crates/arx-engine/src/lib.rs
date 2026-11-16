//! The store's judging half: landscape lookup, lexical recall, lifecycle. Filled by L1-06 and L2-04.

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub mod engine;
pub use engine::Engine;
pub use engine::{EmbedError, Embedder, HashEmbedder, OllamaEmbedder};

const LANDSCAPE_N: usize = 5;
const DEFAULT_LIMIT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimKind {
    Fact,
    Decision,
    Ember,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    #[default]
    InForce,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Validity {
    #[default]
    Valid,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimHandle {
    pub thread: String,
    pub turn: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveIntent {
    pub scope: String,
    pub kind: ClaimKind,
    pub question: String,
    pub answer: String,
    pub why: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Suggestion {
    pub id: String,
    pub action: SuggestionAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionAction {
    Promote,
    Drop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    pub id: String,
    pub kind: ClaimKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub question: String,
    pub answer: String,
    pub why: String,
    pub handle: ClaimHandle,
    #[serde(default)]
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub validity: Validity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalid_reason: Option<String>,
    #[serde(default)]
    pub created_at: u64,
}

fn dropped_is_zero(n: &usize) -> bool {
    *n == 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrainReport {
    pub promoted: usize,
    #[serde(default, skip_serializing_if = "dropped_is_zero")]
    pub dropped: usize,
}

#[derive(Debug, Clone)]
pub struct StoreCall {
    pub verb: String,
    pub args: serde_json::Value,
}

#[derive(Debug)]
pub enum StoreError {
    Io { path: PathBuf, source: std::io::Error },
    BadScope { scope: String },
    MalformedRow { path: PathBuf, line: usize },
    WhyRefused,
    NoScope { scope: String, root: PathBuf },
    MissingField { field: String },
    UnknownClaim { scope: String, id: String },
    NotSupersedable { scope: String, id: String },
    Git { op: String, detail: String },
    MalformedClaimFile { path: PathBuf, detail: String },
    UnsupportedSuggestion { detail: String },
    UncoveredCandidate { id: String },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            StoreError::BadScope { scope } => {
                write!(f, "invalid scope id '{scope}' — must match [A-Za-z0-9_-]+")
            }
            StoreError::MalformedRow { path, line } => {
                write!(f, "{}: malformed store row at line {line}", path.display())
            }
            StoreError::WhyRefused => write!(
                f,
                "why is required and a decision's why must not be 'none'"
            ),
            StoreError::NoScope { scope, root } => write!(
                f,
                "scope '{scope}' has no directory under the store root {root}; mkdir {root}/{scope} to register it",
                root = root.display()
            ),
            StoreError::MissingField { field } => write!(f, "missing field '{field}'"),
            StoreError::UnknownClaim { scope, id } => {
                write!(f, "no claim '{id}' in scope '{scope}'")
            }
            StoreError::NotSupersedable { scope, id } => {
                write!(f, "claim '{id}' in scope '{scope}' is already superseded")
            }
            StoreError::Git { op, detail } => write!(f, "git {op} failed: {detail}"),
            StoreError::MalformedClaimFile { path, detail } => {
                write!(f, "{}: {detail}", path.display())
            }
            StoreError::UnsupportedSuggestion { detail } => write!(f, "unsupported drain suggestion: {detail}"),
            StoreError::UncoveredCandidate { id } => write!(f, "quarantined candidate '{id}' has no suggestion"),
        }
    }
}

impl std::error::Error for StoreError {}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecallOpts {
    pub include_superseded: bool,
    pub include_invalid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Landscape {
    pub claims: Vec<Claim>,
    pub legs: Vec<String>,
    pub degraded: Option<String>,
}

pub trait Store {
    fn save(&mut self, intent: &SaveIntent, handle: &ClaimHandle) -> Result<Vec<Claim>, StoreError>;
    fn recall(&mut self, scope: &str, query: &str, limit: usize) -> Result<Vec<Claim>, StoreError>;
    fn decisions_query(&mut self, scope: &str, query: &str, limit: usize) -> Result<Vec<Claim>, StoreError>;
    fn decision(&mut self, scope: &str, id: &str) -> Result<Option<Claim>, StoreError>;
    fn drain(&mut self) -> Result<DrainReport, StoreError>;
    fn grounding(&mut self, scope: &str) -> Result<Vec<Claim>, StoreError>;

    fn save_with(&mut self, intent: &SaveIntent, handle: &ClaimHandle) -> Result<Landscape, StoreError> {
        Ok(Landscape { claims: self.save(intent, handle)?, legs: vec!["lexical".to_string()], degraded: None })
    }
    fn recall_with(&mut self, scope: &str, query: &str, limit: usize, opts: &RecallOpts) -> Result<Vec<Claim>, StoreError> {
        let _ = opts;
        self.recall(scope, query, limit)
    }
    fn decisions_query_with(&mut self, scope: &str, query: &str, limit: usize, opts: &RecallOpts) -> Result<Vec<Claim>, StoreError> {
        let _ = opts;
        self.decisions_query(scope, query, limit)
    }
    fn drain_with(&mut self, suggestions: Option<&[Suggestion]>, handle: &ClaimHandle) -> Result<DrainReport, StoreError> {
        let _ = handle;
        match suggestions {
            None => self.drain(),
            Some(list) if list.iter().all(|s| s.action == SuggestionAction::Promote && s.supersedes.is_none()) => self.drain(),
            Some(_) => Err(StoreError::UnsupportedSuggestion { detail: "this adapter promotes everything or nothing".to_string() }),
        }
    }
    fn landscape(&mut self, scope: &str, question: &str, answer: &str) -> Result<Landscape, StoreError> {
        let _ = (scope, question, answer);
        Ok(Landscape { claims: Vec::new(), legs: Vec::new(), degraded: None })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuarantineRow {
    id: String,
    scope: String,
    kind: ClaimKind,
    question: String,
    answer: String,
    why: String,
    handle: ClaimHandle,
}

fn io_err(path: &Path, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn validate_scope(scope: &str) -> Result<(), StoreError> {
    let valid = !scope.is_empty()
        && scope.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(StoreError::BadScope {
            scope: scope.to_string(),
        })
    }
}

fn tokens_from(text: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    for word in text.split_whitespace() {
        let lower = word.to_lowercase();
        if lower.len() >= 3 {
            seen.insert(lower);
        }
    }
    seen.into_iter().collect()
}

fn rank_matches(claims: &[Claim], tokens: &[String], limit: usize) -> Vec<Claim> {
    let mut scored: Vec<(usize, usize, &Claim)> = claims
        .iter()
        .enumerate()
        .filter_map(|(idx, claim)| {
            let question = claim.question.to_lowercase();
            let answer = claim.answer.to_lowercase();
            let why = claim.why.to_lowercase();
            let hits = tokens
                .iter()
                .filter(|token| question.contains(token.as_str()) || answer.contains(token.as_str()) || why.contains(token.as_str()))
                .count();
            if hits > 0 {
                Some((hits, idx, claim))
            } else {
                None
            }
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().take(limit).map(|(_, _, claim)| claim.clone()).collect()
}

fn read_committed_lines(path: &Path) -> Result<Vec<String>, StoreError> {
    let mut file = File::open(path).map_err(|source| io_err(path, source))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|source| io_err(path, source))?;
    let committed = match bytes.iter().rposition(|&b| b == b'\n') {
        Some(pos) => &bytes[..pos],
        None => return Ok(Vec::new()),
    };
    committed
        .split(|&b| b == b'\n')
        .map(|chunk| {
            String::from_utf8(chunk.to_vec()).map_err(|source| {
                io_err(path, std::io::Error::new(std::io::ErrorKind::InvalidData, source))
            })
        })
        .collect()
}

fn read_scope_file(path: &Path) -> Result<Vec<Claim>, StoreError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut claims = Vec::new();
    for (idx, line) in read_committed_lines(path)?.into_iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let claim: Claim = serde_json::from_str(&line).map_err(|_| StoreError::MalformedRow {
            path: path.to_path_buf(),
            line: idx + 1,
        })?;
        claims.push(claim);
    }
    Ok(claims)
}

fn truncate_torn_tail(path: &Path) -> Result<(), StoreError> {
    if !path.exists() {
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|source| io_err(path, source))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|source| io_err(path, source))?;
    if bytes.is_empty() || bytes.last() == Some(&b'\n') {
        return Ok(());
    }
    let committed_len = match bytes.iter().rposition(|&b| b == b'\n') {
        Some(pos) => pos + 1,
        None => 0,
    };
    file.set_len(committed_len as u64).map_err(|source| io_err(path, source))
}

fn append_row(path: &Path, claim: &Claim) -> Result<(), StoreError> {
    truncate_torn_tail(path)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| io_err(path, source))?;
    let mut buf = serde_json::to_string(claim).expect("claim always serializes");
    buf.push('\n');
    file.write_all(buf.as_bytes()).map_err(|source| io_err(path, source))?;
    file.flush().map_err(|source| io_err(path, source))
}

fn write_candidate(path: &Path, row: &QuarantineRow) -> Result<(), StoreError> {
    let mut buf = serde_json::to_string(row).expect("quarantine row always serializes");
    buf.push('\n');
    let mut file = File::create(path).map_err(|source| io_err(path, source))?;
    file.write_all(buf.as_bytes()).map_err(|source| io_err(path, source))?;
    file.flush().map_err(|source| io_err(path, source))
}

fn mint_id(seq: u64) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{millis:x}-{seq}")
}

fn unix_seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub struct FileStore {
    root: PathBuf,
    thread: String,
    next_seq: u64,
}

fn recover_next_seq(quarantine_dir: &Path) -> u64 {
    let mut max_seq = 0u64;
    if let Ok(read_dir) = fs::read_dir(quarantine_dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            if let Some(seq) = stem.split('-').next().and_then(|s| s.parse::<u64>().ok()) {
                max_seq = max_seq.max(seq);
            }
        }
    }
    max_seq + 1
}

impl FileStore {
    pub fn open(root: &Path, thread: &str) -> Result<Self, StoreError> {
        fs::create_dir_all(root).map_err(|source| io_err(root, source))?;
        let quarantine_dir = root.join("quarantine").join(thread);
        fs::create_dir_all(&quarantine_dir).map_err(|source| io_err(&quarantine_dir, source))?;
        let next_seq = recover_next_seq(&quarantine_dir);
        Ok(FileStore {
            root: root.to_path_buf(),
            thread: thread.to_string(),
            next_seq,
        })
    }

    fn scope_path(&self, scope: &str) -> PathBuf {
        self.root.join(format!("{scope}.jsonl"))
    }

    fn quarantine_dir(&self) -> PathBuf {
        self.root.join("quarantine").join(&self.thread)
    }
}

impl Store for FileStore {
    fn save(&mut self, intent: &SaveIntent, handle: &ClaimHandle) -> Result<Vec<Claim>, StoreError> {
        if intent.why.trim().is_empty() {
            return Err(StoreError::WhyRefused);
        }
        if intent.kind == ClaimKind::Decision && intent.why.trim().eq_ignore_ascii_case("none") {
            return Err(StoreError::WhyRefused);
        }
        validate_scope(&intent.scope)?;

        let claims = read_scope_file(&self.scope_path(&intent.scope))?;
        let tokens = tokens_from(&format!("{} {}", intent.question, intent.answer));
        let landscape = rank_matches(&claims, &tokens, LANDSCAPE_N);

        let seq = self.next_seq;
        self.next_seq += 1;
        let id = mint_id(seq);

        let row = QuarantineRow {
            id: id.clone(),
            scope: intent.scope.clone(),
            kind: intent.kind,
            question: intent.question.clone(),
            answer: intent.answer.clone(),
            why: intent.why.clone(),
            handle: handle.clone(),
        };

        let quarantine_dir = self.quarantine_dir();
        fs::create_dir_all(&quarantine_dir).map_err(|source| io_err(&quarantine_dir, source))?;
        let candidate_path = quarantine_dir.join(format!("{seq}-{id}.json"));
        write_candidate(&candidate_path, &row)?;

        Ok(landscape)
    }

    fn recall(&mut self, scope: &str, query: &str, limit: usize) -> Result<Vec<Claim>, StoreError> {
        validate_scope(scope)?;
        let claims = read_scope_file(&self.scope_path(scope))?;
        let tokens = tokens_from(query);
        Ok(rank_matches(&claims, &tokens, limit))
    }

    fn decisions_query(&mut self, scope: &str, query: &str, limit: usize) -> Result<Vec<Claim>, StoreError> {
        validate_scope(scope)?;
        let claims: Vec<Claim> = read_scope_file(&self.scope_path(scope))?
            .into_iter()
            .filter(|claim| claim.kind == ClaimKind::Decision)
            .collect();
        let tokens = tokens_from(query);
        Ok(rank_matches(&claims, &tokens, limit))
    }

    fn decision(&mut self, scope: &str, id: &str) -> Result<Option<Claim>, StoreError> {
        validate_scope(scope)?;
        let claims = read_scope_file(&self.scope_path(scope))?;
        Ok(claims.into_iter().find(|claim| claim.id == id))
    }

    fn drain(&mut self) -> Result<DrainReport, StoreError> {
        let quarantine_dir = self.quarantine_dir();
        let mut entries: Vec<(u64, PathBuf)> = Vec::new();
        if quarantine_dir.exists() {
            for entry in fs::read_dir(&quarantine_dir).map_err(|source| io_err(&quarantine_dir, source))? {
                let entry = entry.map_err(|source| io_err(&quarantine_dir, source))?;
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                let seq_str = stem.split('-').next().unwrap_or_default();
                let seq: u64 = match seq_str.parse() {
                    Ok(seq) => seq,
                    Err(_) => continue,
                };
                entries.push((seq, path));
            }
        }
        entries.sort_by_key(|(seq, _)| *seq);

        let mut promoted = 0usize;
        for (_, path) in entries {
            let text = fs::read_to_string(&path).map_err(|source| io_err(&path, source))?;
            let row: QuarantineRow = serde_json::from_str(&text).map_err(|_| StoreError::MalformedRow {
                path: path.clone(),
                line: 1,
            })?;
            validate_scope(&row.scope)?;
            let claim = Claim {
                id: row.id,
                kind: row.kind,
                label: None,
                question: row.question,
                answer: row.answer,
                why: row.why,
                handle: row.handle,
                lifecycle: Lifecycle::InForce,
                validity: Validity::Valid,
                supersedes: None,
                superseded_by: None,
                invalid_reason: None,
                created_at: unix_seconds_now(),
            };
            let scope_path = self.scope_path(&row.scope);
            let already_promoted = read_scope_file(&scope_path)?.iter().any(|existing| existing.id == claim.id);
            if !already_promoted {
                append_row(&scope_path, &claim)?;
                promoted += 1;
            }
            fs::remove_file(&path).map_err(|source| io_err(&path, source))?;
        }
        Ok(DrainReport { promoted, dropped: 0 })
    }

    fn grounding(&mut self, scope: &str) -> Result<Vec<Claim>, StoreError> {
        validate_scope(scope)?;
        let claims = read_scope_file(&self.scope_path(scope))?;
        Ok(claims
            .into_iter()
            .filter(|claim| matches!(claim.label.as_deref(), Some("hot") | Some("ember")))
            .collect())
    }
}

fn value_str(args: &serde_json::Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|value| value.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("missing string field '{key}'"))
}

fn value_limit(args: &serde_json::Value, key: &str, default: usize) -> usize {
    args.get(key)
        .and_then(|value| value.as_u64())
        .map(|n| n as usize)
        .unwrap_or(default)
}

fn serve_save(store: &mut dyn Store, args: &serde_json::Value, handle: &ClaimHandle) -> Result<serde_json::Value, String> {
    let intent: SaveIntent = serde_json::from_value(args.clone()).map_err(|source| source.to_string())?;
    let claims = store.save(&intent, handle).map_err(|err| err.to_string())?;
    serde_json::to_value(claims).map_err(|err| err.to_string())
}

fn serve_recall(store: &mut dyn Store, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let scope = value_str(args, "scope")?;
    let query = value_str(args, "query")?;
    let limit = value_limit(args, "limit", DEFAULT_LIMIT);
    let claims = store.recall(&scope, &query, limit).map_err(|err| err.to_string())?;
    serde_json::to_value(claims).map_err(|err| err.to_string())
}

fn serve_decisions_query(store: &mut dyn Store, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let scope = value_str(args, "scope")?;
    let query = value_str(args, "query")?;
    let limit = value_limit(args, "limit", DEFAULT_LIMIT);
    let claims = store
        .decisions_query(&scope, &query, limit)
        .map_err(|err| err.to_string())?;
    serde_json::to_value(claims).map_err(|err| err.to_string())
}

fn serve_decision(store: &mut dyn Store, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let scope = value_str(args, "scope")?;
    let id = value_str(args, "id")?;
    let claim = store.decision(&scope, &id).map_err(|err| err.to_string())?;
    serde_json::to_value(claim).map_err(|err| err.to_string())
}

fn serve_drain(store: &mut dyn Store) -> Result<serde_json::Value, String> {
    let report = store.drain().map_err(|err| err.to_string())?;
    serde_json::to_value(report).map_err(|err| err.to_string())
}

pub fn serve(store: &mut dyn Store, verb: &str, args: &serde_json::Value, handle: &ClaimHandle) -> serde_json::Value {
    let result = match verb {
        "save" => serve_save(store, args, handle),
        "recall" => serve_recall(store, args),
        "decisions_query" => serve_decisions_query(store, args),
        "decision" => serve_decision(store, args),
        "drain" => serve_drain(store),
        other => Err(format!("unknown verb '{other}'")),
    };
    match result {
        Ok(value) => value,
        Err(message) => serde_json::json!({"error": message}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        let dir = std::env::temp_dir().join(format!("arx-engine-test-{}-{n}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn seed_claim(id: &str, question: &str, answer: &str, why: &str, label: Option<&str>) -> Claim {
        Claim {
            id: id.to_string(),
            kind: ClaimKind::Fact,
            label: label.map(|l| l.to_string()),
            question: question.to_string(),
            answer: answer.to_string(),
            why: why.to_string(),
            handle: ClaimHandle {
                thread: "main".to_string(),
                turn: 1,
            },
            lifecycle: Lifecycle::InForce,
            validity: Validity::Valid,
            supersedes: None,
            superseded_by: None,
            invalid_reason: None,
            created_at: 0,
        }
    }

    fn write_scope_file(root: &Path, scope: &str, claims: &[Claim]) {
        let path = root.join(format!("{scope}.jsonl"));
        let mut body = String::new();
        for claim in claims {
            body.push_str(&serde_json::to_string(claim).unwrap());
            body.push('\n');
        }
        fs::write(path, body).unwrap();
    }

    #[test]
    fn save_returns_landscape_and_lands_in_quarantine() {
        let root = temp_dir();
        let seeded = seed_claim("seed1", "what colour is the sky", "blue", "observed", None);
        write_scope_file(&root, "testscope", &[seeded.clone()]);
        let before = fs::read(root.join("testscope.jsonl")).unwrap();

        let mut store = FileStore::open(&root, "main").unwrap();
        let intent = SaveIntent {
            scope: "testscope".to_string(),
            kind: ClaimKind::Fact,
            question: "why is the sky blue".to_string(),
            answer: "scattering".to_string(),
            why: "physics".to_string(),
            supersedes: None,
        };
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 5,
        };
        let landscape = store.save(&intent, &handle).unwrap();

        assert_eq!(landscape.len(), 1);
        assert_eq!(landscape[0].id, "seed1");

        let candidate_dir = root.join("quarantine").join("main");
        let entries: Vec<_> = fs::read_dir(&candidate_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let candidate_path = entries.into_iter().next().unwrap().unwrap().path();
        let candidate: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&candidate_path).unwrap()).unwrap();
        assert_eq!(candidate["handle"]["thread"], "main");
        assert_eq!(candidate["handle"]["turn"], 5);
        assert!(candidate["id"].as_str().is_some());

        let after = fs::read(root.join("testscope.jsonl")).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn drain_promotes_in_order_and_clears_quarantine() {
        let root = temp_dir();
        let mut store = FileStore::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        store
            .save(
                &SaveIntent {
                    scope: "testscope".to_string(),
                    kind: ClaimKind::Fact,
                    question: "first question".to_string(),
                    answer: "first answer".to_string(),
                    why: "first why".to_string(),
                    supersedes: None,
                },
                &handle,
            )
            .unwrap();
        store
            .save(
                &SaveIntent {
                    scope: "testscope".to_string(),
                    kind: ClaimKind::Fact,
                    question: "second question".to_string(),
                    answer: "second answer".to_string(),
                    why: "second why".to_string(),
                    supersedes: None,
                },
                &handle,
            )
            .unwrap();

        let report = store.drain().unwrap();
        assert_eq!(report, DrainReport { promoted: 2, dropped: 0 });

        let claims = read_scope_file(&root.join("testscope.jsonl")).unwrap();
        assert_eq!(claims.len(), 2);
        assert_eq!(claims[0].question, "first question");
        assert_eq!(claims[1].question, "second question");

        let remaining: Vec<_> = fs::read_dir(root.join("quarantine").join("main")).unwrap().collect();
        assert!(remaining.is_empty());
    }

    #[test]
    fn drain_is_idempotent_when_a_row_is_already_in_the_scope_file() {
        let root = temp_dir();
        let quarantine_dir = root.join("quarantine").join("main");
        fs::create_dir_all(&quarantine_dir).unwrap();
        let stuck = QuarantineRow {
            id: "stuck1".to_string(),
            scope: "testscope".to_string(),
            kind: ClaimKind::Fact,
            question: "stuck question".to_string(),
            answer: "stuck answer".to_string(),
            why: "stuck why".to_string(),
            handle: ClaimHandle {
                thread: "main".to_string(),
                turn: 2,
            },
        };
        fs::write(
            quarantine_dir.join("1-stuck1.json"),
            serde_json::to_string(&stuck).unwrap(),
        )
        .unwrap();
        let already_promoted = Claim {
            id: "stuck1".to_string(),
            kind: ClaimKind::Fact,
            label: None,
            question: "stuck question".to_string(),
            answer: "stuck answer".to_string(),
            why: "stuck why".to_string(),
            handle: ClaimHandle {
                thread: "main".to_string(),
                turn: 2,
            },
            lifecycle: Lifecycle::InForce,
            validity: Validity::Valid,
            supersedes: None,
            superseded_by: None,
            invalid_reason: None,
            created_at: 0,
        };
        write_scope_file(&root, "testscope", &[already_promoted]);

        let mut store = FileStore::open(&root, "main").unwrap();
        let report = store.drain().unwrap();

        assert_eq!(report, DrainReport { promoted: 0, dropped: 0 });
        let claims = read_scope_file(&root.join("testscope.jsonl")).unwrap();
        assert_eq!(claims.len(), 1);
        let remaining: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
        assert!(remaining.is_empty());
    }

    #[test]
    fn read_scope_file_ignores_a_torn_trailing_row() {
        let root = temp_dir();
        let path = root.join("testscope.jsonl");
        let valid = seed_claim("valid1", "valid question", "valid answer", "valid why", None);
        let mut body = serde_json::to_string(&valid).unwrap();
        body.push('\n');
        body.push_str("{\"id\":\"torn1\",\"kind\":\"fact\",\"quest");
        fs::write(&path, body).unwrap();

        let claims = read_scope_file(&path).unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].id, "valid1");
    }

    #[test]
    fn drain_truncates_a_torn_tail_before_appending_and_stays_parseable() {
        let root = temp_dir();
        let path = root.join("testscope.jsonl");
        let valid = seed_claim("valid1", "valid question", "valid answer", "valid why", None);
        let mut body = serde_json::to_string(&valid).unwrap();
        body.push('\n');
        body.push_str("{\"id\":\"torn1\",\"kind\":\"fact\",\"quest");
        fs::write(&path, body).unwrap();

        let quarantine_dir = root.join("quarantine").join("main");
        fs::create_dir_all(&quarantine_dir).unwrap();
        let candidate = QuarantineRow {
            id: "candidate1".to_string(),
            scope: "testscope".to_string(),
            kind: ClaimKind::Fact,
            question: "candidate question".to_string(),
            answer: "candidate answer".to_string(),
            why: "candidate why".to_string(),
            handle: ClaimHandle {
                thread: "main".to_string(),
                turn: 3,
            },
        };
        fs::write(
            quarantine_dir.join("1-candidate1.json"),
            serde_json::to_string(&candidate).unwrap(),
        )
        .unwrap();

        let mut store = FileStore::open(&root, "main").unwrap();
        let report = store.drain().unwrap();
        assert_eq!(report, DrainReport { promoted: 1, dropped: 0 });

        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("torn1"));
        assert!(raw.trim_end().ends_with('}'));

        let claims = read_scope_file(&path).unwrap();
        assert_eq!(claims.len(), 2);
        assert_eq!(claims[0].id, "valid1");
        assert_eq!(claims[1].id, "candidate1");

        let recalled = store.recall("testscope", "candidate", 5).unwrap();
        assert_eq!(recalled.len(), 1);
        assert_eq!(recalled[0].id, "candidate1");
    }

    #[test]
    fn drain_rejects_an_invalid_scope_in_a_quarantine_row() {
        let root = temp_dir();
        let quarantine_dir = root.join("quarantine").join("main");
        fs::create_dir_all(&quarantine_dir).unwrap();
        let hostile = QuarantineRow {
            id: "hostile1".to_string(),
            scope: "../../outside".to_string(),
            kind: ClaimKind::Fact,
            question: "hostile question".to_string(),
            answer: "hostile answer".to_string(),
            why: "hostile why".to_string(),
            handle: ClaimHandle {
                thread: "main".to_string(),
                turn: 1,
            },
        };
        fs::write(
            quarantine_dir.join("1-hostile1.json"),
            serde_json::to_string(&hostile).unwrap(),
        )
        .unwrap();

        let mut store = FileStore::open(&root, "main").unwrap();
        let err = store.drain().unwrap_err();
        assert!(matches!(err, StoreError::BadScope { .. }));

        let outside_path = root.parent().unwrap().join("outside.jsonl");
        assert!(!outside_path.exists());
        let remaining: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn open_recovers_next_seq_from_existing_quarantine_files() {
        let root = temp_dir();
        let quarantine_dir = root.join("quarantine").join("main");
        fs::create_dir_all(&quarantine_dir).unwrap();
        fs::write(quarantine_dir.join("1-abc-1.json"), "{}").unwrap();
        fs::write(quarantine_dir.join("7-abc-7.json"), "{}").unwrap();
        fs::write(quarantine_dir.join("3-abc-3.json"), "{}").unwrap();

        let mut store = FileStore::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        store
            .save(
                &SaveIntent {
                    scope: "testscope".to_string(),
                    kind: ClaimKind::Fact,
                    question: "fresh question".to_string(),
                    answer: "fresh answer".to_string(),
                    why: "fresh why".to_string(),
                    supersedes: None,
                },
                &handle,
            )
            .unwrap();

        let entries: Vec<String> = fs::read_dir(&quarantine_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(entries.iter().any(|name| name.starts_with("8-")));
    }

    #[test]
    fn grounding_returns_hot_and_ember_only() {
        let root = temp_dir();
        let hot = seed_claim("hot1", "hot question", "hot answer", "hot why", Some("hot"));
        let ember = seed_claim("ember1", "ember question", "ember answer", "ember why", Some("ember"));
        let plain = seed_claim("plain1", "plain question", "plain answer", "plain why", None);
        write_scope_file(&root, "testscope", &[hot.clone(), ember.clone(), plain]);

        let mut store = FileStore::open(&root, "main").unwrap();
        let grounded = store.grounding("testscope").unwrap();

        assert_eq!(grounded.len(), 2);
        assert!(grounded.iter().any(|c| c.id == "hot1"));
        assert!(grounded.iter().any(|c| c.id == "ember1"));
    }

    #[test]
    fn recall_matches_substring_case_insensitive_ranked() {
        let root = temp_dir();
        let two_hits = seed_claim("two", "the sky is blue always", "clear", "why", None);
        let one_hit = seed_claim("one", "the sky at night", "dark", "why", None);
        let zero_hits = seed_claim("zero", "grass is green", "green", "why", None);
        write_scope_file(&root, "testscope", &[zero_hits, two_hits.clone(), one_hit.clone()]);

        let mut store = FileStore::open(&root, "main").unwrap();
        let hits = store.recall("testscope", "SKY BLUE", 5).unwrap();

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "two");
        assert_eq!(hits[1].id, "one");
    }

    #[test]
    fn decision_why_none_is_refused() {
        let root = temp_dir();
        let mut store = FileStore::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        for why in ["none", "None", "NONE"] {
            let err = store
                .save(
                    &SaveIntent {
                        scope: "testscope".to_string(),
                        kind: ClaimKind::Decision,
                        question: "q".to_string(),
                        answer: "a".to_string(),
                        why: why.to_string(),
                        supersedes: None,
                    },
                    &handle,
                )
                .unwrap_err();
            assert!(matches!(err, StoreError::WhyRefused));
        }
    }

    #[test]
    fn serve_unknown_verb_returns_error() {
        let root = temp_dir();
        let mut store = FileStore::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        let result = serve(&mut store, "bogus", &serde_json::json!({}), &handle);
        assert_eq!(result, serde_json::json!({"error": "unknown verb 'bogus'"}));
    }
}
