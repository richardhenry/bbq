use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use notify::{RecursiveMode, Watcher};

use bbq::{
    checkout_repo, create_worktree_from, list_repos, list_worktrees, remove_repo,
    remove_worktree_with_force, Repo,
};
use bbq::paths;

use crate::update;

use super::types::{AllData, ChangedFile, WorktreeEntry, WorkerEvent, WorkerRequest};

pub(crate) fn start_background_tasks(
) -> (mpsc::Sender<WorkerRequest>, mpsc::Receiver<WorkerEvent>) {
    let (request_tx, request_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    spawn_worker(request_rx, event_tx.clone());
    spawn_filesystem_watcher(event_tx.clone());
    (request_tx, event_rx)
}

fn spawn_worker(request_rx: mpsc::Receiver<WorkerRequest>, event_tx: mpsc::Sender<WorkerEvent>) {
    thread::spawn(move || {
        for request in request_rx {
            match request {
                WorkerRequest::LoadEnvInfo => {
                    let home_dir = (|| {
                        let bbq_root = paths::bbq_root().ok()?;
                        let user_home = paths::config_root().ok()?.parent()?.to_path_buf();
                        Some(display_path_with_tilde(&bbq_root, &user_home))
                    })();
                    let git_version = command_version("git", &["--version"]);
                    let gh_version = command_version("gh", &["--version"]);
                    let _ = event_tx.send(WorkerEvent::EnvInfoLoaded {
                        home_dir,
                        git_version,
                        gh_version,
                    });
                }
                WorkerRequest::CheckForUpdate => {
                    let latest = update::check_homebrew_update().map(|info| info.latest);
                    let _ = event_tx.send(WorkerEvent::UpdateCheckResult { latest });
                }
                WorkerRequest::RunUpgrade => {
                    let result = update::run_homebrew_upgrade();
                    let _ = event_tx.send(WorkerEvent::UpgradeResult { result });
                }
                WorkerRequest::LoadAll { request_id } => {
                    let result = load_all_data().map_err(|err| err.to_string());
                    let _ = event_tx.send(WorkerEvent::AllDataLoaded { request_id, result });
                }
                WorkerRequest::CheckoutRepo { url } => {
                    let result = checkout_repo(&url).map_err(|err| err.to_string());
                    let _ = event_tx.send(WorkerEvent::CheckoutRepoResult { result });
                }
                WorkerRequest::CreateWorktree {
                    repo,
                    name,
                    branch,
                    source_branch,
                } => {
                    let repo_name = repo.name.clone();
                    let result =
                        create_worktree_from(&repo, &name, &branch, &source_branch).map_err(
                            |err| err.to_string(),
                        );
                    let _ = event_tx.send(WorkerEvent::CreateWorktreeResult { repo_name, result });
                }
                WorkerRequest::DeleteRepo { name } => {
                    let result = remove_repo(&name).map_err(|err| err.to_string());
                    let _ = event_tx.send(WorkerEvent::DeleteRepoResult { name, result });
                }
                WorkerRequest::DeleteWorktree { repo, name, force } => {
                    let repo_name = repo.name.clone();
                    let worktree_name = name.clone();
                    let result =
                        remove_worktree_with_force(&repo, &name, force).map_err(|err| err.to_string());
                    let _ = event_tx.send(WorkerEvent::DeleteWorktreeResult {
                        repo_name,
                        worktree_name,
                        result,
                    });
                }
            }
        }
    });
}

fn spawn_filesystem_watcher(event_tx: mpsc::Sender<WorkerEvent>) {
    thread::spawn(move || {
        let _ = paths::ensure_root_dirs();
        let repos_root = match paths::repos_root() {
            Ok(root) => root,
            Err(_) => return,
        };
        let worktrees_root = match paths::worktrees_root() {
            Ok(root) => root,
            Err(_) => return,
        };

        let (watch_tx, watch_rx) = mpsc::channel();
        let mut watcher = match notify::recommended_watcher(move |res| {
            let _ = watch_tx.send(res);
        }) {
            Ok(watcher) => watcher,
            Err(_) => return,
        };

        if watcher
            .watch(&repos_root, RecursiveMode::Recursive)
            .is_err()
        {
            return;
        }
        let _ = watcher.watch(&worktrees_root, RecursiveMode::NonRecursive);

        let mut last_event = Instant::now() - Duration::from_secs(5);
        let debounce = Duration::from_millis(250);
        for event in watch_rx {
            let event = match event {
                Ok(event) => event,
                Err(_) => continue,
            };
            if !is_relevant_fs_event(&event, &repos_root, &worktrees_root) {
                continue;
            }
            let now = Instant::now();
            if now.duration_since(last_event) < debounce {
                continue;
            }
            last_event = now;
            let _ = event_tx.send(WorkerEvent::FsChanged);
        }
    });
}

fn is_relevant_fs_event(event: &notify::Event, repos_root: &Path, worktrees_root: &Path) -> bool {
    event
        .paths
        .iter()
        .any(|path| is_relevant_path(path, repos_root, worktrees_root))
}

fn is_relevant_path(path: &Path, repos_root: &Path, worktrees_root: &Path) -> bool {
    if path.starts_with(worktrees_root) {
        return path.parent().is_some_and(|parent| parent == worktrees_root);
    }

    if !path.starts_with(repos_root) {
        return false;
    }

    let rel = match path.strip_prefix(repos_root) {
        Ok(rel) => rel,
        Err(_) => return false,
    };
    let mut components = rel.components().filter_map(|component| match component {
        std::path::Component::Normal(name) => Some(name),
        _ => None,
    });

    let _repo_dir = match components.next() {
        Some(name) if !name.is_empty() => name,
        _ => return false,
    };

    let Some(next) = components.next() else {
        return true;
    };

    let next = next.to_string_lossy();
    match next.as_ref() {
        "refs" | "HEAD" | "packed-refs" => true,
        _ => false,
    }
}

fn build_worktree_entries(repo: &Repo) -> bbq::Result<Vec<WorktreeEntry>> {
    let home_dir = home_dir_path();
    let worktrees = list_worktrees(repo)?;
    let mut entries: Vec<WorktreeEntry> = worktrees
        .into_iter()
        .map(|worktree| {
            let info = head_commit_info(&worktree.path);
            let (head_author, head_message) = match info {
                Some(info) => (Some(info.author), Some(info.message)),
                None => (None, None),
            };
            let upstream = worktree_upstream_ref(&worktree.path);
            let sync_status = match upstream.as_ref() {
                Some(upstream) => {
                    format_sync_status(upstream, head_divergence(&worktree.path, &upstream.rev))
                }
                None => "no upstream".to_string(),
            };
            let worktree_path = match home_dir.as_ref() {
                Some(home) => display_path_with_tilde(&worktree.path, home),
                None => worktree.path.display().to_string(),
            };
            let changed_files = git_changed_files(&worktree.path);
            WorktreeEntry {
                worktree,
                head_author,
                head_message,
                upstream: upstream.as_ref().map(|ref_value| ref_value.display.clone()),
                sync_status,
                worktree_path,
                changed_files,
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        compare_path_time(&a.worktree.path, &b.worktree.path)
            .then_with(|| a.worktree.display_name().cmp(&b.worktree.display_name()))
    });

    Ok(entries)
}

fn load_all_data() -> bbq::Result<AllData> {
    let mut repos = list_repos()?;
    repos.sort_by(|a, b| compare_path_time(&a.path, &b.path).then_with(|| a.name.cmp(&b.name)));
    let mut repo_worktrees = HashMap::new();
    let gh_available = command_version("gh", &["--version"]).is_some();
    let mut repo_display = HashMap::new();
    let mut error = None;

    for repo in &repos {
        match build_worktree_entries(repo) {
            Ok(entries) => {
                repo_worktrees.insert(repo.name.clone(), entries);
            }
            Err(err) => {
                error = Some(err.to_string());
                repo_worktrees.insert(repo.name.clone(), Vec::new());
            }
        }
        if gh_available {
            if let Some(display) = repo_github_name(repo) {
                repo_display.insert(repo.name.clone(), display);
            }
        }
    }

    Ok(AllData {
        repos,
        repo_worktrees,
        repo_display,
        error,
    })
}

struct CommitInfo {
    author: String,
    message: String,
}

struct UpstreamRef {
    rev: String,
    display: String,
}

fn head_commit_info(path: &Path) -> Option<CommitInfo> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["log", "-1", "--format=%an%n%s"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let author = lines.next().unwrap_or("").trim().to_string();
    let message = lines.next().unwrap_or("").trim().to_string();
    if author.is_empty() && message.is_empty() {
        return None;
    }

    Some(CommitInfo { author, message })
}

fn worktree_upstream_ref(path: &Path) -> Option<UpstreamRef> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value = stdout.lines().next()?.trim();
    if value.is_empty() {
        return None;
    }

    let display = display_ref_name(value);
    Some(UpstreamRef {
        rev: value.to_string(),
        display,
    })
}

fn display_ref_name(reference: &str) -> String {
    reference
        .strip_prefix("refs/remotes/")
        .or_else(|| reference.strip_prefix("refs/heads/"))
        .unwrap_or(reference)
        .to_string()
}

fn head_divergence(path: &Path, upstream_ref: &str) -> Option<(u32, u32)> {
    let range = format!("HEAD...{upstream_ref}");
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-list", "--left-right", "--count", &range])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut parts = stdout.split_whitespace();
    let ahead = parts.next()?.parse().ok()?;
    let behind = parts.next()?.parse().ok()?;
    Some((ahead, behind))
}

fn repo_github_name(repo: &Repo) -> Option<String> {
    let url = git_remote_url(repo, "origin")?;
    parse_github_name(&url)
}

fn git_remote_url(repo: &Repo, remote: &str) -> Option<String> {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(&repo.path)
        .args(["remote", "get-url", remote])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let url = stdout.lines().next()?.trim();
    if url.is_empty() {
        None
    } else {
        Some(url.to_string())
    }
}

fn parse_github_name(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let trimmed = trimmed.trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);

    let rest = if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("ssh://git@github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("https://www.github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://www.github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("git://github.com/") {
        rest
    } else {
        return None;
    };

    let mut parts = rest.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    Some(format!("{owner}/{repo}"))
}

fn commit_count(count: u32) -> String {
    if count == 1 {
        "1 commit".to_string()
    } else {
        format!("{count} commits")
    }
}

fn format_sync_status(upstream: &UpstreamRef, divergence: Option<(u32, u32)>) -> String {
    match divergence {
        Some((0, 0)) => format!("up to date with {}", upstream.display),
        Some((ahead, 0)) => format!(
            "ahead of {} by {}",
            upstream.display,
            commit_count(ahead)
        ),
        Some((0, behind)) => format!(
            "behind {} by {}",
            upstream.display,
            commit_count(behind)
        ),
        Some((ahead, behind)) => format!(
            "diverged from {} ({} ahead, {} behind)",
            upstream.display,
            commit_count(ahead),
            commit_count(behind)
        ),
        None => format!("unknown vs {}", upstream.display),
    }
}

fn home_dir_path() -> Option<PathBuf> {
    let bbq_home = paths::config_root().ok()?;
    bbq_home.parent().map(|parent| parent.to_path_buf())
}

fn git_changed_files(path: &Path) -> Vec<ChangedFile> {
    let mut diff_stats = git_diff_numstat(path);
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain"])
        .output();
    let output = match output {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();
    for line in stdout.lines() {
        if line.len() < 3 {
            continue;
        }
        let status = &line[..2];
        let path_part = line.get(3..).unwrap_or("").trim();
        if path_part.is_empty() {
            continue;
        }
        let file = if let Some((_, new)) = path_part.split_once("->") {
            new.trim().to_string()
        } else {
            path_part.to_string()
        };
        if file.is_empty() {
            continue;
        }
        let (added, removed) = diff_stats.remove(&file).unwrap_or_else(|| {
            if status == "??" {
                (count_file_lines(path, &file), 0)
            } else {
                (0, 0)
            }
        });
        files.push(ChangedFile {
            path: file,
            added,
            removed,
        });
    }
    files
}

fn git_diff_numstat(path: &Path) -> HashMap<String, (u32, u32)> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["diff", "--numstat", "HEAD"])
        .output();
    let output = match output {
        Ok(output) if output.status.success() => output,
        _ => return HashMap::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut stats = HashMap::new();
    for line in stdout.lines() {
        let mut parts = line.split('\t');
        let added_raw = parts.next().unwrap_or("");
        let removed_raw = parts.next().unwrap_or("");
        let path_raw = parts.next().unwrap_or("").trim();
        if path_raw.is_empty() {
            continue;
        }
        let added = added_raw.parse::<u32>().unwrap_or(0);
        let removed = removed_raw.parse::<u32>().unwrap_or(0);
        let file = if let Some((_, new)) = path_raw.split_once("->") {
            new.trim().to_string()
        } else {
            path_raw.to_string()
        };
        if !file.is_empty() {
            stats.insert(file, (added, removed));
        }
    }
    stats
}

fn count_file_lines(repo_path: &Path, file: &str) -> u32 {
    let path = repo_path.join(file);
    let content = fs::read_to_string(path);
    let content = match content {
        Ok(content) => content,
        Err(_) => return 0,
    };
    let mut lines = content.lines().count() as u32;
    if !content.is_empty() && !content.ends_with('\n') {
        lines += 1;
    }
    lines
}

fn path_timestamp(path: &Path) -> Option<SystemTime> {
    let metadata = fs::metadata(path).ok()?;
    metadata.created().or_else(|_| metadata.modified()).ok()
}

fn compare_path_time(a: &Path, b: &Path) -> Ordering {
    match (path_timestamp(a), path_timestamp(b)) {
        (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn display_path_with_tilde(path: &Path, home: &Path) -> String {
    if path == home {
        return "~".to_string();
    }
    if let Ok(suffix) = path.strip_prefix(home) {
        let suffix = suffix.to_string_lossy();
        if suffix.is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", suffix)
        }
    } else {
        path.display().to_string()
    }
}

fn command_version(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    extract_version(&stdout)
}

fn extract_version(output: &str) -> Option<String> {
    for raw in output.split_whitespace() {
        let trimmed = raw.trim_matches(|ch: char| ch == ',' || ch == ';');
        let trimmed = trimmed.trim_start_matches('v');
        if !trimmed.chars().any(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let mut cleaned = String::new();
        for ch in trimmed.chars() {
            if ch.is_ascii_digit() || ch == '.' || ch == '-' {
                cleaned.push(ch);
            } else {
                break;
            }
        }
        if cleaned.chars().any(|ch| ch.is_ascii_digit()) {
            return Some(cleaned);
        }
    }
    None
}
