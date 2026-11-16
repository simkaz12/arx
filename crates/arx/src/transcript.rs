//! The per-thread transcript log of `eff1f01a`.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::events::{Event, KINDS};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Handle {
    pub thread: String,
    pub turn: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Line {
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    usage: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    verdict: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    boundary: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<u64>,
}

#[derive(Debug)]
pub enum TranscriptError {
    BadThreadName {
        thread: String,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    UnknownThread {
        path: PathBuf,
    },
    UnresolvableTurn {
        thread: String,
        turn: u64,
    },
    MalformedLine {
        path: PathBuf,
        line: usize,
    },
    Locked {
        path: PathBuf,
    },
}

impl fmt::Display for TranscriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TranscriptError::BadThreadName { thread } => {
                write!(f, "invalid thread id '{thread}' — must match [A-Za-z0-9_-]+")
            }
            TranscriptError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            TranscriptError::UnknownThread { path } => {
                write!(f, "no transcript log for thread at {}", path.display())
            }
            TranscriptError::UnresolvableTurn { thread, turn } => {
                write!(f, "thread '{thread}' has no turn {turn}")
            }
            TranscriptError::MalformedLine { path, line } => {
                write!(f, "{}: malformed transcript line at line {line}", path.display())
            }
            TranscriptError::Locked { path } => {
                write!(f, "{}: transcript log is locked by another writer", path.display())
            }
        }
    }
}

impl std::error::Error for TranscriptError {}

fn validate_thread(thread: &str) -> Result<(), TranscriptError> {
    let valid = !thread.is_empty()
        && thread
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(TranscriptError::BadThreadName {
            thread: thread.to_string(),
        })
    }
}

fn log_path(root: &Path, thread: &str) -> PathBuf {
    root.join(format!("{thread}.jsonl"))
}

fn truncate_torn_tail(file: &File, path: &Path) -> Result<u64, TranscriptError> {
    let cap = fs::metadata(path)
        .map_err(|source| TranscriptError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    let mut raw = Vec::new();
    File::open(path)
        .map_err(|source| TranscriptError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .take(cap)
        .read_to_end(&mut raw)
        .map_err(|source| TranscriptError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let committed_len = match raw.iter().rposition(|&b| b == b'\n') {
        Some(pos) => (pos + 1) as u64,
        None => 0,
    };
    if committed_len < raw.len() as u64 {
        file.set_len(committed_len).map_err(|source| TranscriptError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(committed_len)
}

fn recover_next_turn(path: &Path) -> Result<u64, TranscriptError> {
    let file = File::open(path).map_err(|source| TranscriptError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = BufReader::new(file);
    let mut max_turn: u64 = 0;
    for (idx, line) in reader.lines().enumerate() {
        let line = line.map_err(|source| TranscriptError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let parsed: Line = serde_json::from_str(&line).map_err(|_| TranscriptError::MalformedLine {
            path: path.to_path_buf(),
            line: idx + 1,
        })?;
        if parsed.kind == "turn" {
            let turn = parsed.turn.ok_or_else(|| TranscriptError::MalformedLine {
                path: path.to_path_buf(),
                line: idx + 1,
            })?;
            parsed.text.ok_or_else(|| TranscriptError::MalformedLine {
                path: path.to_path_buf(),
                line: idx + 1,
            })?;
            if turn > max_turn {
                max_turn = turn;
            }
        }
    }
    Ok(max_turn + 1)
}

#[derive(Debug)]
pub struct TranscriptLog {
    thread: String,
    path: PathBuf,
    file: File,
    next_turn: u64,
}

impl TranscriptLog {
    pub fn open(root: &Path, thread: &str) -> Result<TranscriptLog, TranscriptError> {
        validate_thread(thread)?;
        fs::create_dir_all(root).map_err(|source| TranscriptError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = log_path(root, thread);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| TranscriptError::Io {
                path: path.clone(),
                source,
            })?;
        file.try_lock().map_err(|err| match err {
            TryLockError::WouldBlock => TranscriptError::Locked { path: path.clone() },
            TryLockError::Error(source) => TranscriptError::Io {
                path: path.clone(),
                source,
            },
        })?;
        let committed_len = truncate_torn_tail(&file, &path)?;
        let next_turn = if committed_len > 0 { recover_next_turn(&path)? } else { 1 };
        Ok(TranscriptLog {
            thread: thread.to_string(),
            path,
            file,
            next_turn,
        })
    }

    pub fn last_handle(&self) -> Option<Handle> {
        if self.next_turn <= 1 {
            None
        } else {
            Some(Handle {
                thread: self.thread.clone(),
                turn: self.next_turn - 1,
            })
        }
    }

    pub fn append_turn(&mut self, text: &str) -> Result<Handle, TranscriptError> {
        self.append_turn_inner(None, text)
    }

    pub fn append_turn_as(&mut self, role: &str, text: &str) -> Result<Handle, TranscriptError> {
        self.append_turn_inner(Some(role), text)
    }

    fn append_turn_inner(&mut self, role: Option<&str>, text: &str) -> Result<Handle, TranscriptError> {
        let turn = self.next_turn;
        let line = Line {
            kind: "turn".to_string(),
            turn: Some(turn),
            text: Some(text.to_string()),
            role: role.map(|r| r.to_string()),
            usage: None,
            tool: None,
            verdict: None,
            boundary: None,
            elapsed_ms: None,
        };
        self.write_line(&line)?;
        self.next_turn += 1;
        Ok(Handle {
            thread: self.thread.clone(),
            turn,
        })
    }

    pub fn append_system(&mut self, static_block: &str, dynamic: &str) -> Result<(), TranscriptError> {
        let text = format!("{static_block}\n\n{dynamic}");
        let boundary = static_block.len() + 2;
        let line = Line {
            kind: "system".to_string(),
            turn: None,
            text: Some(text),
            role: None,
            usage: None,
            tool: None,
            verdict: None,
            boundary: Some(boundary),
            elapsed_ms: None,
        };
        self.write_line(&line)
    }

    pub fn append_usage(&mut self, usage: &serde_json::Value) -> Result<(), TranscriptError> {
        self.append_usage_timed(usage, None)
    }

    pub fn append_usage_timed(&mut self, usage: &serde_json::Value, elapsed_ms: Option<u64>) -> Result<(), TranscriptError> {
        let line = Line {
            kind: "usage".to_string(),
            turn: None,
            text: None,
            role: None,
            usage: Some(usage.clone()),
            tool: None,
            verdict: None,
            boundary: None,
            elapsed_ms,
        };
        self.write_line(&line)
    }

    pub fn append_file(&mut self, text: &str) -> Result<(), TranscriptError> {
        let line = Line {
            kind: "file".to_string(),
            turn: None,
            text: Some(text.to_string()),
            role: None,
            usage: None,
            tool: None,
            verdict: None,
            boundary: None,
            elapsed_ms: None,
        };
        self.write_line(&line)
    }

    pub fn append_gate(&mut self, tool: &str, verdict: &str) -> Result<(), TranscriptError> {
        self.append_event(&Event::Gate {
            tool: tool.to_string(),
            verdict: verdict.to_string(),
            by: None,
            reason: None,
        })
    }

    pub fn append_event(&mut self, event: &Event) -> Result<(), TranscriptError> {
        let mut buf = serde_json::to_string(event).expect("event always serializes");
        buf.push('\n');
        self.write_raw(&buf)
    }

    fn write_line(&mut self, line: &Line) -> Result<(), TranscriptError> {
        let mut buf = serde_json::to_string(line).expect("transcript line always serializes");
        buf.push('\n');
        self.write_raw(&buf)
    }

    fn write_raw(&mut self, buf: &str) -> Result<(), TranscriptError> {
        self.file.write_all(buf.as_bytes()).map_err(|source| TranscriptError::Io {
            path: self.path.clone(),
            source,
        })?;
        self.file.flush().map_err(|source| TranscriptError::Io {
            path: self.path.clone(),
            source,
        })
    }
}

pub fn occupancy(usage: &serde_json::Value) -> u64 {
    if usage.get("input_tokens").and_then(|value| value.as_u64()).is_some() {
        return ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
            .iter()
            .filter_map(|field| usage.get(field).and_then(|value| value.as_u64()))
            .sum();
    }
    if let Some(prompt_tokens) = usage.get("prompt_tokens").and_then(|value| value.as_u64()) {
        return prompt_tokens;
    }
    0
}

fn read_committed_lines(path: &Path) -> Result<Vec<String>, TranscriptError> {
    let mut file = File::open(path).map_err(|source| TranscriptError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|source| TranscriptError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let committed = match bytes.iter().rposition(|&b| b == b'\n') {
        Some(pos) => &bytes[..pos],
        None => return Ok(Vec::new()),
    };
    committed
        .split(|&b| b == b'\n')
        .map(|chunk| {
            String::from_utf8(chunk.to_vec()).map_err(|source| TranscriptError::Io {
                path: path.to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
            })
        })
        .collect()
}

pub fn last_usage(root: &Path, thread: &str) -> Result<Option<serde_json::Value>, TranscriptError> {
    validate_thread(thread)?;
    let path = log_path(root, thread);
    if !path.exists() {
        return Err(TranscriptError::UnknownThread { path });
    }
    let mut found: Option<serde_json::Value> = None;
    for (idx, line) in read_committed_lines(&path)?.into_iter().enumerate() {
        let parsed: Line = serde_json::from_str(&line).map_err(|_| TranscriptError::MalformedLine {
            path: path.clone(),
            line: idx + 1,
        })?;
        if parsed.kind == "usage" {
            if let Some(usage) = parsed.usage {
                found = Some(usage);
            }
        }
    }
    Ok(found)
}

pub fn resolve(root: &Path, handle: &Handle) -> Result<String, TranscriptError> {
    validate_thread(&handle.thread)?;
    let path = log_path(root, &handle.thread);
    if !path.exists() {
        return Err(TranscriptError::UnknownThread { path });
    }
    let mut found: Option<String> = None;
    for (idx, line) in read_committed_lines(&path)?.into_iter().enumerate() {
        let parsed: Line = serde_json::from_str(&line).map_err(|_| TranscriptError::MalformedLine {
            path: path.clone(),
            line: idx + 1,
        })?;
        if parsed.kind == "turn" {
            let turn = parsed.turn.ok_or_else(|| TranscriptError::MalformedLine {
                path: path.clone(),
                line: idx + 1,
            })?;
            let text = parsed.text.ok_or_else(|| TranscriptError::MalformedLine {
                path: path.clone(),
                line: idx + 1,
            })?;
            if turn == handle.turn {
                found = Some(text);
            }
        }
    }
    found.ok_or(TranscriptError::UnresolvableTurn {
        thread: handle.thread.clone(),
        turn: handle.turn,
    })
}

pub fn events(root: &Path, thread: &str) -> Result<Vec<Event>, TranscriptError> {
    validate_thread(thread)?;
    let path = log_path(root, thread);
    if !path.exists() {
        return Err(TranscriptError::UnknownThread { path });
    }
    let mut found = Vec::new();
    for (idx, line) in read_committed_lines(&path)?.into_iter().enumerate() {
        let value: serde_json::Value = serde_json::from_str(&line).map_err(|_| TranscriptError::MalformedLine {
            path: path.clone(),
            line: idx + 1,
        })?;
        let kind = value.get("kind").and_then(|k| k.as_str()).ok_or_else(|| TranscriptError::MalformedLine {
            path: path.clone(),
            line: idx + 1,
        })?;
        if !KINDS.contains(&kind) {
            continue;
        }
        let event: Event = serde_json::from_value(value).map_err(|_| TranscriptError::MalformedLine {
            path: path.clone(),
            line: idx + 1,
        })?;
        found.push(event);
    }
    Ok(found)
}

pub fn kind_counts(root: &Path, thread: &str) -> Result<BTreeMap<String, usize>, TranscriptError> {
    validate_thread(thread)?;
    let path = log_path(root, thread);
    if !path.exists() {
        return Err(TranscriptError::UnknownThread { path });
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (idx, line) in read_committed_lines(&path)?.into_iter().enumerate() {
        let parsed: Line = serde_json::from_str(&line).map_err(|_| TranscriptError::MalformedLine {
            path: path.clone(),
            line: idx + 1,
        })?;
        *counts.entry(parsed.kind).or_insert(0) += 1;
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_thread_id_rejected_on_open() {
        let dir = tempfile::tempdir().unwrap();
        let err = TranscriptLog::open(dir.path(), "bad thread!").unwrap_err();
        assert!(matches!(err, TranscriptError::BadThreadName { .. }));
    }

    #[test]
    fn bad_thread_id_rejected_on_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let handle = Handle {
            thread: "bad thread!".to_string(),
            turn: 1,
        };
        let err = resolve(dir.path(), &handle).unwrap_err();
        assert!(matches!(err, TranscriptError::BadThreadName { .. }));
    }

    #[test]
    fn fresh_file_starts_at_turn_one() {
        let dir = tempfile::tempdir().unwrap();
        let log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        assert_eq!(log.next_turn, 1);
    }

    #[test]
    fn recovery_counts_only_turn_lines() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
            log.append_turn("first").unwrap();
        }
        let path = dir.path().join("alpha.jsonl");
        let mut contents = fs::read_to_string(&path).unwrap();
        contents.push_str("{\"kind\":\"event\",\"detail\":\"noop\"}\n");
        fs::write(&path, contents).unwrap();
        let log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        assert_eq!(log.next_turn, 2);
    }

    #[test]
    fn malformed_line_errors_with_line_number() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alpha.jsonl");
        fs::write(&path, "{\"kind\":\"turn\",\"turn\":1,\"text\":\"ok\"}\nnot json\n").unwrap();
        let err = TranscriptLog::open(dir.path(), "alpha").unwrap_err();
        match err {
            TranscriptError::MalformedLine { line, .. } => assert_eq!(line, 2),
            other => panic!("expected malformed line, got {other:?}"),
        }
    }

    #[test]
    fn resolve_surfaces_malformed_line_after_the_matched_turn() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alpha.jsonl");
        fs::write(&path, "{\"kind\":\"turn\",\"turn\":1,\"text\":\"ok\"}\nnot json\n").unwrap();
        let handle = Handle {
            thread: "alpha".to_string(),
            turn: 1,
        };
        let err = resolve(dir.path(), &handle).unwrap_err();
        match err {
            TranscriptError::MalformedLine { line, .. } => assert_eq!(line, 2),
            other => panic!("expected malformed line, got {other:?}"),
        }
    }

    #[test]
    fn resolve_surfaces_turn_row_missing_text_after_the_matched_turn() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alpha.jsonl");
        fs::write(
            &path,
            "{\"kind\":\"turn\",\"turn\":1,\"text\":\"ok\"}\n{\"kind\":\"turn\",\"turn\":2}\n",
        )
        .unwrap();
        let handle = Handle {
            thread: "alpha".to_string(),
            turn: 1,
        };
        let err = resolve(dir.path(), &handle).unwrap_err();
        match err {
            TranscriptError::MalformedLine { line, .. } => assert_eq!(line, 2),
            other => panic!("expected malformed line, got {other:?}"),
        }
    }

    #[test]
    fn open_recovery_rejects_turn_row_missing_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alpha.jsonl");
        fs::write(&path, "{\"kind\":\"turn\",\"turn\":1}\n").unwrap();
        let err = TranscriptLog::open(dir.path(), "alpha").unwrap_err();
        match err {
            TranscriptError::MalformedLine { line, .. } => assert_eq!(line, 1),
            other => panic!("expected malformed line, got {other:?}"),
        }
    }

    #[test]
    fn append_turn_omits_role() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        log.append_turn("hello").unwrap();
        let contents = fs::read_to_string(dir.path().join("alpha.jsonl")).unwrap();
        assert_eq!(contents, "{\"kind\":\"turn\",\"turn\":1,\"text\":\"hello\"}\n");
    }

    #[test]
    fn append_turn_as_serializes_role() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        log.append_turn_as("operator", "hello").unwrap();
        let contents = fs::read_to_string(dir.path().join("alpha.jsonl")).unwrap();
        assert_eq!(
            contents,
            "{\"kind\":\"turn\",\"turn\":1,\"text\":\"hello\",\"role\":\"operator\"}\n"
        );
    }

    #[test]
    fn system_and_usage_rows_skipped_by_resolve_and_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let (handle_one, handle_two) = {
            let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
            let handle_one = log.append_turn("first").unwrap();
            log.append_system("you are main", "").unwrap();
            log.append_usage(&serde_json::json!({"input_tokens": 1})).unwrap();
            let handle_two = log.append_turn("second").unwrap();
            (handle_one, handle_two)
        };
        assert_eq!(handle_one.turn, 1);
        assert_eq!(handle_two.turn, 2);

        let log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        assert_eq!(log.next_turn, 3);

        assert_eq!(&resolve(dir.path(), &handle_one).unwrap(), "first");
        assert_eq!(&resolve(dir.path(), &handle_two).unwrap(), "second");
    }

    #[test]
    fn last_usage_returns_the_latest_of_several() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        log.append_usage(&serde_json::json!({"input_tokens": 1})).unwrap();
        log.append_turn("middle").unwrap();
        log.append_usage(&serde_json::json!({"input_tokens": 2})).unwrap();
        log.append_usage(&serde_json::json!({"input_tokens": 3})).unwrap();

        let latest = last_usage(dir.path(), "alpha").unwrap();
        assert_eq!(latest, Some(serde_json::json!({"input_tokens": 3})));
    }

    #[test]
    fn last_usage_is_none_without_a_usage_row() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        log.append_turn("only turn").unwrap();

        let latest = last_usage(dir.path(), "alpha").unwrap();
        assert_eq!(latest, None);
    }

    #[test]
    fn last_usage_errors_on_unknown_thread() {
        let dir = tempfile::tempdir().unwrap();
        let err = last_usage(dir.path(), "ghost").unwrap_err();
        assert!(matches!(err, TranscriptError::UnknownThread { .. }));
    }

    #[test]
    fn append_gate_writes_tool_and_verdict() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = TranscriptLog::open(dir.path(), "alpha").unwrap();
        log.append_gate("Bash", "allow").unwrap();
        let handle = log.append_turn("after gate").unwrap();
        assert_eq!(handle.turn, 1);

        let contents = fs::read_to_string(dir.path().join("alpha.jsonl")).unwrap();
        let mut lines = contents.lines();
        assert_eq!(
            lines.next().unwrap(),
            "{\"kind\":\"gate\",\"tool\":\"Bash\",\"verdict\":\"allow\"}"
        );
        assert_eq!(
            lines.next().unwrap(),
            "{\"kind\":\"turn\",\"turn\":1,\"text\":\"after gate\"}"
        );
    }

    #[test]
    fn second_open_of_locked_thread_fails_then_succeeds_once_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let first = TranscriptLog::open(dir.path(), "alpha").unwrap();

        let err = TranscriptLog::open(dir.path(), "alpha").unwrap_err();
        match err {
            TranscriptError::Locked { path } => assert_eq!(path, dir.path().join("alpha.jsonl")),
            other => panic!("expected Locked, got {other:?}"),
        }

        drop(first);
        let mut reopened = TranscriptLog::open(dir.path(), "alpha").unwrap();
        assert_eq!(reopened.next_turn, 1);
        reopened.append_turn("after unlock").unwrap();
    }
}
