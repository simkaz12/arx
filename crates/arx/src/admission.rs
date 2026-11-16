use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::events::Event;

pub const PREVIEW_HEAD_BYTES: usize = 1024;

#[derive(Debug, Clone)]
pub struct AdmissionPolicy {
    pub threshold_tokens: u64,
    pub overrides: BTreeMap<String, u64>,
}

impl Default for AdmissionPolicy {
    fn default() -> Self {
        AdmissionPolicy {
            threshold_tokens: 1000,
            overrides: BTreeMap::new(),
        }
    }
}

impl AdmissionPolicy {
    pub fn threshold_for(&self, tool: &str) -> u64 {
        self.overrides.get(tool).copied().unwrap_or(self.threshold_tokens)
    }
}

pub fn tokens(bytes: usize) -> u64 {
    (bytes as u64).div_ceil(4)
}

pub fn cut_at_char_boundary(s: &str, cap: usize) -> &str {
    if s.len() <= cap {
        return s;
    }
    let mut end = cap;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[derive(Debug, Clone)]
pub struct TraceRoot {
    pub host_dir: PathBuf,
    pub handle_prefix: String,
    pub fallback_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Provenance {
    pub source: String,
    pub sha256: Option<String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Debug, Clone)]
struct CacheEntry {
    handle: String,
    sha256: String,
    preview: String,
}

#[derive(Debug, Default)]
pub struct Cache {
    entries: HashMap<String, CacheEntry>,
}

impl Cache {
    pub fn new() -> Cache {
        Cache::default()
    }
}

fn cache_key(tool: &str, args: &serde_json::Value) -> String {
    let canonical_args = serde_json::to_string(args).unwrap_or_default();
    format!("{tool}\u{0}{canonical_args}")
}

pub struct Request<'a> {
    pub tool: &'a str,
    pub args: &'a serde_json::Value,
    pub body: &'a str,
    pub provenance: Provenance,
    pub trace_rooted: Option<String>,
    pub ranged: bool,
    pub ordinal: u64,
}

pub struct Admitted {
    pub text: String,
    pub outcome: &'static str,
    pub handle: Option<String>,
    pub events: Vec<Event>,
}

pub fn is_trace_rooted(threads_dir: &Path, canonical: &Path) -> bool {
    match canonical.strip_prefix(threads_dir) {
        Ok(rel) => rel.components().count() >= 2,
        Err(_) => false,
    }
}

pub fn slice_lines(text: &str, offset: Option<u64>, limit: Option<u64>) -> Result<String, String> {
    if text.is_empty() {
        return Ok(String::new());
    }
    let offset = offset.unwrap_or(1);
    let bytes = text.as_bytes();
    let mut lines: Vec<&str> = Vec::new();
    let mut start = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            lines.push(&text[start..=i]);
            start = i + 1;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    let total = lines.len() as u64;
    if offset == 0 || offset > total {
        return Err(format!("read_file error: offset {offset} is past the last line ({total})"));
    }
    let start_idx = (offset - 1) as usize;
    let end_idx = match limit {
        Some(l) => std::cmp::min(total, (offset - 1).saturating_add(l)) as usize,
        None => lines.len(),
    };
    Ok(lines[start_idx..end_idx].concat())
}

fn unix_seconds_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn thread_and_label_from_host_dir(host_dir: &Path) -> (String, &'static str) {
    let name = host_dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let stripped = name.strip_suffix(".traces").unwrap_or(name);
    match stripped.split_once('.') {
        Some((thread, _hand)) => (thread.to_string(), "step"),
        None => (stripped.to_string(), "turn"),
    }
}

fn frontmatter_block(tool: &str, source: &str, sha: &str, thread: &str, label: &str, ordinal: u64, captured_at: u64, body_bytes: usize, body_lines: usize) -> String {
    format!(
        "---\nproducer: externalized tool result\ntool: {tool}\nsource: {source}\nsha256: {sha}\nthread: {thread}\n{label}: {ordinal}\ncaptured_at: {captured_at}\nbytes: {body_bytes}\nlines: {body_lines}\n---\n"
    )
}

fn write_trace_file(dir: &Path, name: &str, content: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(name);
    let mut file = OpenOptions::new().create_new(true).write(true).open(&path)?;
    file.write_all(content.as_bytes())
}

fn write_or_fallback(root: &TraceRoot, name: &str, content: &str, tool: &str) -> Result<String, String> {
    match write_trace_file(&root.host_dir, name, content) {
        Ok(()) => Ok(format!("{}/{}", root.handle_prefix, name)),
        Err(_first_err) => match write_trace_file(&root.fallback_dir, name, content) {
            Ok(()) => Ok(root.fallback_dir.join(name).display().to_string()),
            Err(second_err) => Err(format!("{tool} error: could not externalize the result: {second_err}")),
        },
    }
}

fn structured_verdict_head(body: &str, cap: usize) -> String {
    match body.rfind("```yaml") {
        Some(idx) => cut_at_char_boundary(&body[idx..], cap).to_string(),
        None => cut_at_char_boundary(body, cap).to_string(),
    }
}

pub fn admit(policy: &AdmissionPolicy, cache: &mut Cache, root: &TraceRoot, seq: &mut u64, req: Request) -> Result<Admitted, String> {
    let bytes = req.body.len() as u64;
    let threshold = policy.threshold_for(req.tool);

    if tokens(req.body.len()) <= threshold {
        let mut events = Vec::new();
        if let Some(trace_path) = &req.trace_rooted {
            events.push(Event::Pull {
                handle: trace_path.clone(),
                path: req.provenance.source.clone(),
            });
        }
        events.push(Event::Admission {
            tool: req.tool.to_string(),
            outcome: "inline".to_string(),
            bytes,
            handle: None,
        });
        return Ok(Admitted {
            text: req.body.to_string(),
            outcome: "inline",
            handle: None,
            events,
        });
    }

    if req.trace_rooted.is_some() && !req.ranged {
        let handle = req.trace_rooted.clone().expect("checked is_some");
        let file_bytes = req.body.len();
        let file_lines = req.body.lines().count();
        let head = cut_at_char_boundary(req.body, PREVIEW_HEAD_BYTES);
        let text =
            format!("[arx] this result is the whole of {handle}; the handle is unchanged ({file_bytes} bytes, {file_lines} lines including the header below)\n{head}");
        return Ok(Admitted {
            text,
            outcome: "echo",
            handle: Some(handle.clone()),
            events: vec![Event::Echo { handle }],
        });
    }

    if let Some(sha) = req.provenance.sha256.clone() {
        let key = cache_key(req.tool, req.args);
        if let Some(entry) = cache.entries.get(&key) {
            if entry.sha256 == sha {
                let mut events = Vec::new();
                if let Some(trace_path) = &req.trace_rooted {
                    events.push(Event::Pull {
                        handle: trace_path.clone(),
                        path: req.provenance.source.clone(),
                    });
                }
                events.push(Event::Admission {
                    tool: req.tool.to_string(),
                    outcome: "cached".to_string(),
                    bytes,
                    handle: Some(entry.handle.clone()),
                });
                return Ok(Admitted {
                    text: entry.preview.clone(),
                    outcome: "cached",
                    handle: Some(entry.handle.clone()),
                    events,
                });
            }
        }
    }

    let body_bytes = req.body.len();
    let body_lines = req.body.lines().count();
    let (thread, label) = thread_and_label_from_host_dir(&root.host_dir);
    let sha_text = req.provenance.sha256.clone().unwrap_or_else(|| "none: always recheck".to_string());
    let frontmatter = frontmatter_block(
        req.tool,
        &req.provenance.source,
        &sha_text,
        &thread,
        label,
        req.ordinal,
        unix_seconds_now(),
        body_bytes,
        body_lines,
    );
    let content = format!("{frontmatter}{}", req.body);

    *seq += 1;
    let name = format!("{}-{}.md", req.ordinal, seq);
    let handle = write_or_fallback(root, &name, &content, req.tool)?;

    let k = frontmatter.lines().count() + 1;
    let head = cut_at_char_boundary(req.body, PREVIEW_HEAD_BYTES);
    let text = format!("[arx] externalized {} result: {handle} ({body_bytes} bytes, {body_lines} lines; body from line {k})\n{head}", req.tool);

    if let Some(sha) = req.provenance.sha256.clone() {
        cache.entries.insert(
            cache_key(req.tool, req.args),
            CacheEntry {
                handle: handle.clone(),
                sha256: sha,
                preview: text.clone(),
            },
        );
    }

    let mut events = Vec::new();
    if let Some(trace_path) = &req.trace_rooted {
        events.push(Event::Pull {
            handle: trace_path.clone(),
            path: req.provenance.source.clone(),
        });
    }
    events.push(Event::Admission {
        tool: req.tool.to_string(),
        outcome: "externalized".to_string(),
        bytes,
        handle: Some(handle.clone()),
    });

    Ok(Admitted {
        text,
        outcome: "externalized",
        handle: Some(handle),
        events,
    })
}

pub const VERDICT_TOOL: &str = "hand_verdict";

pub struct VerdictRequest<'a> {
    pub hand: &'a str,
    pub body: &'a str,
    pub ordinal: u64,
}

pub fn admit_verdict(policy: &AdmissionPolicy, root: &TraceRoot, seq: &mut u64, req: VerdictRequest) -> Result<Admitted, String> {
    let bytes = req.body.len() as u64;
    let threshold = policy.threshold_for(VERDICT_TOOL);

    if tokens(req.body.len()) <= threshold {
        return Ok(Admitted {
            text: req.body.to_string(),
            outcome: "inline",
            handle: None,
            events: vec![Event::Admission {
                tool: VERDICT_TOOL.to_string(),
                outcome: "inline".to_string(),
                bytes,
                handle: None,
            }],
        });
    }

    let body_bytes = req.body.len();
    let body_lines = req.body.lines().count();
    let (thread, label) = thread_and_label_from_host_dir(&root.host_dir);
    let frontmatter = frontmatter_block(
        VERDICT_TOOL,
        &format!("hand {} verdict", req.hand),
        "none: always recheck",
        &thread,
        label,
        req.ordinal,
        unix_seconds_now(),
        body_bytes,
        body_lines,
    );
    let content = format!("{frontmatter}{}", req.body);

    *seq += 1;
    let name = format!("{}-{}.md", req.ordinal, seq);
    let handle = write_or_fallback(root, &name, &content, VERDICT_TOOL)?;

    let k = frontmatter.lines().count() + 1;
    let head = structured_verdict_head(req.body, PREVIEW_HEAD_BYTES);
    let text = format!(
        "[arx] externalized hand {} verdict: {handle} ({body_bytes} bytes, {body_lines} lines; body from line {k})\n{head}",
        req.hand
    );

    Ok(Admitted {
        text,
        outcome: "externalized",
        handle: Some(handle.clone()),
        events: vec![Event::Admission {
            tool: VERDICT_TOOL.to_string(),
            outcome: "externalized".to_string(),
            bytes,
            handle: Some(handle),
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_in(dir: &Path, name: &str) -> TraceRoot {
        TraceRoot {
            host_dir: dir.join(name),
            handle_prefix: format!("threads/{name}"),
            fallback_dir: dir.join(format!("fallback-{name}")),
        }
    }

    #[test]
    fn threshold_for_falls_back_to_the_default() {
        let mut policy = AdmissionPolicy::default();
        assert_eq!(policy.threshold_for("read_file"), 1000);
        policy.overrides.insert("read_file".to_string(), 5000);
        assert_eq!(policy.threshold_for("read_file"), 5000);
        assert_eq!(policy.threshold_for("other_tool"), 1000);
    }

    #[test]
    fn tokens_is_bytes_over_four_rounded_up() {
        assert_eq!(tokens(0), 0);
        assert_eq!(tokens(1), 1);
        assert_eq!(tokens(4), 1);
        assert_eq!(tokens(5), 2);
        assert_eq!(tokens(4000), 1000);
        assert_eq!(tokens(4001), 1001);
    }

    #[test]
    fn an_inline_result_writes_one_row_and_no_file() {
        let policy = AdmissionPolicy::default();
        let mut cache = Cache::new();
        let dir = tempfile::tempdir().unwrap();
        let root = root_in(dir.path(), "main.traces");
        let mut seq = 0u64;
        let args = serde_json::json!({"path": "small.txt"});
        let admitted = admit(
            &policy,
            &mut cache,
            &root,
            &mut seq,
            Request {
                tool: "read_file",
                args: &args,
                body: "hello",
                provenance: Provenance {
                    source: "small.txt".to_string(),
                    sha256: Some("abc".to_string()),
                },
                trace_rooted: None,
                ranged: false,
                ordinal: 1,
            },
        )
        .unwrap();
        assert_eq!(admitted.outcome, "inline");
        assert_eq!(admitted.text, "hello");
        assert_eq!(admitted.handle, None);
        assert_eq!(
            admitted.events,
            vec![Event::Admission {
                tool: "read_file".to_string(),
                outcome: "inline".to_string(),
                bytes: 5,
                handle: None,
            }]
        );
        assert!(!root.host_dir.exists());
    }

    #[test]
    fn an_externalized_result_is_write_once_with_frontmatter_and_a_head() {
        let policy = AdmissionPolicy::default();
        let mut cache = Cache::new();
        let dir = tempfile::tempdir().unwrap();
        let root = root_in(dir.path(), "main.traces");
        let mut seq = 0u64;
        let body = "x".repeat(5000);

        let args1 = serde_json::json!({"path": "big.txt"});
        let admitted = admit(
            &policy,
            &mut cache,
            &root,
            &mut seq,
            Request {
                tool: "read_file",
                args: &args1,
                body: &body,
                provenance: Provenance {
                    source: "big.txt".to_string(),
                    sha256: Some("sha-1".to_string()),
                },
                trace_rooted: None,
                ranged: false,
                ordinal: 1,
            },
        )
        .unwrap();
        assert_eq!(admitted.outcome, "externalized");
        let handle = admitted.handle.clone().unwrap();
        assert_eq!(handle, "threads/main.traces/1-1.md");
        assert!(
            admitted
                .text
                .starts_with(&format!("[arx] externalized read_file result: {handle} (5000 bytes, 1 lines; body from line 12)")),
            "preview: {}",
            admitted.text
        );

        let file_path = root.host_dir.join("1-1.md");
        let contents = fs::read_to_string(&file_path).unwrap();
        assert!(contents.starts_with("---\nproducer: externalized tool result\n"));
        for key in ["producer", "tool", "source", "sha256", "thread", "turn", "captured_at", "bytes", "lines"] {
            assert!(contents.contains(&format!("{key}: ")), "missing {key} in {contents}");
        }
        assert!(contents.contains("tool: read_file"));
        assert!(contents.contains("source: big.txt"));
        assert!(contents.contains("sha256: sha-1"));
        assert!(contents.contains("bytes: 5000"));
        assert!(contents.contains("lines: 1"));
        assert!(contents.ends_with(&body));

        let args2 = serde_json::json!({"path": "big2.txt"});
        let admitted2 = admit(
            &policy,
            &mut cache,
            &root,
            &mut seq,
            Request {
                tool: "read_file",
                args: &args2,
                body: &body,
                provenance: Provenance {
                    source: "big2.txt".to_string(),
                    sha256: Some("sha-2".to_string()),
                },
                trace_rooted: None,
                ranged: false,
                ordinal: 1,
            },
        )
        .unwrap();
        assert_eq!(admitted2.handle.unwrap(), "threads/main.traces/1-2.md");
        assert!(root.host_dir.join("1-2.md").exists());
    }

    #[test]
    fn a_cache_hit_returns_the_first_preview_and_a_sha_change_misses() {
        let policy = AdmissionPolicy::default();
        let mut cache = Cache::new();
        let dir = tempfile::tempdir().unwrap();
        let root = root_in(dir.path(), "main.traces");
        let mut seq = 0u64;
        let body = "y".repeat(5000);
        let args = serde_json::json!({"path": "big.txt"});

        let first = admit(
            &policy,
            &mut cache,
            &root,
            &mut seq,
            Request {
                tool: "read_file",
                args: &args,
                body: &body,
                provenance: Provenance {
                    source: "big.txt".to_string(),
                    sha256: Some("sha-a".to_string()),
                },
                trace_rooted: None,
                ranged: false,
                ordinal: 1,
            },
        )
        .unwrap();
        assert_eq!(first.outcome, "externalized");

        let second = admit(
            &policy,
            &mut cache,
            &root,
            &mut seq,
            Request {
                tool: "read_file",
                args: &args,
                body: &body,
                provenance: Provenance {
                    source: "big.txt".to_string(),
                    sha256: Some("sha-a".to_string()),
                },
                trace_rooted: None,
                ranged: false,
                ordinal: 2,
            },
        )
        .unwrap();
        assert_eq!(second.outcome, "cached");
        assert_eq!(second.text, first.text);
        assert_eq!(second.handle, first.handle);
        assert_eq!(
            second.events,
            vec![Event::Admission {
                tool: "read_file".to_string(),
                outcome: "cached".to_string(),
                bytes: 5000,
                handle: first.handle.clone(),
            }]
        );

        let changed_body = "z".repeat(5000);
        let third = admit(
            &policy,
            &mut cache,
            &root,
            &mut seq,
            Request {
                tool: "read_file",
                args: &args,
                body: &changed_body,
                provenance: Provenance {
                    source: "big.txt".to_string(),
                    sha256: Some("sha-b".to_string()),
                },
                trace_rooted: None,
                ranged: false,
                ordinal: 3,
            },
        )
        .unwrap();
        assert_eq!(third.outcome, "externalized");
        assert_ne!(third.handle, first.handle);
    }

    #[test]
    fn a_whole_read_of_a_trace_over_the_threshold_is_an_echo() {
        let policy = AdmissionPolicy::default();
        let mut cache = Cache::new();
        let dir = tempfile::tempdir().unwrap();
        let root = root_in(dir.path(), "main.traces");
        let mut seq = 0u64;
        let body = "e".repeat(5000);
        let args = serde_json::json!({"path": "threads/main.traces/1-1.md"});

        let admitted = admit(
            &policy,
            &mut cache,
            &root,
            &mut seq,
            Request {
                tool: "read_file",
                args: &args,
                body: &body,
                provenance: Provenance {
                    source: "threads/main.traces/1-1.md".to_string(),
                    sha256: Some("sha-x".to_string()),
                },
                trace_rooted: Some("threads/main.traces/1-1.md".to_string()),
                ranged: false,
                ordinal: 5,
            },
        )
        .unwrap();
        assert_eq!(admitted.outcome, "echo");
        assert!(
            admitted
                .text
                .starts_with("[arx] this result is the whole of threads/main.traces/1-1.md; the handle is unchanged (5000 bytes, 1 lines including the header below)"),
            "preview: {}",
            admitted.text
        );
        assert_eq!(
            admitted.events,
            vec![Event::Echo {
                handle: "threads/main.traces/1-1.md".to_string()
            }]
        );
        assert!(seq == 0, "echo mints nothing");
        assert!(!root.host_dir.exists());
    }

    #[test]
    fn echo_and_frontmatter_counts_are_labelled_as_different_quantities() {
        let policy = AdmissionPolicy::default();
        let mut cache = Cache::new();
        let dir = tempfile::tempdir().unwrap();
        let root = root_in(dir.path(), "main.traces");
        let mut seq = 0u64;
        let body = format!("{}\n", "a".repeat(39)).repeat(1200);
        assert_eq!(body.len(), 48000);
        assert_eq!(body.lines().count(), 1200);
        let args = serde_json::json!({"path": "big.txt"});

        admit(
            &policy,
            &mut cache,
            &root,
            &mut seq,
            Request {
                tool: "read_file",
                args: &args,
                body: &body,
                provenance: Provenance {
                    source: "big.txt".to_string(),
                    sha256: Some("sha-e02".to_string()),
                },
                trace_rooted: None,
                ranged: false,
                ordinal: 1,
            },
        )
        .unwrap();

        let file_path = root.host_dir.join("1-1.md");
        let file_contents = fs::read_to_string(&file_path).unwrap();
        assert!(file_contents.contains("bytes: 48000"), "frontmatter: {file_contents}");
        assert!(file_contents.contains("lines: 1200"), "frontmatter: {file_contents}");
        let file_len = file_contents.len();
        assert_ne!(file_len, 48000, "the header must add to the body length for this to be a real regression");

        let handle = "threads/main.traces/1-1.md".to_string();
        let echoed = admit(
            &policy,
            &mut cache,
            &root,
            &mut seq,
            Request {
                tool: "read_file",
                args: &serde_json::json!({"path": handle}),
                body: &file_contents,
                provenance: Provenance {
                    source: handle.clone(),
                    sha256: Some("sha-e02".to_string()),
                },
                trace_rooted: Some(handle.clone()),
                ranged: false,
                ordinal: 2,
            },
        )
        .unwrap();

        assert_eq!(echoed.outcome, "echo");
        assert!(
            echoed.text.contains(&format!("({file_len} bytes")),
            "echo did not report the whole file's byte count: {}",
            echoed.text
        );
        assert!(
            !echoed.text.contains("(48000 bytes"),
            "echo must not report the body-only byte count unlabeled: {}",
            echoed.text
        );
        assert!(
            echoed.text.contains("including the header below"),
            "echo carries no words distinguishing its count from the frontmatter's: {}",
            echoed.text
        );
    }

    #[test]
    fn externalize_falls_back_to_tmp_when_the_session_dir_refuses() {
        let policy = AdmissionPolicy::default();
        let mut cache = Cache::new();
        let dir = tempfile::tempdir().unwrap();
        let host_dir = dir.path().join("main.traces");
        fs::write(&host_dir, b"not a directory").unwrap();
        let fallback_dir = dir.path().join("fallback-main");
        let root = TraceRoot {
            host_dir: host_dir.clone(),
            handle_prefix: "threads/main.traces".to_string(),
            fallback_dir: fallback_dir.clone(),
        };
        let mut seq = 0u64;
        let body = "f".repeat(5000);
        let args = serde_json::json!({"path": "big.txt"});

        let admitted = admit(
            &policy,
            &mut cache,
            &root,
            &mut seq,
            Request {
                tool: "read_file",
                args: &args,
                body: &body,
                provenance: Provenance {
                    source: "big.txt".to_string(),
                    sha256: Some("sha-f".to_string()),
                },
                trace_rooted: None,
                ranged: false,
                ordinal: 1,
            },
        )
        .unwrap();
        let handle = admitted.handle.unwrap();
        assert!(handle.starts_with(&fallback_dir.display().to_string()), "handle: {handle}");
        assert!(fallback_dir.join("1-1.md").exists());
    }

    #[test]
    fn slice_lines_is_one_based_and_refuses_a_past_the_end_offset() {
        let text = "one\ntwo\nthree\n";
        assert_eq!(slice_lines(text, None, None).unwrap(), text);
        assert_eq!(slice_lines(text, Some(2), None).unwrap(), "two\nthree\n");
        assert_eq!(slice_lines(text, Some(2), Some(1)).unwrap(), "two\n");
        assert_eq!(slice_lines(text, Some(0), None).unwrap_err(), "read_file error: offset 0 is past the last line (3)");
        assert_eq!(slice_lines(text, Some(10), None).unwrap_err(), "read_file error: offset 10 is past the last line (3)");
        assert_eq!(slice_lines(text, Some(2), Some(u64::MAX)).unwrap(), "two\nthree\n");
    }

    fn verdict_root_in(dir: &Path, name: &str) -> TraceRoot {
        TraceRoot {
            host_dir: dir.join(format!("{name}.traces")),
            handle_prefix: format!("threads/{name}.traces"),
            fallback_dir: dir.join(format!("fallback-{name}")),
        }
    }

    #[test]
    fn admit_verdict_inline_under_threshold_is_byte_identical() {
        let policy = AdmissionPolicy::default();
        let dir = tempfile::tempdir().unwrap();
        let root = verdict_root_in(dir.path(), "main");
        let mut seq = 0u64;
        let body = "```yaml\nverdict: PASS\ngate: ran\nmust_fix: []\n```";

        let admitted = admit_verdict(
            &policy,
            &root,
            &mut seq,
            VerdictRequest { hand: "h1", body, ordinal: 1 },
        )
        .unwrap();

        assert_eq!(admitted.outcome, "inline");
        assert_eq!(admitted.text, body);
        assert_eq!(admitted.handle, None);
        assert!(!root.host_dir.exists());
        assert_eq!(seq, 0, "an inline verdict mints no trace file");
    }

    #[test]
    fn admit_verdict_over_threshold_preserves_the_structured_head() {
        let policy = AdmissionPolicy::default();
        let dir = tempfile::tempdir().unwrap();
        let root = verdict_root_in(dir.path(), "main");
        let mut seq = 0u64;
        let prose = "x".repeat(6000);
        let fence = "```yaml\nverdict: FAIL\ngate: ran\ngate_reason: adversarial findings open\nmust_fix:\n  - fix the thing\n```\n";
        let body = format!("{prose}\n{fence}");

        let admitted = admit_verdict(
            &policy,
            &root,
            &mut seq,
            VerdictRequest { hand: "h1", body: &body, ordinal: 7 },
        )
        .unwrap();

        assert_eq!(admitted.outcome, "externalized");
        let handle = admitted.handle.clone().unwrap();
        assert_eq!(handle, "threads/main.traces/7-1.md");
        assert!(
            admitted.text.starts_with(&format!("[arx] externalized hand h1 verdict: {handle} ")),
            "preview: {}",
            admitted.text
        );
        assert!(admitted.text.contains("verdict: FAIL"), "preview missing verdict: {}", admitted.text);
        assert!(admitted.text.contains("gate: ran"), "preview missing gate: {}", admitted.text);
        assert!(admitted.text.contains("must_fix:"), "preview missing must_fix: {}", admitted.text);
        assert!(!admitted.text.contains("xxxxx"), "preview should skip the leading prose: {}", admitted.text);

        let file_path = root.host_dir.join("7-1.md");
        let contents = fs::read_to_string(&file_path).unwrap();
        assert!(contents.starts_with("---\nproducer: externalized tool result\ntool: hand_verdict\n"));
        assert!(contents.ends_with(&body), "raw verdict text must survive whole in the trace file");
    }

    #[test]
    fn admit_verdict_without_a_fence_falls_back_to_a_leading_slice() {
        let policy = AdmissionPolicy::default();
        let dir = tempfile::tempdir().unwrap();
        let root = verdict_root_in(dir.path(), "main");
        let mut seq = 0u64;
        let body = "y".repeat(6000);

        let admitted = admit_verdict(
            &policy,
            &root,
            &mut seq,
            VerdictRequest { hand: "h2", body: &body, ordinal: 1 },
        )
        .unwrap();

        assert_eq!(admitted.outcome, "externalized");
        let expected_head = cut_at_char_boundary(&body, PREVIEW_HEAD_BYTES);
        assert!(admitted.text.ends_with(expected_head), "preview: {}", admitted.text);
        assert_eq!(expected_head.len(), PREVIEW_HEAD_BYTES);
        assert!(!admitted.text.ends_with(&body), "the whole body must not be inlined once over threshold");
    }

    #[test]
    fn admit_verdict_failure_text_is_bounded_the_same_way() {
        let policy = AdmissionPolicy::default();
        let dir = tempfile::tempdir().unwrap();
        let root = verdict_root_in(dir.path(), "main");
        let mut seq = 0u64;
        let stderr_tail = "e".repeat(6000);
        let body = format!("claude -p failed — exit status: 1; stderr tail: {stderr_tail}");

        let admitted = admit_verdict(
            &policy,
            &root,
            &mut seq,
            VerdictRequest { hand: "h3", body: &body, ordinal: 1 },
        )
        .unwrap();

        assert_eq!(admitted.outcome, "externalized");
        let handle = admitted.handle.unwrap();
        let contents = fs::read_to_string(root.host_dir.join(handle.rsplit('/').next().unwrap())).unwrap();
        assert!(contents.ends_with(&body), "the full failure text must remain retrievable");
    }
}
