use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[path = "../src/container.rs"]
mod container;

use container::{Container, ContainerSpec, Mount, TraceIndex, Worktree};

fn image() -> String {
    "alpine:3.20".to_string()
}

#[test]
fn args_carry_home_masks_readonly_and_only_allowlisted_env() {
    let spec = ContainerSpec {
        name: "arx-h1-1-1".to_string(),
        hand: "h1".to_string(),
        thread: "main".to_string(),
        image: image(),
        user: (1000, 1000),
        workdir: "/hand/repo".to_string(),
        mounts: vec![
            Mount {
                src: PathBuf::from("/tmp/arx-repo"),
                dst: "/hand/repo".to_string(),
                readonly: true,
            },
            Mount {
                src: PathBuf::from("/tmp/arx-home"),
                dst: "/hand/home".to_string(),
                readonly: false,
            },
        ],
        masks: vec!["/hand/repo/store".to_string()],
        env: vec![("ARX_PROBE".to_string(), "yes".to_string()), ("OTHER".to_string(), "1".to_string())],
        argv: vec!["sh".to_string(), "-c".to_string(), "echo hi".to_string()],
    };

    let args = spec.args();
    assert_eq!(
        args,
        vec![
            "run".to_string(),
            "-i".to_string(),
            "--rm".to_string(),
            "--name".to_string(),
            "arx-h1-1-1".to_string(),
            "--label".to_string(),
            "arx.hand=h1".to_string(),
            "--label".to_string(),
            "arx.thread=main".to_string(),
            "--user".to_string(),
            "1000:1000".to_string(),
            "-e".to_string(),
            "HOME=/hand/home".to_string(),
            "-e".to_string(),
            "ARX_PROBE=yes".to_string(),
            "-e".to_string(),
            "OTHER=1".to_string(),
            "--mount".to_string(),
            "type=bind,src=/tmp/arx-repo,dst=/hand/repo,readonly".to_string(),
            "--mount".to_string(),
            "type=bind,src=/tmp/arx-home,dst=/hand/home".to_string(),
            "--tmpfs".to_string(),
            "/hand/repo/store:ro".to_string(),
            "--workdir".to_string(),
            "/hand/repo".to_string(),
            "alpine:3.20".to_string(),
            "sh".to_string(),
            "-c".to_string(),
            "echo hi".to_string(),
        ]
    );
}

fn base_spec(name: &str, argv: Vec<String>) -> ContainerSpec {
    ContainerSpec {
        name: name.to_string(),
        hand: "h1".to_string(),
        thread: "main".to_string(),
        image: image(),
        user: (unsafe_uid(), unsafe_gid()),
        workdir: "/hand/repo".to_string(),
        mounts: Vec::new(),
        masks: Vec::new(),
        env: Vec::new(),
        argv,
    }
}

fn unsafe_uid() -> u32 {
    let output = Command::new("id").arg("-u").output().unwrap();
    String::from_utf8_lossy(&output.stdout).trim().parse().unwrap()
}

fn unsafe_gid() -> u32 {
    let output = Command::new("id").arg("-g").output().unwrap();
    String::from_utf8_lossy(&output.stdout).trim().parse().unwrap()
}

fn unique_name(tag: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("arx-test-{tag}-{}-{nanos}", std::process::id())
}

fn run_and_capture(spec: &ContainerSpec) -> (String, String, i32) {
    let mut container = Container::run(spec).unwrap();
    let stdout_pipe = container.take_stdout().unwrap();
    let stderr_pipe = container.take_stderr().unwrap();
    drop(container.take_stdin());
    let stdout_handle = std::thread::spawn(move || {
        use std::io::Read;
        let mut pipe = stdout_pipe;
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        use std::io::Read;
        let mut pipe = stderr_pipe;
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    });
    let status = container.wait().unwrap();
    let stdout = String::from_utf8_lossy(&stdout_handle.join().unwrap()).into_owned();
    let stderr = String::from_utf8_lossy(&stderr_handle.join().unwrap()).into_owned();
    container.remove();
    (stdout, stderr, status.code().unwrap_or(-1))
}

#[test]
fn a_container_gets_only_the_image_env_plus_home() {
    let baseline = Command::new("docker").args(["run", "--rm", &image(), "env"]).output().unwrap();
    let baseline_names: std::collections::BTreeSet<String> = String::from_utf8_lossy(&baseline.stdout)
        .lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name.to_string()))
        .collect();

    let name = unique_name("env-plus-home");
    let spec = base_spec(&name, vec!["env".to_string()]);
    let (stdout, stderr, code) = run_and_capture(&spec);
    assert_eq!(code, 0, "stderr: {stderr}");

    let observed_names: std::collections::BTreeSet<String> =
        stdout.lines().filter_map(|line| line.split_once('=').map(|(name, _)| name.to_string())).collect();
    assert_eq!(observed_names, baseline_names, "stdout: {stdout}");
    assert!(stdout.lines().any(|line| line == "HOME=/hand/home"), "stdout: {stdout}");
}

#[test]
fn an_allowlisted_variable_passes_through() {
    let name = unique_name("allowlist");
    let mut spec = base_spec(&name, vec!["sh".to_string(), "-c".to_string(), "echo $ARX_PROBE".to_string()]);
    spec.env = vec![("ARX_PROBE".to_string(), "yes".to_string())];
    let (stdout, stderr, code) = run_and_capture(&spec);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout.trim(), "yes");
}

#[test]
fn a_readonly_mount_refuses_writes_and_a_mask_hides_the_store() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("store")).unwrap();
    std::fs::write(dir.path().join("store/claim.md"), "claim").unwrap();

    let name = unique_name("mask");
    let mut spec = base_spec(
        &name,
        vec![
            "sh".to_string(),
            "-c".to_string(),
            "ls /hand/repo/store | wc -l; touch /hand/repo/x 2>&1 || echo denied".to_string(),
        ],
    );
    spec.mounts = vec![Mount {
        src: dir.path().to_path_buf(),
        dst: "/hand/repo".to_string(),
        readonly: true,
    }];
    spec.masks = vec!["/hand/repo/store".to_string()];

    let (stdout, stderr, code) = run_and_capture(&spec);
    assert_eq!(code, 0, "stderr: {stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.first(), Some(&"0"), "stdout: {stdout}");
    assert!(stdout.contains("denied"), "stdout: {stdout}");
}

#[test]
fn kill_removes_the_container() {
    let name = unique_name("kill");
    let spec = base_spec(&name, vec!["sleep".to_string(), "60".to_string()]);
    let mut container = Container::run(&spec).unwrap();
    drop(container.take_stdin());
    drop(container.take_stdout());
    drop(container.take_stderr());
    std::thread::sleep(Duration::from_millis(500));

    container.kill();
    let _ = container.wait();
    container.remove();
    std::thread::sleep(Duration::from_millis(500));

    let output = Command::new("docker")
        .args(["ps", "-a", "--filter", &format!("name={name}"), "--format", "{{.Names}}"])
        .output()
        .unwrap();
    let listed = String::from_utf8_lossy(&output.stdout);
    assert!(listed.trim().is_empty(), "still listed: {listed}");
}

#[test]
fn a_worktree_diff_applies_on_the_base() {
    let repo = tempfile::tempdir().unwrap();
    run_git(repo.path(), &["init", "-q"]);
    run_git(repo.path(), &["config", "user.email", "arx@localhost"]);
    run_git(repo.path(), &["config", "user.name", "arx"]);
    std::fs::write(repo.path().join("seed.txt"), "seed\n").unwrap();
    run_git(repo.path(), &["add", "-A"]);
    run_git(repo.path(), &["commit", "-q", "-m", "seed"]);

    let worktree_path = repo.path().join("wt");
    let worktree = Worktree::add(repo.path(), &worktree_path).unwrap();
    assert!(!worktree.base.is_empty());

    std::fs::write(worktree_path.join("new.txt"), "from the hand\n").unwrap();
    let diff = worktree.diff().unwrap();
    assert!(!diff.is_empty());

    worktree.remove(repo.path());
    assert!(!worktree_path.exists());

    let list = Command::new("git").arg("-C").arg(repo.path()).arg("worktree").arg("list").output().unwrap();
    let list_text = String::from_utf8_lossy(&list.stdout);
    assert_eq!(list_text.lines().count(), 1, "worktree list: {list_text}");

    let diff_path = repo.path().join("apply.diff");
    std::fs::write(&diff_path, &diff).unwrap();
    let apply = Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .arg("apply")
        .arg(&diff_path)
        .output()
        .unwrap();
    assert!(apply.status.success(), "apply stderr: {}", String::from_utf8_lossy(&apply.stderr));
    let applied = std::fs::read_to_string(repo.path().join("new.txt")).unwrap();
    assert_eq!(applied, "from the hand\n");
}

fn run_git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap();
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

#[test]
fn sweep_writes_index_diff_and_out() {
    let root = tempfile::tempdir().unwrap();
    let trace_dir = root.path().join("trace");
    let out_dir = root.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(out_dir.join("stdout.log"), "hello\n").unwrap();
    std::fs::write(out_dir.join("stderr.log"), "warn\n").unwrap();

    let diff = b"diff --git a/x b/x\n".to_vec();
    let index = TraceIndex {
        hand: "h1".to_string(),
        agent: "fake".to_string(),
        thread: "main".to_string(),
        image: image(),
        container: "arx-h1-1-1".to_string(),
        exit: "0".to_string(),
        write: true,
        base: Some("deadbeef".to_string()),
    };

    let result = container::sweep(&trace_dir, &index, &out_dir, Some(&diff), "fake", &[]).unwrap();
    assert_eq!(result.files, 3);
    assert_eq!(result.bytes, diff.len() as u64 + "hello\n".len() as u64 + "warn\n".len() as u64);

    let trace_md = std::fs::read_to_string(trace_dir.join("trace.md")).unwrap();
    assert!(trace_md.contains("hand: h1"), "trace.md: {trace_md}");
    assert!(trace_md.contains("agent: fake"), "trace.md: {trace_md}");
    assert!(trace_md.contains("thread: main"), "trace.md: {trace_md}");
    assert!(trace_md.contains("image: alpine:3.20"), "trace.md: {trace_md}");
    assert!(trace_md.contains("container: arx-h1-1-1"), "trace.md: {trace_md}");
    assert!(trace_md.contains("exit: 0"), "trace.md: {trace_md}");
    assert!(trace_md.contains("write: true"), "trace.md: {trace_md}");
    assert!(trace_md.contains("base: deadbeef"), "trace.md: {trace_md}");
    assert!(trace_md.contains("path: worktree.diff"), "trace.md: {trace_md}");
    assert!(trace_md.contains("path: out/stdout.log"), "trace.md: {trace_md}");
    assert!(trace_md.contains("path: out/stderr.log"), "trace.md: {trace_md}");
    assert!(trace_md.trim_end().ends_with("fake"), "trace.md: {trace_md}");

    let diff_bytes = std::fs::read(trace_dir.join("worktree.diff")).unwrap();
    assert_eq!(diff_bytes, diff);
    let out_stdout = std::fs::read_to_string(trace_dir.join("out/stdout.log")).unwrap();
    assert_eq!(out_stdout, "hello\n");
}

#[test]
fn sweep_copies_extra_designations() {
    let root = tempfile::tempdir().unwrap();
    let trace_dir = root.path().join("trace");
    let out_dir = root.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();

    let extra_source_dir = root.path().join("home/.claude/projects/stub");
    std::fs::create_dir_all(&extra_source_dir).unwrap();
    std::fs::write(extra_source_dir.join("session-id.jsonl"), "{\"type\":\"line1\"}\n").unwrap();
    std::fs::write(extra_source_dir.join("sibling.jsonl"), "not designated\n").unwrap();

    let index = TraceIndex {
        hand: "h1".to_string(),
        agent: "hand-cc".to_string(),
        thread: "main".to_string(),
        image: image(),
        container: "arx-h1-1-1".to_string(),
        exit: "0".to_string(),
        write: false,
        base: None,
    };

    let extra = vec![(extra_source_dir.join("session-id.jsonl"), "session.jsonl".to_string())];
    let result = container::sweep(&trace_dir, &index, &out_dir, None, "claude-code", &extra).unwrap();
    assert_eq!(result.files, 1);
    assert_eq!(result.bytes, "{\"type\":\"line1\"}\n".len() as u64);

    let trace_md = std::fs::read_to_string(trace_dir.join("trace.md")).unwrap();
    assert!(trace_md.contains("path: session.jsonl"), "trace.md: {trace_md}");

    let copied = std::fs::read_to_string(trace_dir.join("session.jsonl")).unwrap();
    assert_eq!(copied, "{\"type\":\"line1\"}\n");

    assert!(!trace_dir.join("sibling.jsonl").exists(), "the sibling file was copied unexpectedly");
}

#[test]
fn exec_runs_a_command_in_the_workdir() {
    let name = unique_name("exec");
    let spec = base_spec(&name, vec!["sh".to_string(), "-c".to_string(), "while :; do sleep 3600; done".to_string()]);
    let mut container = Container::run(&spec).unwrap();
    drop(container.take_stdin());
    drop(container.take_stdout());
    drop(container.take_stderr());
    std::thread::sleep(Duration::from_millis(500));

    let output = container.exec(&["sh".to_string(), "-c".to_string(), "pwd".to_string()]).unwrap();
    assert!(output.status.success(), "exec failed: {output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), container.workdir());

    container.kill();
    let _ = container.wait();
    container.remove();
    std::thread::sleep(Duration::from_millis(500));

    let ps = Command::new("docker")
        .args(["ps", "-a", "--filter", &format!("name={name}"), "--format", "{{.Names}}"])
        .output()
        .unwrap();
    let listed = String::from_utf8_lossy(&ps.stdout);
    assert!(listed.trim().is_empty(), "still listed: {listed}");
}
