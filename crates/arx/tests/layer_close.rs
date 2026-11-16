use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

mod common;
use common::*;

fn pgrep_matches(pattern: &str) -> String {
    let output = StdCommand::new("pgrep").args(["-f", pattern]).output().unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn l1_close_loop_on_host() {
    let (base_url, _state) = fake_openai_server(FakeScript::Sse(vec![
        r#"{"choices":[{"delta":{"role":"assistant"}}]}"#.to_string(),
        r#"{"choices":[{"delta":{"content":"native says skylark"}}]}"#.to_string(),
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_string(),
        r#"{"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":5,"total_tokens":17}}"#.to_string(),
    ]));

    let workspace = native_run_fixture(&base_url);
    let run_dir = workspace.path().join("run");
    let config_path = run_dir.join("arx.toml");

    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str("\n[store]\nscope = \"testscope\"\n");
    fs::write(&config_path, config).unwrap();

    plant_engine_scope(&run_dir.join("store"), "testscope");

    let codex_home = plant_stub_codex_credential(workspace.path());
    ensure_codex_stub_image();
    ensure_claude_stub_image();
    let claude_config = plant_stub_credential(workspace.path());

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scripts/l1-close.json");
    let mut script = load_script(&script_path);
    migrate_retired_judge_dispatch(&mut script);

    let env = [
        ("PATH", claude_hand_path()),
        ("CODEX_HOME", codex_home.to_str().unwrap().to_string()),
        ("CLAUDE_CONFIG_DIR", claude_config.to_str().unwrap().to_string()),
        ("ARX_TEST_FAKE_KEY", "test-key".to_string()),
    ];

    let rows = run_session(&run_dir, "arx.toml", &script.operator, &env);

    if let Err(failures) = check_script(&rows, &script) {
        panic!("{}", failures.join("\n"));
    }

    let quarantine_dir = run_dir.join("store/quarantine/main");
    let entries: Vec<_> = fs::read_dir(&quarantine_dir).unwrap().collect();
    assert_eq!(entries.len(), 1, "quarantine dir entries: {entries:?}");
    let candidate_path = entries.into_iter().next().unwrap().unwrap().path();
    let candidate: serde_json::Value = serde_json::from_str(&fs::read_to_string(&candidate_path).unwrap()).unwrap();
    assert_eq!(candidate["question"], "what is the launch codename");
    assert_eq!(candidate["answer"], "kestrel");
    assert_eq!(candidate["handle"]["thread"], "main");
    let turn = candidate["handle"]["turn"].as_i64().expect("handle.turn is an integer");
    assert!(turn >= 1, "handle.turn: {turn}");

    let save_gate_index = rows
        .iter()
        .position(|row| {
            row["kind"] == "gate"
                && row["tool"].as_str().map(|t| t.ends_with("__save")).unwrap_or(false)
                && row["verdict"] == "allow"
        })
        .expect("save gate row present");
    let save_reply = &rows[save_gate_index + 1];
    assert_eq!(save_reply["kind"], "turn");
    assert_eq!(save_reply["role"], "main");
    let save_reply_text = save_reply["text"].as_str().unwrap();
    assert!(save_reply_text.contains("stub-store:"), "save reply: {save_reply_text}");
    assert!(save_reply_text.contains("skylark"), "save reply: {save_reply_text}");

    let ledger_contents = fs::read_to_string(run_dir.join("threads/main.ledger.jsonl")).unwrap();
    let ledger_rows: Vec<serde_json::Value> = ledger_contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

    let spawn_count = ledger_rows.iter().filter(|row| row["kind"] == "spawn").count();
    assert_eq!(spawn_count, 5, "ledger rows: {ledger_rows:?}");

    let expected_agents = [
        ("h1", "hand-cc", "hand"),
        ("h2", "hand-cx", "hand"),
        ("h3", "hand-ds", "hand"),
        ("h4", "reviewer", "reviewer"),
        ("h5", "fake", "hand"),
    ];
    for (hand, agent, role) in expected_agents {
        let row = ledger_rows
            .iter()
            .find(|row| row["kind"] == "spawn" && row["hand"] == hand)
            .unwrap_or_else(|| panic!("spawn row for {hand} missing: {ledger_rows:?}"));
        assert_eq!(row["agent"], agent, "spawn row for {hand}: {row:?}");
        assert_eq!(row["role"], role, "spawn row for {hand}: {row:?}");
    }

    assert!(
        ledger_rows.iter().any(|row| row["kind"] == "stop" && row["hand"] == "h5"),
        "stop row for h5 missing: {ledger_rows:?}"
    );

    let steer_row = ledger_rows
        .iter()
        .find(|row| row["kind"] == "steer" && row["hand"] == "h5")
        .unwrap_or_else(|| panic!("steer row for h5 missing: {ledger_rows:?}"));
    assert!(!steer_row.to_string().contains("refused"), "steer row: {steer_row:?}");

    for hand in ["h1", "h2", "h3", "h4", "h5"] {
        assert!(
            ledger_rows.iter().any(|row| row["kind"] == "verdict" && row["hand"] == hand),
            "verdict row for {hand} missing: {ledger_rows:?}"
        );
    }

    let arx_text = |needle: &str| -> String {
        rows.iter()
            .find(|row| {
                row["kind"] == "turn"
                    && row["role"] == "arx"
                    && row["text"].as_str().map(|t| t.contains(needle)).unwrap_or(false)
            })
            .unwrap_or_else(|| panic!("no arx row contains {needle}: {rows:?}"))["text"]
            .as_str()
            .unwrap()
            .to_string()
    };

    let cc_text = arx_text("[hand h1 verdict]");
    assert!(cc_text.contains("stub verdict: Summarise seeded.diff for the close"), "cc verdict: {cc_text}");
    assert!(cc_text.contains("(trace: threads/main.h1)"), "cc verdict: {cc_text}");

    let cx_text = arx_text("[hand h2 verdict]");
    assert!(cx_text.contains("stub verdict: Review seeded.diff for the close"), "cx verdict: {cx_text}");
    assert!(cx_text.contains("(trace: threads/main.h2)"), "cx verdict: {cx_text}");

    let reviewer_text = arx_text("[hand h4 verdict]");
    assert!(
        reviewer_text.contains("stub verdict: Reviewer seeded.diff and return a verdict"),
        "reviewer verdict: {reviewer_text}"
    );
    assert!(reviewer_text.contains("(trace: threads/main.h4)"), "reviewer verdict: {reviewer_text}");

    let ds_text = arx_text("[hand h3 verdict]");
    assert!(ds_text.contains("native says skylark"), "ds verdict: {ds_text}");

    let fake_text = arx_text("[hand h5 verdict]");
    assert!(fake_text.contains("fake done"), "fake verdict: {fake_text}");

    let session_contents = fs::read_to_string(run_dir.join("threads/main.h1/session.jsonl")).unwrap();
    let argv_line = session_contents.lines().next().expect("session.jsonl has a first line");
    let argv_value: serde_json::Value = serde_json::from_str(argv_line).unwrap();
    let argv_text = argv_value["argv"].as_str().expect("argv field is a string");
    assert!(argv_text.contains("-p"), "session.jsonl argv line: {argv_text}");

    let cx_session_contents = fs::read_to_string(run_dir.join("threads/main.h2/session.jsonl")).unwrap();
    let cx_argv_line = cx_session_contents.lines().next().expect("session.jsonl has a first line");
    assert!(cx_argv_line.contains("--sandbox"), "session.jsonl argv line: {cx_argv_line}");
    assert!(cx_argv_line.contains("read-only"), "session.jsonl argv line: {cx_argv_line}");
    assert!(cx_argv_line.contains("project_doc_max_bytes=0"), "session.jsonl argv line: {cx_argv_line}");
}

#[test]
fn a_predicate_binds_to_the_first_unbound_row() {
    let rows = vec![
        serde_json::json!({"kind": "gate", "tool": "mcp__arx__save", "verdict": "allow"}),
        serde_json::json!({"kind": "gate", "tool": "mcp__arx__dispatch", "verdict": "allow"}),
    ];
    let script = Script {
        card: "t".to_string(),
        operator: vec![],
        expect: vec![Expect {
            name: "d".to_string(),
            kind: "gate".to_string(),
            tool: Some("dispatch".to_string()),
            verdict: Some("allow".to_string()),
            role: None,
            contains: None,
            after: None,
            hand: None,
            by: None,
        }],
    };
    assert!(check_script(&rows, &script).is_ok());
}

#[test]
fn two_dispatch_predicates_bind_to_two_distinct_rows() {
    let rows = vec![
        serde_json::json!({"kind": "gate", "tool": "mcp__arx__dispatch", "verdict": "allow"}),
        serde_json::json!({"kind": "gate", "tool": "mcp__arx__dispatch", "verdict": "allow"}),
    ];
    let script = Script {
        card: "t".to_string(),
        operator: vec![],
        expect: vec![
            Expect {
                name: "a".to_string(),
                kind: "gate".to_string(),
                tool: Some("dispatch".to_string()),
                verdict: Some("allow".to_string()),
                role: None,
                contains: None,
                after: None,
                hand: None,
                by: None,
            },
            Expect {
                name: "b".to_string(),
                kind: "gate".to_string(),
                tool: Some("dispatch".to_string()),
                verdict: Some("allow".to_string()),
                role: None,
                contains: None,
                after: None,
                hand: None,
                by: None,
            },
        ],
    };
    assert!(check_script(&rows, &script).is_ok());
}

#[test]
fn tool_dispatch_matches_the_prefixed_form_and_not_a_similar_name() {
    let rows = vec![serde_json::json!({"kind": "gate", "tool": "dispatcher", "verdict": "allow"})];
    let script = Script {
        card: "t".to_string(),
        operator: vec![],
        expect: vec![Expect {
            name: "d".to_string(),
            kind: "gate".to_string(),
            tool: Some("dispatch".to_string()),
            verdict: Some("allow".to_string()),
            role: None,
            contains: None,
            after: None,
            hand: None,
            by: None,
        }],
    };
    let err = check_script(&rows, &script).unwrap_err();
    assert_eq!(err, vec!["d: no unbound row matches".to_string()]);

    let rows = vec![serde_json::json!({"kind": "gate", "tool": "mcp__arx__dispatch", "verdict": "allow"})];
    assert!(check_script(&rows, &script).is_ok());
}

#[test]
fn an_after_violation_is_reported_by_both_names() {
    let rows = vec![
        serde_json::json!({"kind": "turn", "role": "arx", "text": "[hand h1 verdict] x"}),
        serde_json::json!({"kind": "gate", "tool": "mcp__arx__steer", "verdict": "allow"}),
    ];
    let script = Script {
        card: "t".to_string(),
        operator: vec![],
        expect: vec![
            Expect {
                name: "steer-gate".to_string(),
                kind: "gate".to_string(),
                tool: Some("steer".to_string()),
                verdict: Some("allow".to_string()),
                role: None,
                contains: None,
                after: None,
                hand: None,
                by: None,
            },
            Expect {
                name: "verdict".to_string(),
                kind: "turn".to_string(),
                tool: None,
                verdict: None,
                role: Some("arx".to_string()),
                contains: Some("verdict".to_string()),
                after: Some("steer-gate".to_string()),
                hand: None,
                by: None,
            },
        ],
    };
    let err = check_script(&rows, &script).unwrap_err();
    assert_eq!(err, vec!["verdict: bound at 0, not after steer-gate at 1".to_string()]);
}

#[test]
fn a_missing_row_is_reported_by_name_and_other_failures_are_still_listed() {
    let rows = vec![serde_json::json!({"kind": "turn", "role": "arx", "text": "[hand h1 verdict] x"})];
    let script = Script {
        card: "t".to_string(),
        operator: vec![],
        expect: vec![
            Expect {
                name: "missing-gate".to_string(),
                kind: "gate".to_string(),
                tool: Some("save".to_string()),
                verdict: None,
                role: None,
                contains: None,
                after: None,
                hand: None,
                by: None,
            },
            Expect {
                name: "missing-turn".to_string(),
                kind: "turn".to_string(),
                tool: None,
                verdict: None,
                role: Some("arx".to_string()),
                contains: Some("nonexistent".to_string()),
                after: None,
                hand: None,
                by: None,
            },
        ],
    };
    let err = check_script(&rows, &script).unwrap_err();
    assert_eq!(
        err,
        vec![
            "missing-gate: no unbound row matches".to_string(),
            "missing-turn: no unbound row matches".to_string(),
        ]
    );
}

#[test]
fn an_after_naming_an_unknown_predicate_is_rejected() {
    let rows = vec![serde_json::json!({"kind": "gate", "tool": "mcp__arx__save", "verdict": "allow"})];
    let script = Script {
        card: "t".to_string(),
        operator: vec![],
        expect: vec![Expect {
            name: "save-gate".to_string(),
            kind: "gate".to_string(),
            tool: Some("save".to_string()),
            verdict: Some("allow".to_string()),
            role: None,
            contains: None,
            after: Some("no-such-predicate".to_string()),
            hand: None,
            by: None,
        }],
    };
    let err = check_script(&rows, &script).unwrap_err();
    assert_eq!(err, vec!["save-gate: after names unknown predicate no-such-predicate".to_string()]);
}

#[test]
fn an_after_naming_a_later_predicate_is_rejected() {
    let rows = vec![
        serde_json::json!({"kind": "gate", "tool": "mcp__arx__save", "verdict": "allow"}),
        serde_json::json!({"kind": "gate", "tool": "mcp__arx__dispatch", "verdict": "allow"}),
    ];
    let script = Script {
        card: "t".to_string(),
        operator: vec![],
        expect: vec![
            Expect {
                name: "save-gate".to_string(),
                kind: "gate".to_string(),
                tool: Some("save".to_string()),
                verdict: Some("allow".to_string()),
                role: None,
                contains: None,
                after: Some("dispatch-gate".to_string()),
                hand: None,
                by: None,
            },
            Expect {
                name: "dispatch-gate".to_string(),
                kind: "gate".to_string(),
                tool: Some("dispatch".to_string()),
                verdict: Some("allow".to_string()),
                role: None,
                contains: None,
                after: None,
                hand: None,
                by: None,
            },
        ],
    };
    let err = check_script(&rows, &script).unwrap_err();
    assert_eq!(
        err,
        vec!["save-gate: after names dispatch-gate, which is not earlier in the script".to_string()]
    );
}

#[test]
fn a_duplicate_predicate_name_is_rejected() {
    let rows = vec![
        serde_json::json!({"kind": "gate", "tool": "mcp__arx__save", "verdict": "allow"}),
        serde_json::json!({"kind": "gate", "tool": "mcp__arx__dispatch", "verdict": "allow"}),
    ];
    let script = Script {
        card: "t".to_string(),
        operator: vec![],
        expect: vec![
            Expect {
                name: "same-name".to_string(),
                kind: "gate".to_string(),
                tool: Some("save".to_string()),
                verdict: Some("allow".to_string()),
                role: None,
                contains: None,
                after: None,
                hand: None,
                by: None,
            },
            Expect {
                name: "same-name".to_string(),
                kind: "gate".to_string(),
                tool: Some("dispatch".to_string()),
                verdict: Some("allow".to_string()),
                role: None,
                contains: None,
                after: None,
                hand: None,
                by: None,
            },
        ],
    };
    let err = check_script(&rows, &script).unwrap_err();
    assert_eq!(err, vec!["same-name: duplicate predicate name".to_string()]);
}

#[test]
fn a_misspelled_predicate_field_is_rejected_by_deserialization() {
    let json = r#"{"name":"x","kind":"gate","contians":"typo"}"#;
    let result: Result<Expect, serde_json::Error> = serde_json::from_str(json);
    assert!(result.is_err(), "expected deserialization to reject the unknown field 'contians'");
}

#[test]
fn a_hanging_session_is_killed_at_its_deadline_instead_of_wedging_the_suite() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let sidecar_script = workspace.path().join("stub-sidecar").join("claude-main.mjs");
    let env: [(&str, String); 0] = [];

    let started = Instant::now();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_session(&run_dir, "arx.toml", &["hang".to_string()], &env)
    }));
    let elapsed = started.elapsed();

    assert!(result.is_err(), "expected a hanging session to panic instead of blocking the suite");
    assert!(elapsed < Duration::from_secs(10), "run_session took {elapsed:?}, longer than its own deadline allows");
    assert!(
        pgrep_matches(sidecar_script.to_str().unwrap()).is_empty(),
        "the stub sidecar for this session survived the deadline"
    );
}

#[test]
fn a_hanging_session_with_oversized_trailing_stdin_still_hits_its_deadline() {
    let workspace = fresh_run_fixture();
    let run_dir = workspace.path().join("run");
    let env: [(&str, String); 0] = [];

    let lines = vec!["hang".to_string(), "x".repeat(2 * 1024 * 1024)];

    let started = Instant::now();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_session(&run_dir, "arx.toml", &lines, &env)));
    let elapsed = started.elapsed();

    assert!(
        result.is_err(),
        "expected a hanging session with oversized trailing stdin to panic instead of blocking on the write"
    );
    assert!(elapsed < Duration::from_secs(10), "run_session took {elapsed:?}, longer than its own deadline allows");
}
