use std::collections::BTreeMap;

use crate::config::{Body, ProviderConfig, RoleRow};
use crate::provider::ProviderKind;

pub(crate) const REVIEWER_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["verdict","summary","must_fix"],
 "properties":{
   "verdict":{"type":"string","enum":["PASS","FAIL"]},
   "summary":{"type":"string"},
   "must_fix":{"type":"array","items":{"type":"string"}}}}"#;

const TOP_FIELDS: [&str; 3] = ["verdict", "summary", "must_fix"];

pub(crate) fn validate_verdict(text: &str) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|err| format!("not JSON: {err}"))?;
    let obj = value.as_object().ok_or_else(|| "not an object".to_string())?;

    for key in obj.keys() {
        if !TOP_FIELDS.contains(&key.as_str()) {
            return Err(format!("unexpected field {key}"));
        }
    }
    for field in TOP_FIELDS {
        if !obj.contains_key(field) {
            return Err(format!("missing field {field}"));
        }
    }

    let verdict = obj["verdict"].as_str().ok_or_else(|| "verdict not one of PASS|FAIL".to_string())?;
    if verdict != "PASS" && verdict != "FAIL" {
        return Err("verdict not one of PASS|FAIL".to_string());
    }

    if !obj["summary"].is_string() {
        return Err("summary not a string".to_string());
    }

    let must_fix = obj["must_fix"].as_array().ok_or_else(|| "must_fix not an array".to_string())?;
    for (i, finding) in must_fix.iter().enumerate() {
        if !finding.is_string() {
            return Err(format!("must_fix[{i}] not a string"));
        }
    }
    if verdict == "PASS" && !must_fix.is_empty() {
        return Err("PASS verdict has non-empty must_fix".to_string());
    }
    if verdict == "FAIL" && must_fix.is_empty() {
        return Err("FAIL verdict has empty must_fix".to_string());
    }

    Ok(())
}

pub(crate) fn family(row: &RoleRow, providers: &BTreeMap<String, ProviderConfig>) -> String {
    family_of(row.body, row.provider.as_deref(), providers)
}

pub(crate) fn family_of(body: Body, provider: Option<&str>, providers: &BTreeMap<String, ProviderConfig>) -> String {
    match body {
        Body::ClaudeCode => "claude".to_string(),
        Body::Codex => "gpt".to_string(),
        Body::Native => {
            let provider_name = provider.unwrap_or_default();
            match providers.get(provider_name) {
                Some(cfg) if cfg.kind == ProviderKind::Anthropic => "claude".to_string(),
                _ => provider_name.to_string(),
            }
        }
    }
}
