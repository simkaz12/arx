use std::fs;

use tempfile::tempdir;

use arx_engine::{ClaimHandle, ClaimKind, Engine, SaveIntent, Store};

#[path = "../src/state_object.rs"]
mod state_object;

use state_object::{check, entries, hash_file, latest, render, write_checked, Header, Probe, StateObject};

fn git_hash(path: &std::path::Path) -> String {
    let output = std::process::Command::new("git")
        .arg("hash-object")
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn render_then_parse_is_identity() {
    let obj = StateObject {
        header: Header {
            thread: "main".to_string(),
            checkpoint: 3,
            at: "2026-09-04T12:00:00Z".to_string(),
            turn: 7,
        },
        body: "## Position\n- file: crates/arx/src/main.rs @ 1234567890abcdef1234567890abcdef12345678\n## Target\nship the state object\n".to_string(),
    };
    let rendered = render(&obj);
    let parsed = state_object::parse(&rendered).unwrap();
    assert_eq!(parsed.header, obj.header);
    assert_eq!(parsed.body, obj.body);
}

#[test]
fn entries_pick_files_decisions_and_hands_and_skip_prose() {
    let body = "## Position\n\
- file: a.txt @ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\
- file: b.txt @ bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n\
- decision: 18b4928b\n\
- just a note, not an entry\n\
\n\
## Hands in flight\n\
- h1 hand-ex tracing the flake\n\
- h2 hand-do implementing scope 1\n\
- not a hand line\n\
- file: c.txt @ shorthash\n";

    let picked = entries(body);
    assert_eq!(
        picked.files,
        vec![
            ("a.txt".to_string(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
            ("b.txt".to_string(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
        ]
    );
    assert_eq!(picked.decisions, vec!["18b4928b".to_string()]);
    assert_eq!(
        picked.hands,
        vec![
            "h1 hand-ex tracing the flake".to_string(),
            "h2 hand-do implementing scope 1".to_string(),
        ]
    );
}

#[test]
fn hash_file_matches_git() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("sample.txt");
    fs::write(&path, "state object hashing\n").unwrap();

    let expected = git_hash(&path);
    let actual = hash_file(&path).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 40);
}

fn plant_decision(store_root: &std::path::Path, scope: &str) -> String {
    fs::create_dir_all(store_root.join(scope)).unwrap();
    let mut engine = Engine::open(store_root, "seed").unwrap();
    let handle = ClaimHandle {
        thread: "seed".to_string(),
        turn: 1,
    };
    engine
        .save_with(
            &SaveIntent {
                scope: scope.to_string(),
                kind: ClaimKind::Decision,
                question: "which sha do we use for a state object entry".to_string(),
                answer: "git hash-object".to_string(),
                why: "the crate takes no new dependency".to_string(),
                supersedes: None,
            },
            &handle,
        )
        .unwrap();
    engine.drain().unwrap();
    engine.decisions_query(scope, "sha", 1).unwrap()[0].id.clone()
}

#[test]
fn check_passes_when_every_probe_holds() {
    let root = tempdir().unwrap();
    let one = root.path().join("one.txt");
    let two = root.path().join("two.txt");
    fs::write(&one, "first file\n").unwrap();
    fs::write(&two, "second file\n").unwrap();
    let hash_one = hash_file(&one).unwrap();
    let hash_two = hash_file(&two).unwrap();

    let store_dir = tempdir().unwrap();
    let decision_id = plant_decision(store_dir.path(), "x");
    let mut engine = Engine::open(store_dir.path(), "seed").unwrap();

    let body = format!(
        "## Position\n- file: one.txt @ {hash_one}\n- file: two.txt @ {hash_two}\n- decision: {decision_id}\n"
    );

    let result = check(&body, root.path(), &mut engine, "x");
    assert!(result.is_ok());
}

#[test]
fn check_reports_every_failure_by_probe_and_entry() {
    let root = tempdir().unwrap();
    let present = root.path().join("present.txt");
    fs::write(&present, "original content\n").unwrap();
    let original_hash = hash_file(&present).unwrap();
    fs::write(&present, "rewritten content\n").unwrap();

    let store_dir = tempdir().unwrap();
    fs::create_dir_all(store_dir.path().join("x")).unwrap();
    let mut engine = Engine::open(store_dir.path(), "seed").unwrap();

    let body = format!(
        "## Position\n- file: missing.txt @ {zero}\n- file: present.txt @ {original_hash}\n- decision: not-a-real-id\n",
        zero = "0000000000000000000000000000000000000000",
    );

    let failures = check(&body, root.path(), &mut engine, "x").unwrap_err();
    assert_eq!(failures.len(), 3);
    assert_eq!(failures[0].probe, Probe::FileExists);
    assert_eq!(failures[0].entry, "file: missing.txt @ 0000000000000000000000000000000000000000");
    assert_eq!(failures[1].probe, Probe::FileHash);
    assert_eq!(failures[1].entry, format!("file: present.txt @ {original_hash}"));
    assert_eq!(failures[2].probe, Probe::DecisionResolves);
    assert_eq!(failures[2].entry, "decision: not-a-real-id");
}

#[test]
fn write_checked_writes_only_on_pass_and_never_overwrites() {
    let root = tempdir().unwrap();
    let file_path = root.path().join("hot.txt");
    fs::write(&file_path, "hot file\n").unwrap();
    let hash = hash_file(&file_path).unwrap();

    let store_dir = tempdir().unwrap();
    let decision_id = plant_decision(store_dir.path(), "x");
    let mut engine = Engine::open(store_dir.path(), "seed").unwrap();

    let checkpoints_dir = tempdir().unwrap();

    let failing_obj = StateObject {
        header: Header {
            thread: "main".to_string(),
            checkpoint: 1,
            at: "2026-09-04T12:00:00Z".to_string(),
            turn: 1,
        },
        body: "## Position\n- file: missing.txt @ 0000000000000000000000000000000000000000\n".to_string(),
    };
    let err = write_checked(checkpoints_dir.path(), &failing_obj, root.path(), &mut engine, "x").unwrap_err();
    assert_eq!(err[0].probe, Probe::FileExists);
    assert!(fs::read_dir(checkpoints_dir.path()).map(|mut it| it.next().is_none()).unwrap_or(true));

    let passing_obj = StateObject {
        header: Header {
            thread: "main".to_string(),
            checkpoint: 1,
            at: "2026-09-04T12:00:00Z".to_string(),
            turn: 1,
        },
        body: format!("## Position\n- file: hot.txt @ {hash}\n- decision: {decision_id}\n"),
    };
    let written = write_checked(checkpoints_dir.path(), &passing_obj, root.path(), &mut engine, "x").unwrap();
    assert_eq!(written, checkpoints_dir.path().join("1.md"));
    let first_contents = fs::read_to_string(&written).unwrap();
    assert_eq!(first_contents, render(&passing_obj));

    let second_err = write_checked(checkpoints_dir.path(), &passing_obj, root.path(), &mut engine, "x").unwrap_err();
    assert_eq!(second_err.len(), 1);
    assert_eq!(second_err[0].probe, Probe::FileExists);
    assert_eq!(second_err[0].detail, "already exists");
    let unchanged_contents = fs::read_to_string(&written).unwrap();
    assert_eq!(unchanged_contents, first_contents);
}

#[test]
fn latest_picks_the_highest_ordinal() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("1.md"), "one").unwrap();
    fs::write(dir.path().join("3.md"), "three").unwrap();
    fs::write(dir.path().join("2.md"), "two").unwrap();
    fs::write(dir.path().join("notes.txt"), "not a checkpoint").unwrap();

    let found = latest(dir.path()).unwrap();
    assert_eq!(found, Some((3, dir.path().join("3.md"))));

    let empty = tempdir().unwrap();
    let none = latest(empty.path()).unwrap();
    assert_eq!(none, None);
}
