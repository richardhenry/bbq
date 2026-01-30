use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn repo_list_empty() {
    let ctx = TestContext::new("repo_list_empty");

    let output = ctx.bbq(&["repo", "list"]);
    let stdout = assert_success(output);
    assert_eq!(stdout.trim(), "no repos");
}

#[test]
fn repo_clone_and_list() {
    let ctx = TestContext::new("repo_clone_and_list");
    let src_repo = ctx.root.join("source");
    init_repo(&src_repo);

    let output = ctx.bbq(&["repo", "clone", src_repo.to_str().expect("repo path")]);
    assert_success_contains(output, "checked out source");

    let output = ctx.bbq(&["repo", "list"]);
    let stdout = assert_success(output);
    assert_eq!(stdout.trim(), "source");
}

#[test]
fn repo_clone_custom_name() {
    let ctx = TestContext::new("repo_clone_custom_name");
    let src_repo = ctx.root.join("source");
    init_repo(&src_repo);

    let output = ctx.bbq(&[
        "repo",
        "clone",
        src_repo.to_str().expect("repo path"),
        "custom",
    ]);
    assert_success_contains(output, "checked out custom");

    let output = ctx.bbq(&["repo", "list"]);
    let stdout = assert_success(output);
    assert_eq!(stdout.trim(), "custom");
}

#[test]
fn repo_rm_removes_repo() {
    let ctx = TestContext::new("repo_rm_removes_repo");
    let src_repo = ctx.root.join("source");
    init_repo(&src_repo);

    let output = ctx.bbq(&["repo", "clone", src_repo.to_str().expect("repo path")]);
    assert_success_contains(output, "checked out source");

    let output = ctx.bbq(&["repo", "rm", "source"]);
    assert_success_contains(output, "removed source");

    let output = ctx.bbq(&["repo", "list"]);
    let stdout = assert_success(output);
    assert_eq!(stdout.trim(), "no repos");
}

#[test]
fn worktree_create_list_rm() {
    let ctx = TestContext::new("worktree_create_list_rm");
    let src_repo = ctx.root.join("source");
    init_repo(&src_repo);

    let output = ctx.bbq(&["repo", "clone", src_repo.to_str().expect("repo path")]);
    assert_success_contains(output, "checked out source");

    let output = ctx.bbq(&[
        "worktree",
        "create",
        "source",
        "--branch",
        "feature-test",
    ]);
    assert_success_contains(output, "created feature-test");

    let output = ctx.bbq(&["worktree", "list", "source"]);
    let stdout = assert_success(output);
    assert!(stdout.contains("feature-test"));

    let output = ctx.bbq(&["worktree", "rm", "source", "feature-test"]);
    assert_success_contains(output, "removed feature-test");

    let output = ctx.bbq(&["worktree", "list", "source"]);
    let stdout = assert_success(output);
    assert_eq!(stdout.trim(), "no worktrees");
}

#[test]
fn worktree_open_unknown_target_fails() {
    let ctx = TestContext::new("worktree_open_unknown_target_fails");
    let src_repo = ctx.root.join("source");
    init_repo(&src_repo);

    let output = ctx.bbq(&["repo", "clone", src_repo.to_str().expect("repo path")]);
    assert_success_contains(output, "checked out source");

    let output = ctx.bbq(&[
        "worktree",
        "create",
        "source",
        "--branch",
        "feature-test",
    ]);
    assert_success_contains(output, "created feature-test");

    let output = ctx.bbq(&[
        "worktree",
        "open",
        "source",
        "feature-test",
        "--target",
        "nope",
    ]);
    assert_failure_contains(output, "unknown target: nope");
}

#[test]
fn worktree_list_shows_multiple_entries() {
    let ctx = TestContext::new("worktree_list_shows_multiple_entries");
    let src_repo = ctx.root.join("source");
    init_repo(&src_repo);

    let output = ctx.bbq(&["repo", "clone", src_repo.to_str().expect("repo path")]);
    assert_success_contains(output, "checked out source");

    let output = ctx.bbq(&[
        "worktree",
        "create",
        "source",
        "--branch",
        "alpha",
    ]);
    assert_success_contains(output, "created alpha");

    let output = ctx.bbq(&[
        "worktree",
        "create",
        "source",
        "--branch",
        "beta",
    ]);
    assert_success_contains(output, "created beta");

    let output = ctx.bbq(&["worktree", "list", "source"]);
    let stdout = assert_success(output);
    assert!(stdout.contains("alpha\t"));
    assert!(stdout.contains("beta\t"));
}

#[test]
fn repo_rm_fails_with_worktrees() {
    let ctx = TestContext::new("repo_rm_fails_with_worktrees");
    let src_repo = ctx.root.join("source");
    init_repo(&src_repo);

    let output = ctx.bbq(&["repo", "clone", src_repo.to_str().expect("repo path")]);
    assert_success_contains(output, "checked out source");

    let output = ctx.bbq(&[
        "worktree",
        "create",
        "source",
        "--branch",
        "feature-test",
    ]);
    assert_success_contains(output, "created feature-test");

    let output = ctx.bbq(&["repo", "rm", "source"]);
    assert_failure_contains(output, "RepoHasWorktrees");
}

#[test]
fn worktree_rm_missing_errors() {
    let ctx = TestContext::new("worktree_rm_missing_errors");
    let src_repo = ctx.root.join("source");
    init_repo(&src_repo);

    let output = ctx.bbq(&["repo", "clone", src_repo.to_str().expect("repo path")]);
    assert_success_contains(output, "checked out source");

    let output = ctx.bbq(&["worktree", "rm", "source", "missing"]);
    assert_failure_contains(output, "WorktreeNotFound(\"missing\")");
}

#[test]
fn worktree_open_uses_default_open_from_config() {
    let ctx = TestContext::new("worktree_open_uses_default_open_from_config");
    let src_repo = ctx.root.join("source");
    init_repo(&src_repo);

    let output = ctx.bbq(&["repo", "clone", src_repo.to_str().expect("repo path")]);
    assert_success_contains(output, "checked out source");

    let output = ctx.bbq(&[
        "worktree",
        "create",
        "source",
        "--branch",
        "feature-test",
    ]);
    assert_success_contains(output, "created feature-test");

    ctx.write_config("default_open = \"code\"");

    let bin_dir = ctx.root.join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_stub_command(&bin_dir, "code", "exit 0");
    let path = format!("{}:{}", bin_dir.display(), ctx.path);

    let output = ctx.bbq_with_path(
        &["worktree", "open", "source", "feature-test"],
        &path,
    );
    assert_success_contains(output, "opened feature-test in VSCode");
}

#[test]
fn worktree_create_uses_default_city_name_when_configured() {
    let ctx = TestContext::new("worktree_create_uses_default_city_name_when_configured");
    let src_repo = ctx.root.join("source");
    init_repo(&src_repo);

    let output = ctx.bbq(&["repo", "clone", src_repo.to_str().expect("repo path")]);
    assert_success_contains(output, "checked out source");

    ctx.write_config("default_worktree_name = \"cities\"");

    let output = ctx.bbq(&["worktree", "create", "source"]);
    let stdout = assert_success(output);
    let name = stdout
        .trim()
        .strip_prefix("created ")
        .unwrap_or(stdout.trim())
        .to_string();
    assert_ne!(name, "main");
    assert!(bbq::validate_worktree_name(&name).is_ok());

    let output = ctx.bbq(&["worktree", "list", "source"]);
    assert_success_contains(output, &name);
}

struct TestContext {
    root: PathBuf,
    home: PathBuf,
    path: String,
}

impl TestContext {
    fn new(test_name: &str) -> Self {
        let root = unique_root(test_name);
        let home = root.join("home");
        fs::create_dir_all(&home).expect("create home");
        let path = std::env::var("PATH").unwrap_or_default();
        Self { root, home, path }
    }

    fn bbq(&self, args: &[&str]) -> Output {
        self.bbq_with_path(args, &self.path)
    }

    fn bbq_with_path(&self, args: &[&str], path: &str) -> Output {
        Command::new(bbq_bin())
            .args(args)
            .env("BBQ_ROOT_DIR", &self.root)
            .env("HOME", &self.home)
            .env("PATH", path)
            .output()
            .expect("run bbq")
    }

    fn write_config(&self, contents: &str) {
        let config_dir = self.home.join(".bbq");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(config_dir.join("config.toml"), contents).expect("write config");
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        cleanup_root(&self.root);
    }
}

fn bbq_bin() -> &'static str {
    env!("CARGO_BIN_EXE_bbq")
}

fn unique_root(test_name: &str) -> PathBuf {
    let workspace_root = workspace_root();
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let pid = std::process::id();
    workspace_root
        .join(".bbq-cli-test")
        .join(format!("{test_name}-{pid}-{seed}"))
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("workspace root")
}

fn init_repo(path: &Path) {
    fs::create_dir_all(path).expect("create repo dir");
    run_git(&["init", "--quiet"], path);
    run_git(&["config", "user.email", "bbq-test@example.com"], path);
    run_git(&["config", "user.name", "bbq-test"], path);
    fs::write(path.join("README.md"), "hello").expect("write README");
    run_git(&["add", "README.md"], path);
    run_git(&["commit", "--quiet", "-m", "init"], path);
}

fn run_git(args: &[&str], cwd: &Path) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run git");

    if !output.status.success() {
        panic!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn cleanup_root(root: &Path) {
    if root.exists() {
        fs::remove_dir_all(root).expect("cleanup root");
    }
}

fn write_stub_command(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    let contents = format!("#!/bin/sh\n{}\n", body);
    fs::write(&path, contents).expect("write stub command");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("set permissions");
    }
    path
}

fn assert_success(output: Output) -> String {
    if !output.status.success() {
        panic!(
            "command failed: {}\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn assert_success_contains(output: Output, needle: &str) {
    let stdout = assert_success(output);
    assert!(
        stdout.contains(needle),
        "expected stdout to contain {:?}, got {:?}",
        needle,
        stdout
    );
}

fn assert_failure_contains(output: Output, needle: &str) {
    if output.status.success() {
        panic!(
            "expected failure, got success.\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(needle),
        "expected stderr to contain {:?}, got {:?}",
        needle,
        stderr
    );
}
