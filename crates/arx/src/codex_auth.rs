use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn codex_credential_source() -> PathBuf {
    if let Ok(dir) = std::env::var("CODEX_HOME") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("auth.json");
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    Path::new(&home).join(".codex").join("auth.json")
}

pub fn inject_codex_credential(home: &Path) -> Result<(), String> {
    let source = codex_credential_source();
    let refusal = || format!("dispatch: no Codex credential at {}; log in on the host first", source.display());

    let text = fs::read_to_string(&source).map_err(|_| refusal())?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| refusal())?;
    let object = value.as_object().ok_or_else(refusal)?;
    let has_tokens = object.get("tokens").map(|v| v.is_object()).unwrap_or(false);
    let has_api_key = object.get("OPENAI_API_KEY").map(|v| v.is_string()).unwrap_or(false);
    if !has_tokens && !has_api_key {
        return Err(refusal());
    }

    let dest_dir = home.join(".codex");
    fs::create_dir_all(&dest_dir).map_err(|err| format!("dispatch: failed to prepare credential directory: {err}"))?;
    fs::set_permissions(&dest_dir, fs::Permissions::from_mode(0o700))
        .map_err(|err| format!("dispatch: failed to set credential directory mode: {err}"))?;

    let dest = dest_dir.join("auth.json");
    fs::copy(&source, &dest).map_err(|err| format!("dispatch: failed to write credential: {err}"))?;
    fs::set_permissions(&dest, fs::Permissions::from_mode(0o600))
        .map_err(|err| format!("dispatch: failed to set credential mode: {err}"))?;
    Ok(())
}
