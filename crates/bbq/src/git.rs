use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{BbqError, Result};
use crate::model::{Repo, Worktree};
use crate::paths::{config_root, ensure_root_dirs, repos_root, worktrees_root};

pub fn list_repos() -> Result<Vec<Repo>> {
    ensure_root_dirs()?;
    let root = repos_root()?;
    let mut repos = Vec::new();

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let head = path.join("HEAD");
        if !head.is_file() {
            continue;
        }

        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let name = name.trim_end_matches(".git").to_string();

        repos.push(Repo { name, path });
    }

    repos.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(repos)
}

pub fn checkout_repo(url: &str) -> Result<Repo> {
    checkout_repo_internal(url, None)
}

pub fn checkout_repo_with_name(url: &str, name: &str) -> Result<Repo> {
    checkout_repo_internal(url, Some(name))
}

fn checkout_repo_internal(source: &str, name_override: Option<&str>) -> Result<Repo> {
    ensure_root_dirs()?;
    let source = source.trim();
    if source.is_empty() {
        return Err(BbqError::InvalidGitUrl);
    }

    let name = match name_override {
        Some(name) => sanitize_name(name),
        None => repo_name_from_url(source)?,
    };
    if name.is_empty() {
        return Err(BbqError::InvalidRepoName);
    }
    let dest = repos_root()?.join(format!("{name}.git"));

    if dest.exists() {
        return Err(BbqError::RepoAlreadyExists(name));
    }

    if let Some(slug) = github_slug_from_source(source) {
        if !gh_available() {
            return Err(BbqError::GitHubCliMissing);
        }
        run_gh_clone(&slug, &dest)?;
    } else {
        run_git_clone(source, &dest)?;
    }

    Ok(Repo { name, path: dest })
}

fn run_git_clone(source: &str, dest: &Path) -> Result<()> {
    let args = vec![
        OsString::from("clone"),
        OsString::from("--bare"),
        OsString::from(source.trim()),
        dest.as_os_str().to_os_string(),
    ];
    run_git(args)
}

fn run_gh_clone(slug: &str, dest: &Path) -> Result<()> {
    let args = vec![
        OsString::from("repo"),
        OsString::from("clone"),
        OsString::from(slug),
        dest.as_os_str().to_os_string(),
        OsString::from("--"),
        OsString::from("--bare"),
    ];
    run_gh(args)
}

fn gh_available() -> bool {
    gh_command()
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn list_worktrees(repo: &Repo) -> Result<Vec<Worktree>> {
    let args = vec![
        OsString::from("--git-dir"),
        repo.path.as_os_str().to_os_string(),
        OsString::from("worktree"),
        OsString::from("list"),
        OsString::from("--porcelain"),
    ];
    let output = run_git_capture(args)?;
    Ok(parse_worktrees(&output, &repo.path))
}

pub fn default_remote_branch(repo: &Repo) -> Result<Option<String>> {
    let args = vec![
        OsString::from("--git-dir"),
        repo.path.as_os_str().to_os_string(),
        OsString::from("symbolic-ref"),
        OsString::from("refs/remotes/origin/HEAD"),
    ];
    let output = git_command().args(&args).output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(line) = stdout.lines().next() else {
        return Ok(None);
    };
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let branch = trimmed
        .strip_prefix("refs/remotes/")
        .unwrap_or(trimmed)
        .trim();
    if branch.is_empty() {
        return Ok(None);
    }
    Ok(Some(branch.to_string()))
}

pub fn default_branch(repo: &Repo) -> Result<Option<String>> {
    if let Some(branch) = default_remote_branch(repo)? {
        return Ok(Some(branch));
    }

    if let Some(branch) = symbolic_head_branch(repo)? {
        if has_remote(repo, "origin")? {
            return Ok(Some(format!("origin/{branch}")));
        }
        return Ok(Some(branch));
    }

    let candidates = [
        "refs/remotes/origin/main",
        "refs/remotes/origin/master",
        "refs/heads/main",
        "refs/heads/master",
    ];
    for reference in candidates {
        if git_ref_exists(&repo.path, reference)? {
            return Ok(Some(ref_to_branch_name(reference)));
        }
    }

    Ok(None)
}

fn has_remote(repo: &Repo, name: &str) -> Result<bool> {
    Ok(list_remotes(repo)?.iter().any(|remote| remote == name))
}

fn symbolic_head_branch(repo: &Repo) -> Result<Option<String>> {
    let args = vec![
        OsString::from("--git-dir"),
        repo.path.as_os_str().to_os_string(),
        OsString::from("symbolic-ref"),
        OsString::from("HEAD"),
    ];
    let output = git_command().args(&args).output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(line) = stdout.lines().next() else {
        return Ok(None);
    };
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let branch = trimmed
        .strip_prefix("refs/heads/")
        .unwrap_or(trimmed)
        .trim();
    if branch.is_empty() {
        return Ok(None);
    }
    Ok(Some(branch.to_string()))
}

fn ref_to_branch_name(reference: &str) -> String {
    reference
        .strip_prefix("refs/remotes/")
        .or_else(|| reference.strip_prefix("refs/heads/"))
        .unwrap_or(reference)
        .to_string()
}

pub fn create_worktree(repo: &Repo, branch: &str) -> Result<Worktree> {
    create_worktree_with_name(repo, branch, branch)
}

pub fn create_worktree_with_name(repo: &Repo, name: &str, branch: &str) -> Result<Worktree> {
    ensure_root_dirs()?;
    let name = name.trim();
    if name.is_empty() {
        return Err(BbqError::InvalidWorktreeName);
    }
    let branch_spec = branch.trim();
    if branch_spec.is_empty() {
        return Err(BbqError::InvalidBranchName);
    }
    let branch_spec = branch_spec.to_string();

    let base_dir = worktrees_root()?.join(&repo.name);
    fs::create_dir_all(&base_dir)?;

    let worktree_path = base_dir.join(name);
    if worktree_path.exists() {
        return Err(BbqError::WorktreeAlreadyExists(name.to_string()));
    }

    let (branch_name, start_point) = match parse_remote_branch(repo, &branch_spec)? {
        Some((remote, remote_branch)) => {
            fetch_repo(repo, Some(&remote))?;
            let remote_ref = format!("refs/remotes/{remote}/{remote_branch}");
            if !git_ref_exists(&repo.path, &remote_ref)? {
                fetch_remote_branch(repo, &remote, &remote_branch)?;
            }
            let branch_ref = format!("refs/heads/{remote_branch}");
            let branch_exists = git_ref_exists(&repo.path, &branch_ref)?;
            if branch_exists {
                (remote_branch, None)
            } else {
                (remote_branch.clone(), Some(format!("{remote}/{remote_branch}")))
            }
        }
        None => {
            let branch_ref = format!("refs/heads/{branch_spec}");
            let branch_exists = git_ref_exists(&repo.path, &branch_ref)?;
            if branch_exists {
                (branch_spec, None)
            } else {
                (branch_spec.clone(), Some("HEAD".to_string()))
            }
        }
    };

    let mut args = vec![
        OsString::from("--git-dir"),
        repo.path.as_os_str().to_os_string(),
        OsString::from("worktree"),
        OsString::from("add"),
    ];

    if let Some(start_point) = start_point {
        args.push(OsString::from("-b"));
        args.push(OsString::from(branch_name.clone()));
        args.push(worktree_path.as_os_str().to_os_string());
        args.push(OsString::from(start_point));
    } else {
        args.push(worktree_path.as_os_str().to_os_string());
        args.push(OsString::from(branch_name.clone()));
    }

    run_git(args)?;

    Ok(Worktree {
        path: worktree_path,
        branch: Some(branch_name),
        head: None,
    })
}

pub fn create_worktree_from(
    repo: &Repo,
    name: &str,
    branch: &str,
    source_branch: &str,
) -> Result<Worktree> {
    ensure_root_dirs()?;
    let name = name.trim();
    if name.is_empty() {
        return Err(BbqError::InvalidWorktreeName);
    }
    let branch = branch.trim();
    if branch.is_empty() {
        return Err(BbqError::InvalidBranchName);
    }
    let source_branch = source_branch.trim();
    if source_branch.is_empty() {
        return Err(BbqError::InvalidBranchName);
    }

    let base_dir = worktrees_root()?.join(&repo.name);
    fs::create_dir_all(&base_dir)?;

    let worktree_path = base_dir.join(name);
    if worktree_path.exists() {
        return Err(BbqError::WorktreeAlreadyExists(name.to_string()));
    }

    let branch_ref = format!("refs/heads/{branch}");
    let branch_exists = git_ref_exists(&repo.path, &branch_ref)?;

    let start_point = if branch_exists {
        None
    } else {
        Some(resolve_source_branch(repo, source_branch)?)
    };

    let mut args = vec![
        OsString::from("--git-dir"),
        repo.path.as_os_str().to_os_string(),
        OsString::from("worktree"),
        OsString::from("add"),
    ];

    if let Some(start_point) = start_point {
        args.push(OsString::from("-b"));
        args.push(OsString::from(branch));
        args.push(worktree_path.as_os_str().to_os_string());
        args.push(OsString::from(start_point));
    } else {
        args.push(worktree_path.as_os_str().to_os_string());
        args.push(OsString::from(branch));
    }

    run_git(args)?;

    Ok(Worktree {
        path: worktree_path,
        branch: Some(branch.to_string()),
        head: None,
    })
}

fn fetch_repo(repo: &Repo, remote: Option<&str>) -> Result<()> {
    let mut args = vec![
        OsString::from("--git-dir"),
        repo.path.as_os_str().to_os_string(),
        OsString::from("fetch"),
    ];
    if let Some(remote) = remote {
        args.push(OsString::from(remote));
    }
    run_git(args)
}

fn list_remotes(repo: &Repo) -> Result<Vec<String>> {
    let args = vec![
        OsString::from("--git-dir"),
        repo.path.as_os_str().to_os_string(),
        OsString::from("remote"),
    ];
    let output = run_git_capture(args)?;
    let remotes = output
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    Ok(remotes)
}

fn parse_remote_branch(repo: &Repo, branch: &str) -> Result<Option<(String, String)>> {
    if !branch.contains('/') {
        return Ok(None);
    }

    let mut parts = branch.splitn(2, '/');
    let Some(remote) = parts.next() else {
        return Ok(None);
    };
    let Some(remote_branch) = parts.next() else {
        return Ok(None);
    };
    if remote_branch.is_empty() {
        return Ok(None);
    }

    let remotes = list_remotes(repo)?;
    if remotes.iter().any(|name| name == remote) {
        Ok(Some((remote.to_string(), remote_branch.to_string())))
    } else {
        Ok(None)
    }
}

fn resolve_source_branch(repo: &Repo, source_branch: &str) -> Result<String> {
    if let Some((remote, remote_branch)) = parse_remote_branch(repo, source_branch)? {
        fetch_repo(repo, Some(&remote))?;
        let remote_ref = format!("refs/remotes/{remote}/{remote_branch}");
        if !git_ref_exists(&repo.path, &remote_ref)? {
            fetch_remote_branch(repo, &remote, &remote_branch)?;
        }
        Ok(format!("{remote}/{remote_branch}"))
    } else {
        Ok(source_branch.to_string())
    }
}

pub fn remove_worktree(repo: &Repo, name: &str) -> Result<()> {
    remove_worktree_with_force(repo, name, false)
}

pub fn remove_worktree_with_force(repo: &Repo, name: &str, force: bool) -> Result<()> {
    let worktrees = list_worktrees(repo)?;
    let worktree = worktrees
        .into_iter()
        .find(|item| worktree_matches_name(item, name))
        .ok_or_else(|| BbqError::WorktreeNotFound(name.to_string()))?;

    let mut args = vec![
        OsString::from("--git-dir"),
        repo.path.as_os_str().to_os_string(),
        OsString::from("worktree"),
        OsString::from("remove"),
    ];

    if force {
        args.push(OsString::from("--force"));
    }
    args.push(worktree.path.as_os_str().to_os_string());

    run_git(args)
}

pub fn remove_repo(name: &str) -> Result<()> {
    let repo = resolve_repo(name)?;
    let worktrees = list_worktrees(&repo)?;
    if !worktrees.is_empty() {
        return Err(BbqError::RepoHasWorktrees);
    }

    fs::remove_dir_all(repo.path)?;
    Ok(())
}

pub fn resolve_repo(name: &str) -> Result<Repo> {
    let mut name = sanitize_name(name);
    if name.is_empty() {
        return Err(BbqError::InvalidRepoName);
    }

    if name.ends_with(".git") {
        name = name.trim_end_matches(".git").to_string();
    }

    let path = repos_root()?.join(format!("{name}.git"));
    if !path.exists() {
        return Err(BbqError::RepoNotFound(name));
    }

    Ok(Repo { name, path })
}

fn git_ref_exists(repo_path: &Path, reference: &str) -> Result<bool> {
    let args = vec![
        OsString::from("--git-dir"),
        repo_path.as_os_str().to_os_string(),
        OsString::from("show-ref"),
        OsString::from("--verify"),
        OsString::from("--quiet"),
        OsString::from(reference),
    ];

    let output = git_command().args(&args).output()?;
    Ok(output.status.success())
}

fn fetch_remote_branch(repo: &Repo, remote: &str, branch: &str) -> Result<()> {
    let refspec = format!("refs/heads/{branch}:refs/remotes/{remote}/{branch}");
    let args = vec![
        OsString::from("--git-dir"),
        repo.path.as_os_str().to_os_string(),
        OsString::from("fetch"),
        OsString::from(remote),
        OsString::from(refspec),
    ];
    run_git(args)
}

fn parse_worktrees(output: &str, repo_path: &Path) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut current = WorktreeBuilder::default();

    for line in output.lines() {
        if line.trim().is_empty() {
            if let Some(worktree) = current.build(repo_path) {
                worktrees.push(worktree);
            }
            current = WorktreeBuilder::default();
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            current.path = Some(PathBuf::from(path));
            continue;
        }

        if let Some(branch) = line.strip_prefix("branch ") {
            let name = branch
                .strip_prefix("refs/heads/")
                .unwrap_or(branch)
                .to_string();
            current.branch = Some(name);
            continue;
        }

        if let Some(head) = line.strip_prefix("HEAD ") {
            current.head = Some(head.to_string());
            continue;
        }

        if line.trim() == "bare" {
            current.is_bare = true;
        }
    }

    if let Some(worktree) = current.build(repo_path) {
        worktrees.push(worktree);
    }

    worktrees.sort_by(|a, b| a.display_name().cmp(&b.display_name()));
    worktrees
}

fn worktree_matches_name(worktree: &Worktree, name: &str) -> bool {
    worktree.display_name() == name
        || worktree
            .branch
            .as_deref()
            .map(|branch| branch == name)
            .unwrap_or(false)
}

#[derive(Default)]
struct WorktreeBuilder {
    path: Option<PathBuf>,
    branch: Option<String>,
    head: Option<String>,
    is_bare: bool,
}

impl WorktreeBuilder {
    fn build(self, repo_path: &Path) -> Option<Worktree> {
        let path = self.path?;
        if self.is_bare || path == repo_path {
            return None;
        }

        Some(Worktree {
            path,
            branch: self.branch,
            head: self.head,
        })
    }
}

fn repo_name_from_url(url: &str) -> Result<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(BbqError::InvalidGitUrl);
    }

    let mut tail = trimmed;
    if let Some(idx) = trimmed.rfind(':') {
        if trimmed[..idx].contains('@') {
            tail = &trimmed[idx + 1..];
        }
    }

    if let Some(idx) = tail.rfind('/') {
        tail = &tail[idx + 1..];
    }

    let name = tail.trim_end_matches(".git");
    let name = sanitize_name(name);

    if name.is_empty() {
        return Err(BbqError::InvalidGitUrl);
    }

    Ok(name)
}

fn github_slug_from_source(source: &str) -> Option<String> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().any(|ch| ch.is_whitespace()) {
        return None;
    }
    if looks_like_url_or_ssh(trimmed) || is_path_like(trimmed) {
        return None;
    }

    let trimmed = trimmed.trim_end_matches('/');
    let trimmed = trimmed.trim_end_matches(".git");
    let mut parts = trimmed.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    if !is_slug_part(owner) || !is_slug_part(repo) {
        return None;
    }

    Some(format!("{}/{}", owner, repo))
}

fn looks_like_url_or_ssh(value: &str) -> bool {
    value.contains("://")
        || value.starts_with("git@")
        || (value.contains('@') && value.contains(':'))
}

fn is_path_like(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("./")
        || value.starts_with("../")
        || value.starts_with("~/")
        || Path::new(value).exists()
}

fn is_slug_part(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
}

fn sanitize_name(raw: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;

    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }

    out.trim_matches('-').to_string()
}

fn run_git(args: Vec<OsString>) -> Result<()> {
    let output = git_command().args(&args).output()?;
    if output.status.success() {
        return Ok(());
    }

    Err(BbqError::GitCommand {
        command: format!("git {}", args_to_string(&args)),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn run_gh(args: Vec<OsString>) -> Result<()> {
    let output = gh_command().args(&args).output().map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            BbqError::GitHubCliMissing
        } else {
            BbqError::Io(err)
        }
    })?;
    if output.status.success() {
        return Ok(());
    }

    Err(BbqError::GitHubCliCommand {
        command: format!("gh {}", args_to_string(&args)),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn run_git_capture(args: Vec<OsString>) -> Result<String> {
    let output = git_command().args(&args).output()?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }

    Err(BbqError::GitCommand {
        command: format!("git {}", args_to_string(&args)),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn args_to_string(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

fn git_command() -> Command {
    let mut command = Command::new("git");
    apply_safe_cwd(&mut command);
    command
}

fn gh_command() -> Command {
    let mut command = Command::new("gh");
    apply_safe_cwd(&mut command);
    command
}

fn apply_safe_cwd(command: &mut Command) {
    if std::env::current_dir().is_ok() {
        return;
    }

    if let Ok(path) = config_root() {
        let _ = fs::create_dir_all(&path);
        command.current_dir(path);
    }
}
