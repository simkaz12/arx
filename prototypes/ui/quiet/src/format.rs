use crate::scenario::{Admission, DeliveryOutcome, Role};

const ASK_SUBJECT_LIMIT: usize = 200;

pub fn ask_subject_display(subject: &str) -> String {
    let replaced: String = subject.chars().map(|c| if c == '\n' { '⏎' } else { c }).collect();
    if replaced.chars().count() > ASK_SUBJECT_LIMIT {
        let truncated: String = replaced.chars().take(ASK_SUBJECT_LIMIT).collect();
        format!("{truncated}…")
    } else {
        replaced
    }
}

pub fn kb(bytes: u64) -> String {
    format!("{:.1} KB", bytes as f64 / 1000.0)
}

pub fn role_label(role: Role) -> &'static str {
    match role {
        Role::Hand => "hand",
        Role::Judge => "judge",
    }
}

pub fn tool_call_line(tool: &str, subject: &str) -> String {
    format!("{tool} {}", ask_subject_display(subject))
}

pub fn admission_lines(admission: &Admission) -> (String, Option<String>) {
    match admission {
        Admission::Inline { bytes } => (format!("inline {}", kb(*bytes)), None),
        Admission::Externalized { bytes, handle, preview } => {
            (format!("externalized {} → {handle}", kb(*bytes)), Some(preview.clone()))
        }
    }
}

pub fn dispatch_line(hand_id: &str, agent: &str, body: &str, model: &str) -> String {
    format!("{hand_id} {agent} · {body} · {model}")
}

pub fn delivery_line(hand_id: &str, outcome: DeliveryOutcome, elapsed_ms: u64, trace: &str) -> String {
    format!("{hand_id} {} {:.1}s → {trace}", outcome.label(), elapsed_ms as f64 / 1000.0)
}

pub fn model_list_line(models: &[String], current: &str) -> String {
    models
        .iter()
        .map(|m| if m == current { format!("{m} (current)") } else { m.clone() })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(20);
    let mut lines = Vec::new();
    for para in text.split('\n') {
        if para.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in para.split(' ') {
            if current.is_empty() {
                current.push_str(word);
            } else if current.chars().count() + 1 + word.chars().count() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(std::mem::take(&mut current));
                current.push_str(word);
            }
        }
        lines.push(current);
    }
    lines
}

pub fn markdown_line(raw_line: &str, width: usize, in_fence: &mut bool) -> Vec<String> {
    if raw_line.trim_start().starts_with("```") {
        *in_fence = !*in_fence;
        return Vec::new();
    }
    if *in_fence {
        return vec![format!("    {raw_line}")];
    }
    if let Some(rest) = raw_line.strip_prefix("# ").or_else(|| raw_line.strip_prefix("## ")) {
        return vec![format!("\x1b[1m{rest}\x1b[0m")];
    }
    if let Some(rest) = raw_line.strip_prefix("- ") {
        let wrapped = wrap(&inline_style(rest), width.saturating_sub(2));
        return wrapped
            .into_iter()
            .enumerate()
            .map(|(i, w)| if i == 0 { format!("• {w}") } else { format!("  {w}") })
            .collect();
    }
    wrap(&inline_style(raw_line), width)
}

pub fn markdown_lite(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for raw_line in text.split('\n') {
        out.extend(markdown_line(raw_line, width, &mut in_fence));
    }
    out
}

fn inline_style(line: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for part in line.split('`') {
        if in_code {
            out.push_str("\x1b[36m`");
            out.push_str(part);
            out.push_str("`\x1b[0m");
        } else {
            out.push_str(part);
        }
        in_code = !in_code;
    }
    out
}

pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c2 in chars.by_ref() {
                    if c2.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}
