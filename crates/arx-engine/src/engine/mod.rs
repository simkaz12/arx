//! The second adapter behind `Store`: a git-backed root of per-claim markdown files,
//! BM25 recall, supersession and validity, embers, and the landscape.

mod bm25;
mod claim_file;
pub mod embed;
mod git;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    Claim, ClaimHandle, ClaimKind, DrainReport, Landscape, Lifecycle, RecallOpts, SaveIntent,
    Store, StoreError, Suggestion, SuggestionAction, Validity,
};

pub use embed::{EmbedError, Embedder, HashEmbedder, OllamaEmbedder};

const LEG_N: usize = 5;
const LANDSCAPE_CAP: usize = 10;
const RESERVED_SCOPES: [&str; 4] = ["quarantine", ".index", ".git", "embers"];

fn unix_seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn valid_scope_name(scope: &str) -> bool {
    !scope.is_empty() && scope.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
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

fn bm25_search_claims(claims: &[Claim], query: &str, limit: usize) -> Vec<Claim> {
    let mut ordered: Vec<Claim> = claims.to_vec();
    ordered.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    let docs: Vec<(String, String)> = ordered
        .iter()
        .map(|claim| (claim.id.clone(), format!("{} {} {}", claim.question, claim.answer, claim.why)))
        .collect();
    let index = bm25::Index::build(&docs);
    index
        .search(query, limit)
        .into_iter()
        .filter_map(|(id, _)| ordered.iter().find(|claim| claim.id == id).cloned())
        .collect()
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    supersedes: Option<String>,
    created_at: u64,
}

fn write_candidate(path: &Path, row: &QuarantineRow) -> Result<(), StoreError> {
    let mut buf = serde_json::to_string(row).expect("quarantine row always serializes");
    buf.push('\n');
    fs::write(path, buf).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_claim_file(path: &Path) -> Result<Claim, StoreError> {
    let text = fs::read_to_string(path).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    claim_file::parse(path, &text)
}

fn already_repaired_by(target: &Claim, promoted_id: &str) -> bool {
    target.lifecycle == Lifecycle::Superseded && target.superseded_by.as_deref() == Some(promoted_id)
}

fn accepts_supersession_from(target: &Claim, promoted_id: &str) -> bool {
    target.lifecycle == Lifecycle::InForce || already_repaired_by(target, promoted_id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorRow {
    id: String,
    model: String,
    v: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
}

fn embedded_text(claim: &Claim) -> String {
    format!("{} {}", claim.question, claim.answer)
}

fn content_hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn read_dir_sorted(dir: &Path) -> Result<Vec<fs::DirEntry>, StoreError> {
    let mut entries: Vec<fs::DirEntry> = Vec::new();
    for entry in fs::read_dir(dir).map_err(|source| StoreError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        entries.push(entry.map_err(|source| StoreError::Io {
            path: dir.to_path_buf(),
            source,
        })?);
    }
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
}

pub struct Engine {
    root: PathBuf,
    thread: String,
    next_seq: u64,
    cache: BTreeMap<String, Vec<Claim>>,
    embedder: Option<Box<dyn Embedder>>,
}

impl Engine {
    pub fn open(root: &Path, thread: &str) -> Result<Engine, StoreError> {
        fs::create_dir_all(root).map_err(|source| StoreError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        if !root.join(".git").exists() {
            git::init(root)?;
        }
        let quarantine_dir = root.join("quarantine").join(thread);
        fs::create_dir_all(&quarantine_dir).map_err(|source| StoreError::Io {
            path: quarantine_dir.clone(),
            source,
        })?;
        let next_seq = recover_next_seq(&quarantine_dir);
        Ok(Engine {
            root: root.to_path_buf(),
            thread: thread.to_string(),
            next_seq,
            cache: BTreeMap::new(),
            embedder: None,
        })
    }

    pub fn with_embedder(mut self, embedder: Box<dyn Embedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    fn quarantine_dir(&self) -> PathBuf {
        self.root.join("quarantine").join(&self.thread)
    }

    fn vector_index_path(&self, scope: &str) -> PathBuf {
        self.root.join(".index").join(format!("{scope}.vec.jsonl"))
    }

    fn load_vector_rows(&self, scope: &str) -> Vec<VectorRow> {
        let path = self.vector_index_path(scope);
        let Ok(text) = fs::read_to_string(&path) else {
            return Vec::new();
        };
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str::<VectorRow>(line).ok())
            .collect()
    }

    fn ensure_vectors(&mut self, scope: &str) -> Result<(), EmbedError> {
        if self.embedder.is_none() {
            return Ok(());
        }
        let model = self.embedder.as_ref().unwrap().model().to_string();
        let rows = self.load_vector_rows(scope);

        let claims = match self.load(scope) {
            Ok(claims) => claims,
            Err(_) => return Ok(()),
        };
        let candidates: Vec<Claim> = claims
            .into_iter()
            .filter(|claim| {
                matches!(claim.kind, ClaimKind::Fact | ClaimKind::Decision)
                    && claim.lifecycle == Lifecycle::InForce
                    && claim.validity == Validity::Valid
            })
            .collect();
        let current_hash: std::collections::HashMap<String, String> = candidates
            .iter()
            .map(|claim| (claim.id.clone(), content_hash(&embedded_text(claim))))
            .collect();

        let mut fresh: Vec<VectorRow> = Vec::new();
        let mut stale_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut retained_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for row in rows {
            if row.model != model {
                fresh.push(row);
                continue;
            }
            match current_hash.get(&row.id) {
                Some(hash) if row.hash.as_deref() == Some(hash.as_str()) => {
                    retained_ids.insert(row.id.clone());
                    fresh.push(row);
                }
                Some(_) => {
                    stale_ids.insert(row.id.clone());
                }
                None => fresh.push(row),
            }
        }

        let missing: Vec<Claim> = candidates
            .into_iter()
            .filter(|claim| stale_ids.contains(&claim.id) || !retained_ids.contains(&claim.id))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }

        let texts: Vec<String> = missing.iter().map(embedded_text).collect();
        let vectors = self.embedder.as_ref().unwrap().embed(&texts)?;

        for (claim, vector) in missing.iter().zip(vectors.into_iter()) {
            fresh.push(VectorRow {
                id: claim.id.clone(),
                model: model.clone(),
                v: vector,
                hash: Some(content_hash(&embedded_text(claim))),
            });
        }

        let index_path = self.vector_index_path(scope);
        if let Some(parent) = index_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut buf = String::new();
        for row in &fresh {
            buf.push_str(&serde_json::to_string(row).expect("vector row always serializes"));
            buf.push('\n');
        }
        let tmp_path = index_path.with_extension("jsonl.tmp");
        fs::write(&tmp_path, buf).map_err(|source| EmbedError { detail: source.to_string() })?;
        fs::rename(&tmp_path, &index_path).map_err(|source| EmbedError { detail: source.to_string() })?;
        Ok(())
    }

    fn require_scope(&self, scope: &str) -> Result<(), StoreError> {
        if !valid_scope_name(scope) || RESERVED_SCOPES.contains(&scope) {
            return Err(StoreError::BadScope {
                scope: scope.to_string(),
            });
        }
        if !self.root.join(scope).is_dir() {
            return Err(StoreError::NoScope {
                scope: scope.to_string(),
                root: self.root.clone(),
            });
        }
        Ok(())
    }

    fn load(&mut self, scope: &str) -> Result<Vec<Claim>, StoreError> {
        self.require_scope(scope)?;
        if let Some(cached) = self.cache.get(scope) {
            return Ok(cached.clone());
        }
        let scope_dir = self.root.join(scope);
        let mut claims = Vec::new();
        for entry in read_dir_sorted(&scope_dir)? {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                claims.push(read_claim_file(&path)?);
            }
        }
        let embers_dir = scope_dir.join("embers");
        if embers_dir.is_dir() {
            for entry in read_dir_sorted(&embers_dir)? {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                    claims.push(read_claim_file(&path)?);
                }
            }
        }
        self.cache.insert(scope.to_string(), claims.clone());
        Ok(claims)
    }

    fn invalidate_cache(&mut self, scope: &str) {
        self.cache.remove(scope);
    }

    pub fn invalidate(&mut self, scope: &str, id: &str, reason: &str) -> Result<Claim, StoreError> {
        self.require_scope(scope)?;
        if reason.trim().is_empty() {
            return Err(StoreError::MissingField {
                field: "reason".to_string(),
            });
        }
        let claims = self.load(scope)?;
        if !claims.iter().any(|claim| claim.id == id) {
            return Err(StoreError::UnknownClaim {
                scope: scope.to_string(),
                id: id.to_string(),
            });
        }
        let path = self.root.join(scope).join(format!("{id}.md"));
        let mut claim = read_claim_file(&path)?;
        claim.validity = Validity::Invalid;
        claim.invalid_reason = Some(reason.to_string());
        fs::write(&path, claim_file::render(&claim)).map_err(|source| StoreError::Io {
            path: path.clone(),
            source,
        })?;
        git::commit(&self.root, &[path], &format!("invalidate {scope}/{id}"))?;
        self.invalidate_cache(scope);
        Ok(claim)
    }

    pub fn ember(&mut self, scope: &str, text: &str, handle: &ClaimHandle) -> Result<Claim, StoreError> {
        self.require_scope(scope)?;
        if text.trim().is_empty() {
            return Err(StoreError::MissingField {
                field: "text".to_string(),
            });
        }
        let embers_dir = self.root.join(scope).join("embers");
        fs::create_dir_all(&embers_dir).map_err(|source| StoreError::Io {
            path: embers_dir.clone(),
            source,
        })?;
        let claim = Claim {
            id: Uuid::new_v4().to_string(),
            kind: ClaimKind::Ember,
            label: Some("ember".to_string()),
            question: String::new(),
            answer: text.to_string(),
            why: String::new(),
            handle: handle.clone(),
            lifecycle: Lifecycle::InForce,
            validity: Validity::Valid,
            supersedes: None,
            superseded_by: None,
            invalid_reason: None,
            created_at: unix_seconds_now(),
        };
        let path = embers_dir.join(format!("{}.md", claim.id));
        fs::write(&path, claim_file::render(&claim)).map_err(|source| StoreError::Io {
            path: path.clone(),
            source,
        })?;
        git::commit(&self.root, &[path], &format!("ember {scope}/{}", claim.id))?;
        self.invalidate_cache(scope);
        Ok(claim)
    }

    fn grounding_impl(&mut self, scope: &str) -> Result<Vec<Claim>, StoreError> {
        self.require_scope(scope)?;
        let claims = self.load(scope)?;
        let mut hot: Vec<Claim> = claims
            .iter()
            .filter(|claim| {
                matches!(claim.kind, ClaimKind::Fact | ClaimKind::Decision)
                    && claim.lifecycle == Lifecycle::InForce
                    && claim.validity == Validity::Valid
                    && claim.label.as_deref() == Some("hot")
            })
            .cloned()
            .collect();
        hot.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        let mut embers: Vec<Claim> = claims
            .iter()
            .filter(|claim| claim.kind == ClaimKind::Ember)
            .cloned()
            .collect();
        embers.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        hot.extend(embers);
        Ok(hot)
    }

    fn decision_impl(&mut self, scope: &str, id: &str) -> Result<Option<Claim>, StoreError> {
        self.require_scope(scope)?;
        let claims = self.load(scope)?;
        Ok(claims
            .into_iter()
            .find(|claim| claim.kind == ClaimKind::Decision && claim.id == id))
    }

    fn recall_with_impl(&mut self, scope: &str, query: &str, limit: usize, opts: &RecallOpts) -> Result<Vec<Claim>, StoreError> {
        self.require_scope(scope)?;
        let claims = self.load(scope)?;
        let filtered: Vec<Claim> = claims
            .into_iter()
            .filter(|claim| {
                matches!(claim.kind, ClaimKind::Fact | ClaimKind::Decision)
                    && (claim.lifecycle == Lifecycle::InForce || opts.include_superseded)
                    && (claim.validity == Validity::Valid || opts.include_invalid)
            })
            .collect();
        Ok(bm25_search_claims(&filtered, query, limit))
    }

    fn decisions_query_with_impl(&mut self, scope: &str, query: &str, limit: usize, opts: &RecallOpts) -> Result<Vec<Claim>, StoreError> {
        self.require_scope(scope)?;
        let claims = self.load(scope)?;
        let filtered: Vec<Claim> = claims
            .into_iter()
            .filter(|claim| {
                claim.kind == ClaimKind::Decision
                    && (claim.lifecycle == Lifecycle::InForce || opts.include_superseded)
                    && (claim.validity == Validity::Valid || opts.include_invalid)
            })
            .collect();
        Ok(bm25_search_claims(&filtered, query, limit))
    }

    fn save_with_impl(&mut self, intent: &SaveIntent, handle: &ClaimHandle) -> Result<Landscape, StoreError> {
        self.require_scope(&intent.scope)?;
        if intent.question.trim().is_empty() {
            return Err(StoreError::MissingField {
                field: "question".to_string(),
            });
        }
        if intent.answer.trim().is_empty() {
            return Err(StoreError::MissingField {
                field: "answer".to_string(),
            });
        }
        if intent.why.trim().is_empty() {
            return Err(StoreError::MissingField {
                field: "why".to_string(),
            });
        }
        if intent.kind == ClaimKind::Decision && intent.why.trim().eq_ignore_ascii_case("none") {
            return Err(StoreError::WhyRefused);
        }

        let claims = self.load(&intent.scope)?;

        if let Some(target_id) = &intent.supersedes {
            let target = claims
                .iter()
                .find(|claim| &claim.id == target_id && matches!(claim.kind, ClaimKind::Fact | ClaimKind::Decision));
            match target {
                None => {
                    return Err(StoreError::UnknownClaim {
                        scope: intent.scope.clone(),
                        id: target_id.clone(),
                    });
                }
                Some(claim) if claim.lifecycle != Lifecycle::InForce => {
                    return Err(StoreError::NotSupersedable {
                        scope: intent.scope.clone(),
                        id: target_id.clone(),
                    });
                }
                _ => {}
            }
        }

        let seq = self.next_seq;
        self.next_seq += 1;
        let row = QuarantineRow {
            id: Uuid::new_v4().to_string(),
            scope: intent.scope.clone(),
            kind: intent.kind,
            question: intent.question.clone(),
            answer: intent.answer.clone(),
            why: intent.why.clone(),
            handle: handle.clone(),
            supersedes: intent.supersedes.clone(),
            created_at: unix_seconds_now(),
        };
        let quarantine_dir = self.quarantine_dir();
        fs::create_dir_all(&quarantine_dir).map_err(|source| StoreError::Io {
            path: quarantine_dir.clone(),
            source,
        })?;
        let candidate_path = quarantine_dir.join(format!("{seq}-{}.json", row.id));
        write_candidate(&candidate_path, &row)?;

        self.landscape_for(&intent.scope, &intent.question, &intent.answer)
    }

    fn landscape_for(&mut self, scope: &str, question: &str, answer: &str) -> Result<Landscape, StoreError> {
        self.require_scope(scope)?;
        let claims = self.load(scope)?;
        let candidates: Vec<Claim> = claims
            .into_iter()
            .filter(|claim| {
                matches!(claim.kind, ClaimKind::Fact | ClaimKind::Decision)
                    && claim.lifecycle == Lifecycle::InForce
                    && claim.validity == Validity::Valid
            })
            .collect();
        let query_text = format!("{} {}", question, answer);
        let landscape_claims = bm25_search_claims(&candidates, &query_text, LEG_N);

        let mut legs = vec!["bm25".to_string()];
        let mut degraded: Option<String> = None;
        let mut cosine_claims: Vec<Claim> = Vec::new();

        if self.embedder.is_some() {
            match self.ensure_vectors(scope) {
                Ok(()) => match self.embedder.as_ref().unwrap().embed(&[query_text.clone()]) {
                    Ok(mut vectors) => {
                        let query_vector = vectors.pop().unwrap_or_default();
                        let model = self.embedder.as_ref().unwrap().model().to_string();
                        let rows = self.load_vector_rows(scope);
                        let mut scored: Vec<(Claim, f32)> = rows
                            .iter()
                            .filter(|row| row.model == model)
                            .filter_map(|row| {
                                candidates
                                    .iter()
                                    .find(|claim| claim.id == row.id)
                                    .map(|claim| (claim.clone(), dot(&query_vector, &row.v)))
                            })
                            .collect();
                        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                        cosine_claims = scored.into_iter().take(LEG_N).map(|(claim, _)| claim).collect();
                        legs.insert(0, "cosine".to_string());
                    }
                    Err(err) => degraded = Some(err.detail),
                },
                Err(err) => degraded = Some(err.detail),
            }
        }

        let mut union: Vec<Claim> = Vec::new();
        for claim in cosine_claims.into_iter().chain(landscape_claims.into_iter()) {
            if !union.iter().any(|existing: &Claim| existing.id == claim.id) {
                union.push(claim);
            }
        }
        union.truncate(LANDSCAPE_CAP);

        Ok(Landscape {
            claims: union,
            legs,
            degraded,
        })
    }

    fn read_quarantine_candidates(&self) -> Result<Vec<(PathBuf, QuarantineRow)>, StoreError> {
        let quarantine_dir = self.quarantine_dir();
        let mut entries: Vec<(u64, PathBuf)> = Vec::new();
        if quarantine_dir.exists() {
            for entry in fs::read_dir(&quarantine_dir).map_err(|source| StoreError::Io {
                path: quarantine_dir.clone(),
                source,
            })? {
                let entry = entry.map_err(|source| StoreError::Io {
                    path: quarantine_dir.clone(),
                    source,
                })?;
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                let seq = match stem.split('-').next().and_then(|s| s.parse::<u64>().ok()) {
                    Some(seq) => seq,
                    None => continue,
                };
                entries.push((seq, path));
            }
        }
        entries.sort_by_key(|(seq, _)| *seq);

        let mut candidates: Vec<(PathBuf, QuarantineRow)> = Vec::new();
        for (_, path) in &entries {
            let text = fs::read_to_string(&path).map_err(|source| StoreError::Io {
                path: path.clone(),
                source,
            })?;
            let row: QuarantineRow = serde_json::from_str(&text).map_err(|err| StoreError::MalformedClaimFile {
                path: path.clone(),
                detail: err.to_string(),
            })?;
            candidates.push((path.clone(), row));
        }
        Ok(candidates)
    }

    fn resolve_drain_plan(
        candidates: Vec<(PathBuf, QuarantineRow)>,
        suggestions: Option<&[Suggestion]>,
    ) -> Result<Vec<CandidatePlan>, StoreError> {
        match suggestions {
            None => Ok(candidates
                .into_iter()
                .map(|(path, row)| {
                    let supersedes = row.supersedes.clone();
                    CandidatePlan {
                        path,
                        row,
                        action: PlannedAction::Promote {
                            supersedes,
                            from_suggestion: false,
                        },
                    }
                })
                .collect()),
            Some(list) => {
                let mut by_id: BTreeMap<&str, &Suggestion> = BTreeMap::new();
                for suggestion in list {
                    if by_id.insert(&suggestion.id, suggestion).is_some() {
                        return Err(StoreError::UnsupportedSuggestion {
                            detail: format!("duplicate suggestion for candidate '{}'", suggestion.id),
                        });
                    }
                }
                let candidate_ids: BTreeSet<&str> = candidates.iter().map(|(_, row)| row.id.as_str()).collect();
                for suggestion in list {
                    if !candidate_ids.contains(suggestion.id.as_str()) {
                        return Err(StoreError::UnsupportedSuggestion {
                            detail: format!("no quarantined candidate '{}'", suggestion.id),
                        });
                    }
                    if suggestion.action == SuggestionAction::Drop && suggestion.supersedes.is_some() {
                        return Err(StoreError::UnsupportedSuggestion {
                            detail: format!("drop for candidate '{}' names supersedes", suggestion.id),
                        });
                    }
                }

                let mut plans = Vec::with_capacity(candidates.len());
                for (path, row) in candidates {
                    let Some(suggestion) = by_id.get(row.id.as_str()) else {
                        return Err(StoreError::UncoveredCandidate { id: row.id.clone() });
                    };
                    let action = match suggestion.action {
                        SuggestionAction::Promote => match &suggestion.supersedes {
                            Some(target) => PlannedAction::Promote {
                                supersedes: Some(target.clone()),
                                from_suggestion: true,
                            },
                            None => PlannedAction::Promote {
                                supersedes: row.supersedes.clone(),
                                from_suggestion: false,
                            },
                        },
                        SuggestionAction::Drop => PlannedAction::Drop,
                    };
                    plans.push(CandidatePlan { path, row, action });
                }
                Ok(plans)
            }
        }
    }

    fn validate_drain_plan(&mut self, plans: &[CandidatePlan]) -> Result<(), StoreError> {
        let mut supersession_targets: BTreeSet<(String, String)> = BTreeSet::new();
        for plan in plans {
            let PlannedAction::Promote { supersedes: Some(target_id), .. } = &plan.action else {
                continue;
            };
            if !supersession_targets.insert((plan.row.scope.clone(), target_id.clone())) {
                return Err(StoreError::UnsupportedSuggestion {
                    detail: format!("duplicate supersession target '{target_id}' in scope '{}'", plan.row.scope),
                });
            }
        }

        for plan in plans {
            self.require_scope(&plan.row.scope).map_err(|err| StoreError::MalformedClaimFile {
                path: plan.path.clone(),
                detail: err.to_string(),
            })?;
            let PlannedAction::Promote { supersedes, from_suggestion } = &plan.action else {
                continue;
            };
            for (field, value) in [("question", &plan.row.question), ("answer", &plan.row.answer), ("why", &plan.row.why)] {
                if value.trim().is_empty() {
                    return Err(StoreError::MalformedClaimFile {
                        path: plan.path.clone(),
                        detail: format!("missing field '{field}'"),
                    });
                }
            }
            let Some(target_id) = supersedes else {
                continue;
            };
            let claims = self.load(&plan.row.scope)?;
            if *from_suggestion {
                let target = claims
                    .iter()
                    .find(|claim| &claim.id == target_id && matches!(claim.kind, ClaimKind::Fact | ClaimKind::Decision));
                match target {
                    None => {
                        return Err(StoreError::UnknownClaim {
                            scope: plan.row.scope.clone(),
                            id: target_id.clone(),
                        });
                    }
                    Some(claim) if !accepts_supersession_from(claim, &plan.row.id) => {
                        return Err(StoreError::NotSupersedable {
                            scope: plan.row.scope.clone(),
                            id: target_id.clone(),
                        });
                    }
                    _ => {}
                }
            } else {
                let target = claims.iter().find(|claim| &claim.id == target_id);
                match target {
                    None => {
                        return Err(StoreError::MalformedClaimFile {
                            path: plan.path.clone(),
                            detail: format!("supersedes target '{target_id}' not found in scope '{}'", plan.row.scope),
                        });
                    }
                    Some(claim) if !accepts_supersession_from(claim, &plan.row.id) => {
                        return Err(StoreError::MalformedClaimFile {
                            path: plan.path.clone(),
                            detail: format!("supersedes target '{target_id}' is already superseded"),
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn apply_drain_plan(&mut self, plans: Vec<CandidatePlan>, subject: impl Fn(usize, usize) -> String) -> Result<DrainReport, StoreError> {
        let mut touched: Vec<PathBuf> = Vec::new();
        let mut to_delete: Vec<PathBuf> = Vec::new();
        let mut promoted = 0usize;
        let mut dropped = 0usize;
        let mut touched_scopes: Vec<String> = Vec::new();

        for plan in &plans {
            let PlannedAction::Promote { supersedes, .. } = &plan.action else {
                dropped += 1;
                to_delete.push(plan.path.clone());
                continue;
            };
            let claim_path = self.root.join(&plan.row.scope).join(format!("{}.md", plan.row.id));
            if claim_path.exists() {
                if git::is_uncommitted(&self.root, &claim_path)? {
                    touched.push(claim_path.clone());
                    touched_scopes.push(plan.row.scope.clone());
                }
            } else {
                let claim = Claim {
                    id: plan.row.id.clone(),
                    kind: plan.row.kind,
                    label: None,
                    question: plan.row.question.clone(),
                    answer: plan.row.answer.clone(),
                    why: plan.row.why.clone(),
                    handle: plan.row.handle.clone(),
                    lifecycle: Lifecycle::InForce,
                    validity: Validity::Valid,
                    supersedes: supersedes.clone(),
                    superseded_by: None,
                    invalid_reason: None,
                    created_at: plan.row.created_at,
                };
                fs::write(&claim_path, claim_file::render(&claim)).map_err(|source| StoreError::Io {
                    path: claim_path.clone(),
                    source,
                })?;
                touched.push(claim_path.clone());
                touched_scopes.push(plan.row.scope.clone());
            }

            if let Some(target_id) = supersedes {
                let target_path = self.root.join(&plan.row.scope).join(format!("{target_id}.md"));
                let target_claim = read_claim_file(&target_path)?;
                let already_repaired = already_repaired_by(&target_claim, &plan.row.id);
                if already_repaired {
                    if git::is_uncommitted(&self.root, &target_path)? {
                        touched.push(target_path);
                    }
                } else {
                    let mut repaired = target_claim;
                    repaired.lifecycle = Lifecycle::Superseded;
                    repaired.superseded_by = Some(plan.row.id.clone());
                    fs::write(&target_path, claim_file::render(&repaired)).map_err(|source| StoreError::Io {
                        path: target_path.clone(),
                        source,
                    })?;
                    touched.push(target_path);
                }
            }

            promoted += 1;
            to_delete.push(plan.path.clone());
        }

        if !touched.is_empty() {
            git::commit(&self.root, &touched, &subject(promoted, dropped))?;
        }

        for path in &to_delete {
            fs::remove_file(path).map_err(|source| StoreError::Io {
                path: path.clone(),
                source,
            })?;
        }

        touched_scopes.sort();
        touched_scopes.dedup();
        for scope in &touched_scopes {
            self.invalidate_cache(scope);
        }
        if self.embedder.is_some() {
            for scope in &touched_scopes {
                let _ = self.ensure_vectors(scope);
            }
        }

        Ok(DrainReport { promoted, dropped })
    }

    fn drain_impl(&mut self) -> Result<DrainReport, StoreError> {
        let candidates = self.read_quarantine_candidates()?;
        let plans = Self::resolve_drain_plan(candidates, None)?;
        self.validate_drain_plan(&plans)?;
        let thread = self.thread.clone();
        self.apply_drain_plan(plans, move |promoted, _dropped| format!("drain {thread}: promoted {promoted}"))
    }

    fn drain_with_impl(&mut self, suggestions: Option<&[Suggestion]>, handle: &ClaimHandle) -> Result<DrainReport, StoreError> {
        let candidates = self.read_quarantine_candidates()?;
        let plans = Self::resolve_drain_plan(candidates, suggestions)?;
        self.validate_drain_plan(&plans)?;
        let thread = handle.thread.clone();
        let turn = handle.turn;
        self.apply_drain_plan(plans, move |promoted, dropped| {
            format!("drain {thread}@{turn}: promoted {promoted}, dropped {dropped}")
        })
    }
}

enum PlannedAction {
    Promote { supersedes: Option<String>, from_suggestion: bool },
    Drop,
}

struct CandidatePlan {
    path: PathBuf,
    row: QuarantineRow,
    action: PlannedAction,
}

impl Store for Engine {
    fn save(&mut self, intent: &SaveIntent, handle: &ClaimHandle) -> Result<Vec<Claim>, StoreError> {
        Ok(self.save_with(intent, handle)?.claims)
    }

    fn recall(&mut self, scope: &str, query: &str, limit: usize) -> Result<Vec<Claim>, StoreError> {
        self.recall_with(scope, query, limit, &RecallOpts::default())
    }

    fn decisions_query(&mut self, scope: &str, query: &str, limit: usize) -> Result<Vec<Claim>, StoreError> {
        self.decisions_query_with(scope, query, limit, &RecallOpts::default())
    }

    fn decision(&mut self, scope: &str, id: &str) -> Result<Option<Claim>, StoreError> {
        self.decision_impl(scope, id)
    }

    fn drain(&mut self) -> Result<DrainReport, StoreError> {
        self.drain_impl()
    }

    fn drain_with(&mut self, suggestions: Option<&[Suggestion]>, handle: &ClaimHandle) -> Result<DrainReport, StoreError> {
        self.drain_with_impl(suggestions, handle)
    }

    fn grounding(&mut self, scope: &str) -> Result<Vec<Claim>, StoreError> {
        self.grounding_impl(scope)
    }

    fn save_with(&mut self, intent: &SaveIntent, handle: &ClaimHandle) -> Result<Landscape, StoreError> {
        self.save_with_impl(intent, handle)
    }

    fn recall_with(&mut self, scope: &str, query: &str, limit: usize, opts: &RecallOpts) -> Result<Vec<Claim>, StoreError> {
        self.recall_with_impl(scope, query, limit, opts)
    }

    fn decisions_query_with(&mut self, scope: &str, query: &str, limit: usize, opts: &RecallOpts) -> Result<Vec<Claim>, StoreError> {
        self.decisions_query_with_impl(scope, query, limit, opts)
    }

    fn landscape(&mut self, scope: &str, question: &str, answer: &str) -> Result<Landscape, StoreError> {
        self.landscape_for(scope, question, answer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_root() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        std::env::temp_dir().join(format!("arx-engine-store-{}-{n}-{nanos}", std::process::id()))
    }

    fn scoped_root(scopes: &[&str]) -> PathBuf {
        let root = temp_root();
        for scope in scopes {
            fs::create_dir_all(root.join(scope)).unwrap();
        }
        root
    }

    fn basic_intent(scope: &str, question: &str, answer: &str, why: &str) -> SaveIntent {
        SaveIntent {
            scope: scope.to_string(),
            kind: ClaimKind::Fact,
            question: question.to_string(),
            answer: answer.to_string(),
            why: why.to_string(),
            supersedes: None,
        }
    }

    #[test]
    fn open_initialises_a_git_repo_with_gitignore() {
        let root = temp_root();
        let engine = Engine::open(&root, "main").unwrap();
        assert!(root.join(".git").is_dir());
        assert_eq!(git::head_count(&root).unwrap(), 1);
        let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert_eq!(gitignore, "quarantine/\n.index/\n");
        drop(engine);

        let _second = Engine::open(&root, "main").unwrap();
        assert_eq!(git::head_count(&root).unwrap(), 1);
    }

    #[test]
    fn save_writes_only_quarantine() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let before_head = git::head_count(&root).unwrap();
        let before_entries: Vec<_> = fs::read_dir(root.join("testscope")).unwrap().collect();

        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        let landscape = engine
            .save_with(&basic_intent("testscope", "why is the sky blue", "scattering", "physics"), &handle)
            .unwrap();
        assert!(landscape.claims.is_empty());
        assert_eq!(landscape.legs, vec!["bm25".to_string()]);
        assert_eq!(landscape.degraded, None);

        let after_entries: Vec<_> = fs::read_dir(root.join("testscope")).unwrap().collect();
        assert_eq!(before_entries.len(), after_entries.len());
        assert_eq!(git::head_count(&root).unwrap(), before_head);

        let candidates: Vec<_> = fs::read_dir(root.join("quarantine").join("main")).unwrap().collect();
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn drain_promotes_as_one_commit_and_clears_quarantine() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        engine
            .save(&basic_intent("testscope", "first question", "first answer", "first why"), &handle)
            .unwrap();
        engine
            .save(&basic_intent("testscope", "second question", "second answer", "second why"), &handle)
            .unwrap();

        let before_head = git::head_count(&root).unwrap();
        let report = engine.drain().unwrap();
        assert_eq!(report.promoted, 2);
        assert_eq!(git::head_count(&root).unwrap(), before_head + 1);

        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .arg("show")
            .arg("--name-only")
            .arg("--format=")
            .arg("HEAD")
            .output()
            .unwrap();
        let mut listed: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect();
        listed.sort();

        let mut claim_files: Vec<String> = fs::read_dir(root.join("testscope"))
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
            .map(|entry| format!("testscope/{}", entry.file_name().to_string_lossy()))
            .collect();
        claim_files.sort();
        assert_eq!(listed, claim_files);

        let remaining: Vec<_> = fs::read_dir(root.join("quarantine").join("main")).unwrap().collect();
        assert!(remaining.is_empty());
    }

    #[test]
    fn drain_is_all_or_nothing() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        engine
            .save(&basic_intent("testscope", "good question", "good answer", "good why"), &handle)
            .unwrap();

        let quarantine_dir = root.join("quarantine").join("main");
        let bad_row = QuarantineRow {
            id: Uuid::new_v4().to_string(),
            scope: "testscope".to_string(),
            kind: ClaimKind::Fact,
            question: "bad question".to_string(),
            answer: "bad answer".to_string(),
            why: "   ".to_string(),
            handle: handle.clone(),
            supersedes: None,
            created_at: 1,
        };
        let bad_path = quarantine_dir.join(format!("99-{}.json", bad_row.id));
        fs::write(&bad_path, serde_json::to_string(&bad_row).unwrap()).unwrap();

        let before_head = git::head_count(&root).unwrap();
        let err = engine.drain().unwrap_err();
        match err {
            StoreError::MalformedClaimFile { path, .. } => assert_eq!(path, bad_path),
            other => panic!("expected MalformedClaimFile naming {bad_path:?}, got {other:?}"),
        }

        assert_eq!(git::head_count(&root).unwrap(), before_head);
        let remaining: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
        assert_eq!(remaining.len(), 2);
        let claim_files: Vec<_> = fs::read_dir(root.join("testscope"))
            .unwrap()
            .filter(|entry| {
                entry.as_ref().unwrap().path().extension().and_then(|ext| ext.to_str()) == Some("md")
            })
            .collect();
        assert!(claim_files.is_empty());
    }

    #[test]
    fn drain_repairs_predecessor_after_successor_survives_a_partial_crash() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };

        engine
            .save(&basic_intent("testscope", "predecessor question", "predecessor answer", "predecessor why"), &handle)
            .unwrap();
        engine.drain().unwrap();
        let predecessor_id = engine.recall("testscope", "predecessor question", 1).unwrap()[0].id.clone();

        engine
            .save(
                &SaveIntent {
                    scope: "testscope".to_string(),
                    kind: ClaimKind::Fact,
                    question: "predecessor question".to_string(),
                    answer: "successor answer".to_string(),
                    why: "successor why".to_string(),
                    supersedes: Some(predecessor_id.clone()),
                },
                &handle,
            )
            .unwrap();

        let candidates = engine.read_quarantine_candidates().unwrap();
        let (candidate_path, row) = candidates
            .into_iter()
            .find(|(_, row)| row.answer == "successor answer")
            .expect("the supersede candidate is quarantined");

        let successor_path = root.join("testscope").join(format!("{}.md", row.id));
        let crashed_write = Claim {
            id: row.id.clone(),
            kind: row.kind,
            label: None,
            question: row.question.clone(),
            answer: row.answer.clone(),
            why: row.why.clone(),
            handle: row.handle.clone(),
            lifecycle: Lifecycle::InForce,
            validity: Validity::Valid,
            supersedes: row.supersedes.clone(),
            superseded_by: None,
            invalid_reason: None,
            created_at: row.created_at,
        };
        fs::write(&successor_path, claim_file::render(&crashed_write)).unwrap();

        let report = engine.drain().unwrap();

        let opts = RecallOpts {
            include_superseded: true,
            include_invalid: false,
        };
        let predecessor = engine
            .recall_with("testscope", "predecessor question", 5, &opts)
            .unwrap()
            .into_iter()
            .find(|claim| claim.id == predecessor_id)
            .expect("predecessor still recallable");
        assert_eq!(predecessor.lifecycle, Lifecycle::Superseded, "predecessor must come back superseded, not left in force");
        assert_eq!(predecessor.superseded_by.as_deref(), Some(row.id.as_str()));

        assert!(!candidate_path.exists(), "quarantine row should be gone once the repair actually completed");
        let remaining: Vec<_> = fs::read_dir(root.join("quarantine").join("main")).unwrap().collect();
        assert!(remaining.is_empty());

        assert_eq!(report.promoted, 1);
    }

    #[test]
    fn drain_keeps_quarantine_row_when_commit_fails_then_recovers_on_retry() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        engine
            .save(&basic_intent("testscope", "lockstep question", "lockstep answer", "lockstep why"), &handle)
            .unwrap();

        let quarantine_dir = root.join("quarantine").join("main");
        assert_eq!(fs::read_dir(&quarantine_dir).unwrap().count(), 1);

        let lock_path = root.join(".git").join("index.lock");
        fs::write(&lock_path, b"").unwrap();

        let before_head = git::head_count(&root).unwrap();
        let err = engine.drain().unwrap_err();
        assert!(matches!(err, StoreError::Git { .. }), "expected StoreError::Git, got {err:?}");
        assert_eq!(git::head_count(&root).unwrap(), before_head);
        assert_eq!(fs::read_dir(&quarantine_dir).unwrap().count(), 1, "quarantine row must survive a failed commit");

        fs::remove_file(&lock_path).unwrap();

        let report = engine.drain().unwrap();
        assert_eq!(report.promoted, 1);
        assert_eq!(git::head_count(&root).unwrap(), before_head + 1);
        assert_eq!(fs::read_dir(&quarantine_dir).unwrap().count(), 0);
    }

    fn quarantined_supersede_candidate(
        engine: &mut Engine,
        handle: &ClaimHandle,
        predecessor_id: &str,
    ) -> (PathBuf, QuarantineRow) {
        engine
            .save(
                &SaveIntent {
                    scope: "testscope".to_string(),
                    kind: ClaimKind::Fact,
                    question: "predecessor question".to_string(),
                    answer: "successor answer".to_string(),
                    why: "successor why".to_string(),
                    supersedes: Some(predecessor_id.to_string()),
                },
                handle,
            )
            .unwrap();
        engine
            .read_quarantine_candidates()
            .unwrap()
            .into_iter()
            .find(|(_, row)| row.answer == "successor answer")
            .expect("the supersede candidate is quarantined")
    }

    fn land_predecessor_rewrite_on_disk(root: &Path, row: &QuarantineRow, predecessor_id: &str) -> (PathBuf, PathBuf) {
        let successor_path = root.join("testscope").join(format!("{}.md", row.id));
        let successor_claim = Claim {
            id: row.id.clone(),
            kind: row.kind,
            label: None,
            question: row.question.clone(),
            answer: row.answer.clone(),
            why: row.why.clone(),
            handle: row.handle.clone(),
            lifecycle: Lifecycle::InForce,
            validity: Validity::Valid,
            supersedes: row.supersedes.clone(),
            superseded_by: None,
            invalid_reason: None,
            created_at: row.created_at,
        };
        fs::write(&successor_path, claim_file::render(&successor_claim)).unwrap();

        let predecessor_path = root.join("testscope").join(format!("{predecessor_id}.md"));
        let mut repaired_predecessor = read_claim_file(&predecessor_path).unwrap();
        repaired_predecessor.lifecycle = Lifecycle::Superseded;
        repaired_predecessor.superseded_by = Some(row.id.clone());
        fs::write(&predecessor_path, claim_file::render(&repaired_predecessor)).unwrap();

        (successor_path, predecessor_path)
    }

    #[test]
    fn drain_recovers_when_predecessor_rewrite_landed_before_the_commit() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };

        engine
            .save(&basic_intent("testscope", "predecessor question", "predecessor answer", "predecessor why"), &handle)
            .unwrap();
        engine.drain().unwrap();
        let predecessor_id = engine.recall("testscope", "predecessor question", 1).unwrap()[0].id.clone();

        let (candidate_path, row) = quarantined_supersede_candidate(&mut engine, &handle, &predecessor_id);
        land_predecessor_rewrite_on_disk(&root, &row, &predecessor_id);
        assert!(candidate_path.exists(), "quarantine row must still be present: the crash is before deletion");

        drop(engine);
        let mut engine = Engine::open(&root, "main").unwrap();
        let before_head = git::head_count(&root).unwrap();

        let report = engine.drain().unwrap();
        assert_eq!(report.promoted, 1);
        assert_eq!(
            git::head_count(&root).unwrap(),
            before_head + 1,
            "the landed rewrite was never committed, so the retry still owes one commit"
        );
        assert!(!candidate_path.exists(), "quarantine row should be gone once the retry completes");

        let opts = RecallOpts {
            include_superseded: true,
            include_invalid: false,
        };
        let predecessor = engine
            .recall_with("testscope", "predecessor question", 5, &opts)
            .unwrap()
            .into_iter()
            .find(|claim| claim.id == predecessor_id)
            .expect("predecessor still recallable");
        assert_eq!(predecessor.lifecycle, Lifecycle::Superseded);
        assert_eq!(predecessor.superseded_by.as_deref(), Some(row.id.as_str()));

        engine
            .save(&basic_intent("testscope", "later question", "later answer", "later why"), &handle)
            .unwrap();
        let later_report = engine.drain().unwrap();
        assert_eq!(later_report.promoted, 1, "a later drain for the same thread must not stay blocked by the recovered candidate");
    }

    #[test]
    fn drain_recovers_when_predecessor_rewrite_landed_after_the_commit_but_before_the_delete() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };

        engine
            .save(&basic_intent("testscope", "predecessor question", "predecessor answer", "predecessor why"), &handle)
            .unwrap();
        engine.drain().unwrap();
        let predecessor_id = engine.recall("testscope", "predecessor question", 1).unwrap()[0].id.clone();

        let (candidate_path, row) = quarantined_supersede_candidate(&mut engine, &handle, &predecessor_id);
        let (successor_path, predecessor_path) = land_predecessor_rewrite_on_disk(&root, &row, &predecessor_id);
        git::commit(&root, &[successor_path, predecessor_path], "simulated crash: commit landed, delete did not").unwrap();
        assert!(candidate_path.exists(), "quarantine row must still be present: the crash is before deletion");

        drop(engine);
        let mut engine = Engine::open(&root, "main").unwrap();
        let before_head = git::head_count(&root).unwrap();

        let report = engine.drain().unwrap();
        assert_eq!(report.promoted, 1);
        assert_eq!(
            git::head_count(&root).unwrap(),
            before_head,
            "the rewrite was already committed before the crash, so the retry owes no new commit"
        );
        assert!(!candidate_path.exists(), "quarantine row should be gone once the retry completes");

        let opts = RecallOpts {
            include_superseded: true,
            include_invalid: false,
        };
        let predecessor = engine
            .recall_with("testscope", "predecessor question", 5, &opts)
            .unwrap()
            .into_iter()
            .find(|claim| claim.id == predecessor_id)
            .expect("predecessor still recallable");
        assert_eq!(predecessor.lifecycle, Lifecycle::Superseded);
        assert_eq!(predecessor.superseded_by.as_deref(), Some(row.id.as_str()));

        engine
            .save(&basic_intent("testscope", "later question", "later answer", "later why"), &handle)
            .unwrap();
        let later_report = engine.drain().unwrap();
        assert_eq!(later_report.promoted, 1, "a later drain for the same thread must not stay blocked by the recovered candidate");
    }

    #[test]
    fn drain_rejects_retry_when_predecessor_was_repaired_by_a_different_successor() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };

        engine
            .save(&basic_intent("testscope", "predecessor question", "predecessor answer", "predecessor why"), &handle)
            .unwrap();
        engine.drain().unwrap();
        let predecessor_id = engine.recall("testscope", "predecessor question", 1).unwrap()[0].id.clone();

        let (candidate_path, row) = quarantined_supersede_candidate(&mut engine, &handle, &predecessor_id);
        let mut other_row = row.clone();
        other_row.id = Uuid::new_v4().to_string();
        land_predecessor_rewrite_on_disk(&root, &other_row, &predecessor_id);
        assert!(candidate_path.exists(), "quarantine row must still be present: no drain has run yet");

        drop(engine);
        let mut engine = Engine::open(&root, "main").unwrap();
        let before_head = git::head_count(&root).unwrap();

        let err = engine.drain().unwrap_err();
        match err {
            StoreError::MalformedClaimFile { path, detail } => {
                assert_eq!(path, candidate_path);
                assert!(
                    detail.contains(&format!("supersedes target '{predecessor_id}' is already superseded")),
                    "expected rejection naming the still-mismatched predecessor, got: {detail}"
                );
            }
            other => panic!("expected MalformedClaimFile rejecting the mismatched supersession, got {other:?}"),
        }

        assert_eq!(git::head_count(&root).unwrap(), before_head, "a rejected drain must not commit anything");
        assert!(candidate_path.exists(), "a rejected drain must leave the quarantine candidate untouched");

        let opts = RecallOpts {
            include_superseded: true,
            include_invalid: false,
        };
        let predecessor = engine
            .recall_with("testscope", "predecessor question", 5, &opts)
            .unwrap()
            .into_iter()
            .find(|claim| claim.id == predecessor_id)
            .expect("predecessor still recallable");
        assert_eq!(predecessor.lifecycle, Lifecycle::Superseded);
        assert_eq!(
            predecessor.superseded_by.as_deref(),
            Some(other_row.id.as_str()),
            "the predecessor must still name the successor that actually repaired it, not the retried candidate"
        );
    }

    #[test]
    fn ember_writes_commits_and_grounds() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        let before_head = git::head_count(&root).unwrap();
        let claim = engine.ember("testscope", "the owner said ship it", &handle).unwrap();
        assert_eq!(claim.kind, ClaimKind::Ember);
        assert_eq!(git::head_count(&root).unwrap(), before_head + 1);

        let grounded = engine.grounding("testscope").unwrap();
        assert_eq!(grounded.len(), 1);
        assert_eq!(grounded[0].id, claim.id);
    }

    #[test]
    fn reserved_and_absent_scopes_are_refused() {
        let root = scoped_root(&[]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };

        for reserved in ["quarantine", ".index", ".git", "embers"] {
            let err = engine
                .save(&basic_intent(reserved, "q", "a", "w"), &handle)
                .unwrap_err();
            assert!(matches!(err, StoreError::BadScope { .. }), "expected BadScope for {reserved}, got {err:?}");
        }

        let err = engine.save(&basic_intent("nosuchscope", "q", "a", "w"), &handle).unwrap_err();
        assert!(matches!(err, StoreError::NoScope { .. }));
    }

    struct FailingEmbedder;

    impl Embedder for FailingEmbedder {
        fn model(&self) -> &str {
            "failing"
        }

        fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
            Err(EmbedError {
                detail: "embedder unavailable".to_string(),
            })
        }
    }

    #[test]
    fn landscape_unions_cosine_and_bm25_legs() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap().with_embedder(Box::new(HashEmbedder::new()));
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };

        engine
            .save(&basic_intent("testscope", "cobalt readings", "steady", "logged hourly"), &handle)
            .unwrap();
        engine.drain().unwrap();

        let landscape = engine
            .save_with(&basic_intent("testscope", "vector drift", "unclear", "needs review"), &handle)
            .unwrap();

        assert_eq!(landscape.legs, vec!["cosine".to_string(), "bm25".to_string()]);
        assert!(!landscape.claims.is_empty(), "cosine leg should surface the collision-matched claim");
        assert_eq!(landscape.claims[0].question, "cobalt readings");
    }

    #[test]
    fn landscape_degrades_when_embedder_fails() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap().with_embedder(Box::new(FailingEmbedder));
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };

        let landscape = engine
            .save_with(&basic_intent("testscope", "why is the sky blue", "scattering", "physics"), &handle)
            .unwrap();

        assert_eq!(landscape.legs, vec!["bm25".to_string()]);
        assert_eq!(landscape.degraded.as_deref(), Some("embedder unavailable"));

        let candidates: Vec<_> = fs::read_dir(root.join("quarantine").join("main")).unwrap().collect();
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn index_rows_with_another_model_are_reembedded() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        engine
            .save(&basic_intent("testscope", "how many workers run", "eight", "sized to cpu"), &handle)
            .unwrap();
        engine.drain().unwrap();
        let seeded_id = engine.recall("testscope", "how many workers run", 1).unwrap()[0].id.clone();

        let index_dir = root.join(".index");
        fs::create_dir_all(&index_dir).unwrap();
        let stale_row = serde_json::json!({"id": seeded_id, "model": "stale-model", "v": [0.1, 0.2]});
        fs::write(index_dir.join("testscope.vec.jsonl"), format!("{stale_row}\n")).unwrap();

        let mut engine = engine.with_embedder(Box::new(HashEmbedder::new()));
        engine.ensure_vectors("testscope").unwrap();

        let content = fs::read_to_string(index_dir.join("testscope.vec.jsonl")).unwrap();
        let rows: Vec<serde_json::Value> = content.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| row["model"] == "stale-model"));
        assert!(rows.iter().any(|row| row["model"] == "hash-64" && row["id"] == seeded_id));
    }

    #[test]
    fn index_rows_with_changed_content_are_reembedded() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap().with_embedder(Box::new(HashEmbedder::new()));
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        engine
            .save(&basic_intent("testscope", "how many workers run", "eight", "sized to cpu"), &handle)
            .unwrap();
        engine.drain().unwrap();
        let seeded_id = engine.recall("testscope", "how many workers run", 1).unwrap()[0].id.clone();

        engine.ensure_vectors("testscope").unwrap();
        let index_path = root.join(".index").join("testscope.vec.jsonl");
        let before = fs::read_to_string(&index_path).unwrap();
        let before_rows: Vec<serde_json::Value> = before.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(before_rows.len(), 1);
        let stale_vector = before_rows[0]["v"].clone();

        let claim_path = root.join("testscope").join(format!("{seeded_id}.md"));
        let mut claim = read_claim_file(&claim_path).unwrap();
        claim.answer = "sixteen".to_string();
        fs::write(&claim_path, claim_file::render(&claim)).unwrap();
        engine.invalidate_cache("testscope");

        engine.ensure_vectors("testscope").unwrap();
        let after = fs::read_to_string(&index_path).unwrap();
        let after_rows: Vec<serde_json::Value> = after.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(after_rows.len(), 1, "the stale row must be discarded, not accumulated");
        assert_eq!(after_rows[0]["id"], seeded_id.as_str());
        assert_ne!(after_rows[0]["v"], stale_vector, "changed question/answer text must force recomputation");
    }

    #[test]
    fn metadata_only_edit_does_not_force_reembedding() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap().with_embedder(Box::new(HashEmbedder::new()));
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        engine
            .save(&basic_intent("testscope", "how many workers run", "eight", "sized to cpu"), &handle)
            .unwrap();
        engine.drain().unwrap();
        let seeded_id = engine.recall("testscope", "how many workers run", 1).unwrap()[0].id.clone();

        engine.ensure_vectors("testscope").unwrap();
        let index_path = root.join(".index").join("testscope.vec.jsonl");
        let before = fs::read_to_string(&index_path).unwrap();
        let before_ino = std::os::unix::fs::MetadataExt::ino(&fs::metadata(&index_path).unwrap());

        let claim_path = root.join("testscope").join(format!("{seeded_id}.md"));
        let mut claim = read_claim_file(&claim_path).unwrap();
        claim.why = "revised staffing note, headcount unchanged".to_string();
        fs::write(&claim_path, claim_file::render(&claim)).unwrap();
        engine.invalidate_cache("testscope");

        engine.ensure_vectors("testscope").unwrap();
        let after = fs::read_to_string(&index_path).unwrap();
        let after_ino = std::os::unix::fs::MetadataExt::ino(&fs::metadata(&index_path).unwrap());
        assert_eq!(before, after, "a why-only edit does not touch question or answer, so the vector row must be untouched");
        assert_eq!(before_ino, after_ino, "a why-only edit must not trigger an index rewrite, not merely produce identical bytes");
    }

    #[test]
    fn index_rows_without_stored_hash_are_rebuilt_not_trusted() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap();
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        engine
            .save(&basic_intent("testscope", "how many workers run", "eight", "sized to cpu"), &handle)
            .unwrap();
        engine.drain().unwrap();
        let seeded_id = engine.recall("testscope", "how many workers run", 1).unwrap()[0].id.clone();

        let index_dir = root.join(".index");
        fs::create_dir_all(&index_dir).unwrap();
        let legacy_row = serde_json::json!({"id": seeded_id, "model": "hash-64", "v": [9.9, 9.9]});
        fs::write(index_dir.join("testscope.vec.jsonl"), format!("{legacy_row}\n")).unwrap();

        let mut engine = engine.with_embedder(Box::new(HashEmbedder::new()));
        engine.ensure_vectors("testscope").unwrap();

        let content = fs::read_to_string(index_dir.join("testscope.vec.jsonl")).unwrap();
        let rows: Vec<serde_json::Value> = content.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(rows.len(), 1, "the hash-less legacy row must be replaced, not duplicated");
        assert_eq!(rows[0]["id"], seeded_id.as_str());
        assert_eq!(rows[0]["model"], "hash-64");
        assert_ne!(rows[0]["v"], legacy_row["v"], "a fabricated legacy vector must never be trusted as current");
        assert!(rows[0]["hash"].is_string(), "the rebuilt row must record a content hash");
    }

    #[test]
    fn a_failed_index_rewrite_preserves_the_existing_rows() {
        let root = scoped_root(&["testscope"]);
        let mut engine = Engine::open(&root, "main").unwrap().with_embedder(Box::new(HashEmbedder::new()));
        let handle = ClaimHandle {
            thread: "main".to_string(),
            turn: 1,
        };
        engine
            .save(&basic_intent("testscope", "how many workers run", "eight", "sized to cpu"), &handle)
            .unwrap();
        engine.drain().unwrap();
        let seeded_id = engine.recall("testscope", "how many workers run", 1).unwrap()[0].id.clone();

        let index_path = root.join(".index").join("testscope.vec.jsonl");
        let before = fs::read_to_string(&index_path).unwrap();
        assert!(!before.is_empty(), "drain already embeds the seeded claim");

        let claim_path = root.join("testscope").join(format!("{seeded_id}.md"));
        let mut claim = read_claim_file(&claim_path).unwrap();
        claim.answer = "sixteen".to_string();
        fs::write(&claim_path, claim_file::render(&claim)).unwrap();
        engine.invalidate_cache("testscope");

        let tmp_path = index_path.with_extension("jsonl.tmp");
        fs::create_dir_all(&tmp_path).unwrap();

        let err = engine.ensure_vectors("testscope").unwrap_err();
        assert!(!err.detail.is_empty());

        let after = fs::read_to_string(&index_path).unwrap();
        assert_eq!(before, after, "a failed rewrite must not clobber the existing index");
    }
}
