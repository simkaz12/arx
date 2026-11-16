use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roster {
    pins: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RosterFile {
    #[serde(default)]
    pins: BTreeMap<String, String>,
}

#[derive(Debug)]
pub enum RosterError {
    Io { path: PathBuf, source: std::io::Error },
    TomlParse { path: PathBuf, source: toml::de::Error },
    TomlWrite { path: PathBuf, source: toml::ser::Error },
}

impl fmt::Display for RosterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RosterError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            RosterError::TomlParse { path, source } => write!(f, "{}: {source}", path.display()),
            RosterError::TomlWrite { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for RosterError {}

impl Roster {
    pub fn load(path: &Path) -> Result<Roster, RosterError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Roster::default()),
            Err(source) => {
                return Err(RosterError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        let parsed: RosterFile = toml::from_str(&text).map_err(|source| RosterError::TomlParse {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Roster { pins: parsed.pins })
    }

    /// Writes to a sibling temp file first and renames it over `path` —
    /// same-directory rename is atomic on POSIX, so a write failure or a
    /// process kill mid-write never leaves a truncated or partial roster
    /// behind; the durable copy is either the old content or the new one,
    /// never something in between.
    pub fn save(&self, path: &Path) -> Result<(), RosterError> {
        self.prepare(path)?.commit()
    }

    pub fn prepare(&self, path: &Path) -> Result<PreparedRoster, RosterError> {
        if path.is_dir() {
            return Err(RosterError::Io {
                path: path.to_path_buf(),
                source: std::io::Error::other("roster path is a directory"),
            });
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|source| RosterError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let file = RosterFile { pins: self.pins.clone() };
        let text = toml::to_string(&file).map_err(|source| RosterError::TomlWrite {
            path: path.to_path_buf(),
            source,
        })?;
        let tmp_path = path.with_extension("toml.tmp");
        fs::write(&tmp_path, text).map_err(|source| RosterError::Io {
            path: tmp_path.clone(),
            source,
        })?;
        Ok(PreparedRoster {
            temporary: tmp_path, path: path.to_path_buf(),
        })
    }

    pub fn pin(&mut self, position: &str, id: &str) {
        self.pins.insert(position.to_string(), id.to_string());
    }

    pub fn clear(&mut self, position: &str) -> bool {
        self.pins.remove(position).is_some()
    }

    pub fn migrate(&mut self, from: &str, to: &str) -> bool {
        let Some(id) = self.pins.remove(from) else {
            return false;
        };
        self.pins.entry(to.to_string()).or_insert(id);
        true
    }

    pub fn get(&self, position: &str) -> Option<&str> {
        self.pins.get(position).map(String::as_str)
    }

    pub fn pins(&self) -> impl Iterator<Item = (&str, &str)> {
        self.pins.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

fn absolute_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    })
}

/// `~/.local/share/arx/roster/<config-parent-dirname>-<16 hex chars of sha256(abs
/// config path)>.toml`, honouring `ARX_ROSTER_DIR` as an override — the
/// `ARX_USER_GATE` precedent (`config.rs:798`). Pure path arithmetic: never
/// creates anything, never touches the filesystem beyond reading `config_path`
/// to canonicalize it.
pub fn roster_path(config_path: &Path) -> PathBuf {
    let base = match std::env::var("ARX_ROSTER_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(home).join(".local/share/arx/roster")
        }
    };
    let absolute = absolute_path(config_path);
    let parent_name = absolute
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("config");
    let mut hasher = Sha256::new();
    hasher.update(absolute.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    base.join(format!("{parent_name}-{hex}.toml"))
}

pub struct PreparedRoster {
    temporary: PathBuf,
    path: PathBuf,
}
impl PreparedRoster {
    pub fn commit(self) -> Result<(), RosterError> {
        fs::rename(&self.temporary, &self.path).map_err(|source| RosterError::Io {
            path: self.path.clone(),
            source,
        })
    }
}
impl Drop for PreparedRoster {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.temporary);
    }
}
