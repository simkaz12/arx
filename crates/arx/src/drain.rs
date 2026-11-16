use arx_engine::{Suggestion, SuggestionAction};

pub(crate) const DRAIN_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["suggestions"],
 "properties":{
   "suggestions":{"type":"array","items":{"type":"object","additionalProperties":false,
     "required":["id","action","supersedes","reason"],
     "properties":{"id":{"type":"string"},
                   "action":{"type":"string","enum":["promote","drop"]},
                   "supersedes":{"type":["string","null"]},
                   "reason":{"type":"string"}}}}}}"#;

pub(crate) const DRAIN_BRIEF: &str = "You are the drain for one thread. The candidates in this thread's quarantine, and each one's landscape, are in /hand/out/drain-input.json. A candidate's handle is turn handle.turn of /hand/threads/<thread>.jsonl; the hand traces that produced it sit beside that file. The repository is at /hand/repo. For each candidate reply promote, promote with supersedes set to an in-force claim's id when the candidate answers the same question differently, or drop when the repository or an in-force claim already says it — name the path or id in the reason. Reply with JSON under /hand/out/drain-schema.json and nothing else.";

pub(crate) fn drain_brief(note: Option<&str>) -> String {
    match note {
        Some(note) if !note.trim().is_empty() => format!("{DRAIN_BRIEF}\n\nNote from main:\n{note}"),
        _ => DRAIN_BRIEF.to_string(),
    }
}

pub(crate) fn build_drain_input(thread: &str, candidates: &[serde_json::Value]) -> String {
    let value = serde_json::json!({
        "thread": thread,
        "candidates": candidates,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalReply {
    suggestions: Vec<Suggestion>,
}

const SUGGESTION_FIELDS: [&str; 4] = ["id", "action", "supersedes", "reason"];

pub(crate) fn validate_proposal(text: &str, snapshot_ids: &[String]) -> Result<Vec<Suggestion>, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|err| format!("not JSON matching the drain schema: {err}"))?;
    let raw_suggestions = value
        .as_object()
        .and_then(|object| object.get("suggestions"))
        .and_then(|suggestions| suggestions.as_array())
        .ok_or_else(|| "not JSON matching the drain schema: missing 'suggestions' array".to_string())?;
    for (index, item) in raw_suggestions.iter().enumerate() {
        let object = item
            .as_object()
            .ok_or_else(|| format!("not JSON matching the drain schema: suggestions[{index}] is not an object"))?;
        for field in SUGGESTION_FIELDS {
            if !object.contains_key(field) {
                return Err(format!("not JSON matching the drain schema: suggestions[{index}] missing '{field}'"));
            }
        }
    }

    let parsed: ProposalReply = serde_json::from_value(value).map_err(|err| format!("not JSON matching the drain schema: {err}"))?;
    let suggestions = parsed.suggestions;

    for suggestion in &suggestions {
        if suggestion.action == SuggestionAction::Drop && suggestion.supersedes.is_some() {
            return Err(format!("{}: drop cannot carry supersedes", suggestion.id));
        }
    }

    let mut seen: Vec<&str> = Vec::new();
    for suggestion in &suggestions {
        if !snapshot_ids.iter().any(|id| id == &suggestion.id) {
            return Err(format!("{} is not in the drain snapshot", suggestion.id));
        }
        if seen.contains(&suggestion.id.as_str()) {
            return Err(format!("{} appears more than once", suggestion.id));
        }
        seen.push(suggestion.id.as_str());
    }
    for id in snapshot_ids {
        if !seen.contains(&id.as_str()) {
            return Err(format!("missing {id}"));
        }
    }

    Ok(suggestions)
}

pub(crate) fn render_proposal(hand: &str, trace: &str, suggestions: &[Suggestion]) -> String {
    let mut lines = vec![format!("[hand {hand} drain proposal] {} candidates (trace: {trace})", suggestions.len())];
    for suggestion in suggestions {
        let line = match (&suggestion.action, &suggestion.supersedes) {
            (SuggestionAction::Promote, Some(target)) => {
                format!("- {} promote superseding {} — {}", suggestion.id, target, suggestion.reason)
            }
            (SuggestionAction::Promote, None) => format!("- {} promote — {}", suggestion.id, suggestion.reason),
            (SuggestionAction::Drop, _) => format!("- {} drop — {}", suggestion.id, suggestion.reason),
        };
        lines.push(line);
    }
    lines.join("\n")
}
