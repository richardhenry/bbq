use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use bbq::{
    checkout_repo, checkout_repo_with_name, create_worktree, create_worktree_from,
    create_worktree_with_name, default_branch, default_remote_branch, list_repos, list_worktrees,
    remove_repo, remove_worktree, resolve_repo, BbqError,
};
use bbq::paths::{bbq_root, config_root, ensure_root_dirs, repos_root, worktrees_root};

static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn checkout_repo_and_list() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("checkout_repo_and_list");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);

    let repo = checkout_repo(src_repo.to_str().expect("repo path")).
        expect("checkout repo");
    assert_eq!(repo.name, "source");

    let repos = list_repos().expect("list repos");
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].name, "source");

    cleanup_root(&root);
}

#[test]
fn checkout_repo_with_custom_name() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("checkout_repo_with_custom_name");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);

    let repo = checkout_repo_with_name(src_repo.to_str().expect("repo path"), "custom")
        .expect("checkout repo");
    assert_eq!(repo.name, "custom");

    let resolved = resolve_repo("custom").expect("resolve repo");
    assert_eq!(resolved.name, "custom");

    cleanup_root(&root);
}

#[test]
fn checkout_repo_rejects_empty_url() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("checkout_repo_rejects_empty_url");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let err = checkout_repo("").expect_err("empty url should fail");
    assert!(matches!(err, BbqError::InvalidGitUrl));

    cleanup_root(&root);
}

#[test]
fn checkout_repo_with_invalid_name_is_rejected() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("checkout_repo_with_invalid_name_is_rejected");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);

    let err = checkout_repo_with_name(src_repo.to_str().expect("repo path"), "!!!")
        .expect_err("invalid name should fail");
    assert!(matches!(err, BbqError::InvalidRepoName));

    cleanup_root(&root);
}

#[test]
fn checkout_repo_duplicate_fails() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("checkout_repo_duplicate_fails");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);

    let repo = checkout_repo(src_repo.to_str().expect("repo path")).expect("checkout repo");
    let err = checkout_repo(src_repo.to_str().expect("repo path"))
        .expect_err("duplicate checkout should fail");
    assert!(matches!(err, BbqError::RepoAlreadyExists(name) if name == repo.name));

    cleanup_root(&root);
}

#[test]
fn create_list_and_remove_worktree() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("create_list_and_remove_worktree");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);

    let repo = checkout_repo(src_repo.to_str().expect("repo path")).expect("checkout repo");

    let worktree = create_worktree(&repo, "feature-test").expect("create worktree");
    assert_eq!(worktree.display_name(), "feature-test");

    let worktrees = list_worktrees(&repo).expect("list worktrees");
    assert_eq!(worktrees.len(), 1);
    assert_eq!(worktrees[0].display_name(), "feature-test");

    remove_worktree(&repo, "feature-test").expect("remove worktree");

    let worktrees = list_worktrees(&repo).expect("list worktrees after remove");
    assert!(worktrees.is_empty());

    remove_repo(&repo.name).expect("remove repo");

    cleanup_root(&root);
}

#[test]
fn create_worktree_from_source_branch() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("create_worktree_from_source_branch");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);

    let repo = checkout_repo(src_repo.to_str().expect("repo path")).expect("checkout repo");

    let worktree = create_worktree_from(
        &repo,
        "feature-from-head",
        "user/feature-from-head",
        "HEAD",
    )
    .expect("create worktree from source");
    assert_eq!(worktree.display_name(), "feature-from-head");
    assert_eq!(
        worktree.branch.as_deref(),
        Some("user/feature-from-head")
    );

    let worktrees = list_worktrees(&repo).expect("list worktrees");
    assert_eq!(worktrees.len(), 1);
    assert_eq!(worktrees[0].display_name(), "feature-from-head");

    remove_worktree(&repo, "feature-from-head").expect("remove worktree");
    remove_repo(&repo.name).expect("remove repo");
    cleanup_root(&root);
}

#[test]
fn create_worktree_with_remote_branch() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("create_worktree_with_remote_branch");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);
    run_git(&["branch", "feature/test"], &src_repo);

    let repo = checkout_repo(src_repo.to_str().expect("repo path")).expect("checkout repo");

    let worktree =
        create_worktree_with_name(&repo, "feature-test", "origin/feature/test")
            .expect("create worktree from remote branch");
    assert_eq!(worktree.display_name(), "feature-test");
    assert_eq!(worktree.branch.as_deref(), Some("feature/test"));

    remove_worktree(&repo, "feature-test").expect("remove worktree");
    remove_repo(&repo.name).expect("remove repo");
    cleanup_root(&root);
}

#[test]
fn create_worktree_with_remote_branch_without_fetch_refspec() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("create_worktree_with_remote_branch_without_fetch_refspec");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);
    run_git(&["branch", "feature/test"], &src_repo);

    let repo = checkout_repo(src_repo.to_str().expect("repo path")).expect("checkout repo");
    run_git(
        &[
            "config",
            "--replace-all",
            "remote.origin.fetch",
            "+HEAD:refs/remotes/origin/HEAD",
        ],
        &repo.path,
    );
    let remote_ref = repo
        .path
        .join("refs")
        .join("remotes")
        .join("origin")
        .join("feature")
        .join("test");
    let _ = fs::remove_file(remote_ref);

    let worktree =
        create_worktree_with_name(&repo, "feature-test", "origin/feature/test")
            .expect("create worktree from remote branch");
    assert_eq!(worktree.display_name(), "feature-test");
    assert_eq!(worktree.branch.as_deref(), Some("feature/test"));

    remove_worktree(&repo, "feature-test").expect("remove worktree");
    remove_repo(&repo.name).expect("remove repo");
    cleanup_root(&root);
}

#[test]
fn remove_repo_fails_with_worktrees() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("remove_repo_fails_with_worktrees");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);

    let repo = checkout_repo(src_repo.to_str().expect("repo path")).expect("checkout repo");
    let _worktree = create_worktree(&repo, "feature-test").expect("create worktree");

    let err = remove_repo(&repo.name).expect_err("repo with worktrees should fail");
    assert!(matches!(err, BbqError::RepoHasWorktrees));

    cleanup_root(&root);
}

#[test]
fn remove_worktree_missing_returns_error() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("remove_worktree_missing_returns_error");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);
    let repo = checkout_repo(src_repo.to_str().expect("repo path")).expect("checkout repo");

    let err = remove_worktree(&repo, "missing").expect_err("missing worktree should fail");
    assert!(matches!(err, BbqError::WorktreeNotFound(name) if name == "missing"));

    remove_repo(&repo.name).expect("remove repo");
    cleanup_root(&root);
}

#[test]
fn resolve_repo_trims_git_suffix_and_rejects_invalid() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("resolve_repo_trims_git_suffix_and_rejects_invalid");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);
    let repo = checkout_repo(src_repo.to_str().expect("repo path")).expect("checkout repo");

    let resolved = resolve_repo("source.git").expect("resolve repo");
    assert_eq!(resolved.name, repo.name);

    let err = resolve_repo("!!!").expect_err("invalid repo name should fail");
    assert!(matches!(err, BbqError::InvalidRepoName));

    remove_repo(&repo.name).expect("remove repo");
    cleanup_root(&root);
}

#[test]
fn list_repos_ignores_non_git_dirs_and_sorts() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("list_repos_ignores_non_git_dirs_and_sorts");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_alpha = root.join("alpha");
    let src_beta = root.join("beta");
    init_repo(&src_alpha);
    init_repo(&src_beta);

    let _alpha = checkout_repo(src_alpha.to_str().expect("repo path")).expect("checkout alpha");
    let _beta = checkout_repo(src_beta.to_str().expect("repo path")).expect("checkout beta");

    let junk = repos_root().expect("repos root").join("junk.git");
    fs::create_dir_all(&junk).expect("create junk repo dir");

    let repos = list_repos().expect("list repos");
    let names: Vec<_> = repos.into_iter().map(|repo| repo.name).collect();
    assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);

    cleanup_root(&root);
}

#[test]
fn default_remote_branch_returns_origin_head() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("default_remote_branch_returns_origin_head");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);
    let branch = current_branch(&src_repo);

    let repo = checkout_repo(src_repo.to_str().expect("repo path")).expect("checkout repo");
    let origin_head = format!("refs/remotes/origin/{branch}");
    let output = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD", &origin_head])
        .current_dir(&repo.path)
        .output()
        .expect("set origin/HEAD");
    assert!(output.status.success(), "set origin/HEAD");
    let default = default_remote_branch(&repo)
        .expect("default remote branch")
        .expect("origin/HEAD should exist");
    assert_eq!(default, format!("origin/{branch}"));

    remove_repo(&repo.name).expect("remove repo");
    cleanup_root(&root);
}

#[test]
fn default_branch_falls_back_to_head_when_origin_head_missing() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("default_branch_falls_back_to_head_when_origin_head_missing");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    let src_repo = root.join("source");
    init_repo(&src_repo);
    let branch = current_branch(&src_repo);

    let repo = checkout_repo(src_repo.to_str().expect("repo path")).expect("checkout repo");
    let origin_head = repo.path.join("refs").join("remotes").join("origin").join("HEAD");
    let _ = fs::remove_file(origin_head);

    let default = default_branch(&repo)
        .expect("default branch")
        .expect("should have default branch");
    assert_eq!(default, format!("origin/{branch}"));

    remove_repo(&repo.name).expect("remove repo");
    cleanup_root(&root);
}

#[test]
fn paths_prefers_env_var_over_config() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("paths_prefers_env_var_over_config");
    let home = root.join("home");
    fs::create_dir_all(&home).expect("create home");
    let _home_env = EnvGuard::set("HOME", &home);
    let env_root = root.join("env-root");
    let _bbq_env = EnvGuard::set("BBQ_ROOT_DIR", &env_root);

    write_config(&home, "root_dir = \"~/config-root\"");

    let resolved = bbq_root().expect("bbq_root");
    assert_eq!(resolved, env_root);

    cleanup_root(&root);
}

#[test]
fn paths_reads_config_root_dir_with_tilde() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("paths_reads_config_root_dir_with_tilde");
    let home = root.join("home");
    fs::create_dir_all(&home).expect("create home");
    let _home_env = EnvGuard::set("HOME", &home);
    let _bbq_env = EnvGuard::unset("BBQ_ROOT_DIR");

    write_config(&home, "root_dir = \"~/bbq-data\"");

    let resolved = bbq_root().expect("bbq_root");
    assert_eq!(resolved, home.join("bbq-data"));

    cleanup_root(&root);
}

#[test]
fn paths_empty_root_dir_falls_back_to_config_root() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("paths_empty_root_dir_falls_back_to_config_root");
    let home = root.join("home");
    fs::create_dir_all(&home).expect("create home");
    let _home_env = EnvGuard::set("HOME", &home);
    let _bbq_env = EnvGuard::unset("BBQ_ROOT_DIR");

    write_config(&home, "root_dir = ''");

    let resolved = bbq_root().expect("bbq_root");
    assert_eq!(resolved, config_root().expect("config root"));

    cleanup_root(&root);
}

#[test]
fn ensure_root_dirs_creates_structure() {
    let _guard = TEST_MUTEX.lock().expect("lock test mutex");
    let root = unique_root("ensure_root_dirs_creates_structure");
    let _env = EnvGuard::set("BBQ_ROOT_DIR", &root);

    ensure_root_dirs().expect("ensure root dirs");
    assert!(repos_root().expect("repos root").is_dir());
    assert!(worktrees_root().expect("worktrees root").is_dir());

    cleanup_root(&root);
}

fn unique_root(test_name: &str) -> PathBuf {
    let workspace_root = workspace_root();
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let pid = std::process::id();
    workspace_root
        .join(".bbq-test")
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

fn run_git_capture(args: &[&str], cwd: &Path) -> String {
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

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn current_branch(path: &Path) -> String {
    run_git_capture(&["symbolic-ref", "--short", "HEAD"], path)
}

fn cleanup_root(root: &Path) {
    if root.exists() {
        fs::remove_dir_all(root).expect("cleanup root");
    }
}

struct EnvGuard {
    key: &'static str,
    prev: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &Path) -> Self {
        let prev = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, prev }
    }

    fn unset(key: &'static str) -> Self {
        let prev = std::env::var_os(key);
        std::env::remove_var(key);
        Self { key, prev }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(prev) = &self.prev {
            std::env::set_var(self.key, prev);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn write_config(home: &Path, contents: &str) {
    let config_dir = home.join(".bbq");
    fs::create_dir_all(&config_dir).expect("create config dir");
    fs::write(config_dir.join("config.toml"), contents).expect("write config");
}
