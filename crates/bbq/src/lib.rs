pub mod error;
pub mod git;
pub mod model;
pub mod paths;
pub mod scripts;
pub mod validate;
pub mod worktree_names;

pub use error::{BbqError, Result};
pub use git::{
    checkout_repo, checkout_repo_with_name, create_worktree, create_worktree_from,
    create_worktree_with_name, default_branch, default_remote_branch, list_repos, list_worktrees,
    remove_repo, remove_worktree, remove_worktree_with_force, resolve_repo,
};
pub use model::{Repo, Worktree};
pub use scripts::{
    find_post_create_script, post_create_script_path, run_post_create_script, ScriptOutput,
    POST_CREATE_SCRIPT_RELATIVE,
};
pub use validate::{validate_branch_name, validate_worktree_name};
pub use worktree_names::{city_worktree_name, suggest_worktree_name, DefaultWorktreeNameMode};
