use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

pub fn toplevel(dir: &Path) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string()))
}

pub fn head(toplevel: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(toplevel)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub struct HandRoot {
    pub root: PathBuf,
    pub home: PathBuf,
    pub out: PathBuf,
    pub worktree: Option<PathBuf>,
}

impl HandRoot {
    pub fn create(hand: &str, nonce: &str, with_worktree: bool) -> io::Result<HandRoot> {
        let root = std::env::temp_dir().join("arx-hands").join(format!("{hand}-{nonce}"));
        let home = root.join("home");
        let out = root.join("out");
        fs::create_dir_all(&home)?;
        fs::create_dir_all(&out)?;
        let worktree = if with_worktree { Some(root.join("worktree")) } else { None };
        Ok(HandRoot { root, home, out, worktree })
    }

    pub fn uid_gid(&self) -> (u32, u32) {
        let metadata = fs::metadata(&self.root).expect("hand root exists once created");
        (metadata.uid(), metadata.gid())
    }

    pub fn remove(&self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub struct Worktree {
    pub path: PathBuf,
    pub base: String,
}

impl Worktree {
    pub fn add(toplevel: &Path, path: &Path) -> Result<Worktree, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(toplevel)
            .arg("worktree")
            .arg("add")
            .arg("--detach")
            .arg(path)
            .arg("HEAD")
            .output()
            .map_err(|err| err.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        let base = head(toplevel)?;
        Ok(Worktree { path: path.to_path_buf(), base })
    }

    pub fn diff(&self) -> Result<Vec<u8>, String> {
        let add = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .arg("add")
            .arg("-A")
            .output()
            .map_err(|err| err.to_string())?;
        if !add.status.success() {
            return Err(String::from_utf8_lossy(&add.stderr).trim().to_string());
        }
        let diff = Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .arg("diff")
            .arg("--cached")
            .arg("--binary")
            .arg("HEAD")
            .output()
            .map_err(|err| err.to_string())?;
        if !diff.status.success() {
            return Err(String::from_utf8_lossy(&diff.stderr).trim().to_string());
        }
        Ok(diff.stdout)
    }

    pub fn remove(self, toplevel: &Path) {
        let _ = Command::new("git")
            .arg("-C")
            .arg(toplevel)
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&self.path)
            .output();
        let _ = Command::new("git").arg("-C").arg(toplevel).arg("worktree").arg("prune").output();
    }
}

pub struct Mount {
    pub src: PathBuf,
    pub dst: String,
    pub readonly: bool,
}

pub struct ContainerSpec {
    pub name: String,
    pub hand: String,
    pub thread: String,
    pub image: String,
    pub user: (u32, u32),
    pub workdir: String,
    pub mounts: Vec<Mount>,
    pub masks: Vec<String>,
    pub env: Vec<(String, String)>,
    pub argv: Vec<String>,
}

impl ContainerSpec {
    pub fn args(&self) -> Vec<String> {
        let mut out = vec!["run".to_string(), "-i".to_string(), "--rm".to_string()];
        out.push("--name".to_string());
        out.push(self.name.clone());
        out.push("--label".to_string());
        out.push(format!("arx.hand={}", self.hand));
        out.push("--label".to_string());
        out.push(format!("arx.thread={}", self.thread));
        out.push("--user".to_string());
        out.push(format!("{}:{}", self.user.0, self.user.1));
        out.push("-e".to_string());
        out.push("HOME=/hand/home".to_string());
        for (key, value) in &self.env {
            out.push("-e".to_string());
            out.push(format!("{key}={value}"));
        }
        for mount in &self.mounts {
            out.push("--mount".to_string());
            let mut spec = format!("type=bind,src={},dst={}", mount.src.display(), mount.dst);
            if mount.readonly {
                spec.push_str(",readonly");
            }
            out.push(spec);
        }
        for mask in &self.masks {
            out.push("--tmpfs".to_string());
            out.push(format!("{mask}:ro"));
        }
        out.push("--workdir".to_string());
        out.push(self.workdir.clone());
        out.push(self.image.clone());
        out.extend(self.argv.iter().cloned());
        out
    }
}

pub struct Container {
    pub name: String,
    child: Child,
    workdir: String,
}

impl Container {
    pub fn run(spec: &ContainerSpec) -> io::Result<Container> {
        let mut command = Command::new("docker");
        command.args(spec.args()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let child = command.spawn()?;
        Ok(Container {
            name: spec.name.clone(),
            child,
            workdir: spec.workdir.clone(),
        })
    }

    pub fn workdir(&self) -> &str {
        &self.workdir
    }

    pub fn exec(&self, argv: &[String]) -> io::Result<std::process::Output> {
        Command::new("docker")
            .arg("exec")
            .arg("-w")
            .arg(&self.workdir)
            .arg(&self.name)
            .args(argv)
            .output()
    }

    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub fn kill(&self) {
        let _ = Command::new("docker").arg("kill").arg(&self.name).output();
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub fn remove(&self) {
        let _ = Command::new("docker").arg("rm").arg("-f").arg(&self.name).output();
    }
}

pub struct TraceIndex {
    pub hand: String,
    pub agent: String,
    pub thread: String,
    pub image: String,
    pub container: String,
    pub exit: String,
    pub write: bool,
    pub base: Option<String>,
}

pub struct Sweep {
    pub dir: PathBuf,
    pub bytes: u64,
    pub files: usize,
    pub diff: bool,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unix_seconds_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn copy_dir_all(src: &Path, dst: &Path, prefix: &str, paths: &mut Vec<(String, u64, String)>) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    if !src.exists() {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(src)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy().to_string();
        let rel = format!("{prefix}/{name_str}");
        let dest_path = dst.join(&name);
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &dest_path, &rel, paths)?;
        } else {
            let contents = fs::read(entry.path())?;
            fs::write(&dest_path, &contents)?;
            let bytes = contents.len() as u64;
            let sha256 = sha256_hex(&contents);
            paths.push((rel, bytes, sha256));
        }
    }
    Ok(())
}

fn write_trace_md(trace_dir: &Path, index: &TraceIndex, captured_at: u64, paths: &[(String, u64, String)], body: &str) -> io::Result<()> {
    let mut text = String::new();
    text.push_str("---\n");
    text.push_str(&format!("hand: {}\n", index.hand));
    text.push_str(&format!("agent: {}\n", index.agent));
    text.push_str(&format!("thread: {}\n", index.thread));
    text.push_str(&format!("image: {}\n", index.image));
    text.push_str(&format!("container: {}\n", index.container));
    text.push_str(&format!("exit: {}\n", index.exit));
    text.push_str(&format!("write: {}\n", index.write));
    if let Some(base) = &index.base {
        text.push_str(&format!("base: {base}\n"));
    }
    text.push_str(&format!("captured_at: {captured_at}\n"));
    text.push_str("paths:\n");
    for (path, bytes, sha256) in paths {
        text.push_str(&format!("  - path: {path}\n"));
        text.push_str(&format!("    bytes: {bytes}\n"));
        text.push_str(&format!("    sha256: {sha256}\n"));
    }
    text.push_str("---\n");
    text.push_str(body);
    text.push('\n');
    fs::write(trace_dir.join("trace.md"), text)
}

pub fn sweep(
    trace_dir: &Path,
    index: &TraceIndex,
    out: &Path,
    diff: Option<&[u8]>,
    body: &str,
    extra: &[(PathBuf, String)],
) -> io::Result<Sweep> {
    fs::create_dir_all(trace_dir)?;
    let mut paths: Vec<(String, u64, String)> = Vec::new();
    if let Some(diff_bytes) = diff {
        fs::write(trace_dir.join("worktree.diff"), diff_bytes)?;
        paths.push(("worktree.diff".to_string(), diff_bytes.len() as u64, sha256_hex(diff_bytes)));
    }
    copy_dir_all(out, &trace_dir.join("out"), "out", &mut paths)?;
    for (src, dst) in extra {
        let dest_path = trace_dir.join(dst);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = fs::read(src)?;
        fs::write(&dest_path, &contents)?;
        let bytes = contents.len() as u64;
        let sha256 = sha256_hex(&contents);
        paths.push((dst.clone(), bytes, sha256));
    }
    let bytes: u64 = paths.iter().map(|(_, bytes, _)| *bytes).sum();
    let files = paths.len();
    let captured_at = unix_seconds_now();
    write_trace_md(trace_dir, index, captured_at, &paths, body)?;
    Ok(Sweep { dir: trace_dir.to_path_buf(), bytes, files, diff: diff.is_some() })
}
