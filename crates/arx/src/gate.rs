use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde::{Deserialize, Serialize};

pub struct GateCall {
    pub tool: String,
    pub input: serde_json::Value,
}

#[derive(Debug, PartialEq)]
pub enum GateVerdict {
    Allow,
    Deny { reason: String },
}

impl Serialize for GateVerdict {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        match self {
            GateVerdict::Allow => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("permissionDecision", "allow")?;
                map.end()
            }
            GateVerdict::Deny { reason } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("permissionDecision", "deny")?;
                map.serialize_entry("permissionDecisionReason", reason)?;
                map.end()
            }
        }
    }
}

pub trait Gate {
    fn decide(&mut self, call: &GateCall) -> GateVerdict;
}

pub struct AllowAll;

impl Gate for AllowAll {
    fn decide(&mut self, _call: &GateCall) -> GateVerdict {
        GateVerdict::Allow
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GateConfig {
    pub deny: Vec<String>,
    pub ask: Vec<String>,
    pub allow: Vec<String>,
    pub hooks: Vec<String>,
}

fn subject_field(tool: &str) -> Option<&'static str> {
    match tool {
        "Bash" => Some("command"),
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => Some("file_path"),
        "WebFetch" => Some("url"),
        "WebSearch" => Some("query"),
        "Grep" | "Glob" => Some("pattern"),
        "read_file" => Some("path"),
        "list_files" => Some("path"),
        "recall" => Some("query"),
        "mcp__arx__edit_file" => Some("path"),
        "mcp__arx__read_file" => Some("path"),
        "mcp__arx__list_files" => Some("path"),
        _ => None,
    }
}

pub fn subject(call: &GateCall) -> Option<String> {
    let field = subject_field(&call.tool)?;
    Some(call.input.get(field).and_then(|value| value.as_str()).unwrap_or("").to_string())
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut match_from = 0usize;
    while ti < t.len() {
        if pi < p.len() && p[pi] != '*' && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            match_from = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            match_from += 1;
            ti = match_from;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[derive(Debug)]
pub struct Rule {
    pub tool: String,
    pub pattern: Option<String>,
    pub text: String,
}

impl Rule {
    pub fn parse(text: &str) -> Result<Rule, String> {
        let trimmed = text.trim();
        match trimmed.find('(') {
            Some(open) => {
                if !trimmed.ends_with(')') {
                    return Err("unbalanced parentheses".to_string());
                }
                let tool = trimmed[..open].trim().to_string();
                if tool.is_empty() {
                    return Err("empty tool name".to_string());
                }
                let inner = &trimmed[open + 1..trimmed.len() - 1];
                if inner.contains('(') || inner.contains(')') {
                    return Err("unbalanced parentheses".to_string());
                }
                let pattern = match inner.strip_suffix(":*") {
                    Some(prefix) => format!("{prefix}*"),
                    None => inner.to_string(),
                };
                Ok(Rule {
                    tool,
                    pattern: Some(pattern),
                    text: text.to_string(),
                })
            }
            None => {
                if trimmed.is_empty() {
                    return Err("empty tool name".to_string());
                }
                Ok(Rule {
                    tool: trimmed.to_string(),
                    pattern: None,
                    text: text.to_string(),
                })
            }
        }
    }

    pub fn matches(&self, call: &GateCall) -> bool {
        if self.tool != call.tool {
            return false;
        }
        match &self.pattern {
            None => true,
            Some(pattern) => glob_match(pattern, &subject(call).unwrap_or_default()),
        }
    }

    pub fn is_exact(&self) -> bool {
        match &self.pattern {
            None => true,
            Some(pattern) => !pattern.contains('*'),
        }
    }

    pub fn exact_for(call: &GateCall) -> String {
        match subject_field(&call.tool) {
            Some(_) => format!("{}({})", call.tool, subject(call).unwrap_or_default()),
            None => call.tool.clone(),
        }
    }
}

pub fn parse_tier_rules(rules: &[String], label: &str) -> Result<Vec<Rule>, String> {
    rules.iter().map(|text| parse_checked_rule(text, label)).collect()
}

fn parse_checked_rule(text: &str, label: &str) -> Result<Rule, String> {
    let rule = Rule::parse(text).map_err(|why| format!("{label}: rule \"{text}\": {why}"))?;
    if rule.pattern.is_some() && subject_field(&rule.tool).is_none() {
        return Err(format!(
            "{label}: rule \"{text}\": {} has no subject — use the bare tool name",
            rule.tool
        ));
    }
    Ok(rule)
}

pub struct Tier {
    label: String,
    deny: Vec<Rule>,
    allow: Vec<Rule>,
    hooks: Vec<String>,
}

pub(crate) fn reject_ask_rules(cfg: &GateConfig, label: &str) -> Result<(), String> {
    match cfg.ask.first() {
        Some(rule_text) => Err(format!(
            "{label}: rule \"{rule_text}\" in [gate] ask — \"ask\" is no longer a valid gate verdict, remove it"
        )),
        None => Ok(()),
    }
}

fn build_tier(cfg: &GateConfig, label: &str) -> Result<Tier, String> {
    reject_ask_rules(cfg, label)?;
    Ok(Tier {
        label: label.to_string(),
        deny: parse_tier_rules(&cfg.deny, label)?,
        allow: parse_tier_rules(&cfg.allow, label)?,
        hooks: cfg.hooks.clone(),
    })
}

pub struct Decision {
    pub verdict: GateVerdict,
    pub by: Option<String>,
    pub reason: Option<String>,
}

pub struct DecisionNote {
    pub by: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum HookAnswer {
    Deny(String),
    NoOpinion,
    Failed(String),
}

pub fn hook_payload(thread: &str, root: &Path, call: &GateCall) -> serde_json::Value {
    serde_json::json!({
        "hook_event_name": "PreToolUse",
        "session_id": thread,
        "cwd": root.display().to_string(),
        "tool_name": call.tool,
        "tool_input": call.input,
    })
}

pub fn run_hook(command: &str, payload: &serde_json::Value, timeout: std::time::Duration) -> HookAnswer {
    run_hook_full(command, payload, timeout).0
}

const HOOK_SIGKILL: i32 = 9;

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

fn kill_hook_process_group(child: &mut std::process::Child) {
    unsafe {
        kill(-(child.id() as i32), HOOK_SIGKILL);
    }
    let _ = child.wait();
}

fn run_hook_full(command: &str, payload: &serde_json::Value, timeout: std::time::Duration) -> (HookAnswer, bool) {
    use std::io::{Read, Write};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
    {
        Ok(child) => child,
        Err(err) => return (HookAnswer::Failed(format!("hook {command}: {err}")), false),
    };

    if let Some(mut stdin) = child.stdin.take() {
        let body = payload.to_string();
        let _ = stdin.write_all(body.as_bytes());
    }

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_handle = stdout_pipe.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            buf
        })
    });
    let stderr_handle = stderr_pipe.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            buf
        })
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    kill_hook_process_group(&mut child);
                    break None;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => break None,
        }
    };

    let stdout_bytes = stdout_handle.map(|handle| handle.join().unwrap_or_default()).unwrap_or_default();
    let stderr_bytes = stderr_handle.map(|handle| handle.join().unwrap_or_default()).unwrap_or_default();

    let Some(status) = status else {
        return (HookAnswer::Failed(format!("hook {command}: timeout")), false);
    };

    let stdout_text = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
    let stderr_text = String::from_utf8_lossy(&stderr_bytes).trim().to_string();

    match status.code() {
        Some(2) => (HookAnswer::Deny(stderr_text), false),
        Some(0) => {
            if stdout_text.is_empty() {
                return (HookAnswer::NoOpinion, false);
            }
            let value: serde_json::Value = match serde_json::from_str(&stdout_text) {
                Ok(value) => value,
                Err(_) => return (HookAnswer::Failed(format!("hook {command}: stdout not JSON")), false),
            };
            let has_updated_input = value.get("updatedInput").is_some();
            let hook_specific = value.get("hookSpecificOutput");
            let answer = if let Some(decision) = hook_specific.and_then(|h| h.get("permissionDecision")).and_then(|d| d.as_str()) {
                let reason = hook_specific
                    .and_then(|h| h.get("permissionDecisionReason"))
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string();
                match decision {
                    "deny" => HookAnswer::Deny(reason),
                    "ask" => HookAnswer::Failed(format!(
                        "hook {command}: answered \"ask\", which arx no longer honors — {reason}"
                    )),
                    _ => HookAnswer::NoOpinion,
                }
            } else if let Some(decision) = value.get("decision").and_then(|d| d.as_str()) {
                let reason = value.get("reason").and_then(|r| r.as_str()).unwrap_or("").to_string();
                match decision {
                    "block" => HookAnswer::Deny(reason),
                    _ => HookAnswer::NoOpinion,
                }
            } else {
                HookAnswer::NoOpinion
            };
            (answer, has_updated_input)
        }
        Some(code) => (HookAnswer::Failed(format!("hook {command}: exit {code}")), false),
        None => (HookAnswer::Failed(format!("hook {command}: killed by signal")), false),
    }
}

fn rule_decision(label: &str, rule: &Rule, tier: &Tier) -> Decision {
    let reason = format!("{label} \"{}\" in {}", rule.text, tier.label);
    let verdict = match label {
        "deny" => GateVerdict::Deny { reason: reason.clone() },
        _ => GateVerdict::Allow,
    };
    Decision {
        verdict,
        by: Some("rule".to_string()),
        reason: Some(reason),
    }
}

fn hook_deny_decision(command: &str, answer: &HookAnswer) -> Option<Decision> {
    match answer {
        HookAnswer::Deny(reason) => Some(Decision {
            verdict: GateVerdict::Deny { reason: reason.clone() },
            by: Some("hook".to_string()),
            reason: Some(format!("hook {command}: {reason}")),
        }),
        HookAnswer::Failed(reason) => Some(Decision {
            verdict: GateVerdict::Deny { reason: reason.clone() },
            by: Some("hook".to_string()),
            reason: Some(reason.clone()),
        }),
        _ => None,
    }
}

fn run_tier_hooks(
    tier: &Tier,
    call: &GateCall,
    thread: &str,
    root: &Path,
    updated_input_seen: &mut bool,
    updated_input_notice: &mut Option<String>,
) -> Vec<(String, HookAnswer)> {
    let mut answers = Vec::new();
    for command in &tier.hooks {
        let payload = hook_payload(thread, root, call);
        let (answer, has_updated_input) = run_hook_full(command, &payload, std::time::Duration::from_secs(5));
        if has_updated_input && !*updated_input_seen {
            *updated_input_seen = true;
            *updated_input_notice = Some(format!("[arx] gate: hook {command} returned updatedInput, ignored"));
        }
        answers.push((command.clone(), answer));
    }
    answers
}

fn tier_decide(
    tier: &Tier,
    call: &GateCall,
    thread: &str,
    root: &Path,
    updated_input_seen: &mut bool,
    updated_input_notice: &mut Option<String>,
) -> Option<Decision> {
    if let Some(rule) = tier.deny.iter().find(|rule| rule.matches(call)) {
        return Some(rule_decision("deny", rule, tier));
    }

    let hook_answers = run_tier_hooks(tier, call, thread, root, updated_input_seen, updated_input_notice);

    if let Some(decision) = hook_answers.iter().find_map(|(command, answer)| hook_deny_decision(command, answer)) {
        return Some(decision);
    }
    if let Some(rule) = tier.allow.iter().find(|rule| rule.is_exact() && rule.matches(call)) {
        return Some(rule_decision("allow", rule, tier));
    }
    if let Some(rule) = tier.allow.iter().find(|rule| rule.matches(call)) {
        return Some(rule_decision("allow", rule, tier));
    }
    None
}

pub struct Table {
    repo: Rc<RefCell<Tier>>,
    user: Tier,
    note: Rc<RefCell<Option<DecisionNote>>>,
    thread: String,
    root: PathBuf,
    updated_input_seen: bool,
    updated_input_notice: Rc<RefCell<Option<String>>>,
}

impl Table {
    pub fn new(repo: &GateConfig, repo_label: &str, user: &GateConfig, user_label: &str) -> Result<Table, String> {
        let repo_tier = build_tier(repo, repo_label)?;
        let user_tier = build_tier(user, user_label)?;
        Ok(Table {
            repo: Rc::new(RefCell::new(repo_tier)),
            user: user_tier,
            note: Rc::new(RefCell::new(None)),
            thread: String::new(),
            root: PathBuf::new(),
            updated_input_seen: false,
            updated_input_notice: Rc::new(RefCell::new(None)),
        })
    }

    pub fn set_context(&mut self, thread: &str, root: &Path) {
        self.thread = thread.to_string();
        self.root = root.to_path_buf();
    }

    pub fn note_handle(&self) -> Rc<RefCell<Option<DecisionNote>>> {
        Rc::clone(&self.note)
    }

    pub fn repo_handle(&self) -> Rc<RefCell<Tier>> {
        Rc::clone(&self.repo)
    }

    pub fn updated_input_notice_handle(&self) -> Rc<RefCell<Option<String>>> {
        Rc::clone(&self.updated_input_notice)
    }

    pub fn decide_with(&mut self, call: &GateCall) -> Decision {
        let thread = self.thread.clone();
        let root = self.root.clone();
        let mut seen = self.updated_input_seen;
        let mut notice: Option<String> = None;

        let result = {
            let repo = self.repo.borrow();
            tier_decide(&repo, call, &thread, &root, &mut seen, &mut notice)
        };
        let result = result.or_else(|| tier_decide(&self.user, call, &thread, &root, &mut seen, &mut notice));

        self.updated_input_seen = seen;
        if let Some(message) = notice {
            *self.updated_input_notice.borrow_mut() = Some(message);
        }

        result.unwrap_or(Decision {
            verdict: GateVerdict::Allow,
            by: None,
            reason: None,
        })
    }
}

impl Gate for Table {
    fn decide(&mut self, call: &GateCall) -> GateVerdict {
        let decision = self.decide_with(call);
        *self.note.borrow_mut() = Some(DecisionNote {
            by: decision.by,
            reason: decision.reason,
        });
        decision.verdict
    }
}

pub fn append_rule(path: &Path, list: &str, rule_text: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|err| format!("{}: {err}", path.display()))?;
    let gate_table = doc
        .entry("gate")
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or_else(|| format!("{}: [gate] is not a table", path.display()))?;
    let array = gate_table
        .entry(list)
        .or_insert(toml_edit::Item::Value(toml_edit::Value::Array(toml_edit::Array::new())))
        .as_array_mut()
        .ok_or_else(|| format!("{}: [gate].{list} is not an array", path.display()))?;
    array.push(rule_text);
    std::fs::write(path, doc.to_string()).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_serializes_to_pre_tool_use_json() {
        assert_eq!(
            serde_json::to_value(GateVerdict::Allow).unwrap(),
            serde_json::json!({"permissionDecision": "allow"})
        );
        assert_eq!(
            serde_json::to_value(GateVerdict::Deny {
                reason: "no".to_string()
            })
            .unwrap(),
            serde_json::json!({"permissionDecision": "deny", "permissionDecisionReason": "no"})
        );
    }

    #[test]
    fn a_caller_matches_both_verdicts() {
        for verdict in [
            GateVerdict::Allow,
            GateVerdict::Deny {
                reason: "denied".to_string(),
            },
        ] {
            let label = match verdict {
                GateVerdict::Allow => "allow",
                GateVerdict::Deny { .. } => "deny",
            };
            assert!(!label.is_empty());
        }
    }

    #[test]
    fn a_repo_rule_naming_ask_fails_to_load_naming_the_file_and_the_rule() {
        let repo = GateConfig {
            ask: vec!["Bash(git push*)".to_string()],
            ..GateConfig::default()
        };
        let user = GateConfig::default();
        let err = Table::new(&repo, "./arx.toml", &user, "user.toml").err().expect("ask rule must fail to load");
        assert!(err.contains("./arx.toml"), "err: {err}");
        assert!(err.contains("Bash(git push*)"), "err: {err}");
    }

    #[test]
    fn allow_all_allows_any_call() {
        let mut gate = AllowAll;
        let call = GateCall {
            tool: "Bash".to_string(),
            input: serde_json::json!({"command": "ls"}),
        };
        assert_eq!(gate.decide(&call), GateVerdict::Allow);
    }
}
