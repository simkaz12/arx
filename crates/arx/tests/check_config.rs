use std::path::PathBuf;

use assert_cmd::Command;
use predicates::str::contains;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join("arx.toml")
}

const SUPPORTED_CODEX_MODELS: &[&str] = &["gpt-5.6-sol"];

fn frontmatter_value<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.lines().find_map(|line| line.trim().strip_prefix(prefix).map(str::trim))
}

fn models_block(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim() == "[[models]]" {
            let mut block = String::new();
            while let Some(next) = lines.peek() {
                if next.trim().starts_with('[') {
                    break;
                }
                block.push_str(lines.next().unwrap());
                block.push('\n');
            }
            blocks.push(block);
        }
    }
    blocks
}

#[test]
fn valid_config_resolves_all_roles() {
    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", fixture("valid").to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("role"))
        .stdout(contains("main"))
        .stdout(contains("reviewer"))
        .stdout(contains("hand-ex"))
        .stdout(contains("hand-native"))
        .stdout(contains("agent-sdk"))
        .stdout(contains("codex exec"))
        .stdout(contains("claude -p"))
        .stdout(contains("native loop"));
}

#[test]
fn drive_surface_configured_is_rejected() {
    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", fixture("drive-surface").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains(
            "the drive surface is never configured — it follows from the role (0bd4df2f)",
        ));
}

#[test]
fn main_override_is_rejected() {
    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", fixture("main-override").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains(
            "main's block is startup config, fixed per run — no per-dispatch body override (0bd4df2f)",
        ));
}

#[test]
fn provider_on_cli_body_is_rejected() {
    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", fixture("provider-on-cli").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains(
            "a body is not a provider; provider applies only to body = native (975f65cd)",
        ));
}

#[test]
fn native_without_provider_is_rejected() {
    Command::cargo_bin("arx")
        .unwrap()
        .args([
            "check-config",
            fixture("native-no-provider").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("body 'native' requires a provider"));
}

#[test]
fn native_with_unknown_provider_is_rejected() {
    Command::cargo_bin("arx")
        .unwrap()
        .args([
            "check-config",
            fixture("native-unknown-provider").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(contains(
            "model 'claude-haiku-4-5' names provider 'nowhere' which has no [providers.nowhere] table",
        ));
}

#[test]
fn no_reviewer_is_rejected() {
    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", fixture("no-reviewer").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains(
            "agents dir names no 'reviewer' definition — the reviewer role is unfilled (0bd4df2f)",
        ));
}

#[test]
fn no_runner_is_rejected() {
    let workspace = tempfile::tempdir().unwrap();
    let fixture_root = fixture("valid").parent().unwrap().to_path_buf();
    copy_dir_recursive(&fixture_root, &workspace.path().join("valid"));
    std::fs::remove_file(workspace.path().join("valid/agents/runner.md")).unwrap();
    Command::cargo_bin("arx")
        .unwrap()
        .args([
            "check-config",
            workspace.path().join("valid/arx.toml").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(contains(
            "agents dir names no 'runner' definition — the runner role is unfilled",
        ));
}

#[test]
fn native_runner_default_is_rejected() {
    let workspace = tempfile::tempdir().unwrap();
    let fixture_root = fixture("valid").parent().unwrap().to_path_buf();
    copy_dir_recursive(&fixture_root, &workspace.path().join("valid"));
    let config_path = workspace.path().join("valid/arx.toml");
    let config = std::fs::read_to_string(&config_path)
        .unwrap()
        .replace("runner = \"gpt-5.6-sol\"", "runner = \"claude-haiku-4-5\"");
    std::fs::write(&config_path, config).unwrap();
    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", config_path.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains(
            "runner requires claude-code or codex so it can implement the brief",
        ));
}

#[test]
fn codex_bodied_fixture_registry_entries_use_a_supported_model_id() {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut visited = 0usize;
    let mut offenders: Vec<String> = Vec::new();

    for fixture_entry in std::fs::read_dir(&fixtures_dir).unwrap() {
        let fixture_entry = fixture_entry.unwrap();
        if !fixture_entry.file_type().unwrap().is_dir() {
            continue;
        }
        let config_path = fixture_entry.path().join("arx.toml");
        let Ok(text) = std::fs::read_to_string(&config_path) else {
            continue;
        };
        for block in models_block(&text) {
            let Some(body) = frontmatter_value(&block, "body =") else {
                continue;
            };
            let body = body.trim_matches('"');
            if body != "codex" {
                continue;
            }
            visited += 1;
            let id = frontmatter_value(&block, "id =").unwrap_or_default().trim_matches('"');
            if !SUPPORTED_CODEX_MODELS.contains(&id) {
                offenders.push(format!("{}: {id}", config_path.display()));
            }
        }
    }

    assert!(visited >= 7, "expected at least seven codex-bodied registry entries, visited {visited}");
    assert!(offenders.is_empty(), "codex-bodied registry entries on unsupported model ids: {offenders:?}");
}

#[test]
fn edit_file_rule_is_admitted() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("agents")).unwrap();
    std::fs::write(workspace.path().join("agents/runner.md"), "---\ntools: [Read, Bash]\nwrite: true\n---\nrunner\n").unwrap();
    std::fs::write(workspace.path().join("agents/reviewer.md"), "---\n{}\n---\nreviewer\n").unwrap();
    std::fs::write(workspace.path().join("agents/hand-ex.md"), "---\n{}\n---\nhand\n").unwrap();
    std::fs::write(
        workspace.path().join("arx.toml"),
        "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\n\n[gate]\ndeny = [\"mcp__arx__edit_file(store/*)\"]\nallow = [\"mcp__arx__edit_file\"]\n",
    )
    .unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn an_admission_table_loads_and_a_zero_threshold_refuses() {
    let workspace = tempfile::tempdir().unwrap();
    copy_dir_recursive(&fixture("valid").parent().unwrap().to_path_buf(), &workspace.path().join("valid"));
    let config_path = workspace.path().join("valid/arx.toml");

    let plain = Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", config_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(plain.status.success());

    let mut contents = std::fs::read_to_string(&config_path).unwrap();
    contents.push_str("\n[admission]\nthreshold_tokens = 2000\n\n[admission.overrides]\nread_file = 8000\n");
    std::fs::write(&config_path, &contents).unwrap();

    let with_admission = Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", config_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(with_admission.status.success());
    assert_eq!(with_admission.stdout, plain.stdout);

    let zero_path = workspace.path().join("valid/arx-zero.toml");
    let zero_contents = contents.replace("threshold_tokens = 2000", "threshold_tokens = 0");
    std::fs::write(&zero_path, &zero_contents).unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", zero_path.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("admission: threshold must be at least 1 token"));
}

fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

#[test]
fn main_window_and_reserve_load_with_their_refusals() {
    let workspace = tempfile::tempdir().unwrap();
    copy_dir_recursive(&fixture("valid").parent().unwrap().to_path_buf(), &workspace.path().join("valid"));
    let config_path = workspace.path().join("valid/arx.toml");

    let plain = Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", config_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(plain.status.success());

    let plain_contents = std::fs::read_to_string(&config_path).unwrap();
    let needle = "body = \"claude-code\"\n";
    assert!(plain_contents.contains(needle), "config: {plain_contents}");
    let contents = plain_contents.replacen(needle, &format!("{needle}window = 800000\nreserve = 16000\n"), 1);
    std::fs::write(&config_path, &contents).unwrap();

    let with_reserve = Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", config_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(with_reserve.status.success());
    assert_eq!(with_reserve.stdout, plain.stdout);

    let over_ceiling_path = workspace.path().join("valid/arx-over.toml");
    let over_ceiling_contents = contents.replace("window = 800000", "window = 950000");
    std::fs::write(&over_ceiling_path, &over_ceiling_contents).unwrap();
    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", over_ceiling_path.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains(
            "main: window must stay below 900000; the body compacts near 967k of the 1M compaction window arx sets",
        ));

    let zero_reserve_path = workspace.path().join("valid/arx-zero-reserve.toml");
    let zero_reserve_contents = contents.replace("reserve = 16000", "reserve = 0");
    std::fs::write(&zero_reserve_path, &zero_reserve_contents).unwrap();
    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", zero_reserve_path.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("main: reserve must be at least 1 token"));

    let inverted_path = workspace.path().join("valid/arx-inverted.toml");
    let inverted_contents = contents.replace("window = 800000", "window = 1000").replace("reserve = 16000", "reserve = 1000");
    std::fs::write(&inverted_path, &inverted_contents).unwrap();
    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", inverted_path.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("main: reserve must be below window"));
}

#[test]
fn check_config_never_prints_a_main_models_line_now_that_the_registry_is_the_sole_list() {
    let workspace = tempfile::tempdir().unwrap();
    copy_dir_recursive(&fixture("valid").parent().unwrap().to_path_buf(), &workspace.path().join("valid"));
    let config_path = workspace.path().join("valid/arx.toml");

    let plain = Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", config_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(plain.status.success());
    let plain_stdout = String::from_utf8_lossy(&plain.stdout).into_owned();
    assert!(!plain_stdout.contains("main.models"), "stdout: {plain_stdout}");

    let contents = std::fs::read_to_string(&config_path).unwrap();
    let updated = format!("{contents}\n[[models]]\nid = \"claude-opus-5\"\nbody = \"claude-code\"\n");
    std::fs::write(&config_path, updated).unwrap();

    let with_extra_model = Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", config_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(with_extra_model.status.success());
    let with_extra_stdout = String::from_utf8_lossy(&with_extra_model.stdout).into_owned();
    assert!(!with_extra_stdout.contains("main.models"), "stdout: {with_extra_stdout}");
    assert_eq!(with_extra_stdout, plain_stdout, "an unbound registry entry must not change the printed table");
}

#[test]
fn a_hand_with_a_small_window_and_no_reserve_still_loads() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("agents")).unwrap();
    std::fs::write(workspace.path().join("agents/runner.md"), "---\ntools: [Read, Bash]\nwrite: true\n---\nrunner\n").unwrap();
    std::fs::write(workspace.path().join("agents/reviewer.md"), "---\n{}\n---\nreviewer\n").unwrap();
    std::fs::write(workspace.path().join("agents/hand-ex.md"), "---\nwindow: 8000\n---\nhand\n").unwrap();
    std::fs::write(
        workspace.path().join("arx.toml"),
        "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\n",
    )
    .unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .assert()
        .success();
}

fn write_drain_workspace(dir: &std::path::Path, drain_frontmatter: &str) {
    std::fs::create_dir_all(dir.join("agents")).unwrap();
    std::fs::write(dir.join("agents/runner.md"), "---\ntools: [Read, Bash]\nwrite: true\n---\nrunner\n").unwrap();
    std::fs::write(dir.join("agents/reviewer.md"), "---\ntools: []\n---\nreviewer\n").unwrap();
    std::fs::write(dir.join("agents/hand-ex.md"), "---\ntools: []\n---\nhand\n").unwrap();
    std::fs::write(dir.join("agents/drain.md"), drain_frontmatter).unwrap();
    std::fs::write(
        dir.join("arx.toml"),
        "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\ndrain = \"claude-sonnet-5\"\n",
    )
    .unwrap();
}

#[test]
fn drain_with_no_frontmatter_block_is_rejected() {
    let workspace = tempfile::tempdir().unwrap();
    write_drain_workspace(workspace.path(), "drain has no marker line at all\n");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("agents/drain.md: no frontmatter"));
}

#[test]
fn drain_carrying_retired_key_body_is_refused_with_the_moved_message() {
    let workspace = tempfile::tempdir().unwrap();
    write_drain_workspace(workspace.path(), "---\nbody: claude-code\n---\ndrain\n");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("agents/drain.md: 'body' belongs to [[models]] and [roles]"));
}

#[test]
fn drain_carrying_retired_key_model_is_refused_with_the_moved_message() {
    let workspace = tempfile::tempdir().unwrap();
    write_drain_workspace(workspace.path(), "---\nmodel: claude-sonnet-5\n---\ndrain\n");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("agents/drain.md: 'model' belongs to [[models]] and [roles]"));
}

#[test]
fn drain_carrying_retired_key_provider_is_refused_with_the_moved_message() {
    let workspace = tempfile::tempdir().unwrap();
    write_drain_workspace(workspace.path(), "---\nprovider: anthropic\n---\ndrain\n");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("agents/drain.md: 'provider' belongs to [[models]] and [roles]"));
}

#[test]
fn drain_carrying_retired_key_image_is_refused_with_the_moved_message() {
    let workspace = tempfile::tempdir().unwrap();
    write_drain_workspace(workspace.path(), "---\nimage: custom:latest\n---\ndrain\n");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("agents/drain.md: 'image' belongs to [[models]] and [roles]"));
}

#[test]
fn drain_with_a_malformed_field_is_a_generic_frontmatter_parse_error() {
    let workspace = tempfile::tempdir().unwrap();
    write_drain_workspace(workspace.path(), "---\nsteps: \"five\"\n---\ndrain\n");

    let output = Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("agents/drain.md:"), "stderr: {stderr}");
    assert!(stderr.contains("steps"), "stderr: {stderr}");
    assert!(!stderr.contains("belongs to [[models]] and [roles]"), "stderr: {stderr}");
}

#[test]
fn drain_steps_zero_is_refused_with_the_drain_role_tag() {
    let workspace = tempfile::tempdir().unwrap();
    write_drain_workspace(workspace.path(), "---\nsteps: 0\n---\ndrain\n");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("drain.steps must be positive"));
}

#[test]
fn drain_window_zero_is_refused_with_the_drain_role_tag() {
    let workspace = tempfile::tempdir().unwrap();
    write_drain_workspace(workspace.path(), "---\nwindow: 0\n---\ndrain\n");

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("drain.window must be positive"));
}

#[test]
fn read_file_rule_is_admitted_for_main() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("agents")).unwrap();
    std::fs::write(workspace.path().join("agents/runner.md"), "---\ntools: [Read, Bash]\nwrite: true\n---\nrunner\n").unwrap();
    std::fs::write(workspace.path().join("agents/reviewer.md"), "---\n{}\n---\nreviewer\n").unwrap();
    std::fs::write(workspace.path().join("agents/hand-ex.md"), "---\n{}\n---\nhand\n").unwrap();
    std::fs::write(
        workspace.path().join("arx.toml"),
        "[main]\nbody = \"claude-code\"\n\n[[models]]\nid = \"claude-sonnet-5\"\nbody = \"claude-code\"\n\n[roles]\nmain = \"claude-sonnet-5\"\nrunner = \"claude-sonnet-5\"\nreviewer = \"claude-sonnet-5\"\nhand-ex = \"claude-sonnet-5\"\n\n[gate]\ndeny = [\"mcp__arx__read_file(store/*)\"]\n",
    )
    .unwrap();

    Command::cargo_bin("arx")
        .unwrap()
        .args(["check-config", workspace.path().join("arx.toml").to_str().unwrap()])
        .assert()
        .success();
}
