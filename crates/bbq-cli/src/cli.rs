use bbq::{
    checkout_repo, checkout_repo_with_name, create_worktree, create_worktree_from, default_branch,
    list_repos, list_worktrees, remove_repo, remove_worktree, resolve_repo, suggest_worktree_name,
    Repo, Worktree,
};
use clap::{Parser, Subcommand};
use std::collections::HashSet;

use crate::config::{
    default_branch_name, load_default_worktree_name_mode, load_editor_command,
    load_terminal_command,
};
use crate::open::{
    detect_open_targets, normalize_target, open_in_editor, open_in_target,
    open_terminal_at_path_with_config, OpenTarget,
};

#[derive(Parser)]
#[command(name = "bbq", version, about = "bbq worktree manager")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Commands>,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommand,
    },
}

#[derive(Subcommand)]
pub(crate) enum RepoCommand {
    Clone { url: String, name: Option<String> },
    List,
    Rm { name: String },
}

#[derive(Subcommand)]
pub(crate) enum WorktreeCommand {
    Create {
        repo: String,
        #[arg(long)]
        branch: Option<String>,
    },
    List { repo: String },
    Open {
        repo: String,
        name: String,
        #[arg(long)]
        target: Option<String>,
    },
    Rm { repo: String, name: String },
}

pub(crate) fn run_command(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Repo { command: repo_cmd } => match repo_cmd {
            RepoCommand::Clone { url, name } => {
                let repo = if let Some(name) = name {
                    checkout_repo_with_name(&url, &name)?
                } else {
                    checkout_repo(&url)?
                };
                println!("checked out {}", repo.name);
            }
            RepoCommand::List => {
                let repos = list_repos()?;
                if repos.is_empty() {
                    println!("no repos");
                } else {
                    for repo in repos {
                        println!("{}", repo.name);
                    }
                }
            }
            RepoCommand::Rm { name } => {
                remove_repo(&name)?;
                println!("removed {}", name);
            }
        },
        Commands::Worktree {
            command: worktree_cmd,
        } => match worktree_cmd {
            WorktreeCommand::Create { repo, branch } => {
                let repo = resolve_repo(&repo)?;
                if let Some(branch) = branch {
                    let branch = branch.trim();
                    if branch.is_empty() {
                        return Err("branch name required".into());
                    }
                    let worktree = create_worktree(&repo, branch)?;
                    println!("created {}", worktree.display_name());
                    return Ok(());
                }

                if let Some(mode) = load_default_worktree_name_mode() {
                    let default_source = default_branch(&repo)
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| "main".to_string());
                    let default_source = default_source.trim();
                    if default_source.is_empty() {
                        return Err("source branch required".into());
                    }

                    let existing_names: HashSet<String> = list_worktrees(&repo)?
                        .into_iter()
                        .map(|worktree| worktree.display_name())
                        .collect();
                    let name = suggest_worktree_name(
                        default_source,
                        default_source,
                        Some(mode),
                        &existing_names,
                    );
                    if name.trim().is_empty() {
                        return Err("worktree name required".into());
                    }
                    let branch_name = default_branch_name(&name);
                    let worktree =
                        create_worktree_from(&repo, &name, &branch_name, default_source)?;
                    println!("created {}", worktree.display_name());
                    return Ok(());
                }

                let branch = default_branch(&repo)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "main".to_string());
                let branch = branch.trim();
                if branch.is_empty() {
                    return Err("branch name required".into());
                }
                let worktree = create_worktree(&repo, branch)?;
                println!("created {}", worktree.display_name());
            }
            WorktreeCommand::List { repo } => {
                let repo = resolve_repo(&repo)?;
                let worktrees = list_worktrees(&repo)?;
                if worktrees.is_empty() {
                    println!("no worktrees");
                } else {
                    for worktree in worktrees {
                        println!("{}\t{}", worktree.display_name(), worktree.path.display());
                    }
                }
            }
            WorktreeCommand::Open { repo, name, target } => {
                let repo = resolve_repo(&repo)?;
                let worktree = find_worktree(&repo, &name)?;
                if let Some(target) = target.as_deref() {
                    let normalized = normalize_target(target);
                    if normalized == "terminal" {
                        open_terminal_at_path_with_config(
                            &worktree.path,
                            load_terminal_command().as_deref(),
                        )?;
                        println!("opened {} in terminal", worktree.display_name());
                        return Ok(());
                    }
                    let selected = OpenTarget::from_config(target)
                        .ok_or_else(|| format!("unknown target: {target}"))?;
                    let available = detect_open_targets();
                    if !available.contains(&selected) {
                        return Err(format!("{} launcher not available", selected.label()).into());
                    }
                    open_in_target(selected, &worktree.path)?;
                    println!("opened {} in {}", worktree.display_name(), selected.label());
                    return Ok(());
                }

                if let Some(command) = load_editor_command().as_deref() {
                    open_in_editor(command, &worktree.path)?;
                    println!("opened {} in editor", worktree.display_name());
                    return Ok(());
                }

                let available = detect_open_targets();
                let selected = available.first().copied().ok_or_else(|| {
                    "no open targets available; install zed, cursor, or vscode".to_string()
                })?;
                open_in_target(selected, &worktree.path)?;
                println!("opened {} in {}", worktree.display_name(), selected.label());
            }
            WorktreeCommand::Rm { repo, name } => {
                let repo = resolve_repo(&repo)?;
                remove_worktree(&repo, &name)?;
                println!("removed {}", name);
            }
        },
    }

    Ok(())
}

fn find_worktree(repo: &Repo, name: &str) -> Result<Worktree, bbq::BbqError> {
    let worktrees = list_worktrees(repo)?;
    worktrees
        .into_iter()
        .find(|item| {
            item.display_name() == name
                || item
                    .branch
                    .as_deref()
                    .map(|branch| branch == name)
                    .unwrap_or(false)
        })
        .ok_or_else(|| bbq::BbqError::WorktreeNotFound(name.to_string()))
}
