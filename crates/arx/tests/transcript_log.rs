use std::fs;
use std::io::Write;

use tempfile::tempdir;

#[path = "../src/events.rs"]
mod events;
#[path = "../src/transcript.rs"]
mod transcript;
#[path = "../src/state_object.rs"]
mod state_object;

use transcript::{last_usage, resolve, Handle, TranscriptError, TranscriptLog};

fn awkward_texts(n: u64) -> Vec<String> {
    (1..=n)
        .map(|turn| format!("turn {turn}\nsecond line \"quoted\" — emoji 🚀 ünïcode"))
        .collect()
}

#[test]
fn synthetic_stream_lands_n_entries() {
    let dir = tempdir().unwrap();
    let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
    let texts = awkward_texts(10);
    for text in &texts {
        log.append_turn(text).unwrap();
    }

    let contents = fs::read_to_string(dir.path().join("alpha.jsonl")).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 10);
    for (idx, line) in lines.iter().enumerate() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(value["kind"], "turn");
        assert_eq!(value["turn"], (idx + 1) as u64);
    }
}

#[test]
fn every_handle_resolves_to_its_turn() {
    let dir = tempdir().unwrap();
    let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
    let texts = awkward_texts(10);
    let handles: Vec<Handle> = texts
        .iter()
        .map(|text| log.append_turn(text).unwrap())
        .collect();

    for (handle, text) in handles.iter().zip(texts.iter()) {
        let resolved = resolve(dir.path(), handle).unwrap();
        assert_eq!(&resolved, text);
    }
}

#[test]
fn window_truncation_leaves_log_byte_identical() {
    let dir = tempdir().unwrap();
    let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
    let mut window: Vec<String> = Vec::new();
    for turn in 1..=5 {
        let text = format!("turn {turn}");
        log.append_turn(&text).unwrap();
        window.push(text);
    }

    let path = dir.path().join("alpha.jsonl");
    let before = fs::read(&path).unwrap();
    window.truncate(2);
    let after = fs::read(&path).unwrap();
    assert_eq!(before, after);
}

#[test]
fn second_thread_gets_own_log_handles_never_cross() {
    let dir = tempdir().unwrap();
    let mut alpha = TranscriptLog::open(dir.path(), "alpha").unwrap();
    let mut beta = TranscriptLog::open(dir.path(), "beta").unwrap();

    let mut alpha_handles = Vec::new();
    let mut beta_handles = Vec::new();
    for turn in 1..=3 {
        let alpha_text = format!("alpha turn {turn}");
        let beta_text = format!("beta turn {turn}");
        alpha_handles.push((alpha.append_turn(&alpha_text).unwrap(), alpha_text));
        beta_handles.push((beta.append_turn(&beta_text).unwrap(), beta_text));
    }

    for (handle, text) in &alpha_handles {
        assert_eq!(&resolve(dir.path(), handle).unwrap(), text);
    }
    for (handle, text) in &beta_handles {
        assert_eq!(&resolve(dir.path(), handle).unwrap(), text);
    }

    let gamma_handle = Handle {
        thread: "gamma".to_string(),
        turn: 1,
    };
    let err = resolve(dir.path(), &gamma_handle).unwrap_err();
    assert!(matches!(err, TranscriptError::UnknownThread { .. }));

    let past_handle = Handle {
        thread: "beta".to_string(),
        turn: 99,
    };
    let err = resolve(dir.path(), &past_handle).unwrap_err();
    assert!(matches!(err, TranscriptError::UnresolvableTurn { .. }));
}

#[test]
fn reopen_continues_numbering() {
    let dir = tempdir().unwrap();
    let mut handles = Vec::new();
    {
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        handles.push(log.append_turn("first").unwrap());
        handles.push(log.append_turn("second").unwrap());
    }
    {
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        handles.push(log.append_turn("third").unwrap());
    }

    let turns: Vec<u64> = handles.iter().map(|h| h.turn).collect();
    assert_eq!(turns, vec![1, 2, 3]);

    let texts = ["first", "second", "third"];
    for (handle, text) in handles.iter().zip(texts.iter()) {
        assert_eq!(&resolve(dir.path(), handle).unwrap(), text);
    }
}

#[test]
fn foreign_kind_row_is_skipped() {
    let dir = tempdir().unwrap();
    let handle_one = {
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        log.append_turn("first").unwrap()
    };

    let path = dir.path().join("alpha.jsonl");
    let expected_turn_one = "{\"kind\":\"turn\",\"turn\":1,\"text\":\"first\"}\n";
    let turn_one_contents = fs::read_to_string(&path).unwrap();
    assert_eq!(turn_one_contents, expected_turn_one);

    let event_line = "{\"kind\":\"event\",\"detail\":\"placeholder\"}\n";
    {
        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        write!(file, "{event_line}").unwrap();
    }

    let handle_two = {
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        log.append_turn("second").unwrap()
    };

    assert_eq!(handle_two.turn, 2);
    assert_eq!(&resolve(dir.path(), &handle_one).unwrap(), "first");
    assert_eq!(&resolve(dir.path(), &handle_two).unwrap(), "second");

    let expected_turn_two = "{\"kind\":\"turn\",\"turn\":2,\"text\":\"second\"}\n";
    let expected = format!("{expected_turn_one}{event_line}{expected_turn_two}");
    let actual = fs::read_to_string(&path).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn unterminated_fragment_ending_mid_utf8_char_is_ignored() {
    let dir = tempdir().unwrap();
    let handle_one = {
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        let handle_one = log.append_turn("first").unwrap();
        log.append_usage(&serde_json::json!({"input_tokens": 1})).unwrap();
        handle_one
    };

    let path = dir.path().join("alpha.jsonl");
    let mut torn_prefix = br#"{"kind":"turn","turn":2,"text":""#.to_vec();
    torn_prefix.extend_from_slice(&"🚀".as_bytes()[..2]);
    {
        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&torn_prefix).unwrap();
    }

    assert_eq!(&resolve(dir.path(), &handle_one).unwrap(), "first");
    assert_eq!(
        last_usage(dir.path(), "alpha").unwrap(),
        Some(serde_json::json!({"input_tokens": 1}))
    );

    let pending_handle = Handle {
        thread: "alpha".to_string(),
        turn: 2,
    };
    let err = resolve(dir.path(), &pending_handle).unwrap_err();
    assert!(matches!(err, TranscriptError::UnresolvableTurn { .. }));
}

#[test]
fn newline_terminated_invalid_utf8_row_still_errors() {
    let dir = tempdir().unwrap();
    {
        TranscriptLog::open(dir.path(), "alpha").unwrap();
    }
    let path = dir.path().join("alpha.jsonl");
    let mut bad_row = br#"{"kind":"turn","turn":1,"text":""#.to_vec();
    bad_row.extend_from_slice(&"🚀".as_bytes()[..2]);
    bad_row.push(b'\n');
    fs::write(&path, &bad_row).unwrap();

    let handle = Handle {
        thread: "alpha".to_string(),
        turn: 1,
    };
    let err = resolve(dir.path(), &handle).unwrap_err();
    assert!(matches!(err, TranscriptError::Io { .. }));
}

#[test]
fn reopen_after_unterminated_ascii_tail_truncates_and_succeeds() {
    let dir = tempdir().unwrap();
    let handle_one = {
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        log.append_turn("first").unwrap()
    };

    let path = dir.path().join("alpha.jsonl");
    let torn = br#"{"kind":"turn","turn":2,"text":"unfinis"#;
    {
        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(torn).unwrap();
    }

    let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
    assert_eq!(log.last_handle(), Some(handle_one.clone()));

    let contents = fs::read(&path).unwrap();
    assert_eq!(contents, b"{\"kind\":\"turn\",\"turn\":1,\"text\":\"first\"}\n".to_vec());

    let handle_two = log.append_turn("second").unwrap();
    assert_eq!(handle_two.turn, 2);
    assert_eq!(&resolve(dir.path(), &handle_one).unwrap(), "first");
    assert_eq!(&resolve(dir.path(), &handle_two).unwrap(), "second");
}

#[test]
fn reopen_after_partial_utf8_tail_truncates_and_succeeds() {
    let dir = tempdir().unwrap();
    let handle_one = {
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        log.append_turn("first").unwrap()
    };

    let path = dir.path().join("alpha.jsonl");
    let mut torn = br#"{"kind":"turn","turn":2,"text":""#.to_vec();
    torn.extend_from_slice(&"🚀".as_bytes()[..2]);
    {
        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&torn).unwrap();
    }

    let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
    assert_eq!(log.last_handle(), Some(handle_one.clone()));

    let contents = fs::read(&path).unwrap();
    assert_eq!(contents, b"{\"kind\":\"turn\",\"turn\":1,\"text\":\"first\"}\n".to_vec());

    let handle_two = log.append_turn("second").unwrap();
    assert_eq!(handle_two.turn, 2);
    assert_eq!(&resolve(dir.path(), &handle_one).unwrap(), "first");
    assert_eq!(&resolve(dir.path(), &handle_two).unwrap(), "second");
}

#[test]
fn append_after_reopen_continues_committed_numbering_and_drops_torn_bytes() {
    let dir = tempdir().unwrap();
    let (handle_one, handle_two) = {
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        let handle_one = log.append_turn("first").unwrap();
        let handle_two = log.append_turn("second").unwrap();
        (handle_one, handle_two)
    };

    let path = dir.path().join("alpha.jsonl");
    let torn = br#"{"kind":"turn","turn":3,"text":"cut off mid-w"#;
    {
        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(torn).unwrap();
    }

    let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
    let handle_three = log.append_turn("third").unwrap();
    assert_eq!(handle_three.turn, 3);

    let expected = concat!(
        "{\"kind\":\"turn\",\"turn\":1,\"text\":\"first\"}\n",
        "{\"kind\":\"turn\",\"turn\":2,\"text\":\"second\"}\n",
        "{\"kind\":\"turn\",\"turn\":3,\"text\":\"third\"}\n",
    );
    let actual = fs::read_to_string(&path).unwrap();
    assert_eq!(actual, expected);

    assert_eq!(&resolve(dir.path(), &handle_one).unwrap(), "first");
    assert_eq!(&resolve(dir.path(), &handle_two).unwrap(), "second");
    assert_eq!(&resolve(dir.path(), &handle_three).unwrap(), "third");
}

#[test]
fn newline_terminated_malformed_row_is_still_fatal_on_reopen() {
    let dir = tempdir().unwrap();
    {
        TranscriptLog::open(dir.path(), "alpha").unwrap();
    }
    let path = dir.path().join("alpha.jsonl");
    fs::write(&path, "{\"kind\":\"turn\",\"turn\":1,\"text\":\"ok\"}\nnot json\n").unwrap();

    let err = TranscriptLog::open(dir.path(), "alpha").unwrap_err();
    match err {
        TranscriptError::MalformedLine { line, .. } => assert_eq!(line, 2),
        other => panic!("expected malformed line, got {other:?}"),
    }
}

#[test]
fn concurrent_open_is_refused_while_torn_tail_awaits_repair() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("alpha.jsonl");
    fs::write(&path, b"{\"kind\":\"turn\",\"turn\":1,\"text\":\"first\"}\ntorn tail no newline").unwrap();

    let mut first = TranscriptLog::open(dir.path(), "alpha").unwrap();

    let err = TranscriptLog::open(dir.path(), "alpha").unwrap_err();
    assert!(matches!(err, TranscriptError::Locked { .. }));

    let contents = fs::read(&path).unwrap();
    assert_eq!(contents, b"{\"kind\":\"turn\",\"turn\":1,\"text\":\"first\"}\n".to_vec());

    let handle_two = first.append_turn("second").unwrap();
    assert_eq!(handle_two.turn, 2);

    drop(first);
    let log = TranscriptLog::open(dir.path(), "alpha").unwrap();
    assert_eq!(log.last_handle(), Some(handle_two));
}

#[test]
fn blank_committed_row_before_first_newline_is_malformed() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("alpha.jsonl");
    fs::write(&path, b"\n").unwrap();

    let handle = Handle {
        thread: "alpha".to_string(),
        turn: 1,
    };
    let err = resolve(dir.path(), &handle).unwrap_err();
    match err {
        TranscriptError::MalformedLine { line, .. } => assert_eq!(line, 1),
        other => panic!("expected malformed line, got {other:?}"),
    }

    let err = last_usage(dir.path(), "alpha").unwrap_err();
    match err {
        TranscriptError::MalformedLine { line, .. } => assert_eq!(line, 1),
        other => panic!("expected malformed line, got {other:?}"),
    }
}

#[test]
fn blank_committed_row_is_malformed_even_with_an_unterminated_tail_after_it() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("alpha.jsonl");
    fs::write(&path, b"\n{\"kind\":\"turn\",\"turn\":2,\"text\":\"partial").unwrap();

    let handle = Handle {
        thread: "alpha".to_string(),
        turn: 1,
    };
    let err = resolve(dir.path(), &handle).unwrap_err();
    match err {
        TranscriptError::MalformedLine { line, .. } => assert_eq!(line, 1),
        other => panic!("expected malformed line, got {other:?}"),
    }
}

#[test]
fn a_state_object_header_handle_resolves_in_the_transcript_log() {
    let dir = tempdir().unwrap();
    let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
    log.append_turn_as("operator", "checkpoint now").unwrap();
    let main_handle = log.append_turn_as("main", "state object drafted").unwrap();
    log.append_turn_as("main", "truth-check complete").unwrap();

    let header = state_object::Header {
        thread: main_handle.thread.clone(),
        checkpoint: 1,
        at: "2026-09-04T00:00:00Z".to_string(),
        turn: main_handle.turn,
    };
    let rendered = state_object::render(&state_object::StateObject {
        header,
        body: "## Position\nnothing in flight\n".to_string(),
    });
    let parsed = state_object::parse(&rendered).unwrap();

    let resolved_handle = Handle {
        thread: parsed.header.thread.clone(),
        turn: parsed.header.turn,
    };
    let text = resolve(dir.path(), &resolved_handle).unwrap();
    assert_eq!(text, "state object drafted");
}

#[test]
fn occupancy_reads_openai_prompt_tokens() {
    let anthropic_shaped = serde_json::json!({
        "input_tokens": 30,
        "cache_creation_input_tokens": 5,
        "cache_read_input_tokens": 2
    });
    assert_eq!(transcript::occupancy(&anthropic_shaped), 37);

    let openai_shaped = serde_json::json!({
        "prompt_tokens": 12,
        "completion_tokens": 5,
        "total_tokens": 17
    });
    assert_eq!(transcript::occupancy(&openai_shaped), 12);

    let neither = serde_json::json!({"completion_tokens": 5});
    assert_eq!(transcript::occupancy(&neither), 0);
}
