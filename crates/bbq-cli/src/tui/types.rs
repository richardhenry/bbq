use std::collections::HashMap;
use std::time::Instant;

use bbq::{Repo, Worktree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusTone {
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub(crate) struct StatusMessage {
    pub(crate) text: String,
    pub(crate) tone: StatusTone,
    pub(crate) deadline: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoadingPriority {
    Background,
    Action,
}

impl LoadingPriority {
    pub(crate) fn rank(self) -> u8 {
        match self {
            LoadingPriority::Background => 0,
            LoadingPriority::Action => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoadingGroup {
    EnvInfo,
    Repos,
    Worktrees,
    Action,
}

#[derive(Debug, Clone)]
pub(crate) struct LoadingMessage {
    pub(crate) group: LoadingGroup,
    pub(crate) text: String,
    pub(crate) started_at: Instant,
    pub(crate) priority: LoadingPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Focus {
    List,
    Input,
}

impl Focus {}

#[derive(Debug, Clone)]
pub(crate) enum InputKind {
    CheckoutRepo,
    CreateWorktreeName { repo: Repo },
    CreateWorktreeSource { repo: Repo, name: String },
    CreateWorktreeBranch {
        repo: Repo,
        name: String,
        source_branch: String,
    },
    DeleteRepo { name: String },
    DeleteWorktree { repo: Repo, name: String },
    DeleteWorktreeForce { repo: Repo, name: String },
}

#[derive(Debug, Clone)]
pub(crate) struct WorktreeEntry {
    pub(crate) worktree: Worktree,
    pub(crate) head_author: Option<String>,
    pub(crate) head_message: Option<String>,
    pub(crate) upstream: Option<String>,
    pub(crate) sync_status: String,
    pub(crate) worktree_path: String,
    pub(crate) changed_files: Vec<ChangedFile>,
}

#[derive(Debug, Clone)]
pub(crate) struct ChangedFile {
    pub(crate) path: String,
    pub(crate) added: u32,
    pub(crate) removed: u32,
}

#[derive(Debug, Clone)]
pub(crate) enum TreeItemKind {
    Repo {
        name: String,
        expanded: bool,
        worktree_count: usize,
    },
    Worktree { repo: String, entry: WorktreeEntry },
}

#[derive(Debug, Clone)]
pub(crate) struct TreeItem {
    pub(crate) left: String,
    pub(crate) right: String,
    pub(crate) kind: TreeItemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TreeKey {
    Repo(String),
    Worktree { repo: String, name: String },
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EnvInfo {
    pub(crate) home_dir: Option<String>,
    pub(crate) git_version: Option<String>,
    pub(crate) gh_version: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AllData {
    pub(crate) repos: Vec<Repo>,
    pub(crate) repo_worktrees: HashMap<String, Vec<WorktreeEntry>>,
    pub(crate) repo_display: HashMap<String, String>,
    pub(crate) error: Option<String>,
}

#[derive(Debug)]
pub(crate) enum WorkerRequest {
    LoadEnvInfo,
    LoadAll { request_id: u64 },
    CheckForUpdate,
    RunUpgrade,
    CheckoutRepo { url: String },
    CreateWorktree {
        repo: Repo,
        name: String,
        branch: String,
        source_branch: String,
    },
    DeleteRepo { name: String },
    DeleteWorktree { repo: Repo, name: String, force: bool },
}

#[derive(Debug)]
pub(crate) enum WorkerEvent {
    AllDataLoaded {
        request_id: u64,
        result: Result<AllData, String>,
    },
    UpdateCheckResult {
        latest: Option<String>,
    },
    UpgradeResult {
        result: Result<(), String>,
    },
    FsChanged,
    EnvInfoLoaded {
        home_dir: Option<String>,
        git_version: Option<String>,
        gh_version: Option<String>,
    },
    CheckoutRepoResult {
        result: Result<Repo, String>,
    },
    WorktreeScriptStarted {
        script: String,
    },
    CreateWorktreeResult {
        repo_name: String,
        result: Result<Worktree, String>,
    },
    DeleteRepoResult {
        name: String,
        result: Result<(), String>,
    },
    DeleteWorktreeResult {
        repo_name: String,
        worktree_name: String,
        result: Result<(), String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct InputState {
    pub(crate) kind: InputKind,
    pub(crate) buffer: String,
    pub(crate) origin: Focus,
}

impl InputState {
    pub(crate) fn label(&self) -> String {
        match &self.kind {
            InputKind::CheckoutRepo => "clone from > ".to_string(),
            InputKind::CreateWorktreeSource { .. } => "source branch > ".to_string(),
            InputKind::CreateWorktreeName { .. } => "worktree name > ".to_string(),
            InputKind::CreateWorktreeBranch { .. } => "new branch > ".to_string(),
            InputKind::DeleteRepo { name } => format!("delete {} repo? > ", name),
            InputKind::DeleteWorktree { name, .. } => format!("delete {} worktree? > ", name),
            InputKind::DeleteWorktreeForce { name, .. } => {
                format!("delete {} worktree and discard changes? > ", name)
            }
        }
    }

    pub(crate) fn placeholder(&self) -> &'static str {
        match &self.kind {
            InputKind::CheckoutRepo => "git url or github user/repo",
            InputKind::CreateWorktreeSource { .. } => "source branch",
            InputKind::CreateWorktreeName { .. } => "worktree name",
            InputKind::CreateWorktreeBranch { .. } => "branch name",
            InputKind::DeleteRepo { .. } | InputKind::DeleteWorktree { .. } => {
                "type 'yes' to confirm"
            }
            InputKind::DeleteWorktreeForce { .. } => "type 'discard' to confirm",
        }
    }
}
