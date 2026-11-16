//! The spawn ledger of `eff1f01a`, `L1-07`: a fake hand behind dispatch/tap/steer/kill.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

use serde_json::json;

use crate::gate::GateConfig;

pub type HandId = String;

#[derive(Debug, Clone, PartialEq)]
pub enum Delivery {
    Verdict {
        hand: HandId,
        text: String,
        trace: Option<String>,
        bytes: Option<u64>,
        usage: Option<serde_json::Value>,
        walls: Vec<String>,
        miss: Option<u64>,
        diff: Option<String>,
    },
    Stop {
        hand: HandId,
        question: String,
        wall: Option<String>,
    },
    Killed {
        hand: HandId,
        trace: Option<String>,
        bytes: Option<u64>,
    },
}

pub const DEFAULT_HAND_IMAGE: &str = "alpine:3.20";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Fake,
    ClaudeCode,
    Codex,
    Native,
}

impl BodyKind {
    pub fn label(&self) -> &'static str {
        match self {
            BodyKind::Fake => "fake",
            BodyKind::ClaudeCode => "claude-code",
            BodyKind::Codex => "codex",
            BodyKind::Native => "native",
        }
    }

    pub fn in_container(&self) -> bool {
        matches!(self, BodyKind::Fake | BodyKind::ClaudeCode | BodyKind::Codex | BodyKind::Native)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandRole {
    Hand,
    Runner,
    Reviewer,
    Drain,
}

impl HandRole {
    pub fn label(&self) -> &'static str {
        match self {
            HandRole::Hand => "hand",
            HandRole::Runner => "runner",
            HandRole::Reviewer => "reviewer",
            HandRole::Drain => "drain",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderSpec {
    pub kind: crate::provider::ProviderKind,
    pub base_url: String,
    pub api_key_env: String,
}

#[derive(Debug, Clone)]
pub struct HandSpec {
    pub agent: String,
    pub kind: BodyKind,
    pub role: HandRole,
    pub model: String,
    pub cwd: PathBuf,
    pub threads_dir: PathBuf,
    pub thread: String,
    pub(crate) provider: Option<ProviderSpec>,
    pub write: bool,
    pub image: String,
    pub env: Vec<String>,
    pub store_root: PathBuf,
    pub config_dir: PathBuf,
    pub tools: Vec<String>,
    pub steps: u32,
    pub window: Option<u64>,
    pub gate: GateConfig,
    pub user_gate: GateConfig,
    pub store_scope: Option<String>,
    pub drain_input: Option<String>,
    pub admission: crate::admission::AdmissionPolicy,
    pub role_line: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteerOutcome {
    Accepted,
    NotEligible,
    Refused(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandState {
    Running,
    Stopped,
    Done,
    Killed,
}

impl HandState {
    fn as_str(&self) -> &'static str {
        match self {
            HandState::Running => "running",
            HandState::Stopped => "stopped",
            HandState::Done => "done",
            HandState::Killed => "killed",
        }
    }
}

#[derive(Debug)]
pub enum LedgerError {
    BadThreadName { thread: String },
    Io { path: PathBuf, source: std::io::Error },
    Refused { message: String },
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LedgerError::BadThreadName { thread } => {
                write!(f, "invalid thread id '{thread}' — must match [A-Za-z0-9_-]+")
            }
            LedgerError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            LedgerError::Refused { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for LedgerError {}

fn validate_thread(thread: &str) -> Result<(), LedgerError> {
    let valid = !thread.is_empty()
        && thread
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(LedgerError::BadThreadName {
            thread: thread.to_string(),
        })
    }
}

fn ledger_path(root: &Path, thread: &str) -> PathBuf {
    root.join(format!("{thread}.ledger.jsonl"))
}

fn read_committed_lines(path: &Path) -> Result<Vec<String>, LedgerError> {
    let mut file = File::open(path).map_err(|source| LedgerError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|source| LedgerError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let committed = match bytes.iter().rposition(|&b| b == b'\n') {
        Some(pos) => &bytes[..pos],
        None => return Ok(Vec::new()),
    };
    Ok(committed
        .split(|&b| b == b'\n')
        .filter_map(|chunk| std::str::from_utf8(chunk).ok().map(|s| s.to_string()))
        .collect())
}

fn truncate_torn_tail(path: &Path) -> Result<(), LedgerError> {
    if !path.exists() {
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|source| LedgerError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|source| LedgerError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if bytes.is_empty() || bytes.last() == Some(&b'\n') {
        return Ok(());
    }
    let committed_len = match bytes.iter().rposition(|&b| b == b'\n') {
        Some(pos) => pos + 1,
        None => 0,
    };
    file.set_len(committed_len as u64).map_err(|source| LedgerError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn recover_next_id(path: &Path) -> Result<u64, LedgerError> {
    if !path.exists() {
        return Ok(1);
    }
    let mut max_id = 0u64;
    for line in read_committed_lines(path)? {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(row) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if row.get("kind").and_then(|k| k.as_str()) != Some("spawn") {
            continue;
        }
        let Some(hand) = row.get("hand").and_then(|h| h.as_str()) else {
            continue;
        };
        if let Some(n) = hand.strip_prefix('h').and_then(|rest| rest.parse::<u64>().ok()) {
            max_id = max_id.max(n);
        }
    }
    Ok(max_id + 1)
}

pub(crate) enum Control {
    Steer(String),
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub elapsed_ms: u64,
    pub lag_ms: u64,
}

impl Timing {
    pub fn from_instants(started: Instant, stamp: Instant, now: Instant) -> Timing {
        Timing {
            elapsed_ms: stamp.saturating_duration_since(started).as_millis() as u64,
            lag_ms: now.saturating_duration_since(stamp).as_millis() as u64,
        }
    }
}

fn delivery_hand(delivery: &Delivery) -> &str {
    match delivery {
        Delivery::Verdict { hand, .. } => hand,
        Delivery::Stop { hand, .. } => hand,
        Delivery::Killed { hand, .. } => hand,
    }
}

struct HandEntry {
    state: HandState,
    brief: String,
    pending_question: Option<String>,
    control_tx: Sender<Control>,
    terminal_delivered: bool,
    kind: BodyKind,
    agent: String,
    model: String,
    window: Option<u64>,
    started: Instant,
}

pub struct SpawnLedger {
    file: File,
    path: PathBuf,
    next_id: u64,
    hands: HashMap<HandId, HandEntry>,
    delivery_tx: Sender<(Instant, Delivery)>,
    delivery_rx: Receiver<(Instant, Delivery)>,
    pending: VecDeque<(Instant, Delivery)>,
}

impl SpawnLedger {
    pub fn open(threads_dir: &Path, thread: &str) -> Result<SpawnLedger, LedgerError> {
        validate_thread(thread)?;
        fs::create_dir_all(threads_dir).map_err(|source| LedgerError::Io {
            path: threads_dir.to_path_buf(),
            source,
        })?;
        let path = ledger_path(threads_dir, thread);
        let next_id = recover_next_id(&path)?;
        truncate_torn_tail(&path)?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| LedgerError::Io {
                path: path.clone(),
                source,
            })?;
        let (delivery_tx, delivery_rx) = mpsc::channel();
        Ok(SpawnLedger {
            file,
            path,
            next_id,
            hands: HashMap::new(),
            delivery_tx,
            delivery_rx,
            pending: VecDeque::new(),
        })
    }

    fn write_row(&mut self, value: &serde_json::Value) -> Result<(), LedgerError> {
        truncate_torn_tail(&self.path)?;
        let mut buf = serde_json::to_string(value).expect("ledger row always serializes");
        buf.push('\n');
        self.file.write_all(buf.as_bytes()).map_err(|source| LedgerError::Io {
            path: self.path.clone(),
            source,
        })?;
        self.file.flush().map_err(|source| LedgerError::Io {
            path: self.path.clone(),
            source,
        })
    }

    pub fn dispatch(&mut self, spec: HandSpec, brief: &str) -> Result<HandId, LedgerError> {
        let started = Instant::now();
        let hand = format!("h{}", self.next_id);
        let kind = spec.kind;
        let agent = spec.agent.clone();
        let model = spec.model.clone();
        let window = spec.window;

        if kind.in_container() {
            let prepared = match crate::container_hand::prepare(&spec, &hand) {
                Ok(prepared) => prepared,
                Err(message) => {
                    self.write_row(&json!({"kind": "dispatch", "agent": spec.agent, "refused": true}))?;
                    return Err(LedgerError::Refused { message });
                }
            };
            let root = prepared.root.root.display().to_string();
            self.write_row(&json!({
                "kind": "spawn",
                "hand": hand,
                "agent": spec.agent,
                "role": spec.role.label(),
                "brief": brief,
                "root": root,
                "body": kind.label(),
                "model": spec.model,
            }))?;
            self.next_id += 1;

            let (control_tx, control_rx) = mpsc::channel();
            let (inner_tx, inner_rx) = mpsc::channel::<Delivery>();
            let forward_tx = self.delivery_tx.clone();
            thread::spawn(move || {
                while let Ok(delivery) = inner_rx.recv() {
                    let _ = forward_tx.send((Instant::now(), delivery));
                }
            });
            match kind {
                BodyKind::Native => crate::container_hand::spawn_native_in_container(
                    spec,
                    hand.clone(),
                    brief.to_string(),
                    inner_tx,
                    control_rx,
                    prepared,
                ),
                _ => crate::container_hand::spawn_container_hand(
                    spec,
                    hand.clone(),
                    brief.to_string(),
                    inner_tx,
                    control_rx,
                    prepared,
                ),
            }

            self.hands.insert(
                hand.clone(),
                HandEntry {
                    state: HandState::Running,
                    brief: brief.to_string(),
                    pending_question: None,
                    control_tx,
                    terminal_delivered: false,
                    kind,
                    agent,
                    model,
                    window,
                    started,
                },
            );

            return Ok(hand);
        }

        unreachable!("every BodyKind is in_container(); this arm has no live path")
    }

    pub fn refuse_dispatch(&mut self, agent: &str, requested: &str, definition: BodyKind) -> Result<String, LedgerError> {
        self.drain_available()?;
        self.write_row(&json!({"kind": "dispatch", "agent": agent, "requested_body": requested, "body": definition.label(), "refused": true}))?;
        Ok(format!(
            "dispatch of '{agent}' names body '{requested}' but its definition says '{}'; the body is resolved from config, never per dispatch",
            definition.label()
        ))
    }

    fn is_eligible(&self, hand: &str) -> bool {
        matches!(
            self.hands.get(hand).map(|entry| entry.state),
            Some(HandState::Running) | Some(HandState::Stopped)
        )
    }

    fn drain_available(&mut self) -> Result<(), LedgerError> {
        loop {
            match self.delivery_rx.try_recv() {
                Ok(stamped) => {
                    if let Some(stamped) = self.handle_delivery(stamped)? {
                        self.pending.push_back(stamped);
                    }
                }
                Err(_) => return Ok(()),
            }
        }
    }

    pub fn steer(&mut self, hand: &str, message: &str) -> Result<SteerOutcome, LedgerError> {
        self.drain_available()?;
        if !self.is_eligible(hand) {
            return Ok(SteerOutcome::NotEligible);
        }
        let entry_kind = self.hands.get(hand).map(|entry| entry.kind);
        let entry_state = self.hands.get(hand).map(|entry| entry.state);
        if matches!(entry_kind, Some(BodyKind::ClaudeCode) | Some(BodyKind::Codex) | Some(BodyKind::Native))
            && entry_state == Some(HandState::Running)
        {
            self.write_row(&json!({"kind": "steer", "hand": hand, "message": message, "refused": true}))?;
            let body_label = entry_kind.as_ref().map(BodyKind::label).unwrap_or("claude-code");
            let refusal = format!("{hand} is a {body_label} hand mid-turn; steer is not deliverable — kill and re-dispatch");
            return Ok(SteerOutcome::Refused(refusal));
        }
        let accepted = self
            .hands
            .get(hand)
            .map(|entry| entry.control_tx.send(Control::Steer(message.to_string())).is_ok())
            .unwrap_or(false);
        if !accepted {
            return Ok(SteerOutcome::NotEligible);
        }
        self.write_row(&json!({"kind": "steer", "hand": hand, "message": message}))?;
        if let Some(entry) = self.hands.get_mut(hand) {
            if entry.state == HandState::Stopped {
                entry.state = HandState::Running;
                entry.pending_question = None;
            }
        }
        Ok(SteerOutcome::Accepted)
    }

    pub fn kill(&mut self, hand: &str) -> Result<bool, LedgerError> {
        self.drain_available()?;
        if !self.is_eligible(hand) {
            return Ok(false);
        }
        let kind = self.hands.get(hand).map(|entry| entry.kind);
        let accepted = self
            .hands
            .get(hand)
            .map(|entry| entry.control_tx.send(Control::Kill).is_ok())
            .unwrap_or(false);
        if !accepted {
            return Ok(false);
        }
        if matches!(kind, Some(kind) if kind.in_container()) {
            if let Some(entry) = self.hands.get_mut(hand) {
                entry.state = HandState::Killed;
                entry.pending_question = None;
            }
            return Ok(true);
        }
        self.write_row(&json!({"kind": "killed", "hand": hand}))?;
        if let Some(entry) = self.hands.get_mut(hand) {
            entry.state = HandState::Killed;
            entry.pending_question = None;
        }
        let _ = self.delivery_tx.send((
            Instant::now(),
            Delivery::Killed {
                hand: hand.to_string(),
                trace: None,
                bytes: None,
            },
        ));
        Ok(true)
    }

    pub fn hand_identity(&self, hand: &str) -> Option<(&str, &str)> {
        self.hands.get(hand).map(|entry| (entry.agent.as_str(), entry.model.as_str()))
    }

    pub fn tap(&self, hand: &str) -> Option<serde_json::Value> {
        let entry = self.hands.get(hand)?;
        let mut value = json!({
            "hand": hand,
            "state": entry.state.as_str(),
            "brief": entry.brief,
        });
        if let Some(question) = &entry.pending_question {
            value["stop_question"] = json!(question);
        }
        Some(value)
    }

    fn handle_delivery(&mut self, stamped: (Instant, Delivery)) -> Result<Option<(Instant, Delivery)>, LedgerError> {
        let (stamp, delivery) = stamped;
        match &delivery {
            Delivery::Verdict { hand, text, trace, usage, diff, .. } => {
                let deliverable = self
                    .hands
                    .get(hand)
                    .map(|entry| !entry.terminal_delivered && matches!(entry.state, HandState::Running | HandState::Stopped))
                    .unwrap_or(false);
                if !deliverable {
                    return Ok(None);
                }
                let mut row = json!({"kind": "verdict", "hand": hand, "text": text, "trace": trace, "usage": usage});
                if let Some(diff) = diff {
                    row["diff"] = json!(diff);
                }
                let window = self.hands.get(hand).and_then(|entry| entry.window);
                if let (Some(usage), Some(window)) = (usage, window) {
                    row["occupancy"] = json!(crate::transcript::occupancy(usage));
                    row["window"] = json!(window);
                }
                self.write_row(&row)?;
                if let Some(entry) = self.hands.get_mut(hand) {
                    entry.state = HandState::Done;
                    entry.pending_question = None;
                    entry.terminal_delivered = true;
                }
            }
            Delivery::Stop { hand, question, .. } => {
                if !self.is_eligible(hand) {
                    return Ok(None);
                }
                self.write_row(&json!({"kind": "stop", "hand": hand, "question": question}))?;
                if let Some(entry) = self.hands.get_mut(hand) {
                    entry.state = HandState::Stopped;
                    entry.pending_question = Some(question.clone());
                }
            }
            Delivery::Killed { hand, trace, .. } => {
                let already_delivered = self.hands.get(hand).map(|entry| entry.terminal_delivered).unwrap_or(true);
                if already_delivered {
                    return Ok(None);
                }
                let entry_kind = self.hands.get(hand).map(|entry| entry.kind);
                let already_recorded = !entry_kind.map(|kind| kind.in_container()).unwrap_or(false)
                    && matches!(self.hands.get(hand).map(|entry| entry.state), Some(HandState::Killed));
                if !already_recorded {
                    self.write_row(&json!({"kind": "killed", "hand": hand, "trace": trace}))?;
                }
                if let Some(entry) = self.hands.get_mut(hand) {
                    entry.state = HandState::Killed;
                    entry.pending_question = None;
                    entry.terminal_delivered = true;
                }
            }
        }
        Ok(Some((stamp, delivery)))
    }

    fn timing_for(&self, delivery: &Delivery, stamp: Instant) -> Timing {
        let hand = delivery_hand(delivery);
        let started = self.hands.get(hand).map(|entry| entry.started).unwrap_or(stamp);
        Timing::from_instants(started, stamp, Instant::now())
    }

    pub fn try_next(&mut self) -> Result<Option<Delivery>, LedgerError> {
        Ok(self.try_next_timed()?.map(|(delivery, _)| delivery))
    }

    pub fn try_next_timed(&mut self) -> Result<Option<(Delivery, Timing)>, LedgerError> {
        if let Some((stamp, delivery)) = self.pending.pop_front() {
            let timing = self.timing_for(&delivery, stamp);
            return Ok(Some((delivery, timing)));
        }
        loop {
            match self.delivery_rx.try_recv() {
                Ok(stamped) => {
                    if let Some((stamp, delivery)) = self.handle_delivery(stamped)? {
                        let timing = self.timing_for(&delivery, stamp);
                        return Ok(Some((delivery, timing)));
                    }
                }
                Err(_) => return Ok(None),
            }
        }
    }

    pub fn wait_next(&mut self) -> Result<Delivery, LedgerError> {
        let (delivery, _) = self.wait_next_timed()?;
        Ok(delivery)
    }

    pub fn wait_next_timed(&mut self) -> Result<(Delivery, Timing), LedgerError> {
        if let Some((stamp, delivery)) = self.pending.pop_front() {
            let timing = self.timing_for(&delivery, stamp);
            return Ok((delivery, timing));
        }
        loop {
            let stamped = self
                .delivery_rx
                .recv()
                .expect("the ledger keeps its own delivery sender alive for its whole life");
            if let Some((stamp, delivery)) = self.handle_delivery(stamped)? {
                let timing = self.timing_for(&delivery, stamp);
                return Ok((delivery, timing));
            }
        }
    }

    pub fn in_flight(&self) -> usize {
        self.hands.values().filter(|entry| !entry.terminal_delivered).count()
    }

    pub fn in_flight_hands(&self) -> Vec<(HandId, String)> {
        let mut hands: Vec<(HandId, String)> = self
            .hands
            .iter()
            .filter(|(_, entry)| !entry.terminal_delivered)
            .map(|(id, entry)| (id.clone(), entry.agent.clone()))
            .collect();
        hands.sort_by_key(|(id, _)| id.strip_prefix('h').and_then(|rest| rest.parse::<u64>().ok()).unwrap_or(u64::MAX));
        hands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    fn git_repo_with_one_commit() -> PathBuf {
        let dir = tempdir().unwrap().keep();
        let run = |args: &[&str]| {
            let status = Command::new("git").arg("-C").arg(&dir).args(args).status().unwrap();
            assert!(status.success(), "git {args:?} failed in {}", dir.display());
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "arx@localhost"]);
        run(&["config", "user.name", "arx"]);
        run(&["commit", "-q", "--allow-empty", "-m", "init"]);
        fs::create_dir_all(dir.join("store")).unwrap();
        dir
    }

    fn fake_spec() -> HandSpec {
        let config_dir = git_repo_with_one_commit();
        HandSpec {
            agent: "fake".to_string(),
            kind: BodyKind::Fake,
            role: HandRole::Hand,
            model: String::new(),
            cwd: config_dir.clone(),
            threads_dir: PathBuf::from("threads"),
            thread: "main".to_string(),
            provider: None,
            write: false,
            image: DEFAULT_HAND_IMAGE.to_string(),
            env: Vec::new(),
            store_root: config_dir.join("store"),
            config_dir,
            tools: Vec::new(),
            steps: crate::config::DEFAULT_STEPS,
            window: None,
            gate: GateConfig::default(),
            user_gate: GateConfig::default(),
            store_scope: None,
            drain_input: None,
            admission: crate::admission::AdmissionPolicy::default(),
            role_line: None,
        }
    }

    #[test]
    fn spawn_row_precedes_the_fake_running() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let hand = ledger.dispatch(fake_spec(), "[delay:300] slow job").unwrap();

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["kind"], "spawn");
        assert_eq!(rows[0]["hand"], hand);

        assert_eq!(ledger.try_next().unwrap(), None);
    }

    #[test]
    fn spawn_row_carries_the_body_and_model_the_hand_actually_ran_with() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let mut spec = fake_spec();
        spec.model = "gpt-5.6-sol".to_string();
        let hand = ledger.dispatch(spec, "[delay:300] slow job").unwrap();

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(rows[0]["kind"], "spawn");
        assert_eq!(rows[0]["hand"], hand);
        assert_eq!(rows[0]["body"], "fake");
        assert_eq!(rows[0]["model"], "gpt-5.6-sol");
    }

    #[test]
    fn verdict_arrives_after_delay_and_is_rowed() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let hand = ledger.dispatch(fake_spec(), "[delay:20] quick job").unwrap();

        let delivery = ledger.wait_next().unwrap();
        match delivery {
            Delivery::Verdict { hand: h, text, .. } => {
                assert_eq!(h, hand);
                assert_eq!(text, "done: [delay:20] quick job");
            }
            other => panic!("expected verdict, got {other:?}"),
        }

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        assert!(contents.lines().any(|line| line.contains("\"kind\":\"verdict\"")));
    }

    #[test]
    fn steer_text_lands_in_the_verdict() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let hand = ledger.dispatch(fake_spec(), "[delay:150] job").unwrap();

        ledger.steer(&hand, "go faster").unwrap();

        let delivery = ledger.wait_next().unwrap();
        match delivery {
            Delivery::Verdict { text, .. } => assert!(text.contains("go faster"), "text: {text}"),
            other => panic!("expected verdict, got {other:?}"),
        }
    }

    #[test]
    fn stop_holds_until_steer_releases() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let hand = ledger.dispatch(fake_spec(), "[delay:10] [stop:which db?] needs an answer").unwrap();

        let first = ledger.wait_next().unwrap();
        match &first {
            Delivery::Stop { hand: h, question, .. } => {
                assert_eq!(h, &hand);
                assert_eq!(question, "which db?");
            }
            other => panic!("expected stop, got {other:?}"),
        }

        ledger.steer(&hand, "use postgres").unwrap();

        let second = ledger.wait_next().unwrap();
        match second {
            Delivery::Verdict { text, .. } => assert!(text.contains("use postgres"), "text: {text}"),
            other => panic!("expected verdict, got {other:?}"),
        }
    }

    #[test]
    fn kill_yields_a_killed_delivery_and_row() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let hand = ledger.dispatch(fake_spec(), "[delay:60000] doomed").unwrap();

        ledger.kill(&hand).unwrap();
        let delivery = ledger.wait_next().unwrap();
        match delivery {
            Delivery::Killed { hand: h, trace, .. } => {
                assert_eq!(h, hand);
                assert!(trace.is_some(), "expected the container sweep to leave a trace handle");
            }
            other => panic!("expected killed, got {other:?}"),
        }

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        assert!(contents.lines().any(|line| line.contains("\"kind\":\"killed\"")));
    }

    #[test]
    fn ids_increment_in_dispatch_order() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let h1 = ledger.dispatch(fake_spec(), "[delay:300] a").unwrap();
        let h2 = ledger.dispatch(fake_spec(), "[delay:300] b").unwrap();
        let h3 = ledger.dispatch(fake_spec(), "[delay:300] c").unwrap();
        assert_eq!(h1, "h1");
        assert_eq!(h2, "h2");
        assert_eq!(h3, "h3");
    }

    #[test]
    fn reopening_the_ledger_recovers_the_next_id_from_committed_spawn_rows() {
        let dir = tempdir().unwrap();
        {
            let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
            ledger.dispatch(fake_spec(), "[delay:300] a").unwrap();
            ledger.dispatch(fake_spec(), "[delay:300] b").unwrap();
            ledger.dispatch(fake_spec(), "[delay:300] c").unwrap();
        }

        let mut reopened = SpawnLedger::open(dir.path(), "main").unwrap();
        let hand = reopened.dispatch(fake_spec(), "[delay:300] d").unwrap();
        assert_eq!(hand, "h4");
    }

    #[test]
    fn steer_and_kill_on_a_terminal_or_unknown_hand_are_rejected_without_a_row() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let hand = ledger.dispatch(fake_spec(), "[delay:10] quick").unwrap();
        let verdict = ledger.wait_next().unwrap();
        assert!(matches!(verdict, Delivery::Verdict { .. }));

        assert_eq!(ledger.steer(&hand, "too late").unwrap(), SteerOutcome::NotEligible);
        assert_eq!(ledger.kill(&hand).unwrap(), false);
        assert_eq!(ledger.steer("ghost", "nobody home").unwrap(), SteerOutcome::NotEligible);
        assert_eq!(ledger.kill("ghost").unwrap(), false);

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        assert!(!contents.lines().any(|line| line.contains("\"kind\":\"steer\"")));
        assert!(!contents.lines().any(|line| line.contains("\"kind\":\"killed\"")));
    }

    #[test]
    fn a_crash_reported_delivery_still_writes_the_killed_row() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let hand = ledger.dispatch(fake_spec(), "[delay:300] wedged").unwrap();

        ledger
            .delivery_tx
            .send((
                Instant::now(),
                Delivery::Killed {
                    hand: hand.clone(),
                    trace: None,
                    bytes: None,
                },
            ))
            .unwrap();

        let delivery = ledger.wait_next().unwrap();
        assert_eq!(
            delivery,
            Delivery::Killed {
                hand: hand.clone(),
                trace: None,
                bytes: None,
            }
        );

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        assert!(contents.lines().any(|line| line.contains("\"kind\":\"killed\"")));
        assert_eq!(ledger.in_flight(), 0);
    }

    #[test]
    fn an_early_steer_still_orders_after_the_stop_it_answers() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let hand = ledger.dispatch(fake_spec(), "[delay:100] [stop:which db?] needs an answer").unwrap();

        thread::sleep(Duration::from_millis(8000));
        assert_eq!(ledger.steer(&hand, "use postgres").unwrap(), SteerOutcome::Accepted);

        let stop_delivery = ledger.try_next().unwrap().unwrap();
        assert!(matches!(stop_delivery, Delivery::Stop { .. }));

        let verdict_delivery = ledger.wait_next().unwrap();
        match verdict_delivery {
            Delivery::Verdict { text, .. } => assert!(text.contains("use postgres"), "text: {text}"),
            other => panic!("expected verdict, got {other:?}"),
        }

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        let stop_idx = rows.iter().position(|row| row["kind"] == "stop").expect("stop row");
        let steer_idx = rows.iter().position(|row| row["kind"] == "steer").expect("steer row");
        let verdict_idx = rows.iter().position(|row| row["kind"] == "verdict").expect("verdict row");
        assert!(stop_idx < steer_idx, "rows: {rows:?}");
        assert!(steer_idx < verdict_idx, "rows: {rows:?}");
    }

    #[test]
    fn native_entry_steer_is_refused_mid_turn() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let (control_tx, _control_rx) = mpsc::channel();
        ledger.hands.insert(
            "h1".to_string(),
            HandEntry {
                state: HandState::Running,
                brief: "native brief".to_string(),
                pending_question: None,
                control_tx,
                terminal_delivered: false,
                kind: BodyKind::Native,
                agent: "hand-native".to_string(),
                model: "fake-model".to_string(),
                window: None,
                started: Instant::now(),
            },
        );

        let outcome = ledger.steer("h1", "go faster").unwrap();
        match outcome {
            SteerOutcome::Refused(text) => {
                assert!(text.contains("native hand mid-turn"), "text: {text}");
                assert!(text.contains("steer is not deliverable"), "text: {text}");
                assert!(text.contains("kill and re-dispatch"), "text: {text}");
            }
            other => panic!("expected refused, got {other:?}"),
        }

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        assert!(
            contents
                .lines()
                .any(|line| line.contains("\"kind\":\"steer\"") && line.contains("\"refused\":true")),
            "contents: {contents}"
        );
    }

    #[test]
    fn a_native_verdict_row_carries_occupancy_and_window() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let (control_tx, _control_rx) = mpsc::channel();
        ledger.hands.insert(
            "h1".to_string(),
            HandEntry {
                state: HandState::Running,
                brief: "native brief".to_string(),
                pending_question: None,
                control_tx,
                terminal_delivered: false,
                kind: BodyKind::Native,
                agent: "hand-ds".to_string(),
                model: "deepseek-v4-flash".to_string(),
                window: Some(1_000_000),
                started: Instant::now(),
            },
        );

        ledger
            .handle_delivery((
                Instant::now(),
                Delivery::Verdict {
                    hand: "h1".to_string(),
                    text: "done".to_string(),
                    trace: None,
                    bytes: None,
                    usage: Some(json!({"prompt_tokens": 47})),
                    walls: Vec::new(),
                    miss: None,
                    diff: None,
                },
            ))
            .unwrap();

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        let row = contents
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .find(|row| row["kind"] == "verdict")
            .expect("verdict row present");
        assert_eq!(row["occupancy"], 47);
        assert_eq!(row["window"], 1_000_000);
    }

    #[test]
    fn a_completed_write_hand_s_verdict_row_carries_the_diff_path() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let (control_tx, _control_rx) = mpsc::channel();
        ledger.hands.insert(
            "h1".to_string(),
            HandEntry {
                state: HandState::Running,
                brief: "write brief".to_string(),
                pending_question: None,
                control_tx,
                terminal_delivered: false,
                kind: BodyKind::Codex,
                agent: "hand-do".to_string(),
                model: "gpt-5.6-sol".to_string(),
                window: None,
                started: Instant::now(),
            },
        );

        ledger
            .handle_delivery((
                Instant::now(),
                Delivery::Verdict {
                    hand: "h1".to_string(),
                    text: "done".to_string(),
                    trace: Some("threads/main.h1".to_string()),
                    bytes: Some(10),
                    usage: None,
                    walls: Vec::new(),
                    miss: None,
                    diff: Some("threads/main.h1/worktree.diff".to_string()),
                },
            ))
            .unwrap();

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        let row = contents
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .find(|row| row["kind"] == "verdict")
            .expect("verdict row present");
        assert_eq!(row["diff"], "threads/main.h1/worktree.diff");
    }

    #[test]
    fn a_read_only_hand_s_verdict_row_has_no_diff_key() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let (control_tx, _control_rx) = mpsc::channel();
        ledger.hands.insert(
            "h1".to_string(),
            HandEntry {
                state: HandState::Running,
                brief: "read-only brief".to_string(),
                pending_question: None,
                control_tx,
                terminal_delivered: false,
                kind: BodyKind::Codex,
                agent: "hand-ex".to_string(),
                model: "gpt-5.6-sol".to_string(),
                window: None,
                started: Instant::now(),
            },
        );

        ledger
            .handle_delivery((
                Instant::now(),
                Delivery::Verdict {
                    hand: "h1".to_string(),
                    text: "done".to_string(),
                    trace: Some("threads/main.h1".to_string()),
                    bytes: Some(10),
                    usage: None,
                    walls: Vec::new(),
                    miss: None,
                    diff: None,
                },
            ))
            .unwrap();

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        let row = contents
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .find(|row| row["kind"] == "verdict")
            .expect("verdict row present");
        assert!(row.get("diff").is_none(), "row: {row:?}");
    }

    #[test]
    fn refuse_dispatch_writes_a_refused_row_and_mints_no_hand() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();

        let text = ledger.refuse_dispatch("hand-cc", "claude-code", BodyKind::Codex).unwrap();
        assert!(text.contains("claude-code"), "text: {text}");
        assert!(text.contains("codex"), "text: {text}");

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(rows.len(), 1, "rows: {rows:?}");
        assert_eq!(rows[0]["kind"], "dispatch");
        assert_eq!(rows[0]["refused"], true);
        assert_eq!(rows[0]["requested_body"], "claude-code");
        assert_eq!(rows[0]["body"], "codex");

        let hand = ledger.dispatch(fake_spec(), "[delay:300] a").unwrap();
        assert_eq!(hand, "h1");
    }

    #[test]
    fn in_flight_hands_lists_running_hands_with_their_agent() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();
        let h1 = ledger.dispatch(fake_spec(), "[delay:2000] a").unwrap();
        let h2 = ledger.dispatch(fake_spec(), "[delay:2000] b").unwrap();

        ledger.kill(&h1).unwrap();
        let killed = ledger.wait_next().unwrap();
        assert!(matches!(killed, Delivery::Killed { hand, .. } if hand == h1));

        assert_eq!(ledger.in_flight_hands(), vec![(h2, "fake".to_string())]);
    }

    #[test]
    fn spawn_row_carries_the_role() {
        let dir = tempdir().unwrap();
        let mut ledger = SpawnLedger::open(dir.path(), "main").unwrap();

        let mut reserved_spec = fake_spec();
        reserved_spec.role = HandRole::Reviewer;
        ledger.dispatch(reserved_spec, "[delay:300] a").unwrap();
        ledger.dispatch(fake_spec(), "[delay:300] b").unwrap();

        let contents = fs::read_to_string(dir.path().join("main.ledger.jsonl")).unwrap();
        let rows: Vec<serde_json::Value> = contents.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(rows[0]["role"], "reviewer");
        assert_eq!(rows[1]["role"], "hand");
    }
}
