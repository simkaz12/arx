use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use arx_engine::{ClaimHandle, ClaimKind, EmbedError, Embedder, Engine, HashEmbedder, Lifecycle, OllamaEmbedder, RecallOpts, SaveIntent, Store};
use serde::Deserialize;

struct SharedHashEmbedder(Arc<HashEmbedder>);

impl Embedder for SharedHashEmbedder {
    fn model(&self) -> &str {
        self.0.model()
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.0.embed(texts)
    }
}

#[derive(Deserialize)]
struct FixtureClaim {
    id: String,
    kind: String,
    question: String,
    answer: String,
    why: String,
}

#[derive(Deserialize)]
struct FixtureQuery {
    query: String,
    expect_top: Option<String>,
    property: String,
}

#[derive(Deserialize)]
struct Fixture {
    claims: Vec<FixtureClaim>,
    queries: Vec<FixtureQuery>,
}

fn temp_root(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    std::env::temp_dir().join(format!("arx-engine-recall-{label}-{}-{n}-{nanos}", std::process::id()))
}

fn scoped_engine(scope: &str) -> Engine {
    let root = temp_root(scope);
    fs::create_dir_all(root.join(scope)).unwrap();
    Engine::open(&root, "main").unwrap()
}

fn claim_kind(kind: &str) -> ClaimKind {
    match kind {
        "fact" => ClaimKind::Fact,
        "decision" => ClaimKind::Decision,
        other => panic!("unknown fixture kind '{other}'"),
    }
}

fn handle() -> ClaimHandle {
    ClaimHandle {
        thread: "main".to_string(),
        turn: 1,
    }
}

#[test]
fn sanity_fixture_holds() {
    let fixture: Fixture = serde_json::from_str(include_str!("fixtures/recall-sanity.json")).unwrap();
    let mut engine = scoped_engine("kestrel");
    let handle = handle();

    for claim in &fixture.claims {
        engine
            .save(
                &SaveIntent {
                    scope: "kestrel".to_string(),
                    kind: claim_kind(&claim.kind),
                    question: claim.question.clone(),
                    answer: claim.answer.clone(),
                    why: claim.why.clone(),
                    supersedes: None,
                },
                &handle,
            )
            .unwrap();
    }
    engine.drain().unwrap();

    let minted_id: std::collections::HashMap<String, String> = fixture
        .claims
        .iter()
        .map(|claim| {
            let hits = engine.recall("kestrel", &claim.question, 1).unwrap();
            (claim.id.clone(), hits[0].id.clone())
        })
        .collect();

    let mut failures = Vec::new();
    for query in &fixture.queries {
        let results = engine.recall("kestrel", &query.query, 5).unwrap();
        match &query.expect_top {
            Some(expected_id) => {
                let expected_minted = minted_id.get(expected_id).expect("expect_top names a fixture claim id");
                let top = results.first().map(|claim| claim.id.as_str());
                if top != Some(expected_minted.as_str()) {
                    failures.push(format!(
                        "[{}] query '{}' expected top '{expected_id}' got {top:?}",
                        query.property, query.query
                    ));
                }
            }
            None => {
                if !results.is_empty() {
                    let ids: Vec<&str> = results.iter().map(|claim| claim.id.as_str()).collect();
                    failures.push(format!(
                        "[{}] query '{}' expected an empty result, got {ids:?}",
                        query.property, query.query
                    ));
                }
            }
        }
    }

    assert!(failures.is_empty(), "recall sanity fixture failures:\n{}", failures.join("\n"));
}

#[test]
fn superseded_excluded_by_default_included_with_flag() {
    let mut engine = scoped_engine("lineage");
    let handle = handle();

    engine
        .save(
            &SaveIntent {
                scope: "lineage".to_string(),
                kind: ClaimKind::Decision,
                question: "which queue backs the kestrel dispatcher".to_string(),
                answer: "sqs".to_string(),
                why: "matches the original aws footprint".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();
    engine.drain().unwrap();

    let seeded = engine.decisions_query("lineage", "queue backs the kestrel dispatcher", 5).unwrap();
    let predecessor_id = seeded[0].id.clone();

    engine
        .save(
            &SaveIntent {
                scope: "lineage".to_string(),
                kind: ClaimKind::Decision,
                question: "which queue backs the kestrel dispatcher".to_string(),
                answer: "rabbitmq".to_string(),
                why: "sqs cost too much at our volume".to_string(),
                supersedes: Some(predecessor_id.clone()),
            },
            &handle,
        )
        .unwrap();
    engine.drain().unwrap();

    let default_recall = engine.recall("lineage", "queue backs the kestrel dispatcher", 5).unwrap();
    assert_eq!(default_recall[0].answer, "rabbitmq");
    assert_eq!(default_recall[0].supersedes.as_deref(), Some(predecessor_id.as_str()));
    assert!(default_recall.iter().all(|claim| claim.id != predecessor_id));

    let opts = RecallOpts {
        include_superseded: true,
        include_invalid: false,
    };
    let flagged_recall = engine.recall_with("lineage", "queue backs the kestrel dispatcher", 5, &opts).unwrap();
    let successor_id = flagged_recall
        .iter()
        .find(|claim| claim.answer == "rabbitmq")
        .unwrap()
        .id
        .clone();
    let predecessor_hit = flagged_recall
        .iter()
        .find(|claim| claim.id == predecessor_id)
        .expect("predecessor present under include_superseded");
    assert_eq!(predecessor_hit.lifecycle, Lifecycle::Superseded);
    assert_eq!(predecessor_hit.superseded_by.as_deref(), Some(successor_id.as_str()));

    let default_decisions = engine.decisions_query("lineage", "queue backs the kestrel dispatcher", 5).unwrap();
    assert_eq!(default_decisions[0].answer, "rabbitmq");
    assert!(default_decisions.iter().all(|claim| claim.id != predecessor_id));

    let flagged_decisions = engine
        .decisions_query_with("lineage", "queue backs the kestrel dispatcher", 5, &opts)
        .unwrap();
    assert!(flagged_decisions.iter().any(|claim| claim.id == predecessor_id));
}

#[test]
fn invalid_excluded_by_default_included_with_flag() {
    let mut engine = scoped_engine("invalidation");
    let handle = handle();

    engine
        .save(
            &SaveIntent {
                scope: "invalidation".to_string(),
                kind: ClaimKind::Fact,
                question: "which port does the kestrel admin console listen on".to_string(),
                answer: "port 8081".to_string(),
                why: "documented in the ops handbook".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();
    engine.drain().unwrap();

    let seeded = engine.recall("invalidation", "kestrel admin console port", 5).unwrap();
    let claim_id = seeded[0].id.clone();

    engine
        .invalidate("invalidation", &claim_id, "the admin console was retired")
        .unwrap();

    let default_hits = engine.recall("invalidation", "kestrel admin console port", 5).unwrap();
    assert!(default_hits.is_empty());

    let opts = RecallOpts {
        include_superseded: false,
        include_invalid: true,
    };
    let flagged_hits = engine.recall_with("invalidation", "kestrel admin console port", 5, &opts).unwrap();
    assert_eq!(flagged_hits.len(), 1);
    assert_eq!(flagged_hits[0].id, claim_id);
    assert_eq!(flagged_hits[0].invalid_reason.as_deref(), Some("the admin console was retired"));
}

#[test]
fn embers_never_recalled() {
    let mut engine = scoped_engine("embertest");
    let handle = handle();

    engine
        .ember("embertest", "the kestrel launch is postponed to next sprint", &handle)
        .unwrap();

    let recall_hits = engine.recall("embertest", "kestrel launch postponed", 5).unwrap();
    assert!(recall_hits.is_empty());

    let decisions_hits = engine.decisions_query("embertest", "kestrel launch postponed", 5).unwrap();
    assert!(decisions_hits.is_empty());

    let grounded = engine.grounding("embertest").unwrap();
    assert_eq!(grounded.len(), 1);
    assert_eq!(grounded[0].kind, ClaimKind::Ember);
}

#[test]
fn recall_never_embeds() {
    let counter = Arc::new(HashEmbedder::new());
    let mut engine = scoped_engine("counting").with_embedder(Box::new(SharedHashEmbedder(counter.clone())));
    let handle = handle();

    engine.recall("counting", "anything", 5).unwrap();
    engine.decisions_query("counting", "anything", 5).unwrap();
    engine.grounding("counting").unwrap();
    assert_eq!(counter.calls(), 0);

    engine
        .save(
            &SaveIntent {
                scope: "counting".to_string(),
                kind: ClaimKind::Fact,
                question: "why is the sky blue".to_string(),
                answer: "scattering".to_string(),
                why: "physics".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();

    assert!(counter.calls() > 0);
}

#[test]
#[ignore]
fn live_ollama_landscape_surfaces_paraphrase() {
    let check = std::process::Command::new("sh")
        .arg("-c")
        .arg("curl -s http://localhost:11434/api/tags | grep -c nomic-embed-text")
        .output();
    let present = match &check {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim() == "1",
        Err(_) => false,
    };
    if !present {
        let detail = match &check {
            Ok(output) => format!(
                "exit={:?} stdout={:?} stderr={:?}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(err) => err.to_string(),
        };
        eprintln!("blocked: nomic-embed-text not confirmed on http://localhost:11434 ({detail})");
        return;
    }

    let mut engine = scoped_engine("liveparaphrase")
        .with_embedder(Box::new(OllamaEmbedder::new("http://localhost:11434", "nomic-embed-text")));
    let handle = handle();

    engine
        .save(
            &SaveIntent {
                scope: "liveparaphrase".to_string(),
                kind: ClaimKind::Fact,
                question: "which port does the sidecar dev server default to".to_string(),
                answer: "port 4317".to_string(),
                why: "documented in the runbook".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();
    engine.drain().unwrap();

    let landscape = engine
        .save_with(
            &SaveIntent {
                scope: "liveparaphrase".to_string(),
                kind: ClaimKind::Fact,
                question: "what number does the helper process listen on out of the box".to_string(),
                answer: "5000".to_string(),
                why: "checking for a collision".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();

    assert!(landscape.legs.contains(&"cosine".to_string()), "legs was {:?}", landscape.legs);
    assert!(
        landscape.claims.iter().any(|claim| claim.answer == "port 4317"),
        "landscape claims were {:?}",
        landscape.claims.iter().map(|claim| &claim.answer).collect::<Vec<_>>()
    );
}
