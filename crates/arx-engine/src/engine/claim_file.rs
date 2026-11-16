//! One claim, one markdown file: YAML frontmatter plus a fixed body shape.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Claim, ClaimHandle, ClaimKind, Lifecycle, StoreError, Validity};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frontmatter {
    id: String,
    kind: ClaimKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lifecycle: Option<Lifecycle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    validity: Option<Validity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    supersedes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    superseded_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    invalid_reason: Option<String>,
    thread: String,
    turn: u64,
    created_at: u64,
}

fn malformed(path: &Path, detail: impl Into<String>) -> StoreError {
    StoreError::MalformedClaimFile {
        path: path.to_path_buf(),
        detail: detail.into(),
    }
}

pub fn render(claim: &Claim) -> String {
    let is_ember = claim.kind == ClaimKind::Ember;
    let frontmatter = Frontmatter {
        id: claim.id.clone(),
        kind: claim.kind,
        label: if is_ember {
            Some("ember".to_string())
        } else {
            claim.label.clone()
        },
        lifecycle: if is_ember { None } else { Some(claim.lifecycle) },
        validity: if is_ember { None } else { Some(claim.validity) },
        supersedes: if is_ember { None } else { claim.supersedes.clone() },
        superseded_by: if is_ember { None } else { claim.superseded_by.clone() },
        invalid_reason: if is_ember { None } else { claim.invalid_reason.clone() },
        thread: claim.handle.thread.clone(),
        turn: claim.handle.turn,
        created_at: claim.created_at,
    };
    let yaml = serde_yaml::to_string(&frontmatter).expect("frontmatter always serializes");
    let body = if is_ember {
        claim.answer.clone()
    } else {
        format!(
            "## Question\n\n{}\n\n## Answer\n\n{}\n\n## Why\n\n{}",
            claim.question, claim.answer, claim.why
        )
    };
    format!("---\n{yaml}---\n\n{body}\n")
}

fn split_frontmatter(path: &Path, content: &str) -> Result<(String, String), StoreError> {
    let mut lines = content.lines();
    match lines.next() {
        Some("---") => {}
        _ => return Err(malformed(path, "missing opening '---' frontmatter fence")),
    }

    let mut yaml_lines: Vec<&str> = Vec::new();
    let mut closed = false;
    for line in lines.by_ref() {
        if line == "---" {
            closed = true;
            break;
        }
        yaml_lines.push(line);
    }
    if !closed {
        return Err(malformed(path, "missing closing '---' frontmatter fence"));
    }

    let body_lines: Vec<&str> = lines.collect();
    Ok((yaml_lines.join("\n"), body_lines.join("\n")))
}

fn parse_body_sections(path: &Path, raw_body: &str) -> Result<(String, String, String), StoreError> {
    let body = format!("\n{}", raw_body.trim_end());
    let q_marker = "\n## Question";
    let a_marker = "\n## Answer";
    let w_marker = "\n## Why";

    let q_pos = body.find(q_marker);
    let a_pos = body.find(a_marker);
    let w_pos = body.find(w_marker);

    let (q_pos, a_pos, w_pos) = match (q_pos, a_pos, w_pos) {
        (Some(q), Some(a), Some(w)) => (q, a, w),
        _ => return Err(malformed(path, "missing '## Question', '## Answer' or '## Why' heading")),
    };
    if !(q_pos < a_pos && a_pos < w_pos) {
        return Err(malformed(path, "headings out of order, expected Question, Answer, Why"));
    }

    let question = body[q_pos + q_marker.len()..a_pos].trim().to_string();
    let answer = body[a_pos + a_marker.len()..w_pos].trim().to_string();
    let why = body[w_pos + w_marker.len()..].trim().to_string();
    Ok((question, answer, why))
}

pub fn parse(path: &Path, content: &str) -> Result<Claim, StoreError> {
    let (yaml_text, body) = split_frontmatter(path, content)?;

    let frontmatter: Frontmatter = serde_yaml::from_str(&yaml_text)
        .map_err(|err| malformed(path, format!("invalid frontmatter: {err}")))?;

    if frontmatter.kind == ClaimKind::Ember {
        return Ok(Claim {
            id: frontmatter.id,
            kind: ClaimKind::Ember,
            label: Some("ember".to_string()),
            question: String::new(),
            answer: body.trim().to_string(),
            why: String::new(),
            handle: ClaimHandle {
                thread: frontmatter.thread,
                turn: frontmatter.turn,
            },
            lifecycle: Lifecycle::default(),
            validity: Validity::default(),
            supersedes: None,
            superseded_by: None,
            invalid_reason: None,
            created_at: frontmatter.created_at,
        });
    }

    let (question, answer, why) = parse_body_sections(path, &body)?;

    Ok(Claim {
        id: frontmatter.id,
        kind: frontmatter.kind,
        label: frontmatter.label,
        question,
        answer,
        why,
        handle: ClaimHandle {
            thread: frontmatter.thread,
            turn: frontmatter.turn,
        },
        lifecycle: frontmatter.lifecycle.unwrap_or_default(),
        validity: frontmatter.validity.unwrap_or_default(),
        supersedes: frontmatter.supersedes,
        superseded_by: frontmatter.superseded_by,
        invalid_reason: frontmatter.invalid_reason,
        created_at: frontmatter.created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_claim() -> Claim {
        Claim {
            id: "id-1".to_string(),
            kind: ClaimKind::Decision,
            label: Some("hot".to_string()),
            question: "which port".to_string(),
            answer: "4317".to_string(),
            why: "measured".to_string(),
            handle: ClaimHandle {
                thread: "main".to_string(),
                turn: 3,
            },
            lifecycle: Lifecycle::Superseded,
            validity: Validity::Invalid,
            supersedes: Some("id-0".to_string()),
            superseded_by: Some("id-2".to_string()),
            invalid_reason: Some("stale".to_string()),
            created_at: 1_700_000_000,
        }
    }

    fn bare_claim() -> Claim {
        Claim {
            id: "id-3".to_string(),
            kind: ClaimKind::Fact,
            label: None,
            question: "what colour".to_string(),
            answer: "blue".to_string(),
            why: "observed".to_string(),
            handle: ClaimHandle {
                thread: "main".to_string(),
                turn: 1,
            },
            lifecycle: Lifecycle::InForce,
            validity: Validity::Valid,
            supersedes: None,
            superseded_by: None,
            invalid_reason: None,
            created_at: 1_700_000_001,
        }
    }

    #[test]
    fn round_trips_a_claim_with_every_optional_field_set() {
        let claim = full_claim();
        let rendered = render(&claim);
        let parsed = parse(Path::new("full.md"), &rendered).unwrap();
        assert_eq!(parsed, claim);
    }

    #[test]
    fn round_trips_a_claim_with_no_optional_field_set() {
        let claim = bare_claim();
        let rendered = render(&claim);
        let parsed = parse(Path::new("bare.md"), &rendered).unwrap();
        assert_eq!(parsed, claim);
    }

    #[test]
    fn missing_opening_fence_is_malformed() {
        let err = parse(Path::new("x.md"), "no fence here").unwrap_err();
        assert!(matches!(err, StoreError::MalformedClaimFile { .. }));
    }

    #[test]
    fn missing_heading_is_malformed() {
        let text = "---\nid: id-1\nkind: fact\nthread: main\nturn: 1\ncreated_at: 1\n---\n\nno headings here\n";
        let err = parse(Path::new("x.md"), text).unwrap_err();
        assert!(matches!(err, StoreError::MalformedClaimFile { .. }));
    }

    #[test]
    fn misordered_heading_is_malformed() {
        let text = "---\nid: id-1\nkind: fact\nthread: main\nturn: 1\ncreated_at: 1\n---\n\n## Answer\n\na\n\n## Question\n\nq\n\n## Why\n\nw\n";
        let err = parse(Path::new("x.md"), text).unwrap_err();
        assert!(matches!(err, StoreError::MalformedClaimFile { .. }));
    }

    #[test]
    fn unknown_frontmatter_key_is_malformed() {
        let text = "---\nid: id-1\nkind: fact\nthread: main\nturn: 1\ncreated_at: 1\nbogus: yes\n---\n\n## Question\n\nq\n\n## Answer\n\na\n\n## Why\n\nw\n";
        let err = parse(Path::new("x.md"), text).unwrap_err();
        assert!(matches!(err, StoreError::MalformedClaimFile { .. }));
    }
}
