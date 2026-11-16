use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use arx_engine::Store;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub thread: String,
    pub checkpoint: u64,
    pub at: String,
    pub turn: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateObject {
    pub header: Header,
    pub body: String,
}

pub fn checkpoints_dir(threads_dir: &Path, thread: &str) -> PathBuf {
    threads_dir.join(format!("{thread}.checkpoints"))
}

pub fn latest(dir: &Path) -> io::Result<Option<(u64, PathBuf)>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(ordinal) = stem.parse::<u64>() else {
            continue;
        };
        let better = match &best {
            Some((current, _)) => ordinal > *current,
            None => true,
        };
        if better {
            best = Some((ordinal, path));
        }
    }
    Ok(best)
}

pub fn render(obj: &StateObject) -> String {
    let yaml = serde_yaml::to_string(&obj.header).expect("header always serializes");
    format!("---\n{yaml}---\n{}", obj.body)
}

pub fn parse(text: &str) -> Result<StateObject, String> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next().ok_or_else(|| "state object: empty file".to_string())?;
    if first.trim_end_matches(['\n', '\r']) != "---" {
        return Err("state object: missing opening ---".to_string());
    }
    let mut consumed = first.len();
    let mut yaml = String::new();
    let mut closed = false;
    for line in text[consumed..].split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']) == "---" {
            consumed += line.len();
            closed = true;
            break;
        }
        yaml.push_str(line);
        consumed += line.len();
    }
    if !closed {
        return Err("state object: missing closing ---".to_string());
    }
    let header: Header = serde_yaml::from_str(&yaml).map_err(|err| err.to_string())?;
    let body = text[consumed..].to_string();
    Ok(StateObject { header, body })
}

enum ProvEntry {
    File(String, String),
    Decision(String),
}

fn is_lowercase_hex40(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn is_hand_id(s: &str) -> bool {
    match s.strip_prefix('h') {
        Some(rest) => !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

fn scan_body(body: &str) -> (Vec<ProvEntry>, Vec<String>) {
    let mut provenance = Vec::new();
    let mut hands = Vec::new();
    let mut in_hands = false;
    for line in body.lines() {
        let trimmed = line.trim_end();
        if let Some(heading) = trimmed.strip_prefix("## ") {
            in_hands = heading.trim() == "Hands in flight";
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- file: ") {
            if let Some((path, hash)) = rest.rsplit_once(" @ ") {
                if is_lowercase_hex40(hash) {
                    provenance.push(ProvEntry::File(path.to_string(), hash.to_string()));
                }
            }
            continue;
        }
        if let Some(id) = trimmed.strip_prefix("- decision: ") {
            provenance.push(ProvEntry::Decision(id.to_string()));
            continue;
        }
        if in_hands {
            if let Some(rest) = trimmed.strip_prefix("- ") {
                let id = rest.split(' ').next().unwrap_or("");
                if is_hand_id(id) {
                    hands.push(rest.to_string());
                }
            }
        }
    }
    (provenance, hands)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entries {
    pub files: Vec<(String, String)>,
    pub decisions: Vec<String>,
    pub hands: Vec<String>,
}

pub fn entries(body: &str) -> Entries {
    let (provenance, hands) = scan_body(body);
    let mut out = Entries {
        hands,
        ..Entries::default()
    };
    for entry in provenance {
        match entry {
            ProvEntry::File(path, hash) => out.files.push((path, hash)),
            ProvEntry::Decision(id) => out.decisions.push(id),
        }
    }
    out
}

pub fn hash_file(path: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("hash-object")
        .arg(path)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    FileExists,
    FileHash,
    DecisionResolves,
}

#[derive(Debug, PartialEq)]
pub struct CheckFailure {
    pub probe: Probe,
    pub entry: String,
    pub detail: String,
}

pub fn check(body: &str, root: &Path, store: &mut dyn Store, scope: &str) -> Result<(), Vec<CheckFailure>> {
    let (provenance, _hands) = scan_body(body);
    let mut failures = Vec::new();

    for entry in provenance {
        match entry {
            ProvEntry::File(path, hash) => {
                let label = format!("file: {path} @ {hash}");
                let full = root.join(&path);
                if !full.is_file() {
                    failures.push(CheckFailure {
                        probe: Probe::FileExists,
                        entry: label,
                        detail: format!("{} does not exist", full.display()),
                    });
                    continue;
                }
                match hash_file(&full) {
                    Ok(actual) if actual == hash => {}
                    Ok(actual) => failures.push(CheckFailure {
                        probe: Probe::FileHash,
                        entry: label,
                        detail: format!("expected {hash}, found {actual}"),
                    }),
                    Err(detail) => failures.push(CheckFailure {
                        probe: Probe::FileHash,
                        entry: label,
                        detail,
                    }),
                }
            }
            ProvEntry::Decision(id) => {
                let label = format!("decision: {id}");
                match store.decision(scope, &id) {
                    Ok(Some(_)) => {}
                    Ok(None) => failures.push(CheckFailure {
                        probe: Probe::DecisionResolves,
                        entry: label,
                        detail: "no decision with that id in scope".to_string(),
                    }),
                    Err(err) => failures.push(CheckFailure {
                        probe: Probe::DecisionResolves,
                        entry: label,
                        detail: err.to_string(),
                    }),
                }
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

pub fn write_checked(
    dir: &Path,
    obj: &StateObject,
    root: &Path,
    store: &mut dyn Store,
    scope: &str,
) -> Result<PathBuf, Vec<CheckFailure>> {
    check(&obj.body, root, store, scope)?;
    let target = dir.join(format!("{}.md", obj.header.checkpoint));
    if target.exists() {
        return Err(vec![CheckFailure {
            probe: Probe::FileExists,
            entry: target.display().to_string(),
            detail: "already exists".to_string(),
        }]);
    }
    std::fs::create_dir_all(dir).expect("write_checked: create checkpoints dir");
    std::fs::write(&target, render(obj)).expect("write_checked: write state object");
    Ok(target)
}
