//! The store root as a git repository, committed through the `git` binary. No libgit2.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::StoreError;

const GITIGNORE: &str = "quarantine/\n.index/\n";

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

fn run(root: &Path, args: &[&str]) -> Result<Output, StoreError> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("-c")
        .arg("user.name=arx")
        .arg("-c")
        .arg("user.email=arx@localhost")
        .args(args)
        .output()
        .map_err(|source| StoreError::Git {
            op: args.join(" "),
            detail: source.to_string(),
        })
}

/// Runs `git init` at `root`, writes `.gitignore` and commits it as `arx store init`.
pub fn init(root: &Path) -> Result<(), StoreError> {
    let output = run(root, &["init", "-q"])?;
    if !output.status.success() {
        return Err(StoreError::Git {
            op: "init".to_string(),
            detail: stderr_text(&output),
        });
    }
    let gitignore = root.join(".gitignore");
    fs::write(&gitignore, GITIGNORE).map_err(|source| StoreError::Git {
        op: "gitignore".to_string(),
        detail: source.to_string(),
    })?;
    commit(root, &[gitignore], "arx store init")
}

/// Stages `paths` and commits them as one commit with `message`.
pub fn commit(root: &Path, paths: &[PathBuf], message: &str) -> Result<(), StoreError> {
    let mut add_args: Vec<&std::ffi::OsStr> = vec!["add".as_ref(), "-A".as_ref(), "--".as_ref()];
    for path in paths {
        add_args.push(path.as_os_str());
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("-c")
        .arg("user.name=arx")
        .arg("-c")
        .arg("user.email=arx@localhost")
        .args(add_args)
        .output()
        .map_err(|source| StoreError::Git {
            op: "add".to_string(),
            detail: source.to_string(),
        })?;
    if !output.status.success() {
        return Err(StoreError::Git {
            op: "add".to_string(),
            detail: stderr_text(&output),
        });
    }

    let output = run(root, &["commit", "-q", "-m", message])?;
    if !output.status.success() {
        return Err(StoreError::Git {
            op: "commit".to_string(),
            detail: stderr_text(&output),
        });
    }
    Ok(())
}

pub fn is_uncommitted(root: &Path, path: &Path) -> Result<bool, StoreError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("status")
        .arg("--porcelain")
        .arg("--")
        .arg(path)
        .output()
        .map_err(|source| StoreError::Git {
            op: "status".to_string(),
            detail: source.to_string(),
        })?;
    if !output.status.success() {
        return Err(StoreError::Git {
            op: "status".to_string(),
            detail: stderr_text(&output),
        });
    }
    Ok(!output.stdout.is_empty())
}

/// `rev-list --count HEAD`, for tests that assert a commit count.
pub fn head_count(root: &Path) -> Result<u64, StoreError> {
    let output = run(root, &["rev-list", "--count", "HEAD"])?;
    if !output.status.success() {
        return Err(StoreError::Git {
            op: "rev-list".to_string(),
            detail: stderr_text(&output),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.trim().parse::<u64>().map_err(|err| StoreError::Git {
        op: "rev-list".to_string(),
        detail: err.to_string(),
    })
}
