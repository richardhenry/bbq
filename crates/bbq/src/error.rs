use thiserror::Error;

#[derive(Debug, Error)]
pub enum BbqError {
    #[error("home directory not found")]
    HomeDirMissing,
    #[error("invalid git url")]
    InvalidGitUrl,
    #[error("invalid branch name")]
    InvalidBranchName,
    #[error("invalid worktree name")]
    InvalidWorktreeName,
    #[error("repo already exists: {0}")]
    RepoAlreadyExists(String),
    #[error("repo not found: {0}")]
    RepoNotFound(String),
    #[error("worktree already exists: {0}")]
    WorktreeAlreadyExists(String),
    #[error("worktree not found: {0}")]
    WorktreeNotFound(String),
    #[error("repo has worktrees; remove them first")]
    RepoHasWorktrees,
    #[error("invalid repo name")]
    InvalidRepoName,
    #[error("github cli (gh) not found; install it or use a git url")]
    GitHubCliMissing,
    #[error("github cli command failed: {command}\n{stderr}")]
    GitHubCliCommand { command: String, stderr: String },
    #[error("git command failed: {command}\n{stderr}")]
    GitCommand { command: String, stderr: String },
    #[error("script missing shebang: {0}")]
    ScriptMissingShebang(String),
    #[error("script failed: {script}\n{message}")]
    ScriptFailed { script: String, message: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, BbqError>;
