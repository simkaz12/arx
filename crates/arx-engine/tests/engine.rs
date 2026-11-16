use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use arx_engine::{
    serve, ClaimHandle, ClaimKind, DrainReport, Engine, FileStore, Lifecycle, RecallOpts, SaveIntent, Store, StoreError, Suggestion,
    SuggestionAction,
};

fn temp_root(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    std::env::temp_dir().join(format!("arx-engine-donewhen-{label}-{}-{n}-{nanos}", std::process::id()))
}

fn scoped_engine(scope: &str) -> (Engine, PathBuf) {
    let root = temp_root(scope);
    fs::create_dir_all(root.join(scope)).unwrap();
    let engine = Engine::open(&root, "main").unwrap();
    (engine, root)
}

fn handle() -> ClaimHandle {
    ClaimHandle {
        thread: "main".to_string(),
        turn: 1,
    }
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

fn head_count(root: &Path) -> u64 {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-list")
        .arg("--count")
        .arg("HEAD")
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().parse().unwrap()
}

#[test]
fn contradicting_intent_returns_landscape_and_git_log_waits_for_drain() {
    let (mut engine, root) = scoped_engine("contradiction");
    let handle = handle();

    engine
        .save(
            &basic_intent("contradiction", "how many retries does the dispatcher attempt", "three retries", "avoids overload"),
            &handle,
        )
        .unwrap();
    engine.drain().unwrap();
    let seeded_id = engine.recall("contradiction", "retries dispatcher attempt", 1).unwrap()[0].id.clone();

    let before_head = head_count(&root);
    let landscape = engine
        .save_with(
            &basic_intent("contradiction", "how many retries does the dispatcher attempt", "five retries", "measured under new load"),
            &handle,
        )
        .unwrap();
    assert!(landscape.claims.iter().any(|claim| claim.id == seeded_id));
    assert_eq!(head_count(&root), before_head);

    let report = engine.drain().unwrap();
    assert_eq!(report.promoted, 1);
    assert_eq!(head_count(&root), before_head + 1);

    let new_id = engine.recall("contradiction", "five retries", 1).unwrap()[0].id.clone();
    let output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .arg("show")
        .arg("--name-only")
        .arg("--format=")
        .arg("HEAD")
        .output()
        .unwrap();
    let names: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();
    assert_eq!(names, vec![format!("contradiction/{new_id}.md")]);
}

#[test]
fn claim_missing_any_field_is_rejected() {
    let (mut engine, root) = scoped_engine("missingfield");
    let handle = handle();

    for (question, answer, why, field) in [
        ("", "answer", "why", "question"),
        ("question", "", "why", "answer"),
        ("question", "answer", "", "why"),
    ] {
        let err = engine
            .save(
                &SaveIntent {
                    scope: "missingfield".to_string(),
                    kind: ClaimKind::Fact,
                    question: question.to_string(),
                    answer: answer.to_string(),
                    why: why.to_string(),
                    supersedes: None,
                },
                &handle,
            )
            .unwrap_err();
        match err {
            StoreError::MissingField { field: got } => assert_eq!(got, field),
            other => panic!("expected MissingField('{field}'), got {other:?}"),
        }
    }

    engine
        .save(&basic_intent("missingfield", "good question", "good answer", "good why"), &handle)
        .unwrap();

    let quarantine_dir = root.join("quarantine").join("main");
    let entries: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(entries.len(), 1);
    let candidate_path = entries.into_iter().next().unwrap().unwrap().path();
    let text = fs::read_to_string(&candidate_path).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["why"] = serde_json::Value::String("   ".to_string());
    fs::write(&candidate_path, serde_json::to_string(&value).unwrap()).unwrap();

    let before_head = head_count(&root);
    let err = engine.drain().unwrap_err();
    assert!(matches!(err, StoreError::MalformedClaimFile { .. }));
    assert_eq!(head_count(&root), before_head);

    let claim_files: Vec<_> = fs::read_dir(root.join("missingfield"))
        .unwrap()
        .filter(|entry| entry.as_ref().unwrap().path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .collect();
    assert!(claim_files.is_empty());
    let remaining: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(remaining.len(), 1);
}

#[test]
fn supersession_lineage_through_save_and_drain() {
    let (mut engine, _root) = scoped_engine("supersession");
    let handle = handle();

    engine
        .save(
            &SaveIntent {
                scope: "supersession".to_string(),
                kind: ClaimKind::Decision,
                question: "which region hosts the kestrel primary".to_string(),
                answer: "us-east-1".to_string(),
                why: "closest to most customers at launch".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();
    engine.drain().unwrap();

    let predecessor_id = engine.decisions_query("supersession", "region hosts the kestrel primary", 1).unwrap()[0].id.clone();

    let landscape = engine
        .save_with(
            &SaveIntent {
                scope: "supersession".to_string(),
                kind: ClaimKind::Decision,
                question: "which region hosts the kestrel primary".to_string(),
                answer: "eu-west-1".to_string(),
                why: "customer base shifted to europe".to_string(),
                supersedes: Some(predecessor_id.clone()),
            },
            &handle,
        )
        .unwrap();
    assert!(landscape.claims.iter().any(|claim| claim.id == predecessor_id));

    engine.drain().unwrap();

    let successor_id = engine.decisions_query("supersession", "region hosts the kestrel primary", 5).unwrap()[0].id.clone();
    assert_ne!(successor_id, predecessor_id);

    let default_recall = engine.recall("supersession", "region hosts the kestrel primary", 5).unwrap();
    assert_eq!(default_recall.len(), 1);
    assert_eq!(default_recall[0].id, successor_id);

    let opts = RecallOpts {
        include_superseded: true,
        include_invalid: false,
    };
    let flagged_recall = engine.recall_with("supersession", "region hosts the kestrel primary", 5, &opts).unwrap();
    assert_eq!(flagged_recall.len(), 2);
    assert!(flagged_recall
        .iter()
        .any(|claim| claim.id == predecessor_id && claim.lifecycle == Lifecycle::Superseded));

    let default_decisions = engine.decisions_query("supersession", "region hosts the kestrel primary", 5).unwrap();
    assert_eq!(default_decisions.len(), 1);
    assert_eq!(default_decisions[0].id, successor_id);

    let flagged_decisions = engine
        .decisions_query_with("supersession", "region hosts the kestrel primary", 5, &opts)
        .unwrap();
    assert_eq!(flagged_decisions.len(), 2);

    let fetched_predecessor = engine.decision("supersession", &predecessor_id).unwrap().unwrap();
    assert_eq!(fetched_predecessor.lifecycle, Lifecycle::Superseded);
    assert_eq!(fetched_predecessor.superseded_by.as_deref(), Some(successor_id.as_str()));
}

#[test]
fn invalidated_fact_hidden_until_asked() {
    let (mut engine, root) = scoped_engine("invalidated");
    let handle = handle();

    engine
        .save(&basic_intent("invalidated", "what happens when the disk fills up", "writes are refused", "protects the wal"), &handle)
        .unwrap();
    engine.drain().unwrap();
    let claim_id = engine.recall("invalidated", "disk fills up", 1).unwrap()[0].id.clone();
    assert!(!engine.recall("invalidated", "disk fills up", 5).unwrap().is_empty());

    let before_head = head_count(&root);
    engine.invalidate("invalidated", &claim_id, "the wal moved to a different disk").unwrap();
    assert_eq!(head_count(&root), before_head + 1);

    assert!(engine.recall("invalidated", "disk fills up", 5).unwrap().is_empty());

    let opts = RecallOpts {
        include_superseded: false,
        include_invalid: true,
    };
    let flagged = engine.recall_with("invalidated", "disk fills up", 5, &opts).unwrap();
    assert_eq!(flagged.len(), 1);
    assert_eq!(flagged[0].id, claim_id);
    assert_eq!(flagged[0].invalid_reason.as_deref(), Some("the wal moved to a different disk"));
}

#[test]
fn engine_drives_the_l1_call_site_unchanged() {
    let (mut engine, _root) = scoped_engine("callsite");
    let handle = handle();

    engine
        .save(
            &basic_intent("callsite", "what does the harness call this adapter", "engine", "it is the second store behind the trait"),
            &handle,
        )
        .unwrap();
    engine.drain().unwrap();

    let save_args = serde_json::json!({
        "scope": "callsite",
        "kind": "decision",
        "question": "what does the harness call this adapter",
        "answer": "the engine adapter",
        "why": "confirmed against the seeded claim"
    });
    let save_result = serve(&mut engine, "save", &save_args, &handle);
    let claims: Vec<serde_json::Value> = serde_json::from_value(save_result).unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0]["answer"], "engine");

    let drain_result = serve(&mut engine, "drain", &serde_json::json!({}), &handle);
    let report: DrainReport = serde_json::from_value(drain_result).unwrap();
    assert_eq!(report.promoted, 1);

    let recall_result = serve(
        &mut engine,
        "recall",
        &serde_json::json!({"scope": "callsite", "query": "engine adapter", "limit": 5}),
        &handle,
    );
    let recalled: Vec<serde_json::Value> = serde_json::from_value(recall_result).unwrap();
    assert_eq!(recalled.len(), 2);

    let dq_result = serve(
        &mut engine,
        "decisions_query",
        &serde_json::json!({"scope": "callsite", "query": "engine adapter", "limit": 5}),
        &handle,
    );
    let decisions: Vec<serde_json::Value> = serde_json::from_value(dq_result).unwrap();
    assert_eq!(decisions.len(), 1);
    let decision_id = decisions[0]["id"].as_str().unwrap().to_string();

    let decision_result = serve(
        &mut engine,
        "decision",
        &serde_json::json!({"scope": "callsite", "id": decision_id}),
        &handle,
    );
    assert_eq!(decision_result["answer"], "the engine adapter");
}

#[test]
fn hot_label_edited_by_hand_grounds_after_reopen() {
    let (mut engine, root) = scoped_engine("hotlabel");
    let handle = handle();

    engine
        .save(&basic_intent("hotlabel", "where do incident runbooks live", "the wiki", "single source of truth"), &handle)
        .unwrap();
    engine.drain().unwrap();
    let claim_id = engine.recall("hotlabel", "incident runbooks", 1).unwrap()[0].id.clone();

    let claim_path = root.join("hotlabel").join(format!("{claim_id}.md"));
    let text = fs::read_to_string(&claim_path).unwrap();
    let edited = text.replacen("kind: fact\n", "kind: fact\nlabel: hot\n", 1);
    assert_ne!(text, edited, "expected to find 'kind: fact' to rewrite");
    fs::write(&claim_path, edited).unwrap();

    let mut reopened = Engine::open(&root, "main").unwrap();
    let grounded = reopened.grounding("hotlabel").unwrap();
    assert_eq!(grounded.len(), 1);
    assert_eq!(grounded[0].id, claim_id);

    reopened
        .save(
            &SaveIntent {
                scope: "hotlabel".to_string(),
                kind: ClaimKind::Fact,
                question: "where do incident runbooks live".to_string(),
                answer: "the internal wiki space".to_string(),
                why: "moved off the old wiki host".to_string(),
                supersedes: Some(claim_id.clone()),
            },
            &handle,
        )
        .unwrap();
    reopened.drain().unwrap();

    let final_grounding = reopened.grounding("hotlabel").unwrap();
    assert!(final_grounding.iter().all(|claim| claim.id != claim_id), "superseded hot claim should not ground");
}

fn snapshot_dir(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut entries: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            let bytes = fs::read(entry.path()).unwrap();
            (name, bytes)
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

fn git_status_porcelain(root: &Path) -> String {
    let output = Command::new("git").arg("-C").arg(root).arg("status").arg("--porcelain").output().unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn quarantine_id_by_question(root: &Path, thread: &str, question: &str) -> String {
    let dir = root.join("quarantine").join(thread);
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value["question"] == question {
            return value["id"].as_str().unwrap().to_string();
        }
    }
    panic!("no quarantined candidate with question '{question}' under {}", dir.display());
}

#[test]
fn drain_with_applies_promote_supersede_and_drop_in_one_commit() {
    let (mut engine, root) = scoped_engine("twopen");
    let seed_handle = handle();

    engine
        .save(&basic_intent("twopen", "predecessor question", "predecessor answer", "predecessor why"), &seed_handle)
        .unwrap();
    engine.drain().unwrap();
    let predecessor_id = engine.recall("twopen", "predecessor question", 1).unwrap()[0].id.clone();

    engine
        .save(&basic_intent("twopen", "plain promote question", "plain promote answer", "plain promote why"), &seed_handle)
        .unwrap();
    engine
        .save(&basic_intent("twopen", "supersede promote question", "supersede promote answer", "supersede promote why"), &seed_handle)
        .unwrap();
    engine
        .save(&basic_intent("twopen", "drop candidate question", "drop candidate answer", "drop candidate why"), &seed_handle)
        .unwrap();

    let quarantine_dir = root.join("quarantine").join("main");
    let plain_id = quarantine_id_by_question(&root, "main", "plain promote question");
    let supersede_id = quarantine_id_by_question(&root, "main", "supersede promote question");
    let drop_id = quarantine_id_by_question(&root, "main", "drop candidate question");

    let suggestions = vec![
        Suggestion {
            id: plain_id.clone(),
            action: SuggestionAction::Promote,
            supersedes: None,
            reason: "looks right".to_string(),
        },
        Suggestion {
            id: supersede_id.clone(),
            action: SuggestionAction::Promote,
            supersedes: Some(predecessor_id.clone()),
            reason: "supersedes the old answer".to_string(),
        },
        Suggestion {
            id: drop_id.clone(),
            action: SuggestionAction::Drop,
            supersedes: None,
            reason: "duplicate".to_string(),
        },
    ];

    let before_head = head_count(&root);
    let drain_handle = ClaimHandle {
        thread: "t".to_string(),
        turn: 7,
    };
    let report = engine.drain_with(Some(&suggestions), &drain_handle).unwrap();
    assert_eq!(report, DrainReport { promoted: 2, dropped: 1 });
    assert_eq!(head_count(&root), before_head + 1);

    let output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .arg("log")
        .arg("-1")
        .arg("--format=%s")
        .output()
        .unwrap();
    let subject = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(subject, "drain t@7: promoted 2, dropped 1");

    assert!(root.join("twopen").join(format!("{plain_id}.md")).is_file());
    assert!(root.join("twopen").join(format!("{supersede_id}.md")).is_file());
    assert!(!root.join("twopen").join(format!("{drop_id}.md")).exists());

    let opts = RecallOpts {
        include_superseded: true,
        include_invalid: false,
    };
    let predecessor = engine
        .recall_with("twopen", "predecessor question", 5, &opts)
        .unwrap()
        .into_iter()
        .find(|claim| claim.id == predecessor_id)
        .unwrap();
    assert_eq!(predecessor.lifecycle, Lifecycle::Superseded);
    assert_eq!(predecessor.superseded_by.as_deref(), Some(supersede_id.as_str()));

    let remaining: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert!(remaining.is_empty(), "quarantine should be cleared of promoted and dropped candidates");
}

#[test]
fn drain_with_refuses_an_uncovered_candidate_and_writes_nothing() {
    let (mut engine, root) = scoped_engine("uncovered");
    let seed_handle = handle();

    engine
        .save(&basic_intent("uncovered", "first candidate question", "first candidate answer", "first candidate why"), &seed_handle)
        .unwrap();
    engine
        .save(&basic_intent("uncovered", "second candidate question", "second candidate answer", "second candidate why"), &seed_handle)
        .unwrap();

    let quarantine_dir = root.join("quarantine").join("main");
    let entries_before: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(entries_before.len(), 2);

    let first_id = quarantine_id_by_question(&root, "main", "first candidate question");
    let suggestions = vec![Suggestion {
        id: first_id,
        action: SuggestionAction::Promote,
        supersedes: None,
        reason: "ok".to_string(),
    }];

    let before_head = head_count(&root);
    let drain_handle = ClaimHandle {
        thread: "t".to_string(),
        turn: 3,
    };
    let err = engine.drain_with(Some(&suggestions), &drain_handle).unwrap_err();
    assert!(matches!(err, StoreError::UncoveredCandidate { .. }), "expected UncoveredCandidate, got {err:?}");

    assert_eq!(head_count(&root), before_head);
    let entries_after: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(entries_after.len(), 2);
}

#[test]
fn drain_with_none_matches_drain() {
    let (mut engine_a, _root_a) = scoped_engine("nonematch");
    let seed_handle = handle();
    engine_a
        .save(&basic_intent("nonematch", "alpha question", "alpha answer", "alpha why"), &seed_handle)
        .unwrap();
    engine_a
        .save(&basic_intent("nonematch", "beta question", "beta answer", "beta why"), &seed_handle)
        .unwrap();
    let report_a = engine_a.drain().unwrap();
    assert_eq!(report_a, DrainReport { promoted: 2, dropped: 0 });
    let mut shape_a: Vec<(String, String, String)> = engine_a
        .recall("nonematch", "alpha beta", 5)
        .unwrap()
        .iter()
        .map(|claim| (claim.question.clone(), claim.answer.clone(), claim.why.clone()))
        .collect();
    shape_a.sort();

    let (mut engine_b, root_b) = scoped_engine("nonematch");
    engine_b
        .save(&basic_intent("nonematch", "alpha question", "alpha answer", "alpha why"), &seed_handle)
        .unwrap();
    engine_b
        .save(&basic_intent("nonematch", "beta question", "beta answer", "beta why"), &seed_handle)
        .unwrap();
    let drain_handle = ClaimHandle {
        thread: "main".to_string(),
        turn: 9,
    };
    let report_b = engine_b.drain_with(None, &drain_handle).unwrap();
    assert_eq!(report_b, DrainReport { promoted: 2, dropped: 0 });
    let mut shape_b: Vec<(String, String, String)> = engine_b
        .recall("nonematch", "alpha beta", 5)
        .unwrap()
        .iter()
        .map(|claim| (claim.question.clone(), claim.answer.clone(), claim.why.clone()))
        .collect();
    shape_b.sort();

    assert_eq!(shape_a, shape_b);

    let output = Command::new("git")
        .arg("-C")
        .arg(&root_b)
        .arg("log")
        .arg("-1")
        .arg("--format=%s")
        .output()
        .unwrap();
    let subject = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(subject, "drain main@9: promoted 2, dropped 0");

    let filestore_root = temp_root("filestore-drain-with");
    let mut filestore = FileStore::open(&filestore_root, "main").unwrap();
    let drop_only = vec![Suggestion {
        id: "whatever".to_string(),
        action: SuggestionAction::Drop,
        supersedes: None,
        reason: "filestore cannot drop".to_string(),
    }];
    let filestore_err = filestore.drain_with(Some(&drop_only), &drain_handle).unwrap_err();
    assert!(
        matches!(filestore_err, StoreError::UnsupportedSuggestion { .. }),
        "expected UnsupportedSuggestion, got {filestore_err:?}"
    );
}

#[test]
fn landscape_matches_what_save_returns() {
    let (mut engine, root) = scoped_engine("parity");
    let seed_handle = handle();

    engine
        .save(
            &basic_intent("parity", "how many retries does the dispatcher attempt", "three retries", "avoids overload"),
            &seed_handle,
        )
        .unwrap();
    engine
        .save(
            &basic_intent("parity", "what timeout does the dispatcher use", "thirty seconds", "matches the gateway"),
            &seed_handle,
        )
        .unwrap();
    engine
        .save(
            &basic_intent("parity", "which queue backs the dispatcher", "redis", "already deployed"),
            &seed_handle,
        )
        .unwrap();
    engine.drain().unwrap();

    let before_head = head_count(&root);
    let landscape = engine
        .landscape("parity", "how many retries does the dispatcher attempt", "five retries")
        .unwrap();
    assert_eq!(head_count(&root), before_head);

    let saved = engine
        .save_with(
            &basic_intent("parity", "how many retries does the dispatcher attempt", "five retries", "measured under new load"),
            &seed_handle,
        )
        .unwrap();

    let mut landscape_ids: Vec<String> = landscape.claims.iter().map(|claim| claim.id.clone()).collect();
    let mut saved_ids: Vec<String> = saved.claims.iter().map(|claim| claim.id.clone()).collect();
    landscape_ids.sort();
    saved_ids.sort();
    assert_eq!(landscape_ids, saved_ids);
    assert_eq!(landscape.degraded, saved.degraded);
}

#[test]
fn drain_rejects_duplicate_supersession_targets_before_any_mutation() {
    let (mut engine, root) = scoped_engine("dupetarget");
    let seed_handle = handle();

    engine
        .save(&basic_intent("dupetarget", "predecessor question", "predecessor answer", "predecessor why"), &seed_handle)
        .unwrap();
    engine.drain().unwrap();
    let predecessor_id = engine.recall("dupetarget", "predecessor question", 1).unwrap()[0].id.clone();

    engine
        .save(
            &SaveIntent {
                scope: "dupetarget".to_string(),
                kind: ClaimKind::Fact,
                question: "first successor question".to_string(),
                answer: "first successor answer".to_string(),
                why: "first successor why".to_string(),
                supersedes: Some(predecessor_id.clone()),
            },
            &seed_handle,
        )
        .unwrap();
    engine
        .save(
            &SaveIntent {
                scope: "dupetarget".to_string(),
                kind: ClaimKind::Fact,
                question: "second successor question".to_string(),
                answer: "second successor answer".to_string(),
                why: "second successor why".to_string(),
                supersedes: Some(predecessor_id.clone()),
            },
            &seed_handle,
        )
        .unwrap();

    let scope_dir = root.join("dupetarget");
    let quarantine_dir = root.join("quarantine").join("main");
    let claims_before = snapshot_dir(&scope_dir);
    let quarantine_before = snapshot_dir(&quarantine_dir);
    let status_before = git_status_porcelain(&root);
    let head_before = head_count(&root);

    match engine.drain() {
        Ok(report) => {
            let opts = RecallOpts {
                include_superseded: true,
                include_invalid: false,
            };
            let claims = engine.recall_with("dupetarget", "predecessor successor", 10, &opts).unwrap();
            panic!("expected duplicate supersession targets to be rejected before mutation, got {report:?} with claims {claims:#?}");
        }
        Err(err) => {
            assert!(matches!(err, StoreError::UnsupportedSuggestion { .. }), "expected UnsupportedSuggestion, got {err:?}");
        }
    }

    assert_eq!(head_count(&root), head_before, "git head must not advance on rejection");
    assert_eq!(git_status_porcelain(&root), status_before, "git working tree must be unchanged on rejection");
    assert_eq!(snapshot_dir(&scope_dir), claims_before, "claim files must be byte-identical on rejection");
    assert_eq!(snapshot_dir(&quarantine_dir), quarantine_before, "quarantine rows must be byte-identical on rejection");
}

#[test]
fn drain_promotes_two_supersessions_against_different_predecessors() {
    let (mut engine, root) = scoped_engine("twopreds");
    let seed_handle = handle();

    engine
        .save(&basic_intent("twopreds", "first predecessor question", "first predecessor answer", "first predecessor why"), &seed_handle)
        .unwrap();
    engine
        .save(&basic_intent("twopreds", "second predecessor question", "second predecessor answer", "second predecessor why"), &seed_handle)
        .unwrap();
    engine.drain().unwrap();

    let first_predecessor_id = engine.recall("twopreds", "first predecessor question", 1).unwrap()[0].id.clone();
    let second_predecessor_id = engine.recall("twopreds", "second predecessor question", 1).unwrap()[0].id.clone();

    engine
        .save(
            &SaveIntent {
                scope: "twopreds".to_string(),
                kind: ClaimKind::Fact,
                question: "first successor question".to_string(),
                answer: "first successor answer".to_string(),
                why: "first successor why".to_string(),
                supersedes: Some(first_predecessor_id.clone()),
            },
            &seed_handle,
        )
        .unwrap();
    engine
        .save(
            &SaveIntent {
                scope: "twopreds".to_string(),
                kind: ClaimKind::Fact,
                question: "second successor question".to_string(),
                answer: "second successor answer".to_string(),
                why: "second successor why".to_string(),
                supersedes: Some(second_predecessor_id.clone()),
            },
            &seed_handle,
        )
        .unwrap();

    let before_head = head_count(&root);
    let report = engine.drain().unwrap();
    assert_eq!(report, DrainReport { promoted: 2, dropped: 0 });
    assert_eq!(head_count(&root), before_head + 1);

    let opts = RecallOpts {
        include_superseded: true,
        include_invalid: false,
    };
    let claims = engine.recall_with("twopreds", "predecessor successor", 10, &opts).unwrap();
    let first_successor = claims.iter().find(|claim| claim.question == "first successor question").unwrap();
    let second_successor = claims.iter().find(|claim| claim.question == "second successor question").unwrap();
    let first_predecessor = claims.iter().find(|claim| claim.id == first_predecessor_id).unwrap();
    let second_predecessor = claims.iter().find(|claim| claim.id == second_predecessor_id).unwrap();

    assert_eq!(first_predecessor.lifecycle, Lifecycle::Superseded);
    assert_eq!(first_predecessor.superseded_by.as_deref(), Some(first_successor.id.as_str()));
    assert_eq!(second_predecessor.lifecycle, Lifecycle::Superseded);
    assert_eq!(second_predecessor.superseded_by.as_deref(), Some(second_successor.id.as_str()));
    assert_eq!(first_successor.supersedes.as_deref(), Some(first_predecessor_id.as_str()));
    assert_eq!(second_successor.supersedes.as_deref(), Some(second_predecessor_id.as_str()));
}
