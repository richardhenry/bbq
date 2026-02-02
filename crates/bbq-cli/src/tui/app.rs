use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use bbq::{Repo, Worktree};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::config::{
    check_updates_enabled, default_branch_name, default_worktree_name_is_configured,
    editor_is_configured, force_upgrade_prompt_enabled, known_latest_version,
    load_default_worktree_name_mode, load_editor_command,
    load_restore_state, load_terminal_command, load_theme_index, preload_github_username,
    save_check_updates, save_default_worktree_name_mode, save_editor_command,
    save_known_latest_version, save_restore_state, save_terminal_command, save_theme_name,
    terminal_is_configured, RestoreState,
};
use crate::open::{
    detect_open_targets, open_in_editor, open_in_target, open_terminal_at_path_with_config,
};
use crate::theme::{Theme, THEMES};
use crate::tui::constants::{STATUS_MAX_MS, STATUS_MIN_MS, STATUS_PER_CHAR_MS};
use crate::tui::worker::start_background_tasks;
use crate::update;
use bbq::{suggest_worktree_name, DefaultWorktreeNameMode};
use semver::Version;

use super::types::{
    EnvInfo, Focus, InputKind, InputState, LoadingGroup, LoadingMessage, LoadingPriority,
    StatusMessage, StatusTone, TreeItem, TreeItemKind, TreeKey, WorkerEvent, WorkerRequest,
    WorktreeEntry,
};

const DEFAULT_SOURCE_BRANCH: &str = "origin/main";
const BBQ_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) struct App {
    pub(crate) repos: Vec<Repo>,
    pub(crate) tree_items: Vec<TreeItem>,
    pub(crate) tree_state: ListState,
    repo_worktrees: HashMap<String, Vec<WorktreeEntry>>,
    pub(crate) repo_display: HashMap<String, String>,
    expanded_repos: HashSet<String>,
    focus: Focus,
    pub(crate) input: Option<InputState>,
    pub(crate) status: Option<StatusMessage>,
    loading: Vec<LoadingMessage>,
    theme_index: usize,
    editor_command: Option<String>,
    terminal_command: Option<String>,
    default_worktree_name_mode: Option<DefaultWorktreeNameMode>,
    pub(crate) env_info: EnvInfo,
    worker_tx: mpsc::Sender<WorkerRequest>,
    worker_rx: mpsc::Receiver<WorkerEvent>,
    request_seq: u64,
    pending_all_request: Option<u64>,
    needs_reload: bool,
    desired_repo_selection: Option<String>,
    desired_worktree_selection: Option<(String, String)>,
    setup: Option<SetupState>,
    setup_steps: Vec<SetupStep>,
    update_prompt: Option<UpdatePromptState>,
}

impl App {
    pub(crate) fn new() -> Self {
        preload_github_username();
        let (worker_tx, worker_rx) = start_background_tasks();
        let mut app = Self {
            repos: Vec::new(),
            tree_items: Vec::new(),
            tree_state: ListState::default(),
            repo_worktrees: HashMap::new(),
            repo_display: HashMap::new(),
            expanded_repos: HashSet::new(),
            focus: Focus::List,
            input: None,
            status: None,
            loading: Vec::new(),
            theme_index: load_theme_index(),
            editor_command: load_editor_command(),
            terminal_command: load_terminal_command(),
            default_worktree_name_mode: load_default_worktree_name_mode(),
            env_info: EnvInfo::default(),
            worker_tx,
            worker_rx,
            request_seq: 0,
            pending_all_request: None,
            needs_reload: false,
            desired_repo_selection: None,
            desired_worktree_selection: None,
            setup: None,
            setup_steps: Vec::new(),
            update_prompt: None,
        };

        app.init_update_prompt();
        app.init_setup_state();
        app.apply_restore_state();
        app.request_env_info();
        app.request_update_check();
        app.request_all_data(false);
        app
    }

    fn init_setup_state(&mut self) {
        self.setup_steps.clear();
        if !default_worktree_name_is_configured() {
            self.setup_steps.push(SetupStep::DefaultWorktreeName);
        }
        if !editor_is_configured() {
            self.setup_steps.push(SetupStep::Editor);
        }
        if !terminal_is_configured() {
            self.setup_steps.push(SetupStep::Terminal);
        }
        self.start_setup_step();
    }

    fn init_update_prompt(&mut self) {
        if !check_updates_enabled() {
            return;
        }
        if !force_upgrade_prompt_enabled() && !update::is_homebrew_install() {
            return;
        }
        let Some(latest) = known_latest_version() else {
            return;
        };
        if is_newer_version(&latest, BBQ_VERSION) {
            self.update_prompt = Some(UpdatePromptState::new(
                BBQ_VERSION.to_string(),
                latest,
            ));
        }
    }

    fn apply_restore_state(&mut self) {
        let state = load_restore_state();
        self.expanded_repos = state.expanded_repos.into_iter().collect();
        self.desired_repo_selection = None;
        self.desired_worktree_selection = None;
        if let (Some(repo), Some(name)) = (state.selected_worktree_repo, state.selected_worktree_name)
        {
            self.desired_worktree_selection = Some((repo, name));
        } else if let Some(repo) = state.selected_repo {
            self.desired_repo_selection = Some(repo);
        }
    }

    fn start_setup_step(&mut self) {
        let Some(step) = self.setup_steps.first().cloned() else {
            self.setup = None;
            return;
        };
        self.setup = Some(SetupState::from_step(step));
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Left => self.collapse_selected(),
            KeyCode::Right | KeyCode::Tab => self.expand_selected(),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::Enter => {
                if self.selected_worktree_entry().is_some() {
                    self.open_selected_in_editor();
                } else {
                    self.toggle_selected_repo();
                }
            }
            KeyCode::Char(' ') if key.modifiers.is_empty() => self.toggle_selected_repo(),
            KeyCode::Char('c') if key.modifiers.is_empty() => self.open_checkout_prompt(),
            KeyCode::Char('n') if key.modifiers.is_empty() => self.open_worktree_prompt(),
            KeyCode::Char('d') if key.modifiers.is_empty() => self.open_delete_prompt(),
            KeyCode::Char('t') if key.modifiers.is_empty() => self.open_selected_in_terminal(),
            KeyCode::Char('h') if key.modifiers.is_empty() => self.cycle_theme(1),
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.cycle_theme(-1)
            }
            KeyCode::Char('H') => self.cycle_theme(-1),
            KeyCode::Esc => self.clear_status(),
            _ => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return true;
                }
            }
        }

        false
    }

    pub(crate) fn handle_setup_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return true;
        }

        match key.code {
            KeyCode::Up => {
                if let Some(setup) = self.setup.as_mut() {
                    setup.move_selection(-1);
                }
            }
            KeyCode::Down => {
                if let Some(setup) = self.setup.as_mut() {
                    setup.move_selection(1);
                }
            }
            KeyCode::Enter => self.apply_setup_selection(),
            _ => {}
        }

        false
    }

    pub(crate) fn handle_update_prompt_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return true;
        }

        let Some(prompt) = self.update_prompt.as_mut() else {
            return false;
        };

        if prompt.completed {
            return matches!(key.code, KeyCode::Enter);
        }

        if prompt.running {
            return false;
        }

        match key.code {
            KeyCode::Up => prompt.move_selection(-1),
            KeyCode::Down => prompt.move_selection(1),
            KeyCode::Enter => self.apply_update_selection(),
            _ => {}
        }

        false
    }

    pub(crate) fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if let Some(input) = self.input.take() {
                    self.focus = input.origin;
                }
            }
            KeyCode::Enter => {
                if let Some(input) = self.input.take() {
                    let origin = input.origin;
                    let next_focus = self.submit_input(input);
                    self.focus = next_focus.unwrap_or(origin);
                }
            }
            KeyCode::Backspace => {
                if let Some(input) = self.input.as_mut() {
                    input.buffer.pop();
                }
            }
            KeyCode::Char(ch) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    if let Some(input) = self.input.as_mut() {
                        input.buffer.push(ch);
                    }
                }
            }
            _ => {}
        }
    }

    fn toggle_selected_repo(&mut self) {
        let repo_name = match self.selected_tree_item() {
            Some(TreeItem {
                kind: TreeItemKind::Repo { name, .. },
                ..
            }) => name.clone(),
            _ => return,
        };
        if self.expanded_repos.contains(&repo_name) {
            self.expanded_repos.remove(&repo_name);
        } else {
            self.expanded_repos.insert(repo_name.clone());
        }
        self.rebuild_tree_items(Some(TreeKey::Repo(repo_name)));
    }

    fn collapse_selected(&mut self) {
        let repo_name = match self.selected_tree_item() {
            Some(TreeItem {
                kind: TreeItemKind::Repo { name, .. },
                ..
            }) => name.clone(),
            Some(TreeItem {
                kind: TreeItemKind::Worktree { repo, .. },
                ..
            }) => repo.clone(),
            None => return,
        };
        if self.expanded_repos.remove(&repo_name) {
            self.rebuild_tree_items(Some(TreeKey::Repo(repo_name)));
        }
    }

    fn expand_selected(&mut self) {
        let repo_name = match self.selected_tree_item() {
            Some(TreeItem {
                kind: TreeItemKind::Repo { name, .. },
                ..
            }) => name.clone(),
            _ => return,
        };
        if self.expanded_repos.insert(repo_name.clone()) {
            self.rebuild_tree_items(Some(TreeKey::Repo(repo_name)));
        }
    }

    fn open_checkout_prompt(&mut self) {
        self.input = Some(InputState {
            kind: InputKind::CheckoutRepo,
            buffer: String::new(),
            origin: self.focus,
        });
        self.focus = Focus::Input;
    }

    fn open_worktree_prompt(&mut self) {
        let Some(repo) = self.selected_repo().cloned() else {
            self.set_error("Select a repo first");
            return;
        };

        let default_source = default_source_branch(&repo);
        let existing_names = self.worktree_names_for_repo(&repo);
        let default_name = suggest_worktree_name(
            &default_source,
            &default_source,
            self.default_worktree_name_mode,
            &existing_names,
        );
        self.input = Some(InputState {
            kind: InputKind::CreateWorktreeName { repo },
            buffer: default_name,
            origin: self.focus,
        });
        self.focus = Focus::Input;
    }

    fn open_delete_prompt(&mut self) {
        let Some(item) = self.selected_tree_item() else {
            self.set_error("Select a repo or worktree to delete");
            return;
        };

        match item.kind {
            TreeItemKind::Repo { ref name, .. } => {
                self.input = Some(InputState {
                    kind: InputKind::DeleteRepo { name: name.clone() },
                    buffer: String::new(),
                    origin: self.focus,
                });
                self.focus = Focus::Input;
            }
            TreeItemKind::Worktree { .. } => self.open_delete_worktree_prompt(),
        }
    }

    fn open_delete_worktree_prompt(&mut self) {
        let Some(repo) = self.selected_repo().cloned() else {
            self.set_error("Select a repo first");
            return;
        };
        let Some(worktree) = self.selected_worktree().cloned() else {
            self.set_error("Select a worktree first");
            return;
        };
        self.input = Some(InputState {
            kind: InputKind::DeleteWorktree {
                repo,
                name: worktree.display_name(),
            },
            buffer: String::new(),
            origin: self.focus,
        });
        self.focus = Focus::Input;
    }

    fn open_selected_in_editor(&mut self) {
        let Some(worktree) = self.selected_worktree() else {
            self.set_error("Select a worktree first");
            return;
        };

        let label = self.worktree_label_for_repo(self.selected_repo(), worktree);
        let (result, target_label) = if let Some(command) = self.editor_command.as_deref() {
            (open_in_editor(command, &worktree.path), "editor".to_string())
        } else {
            let available = detect_open_targets();
            let selected = available.first().copied();
            let Some(selected) = selected else {
                let err = io::Error::new(
                    io::ErrorKind::NotFound,
                    "no editor configured; set editor in config.toml",
                );
                self.set_error(format!("Failed to open editor: {}", err));
                return;
            };
            (
                open_in_target(selected, &worktree.path),
                selected.label().to_string(),
            )
        };

        match result {
            Ok(()) => self.set_status(format!("Opened {} in {}", label, target_label)),
            Err(err) => self.set_error(format!("Failed to open {}: {}", target_label, err)),
        }
    }

    fn open_selected_in_terminal(&mut self) {
        let Some(worktree) = self.selected_worktree() else {
            self.set_error("Select a worktree first");
            return;
        };

        let label = self.worktree_label_for_repo(self.selected_repo(), worktree);
        match open_terminal_at_path_with_config(&worktree.path, self.terminal_command.as_deref()) {
            Ok(()) => self.set_status(format!("Opened {} in terminal", label)),
            Err(err) => self.set_error(format!("Failed to open terminal: {}", err)),
        }
    }

    fn worktree_names_for_repo(&self, repo: &Repo) -> HashSet<String> {
        self.repo_worktrees
            .get(&repo.name)
            .map(|entries| {
                entries
                    .iter()
                    .map(|entry| entry.worktree.display_name())
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default()
    }

    fn worktree_change_count(&self, repo: &Repo, name: &str) -> Option<usize> {
        self.repo_worktrees.get(&repo.name).and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.worktree.display_name() == name)
                .map(|entry| entry.changed_files.len())
        })
    }

    fn submit_input(&mut self, input: InputState) -> Option<Focus> {
        match input.kind {
            InputKind::CheckoutRepo => {
                let url = input.buffer.trim().to_string();
                if url.is_empty() {
                    self.set_error("Git url required");
                    return None;
                }
                self.set_loading(
                    LoadingGroup::Action,
                    "Cloning repo",
                    LoadingPriority::Action,
                );
                let _ = self.worker_tx.send(WorkerRequest::CheckoutRepo { url });
            }
            InputKind::CreateWorktreeName { repo } => {
                let name = input.buffer.trim();
                if let Err(message) = bbq::validate_worktree_name(name) {
                    self.set_error(message);
                    self.input = Some(InputState {
                        kind: InputKind::CreateWorktreeName { repo },
                        buffer: input.buffer,
                        origin: input.origin,
                    });
                    return Some(Focus::Input);
                }

                let default_source = default_source_branch(&repo);
                self.input = Some(InputState {
                    kind: InputKind::CreateWorktreeSource {
                        repo,
                        name: name.to_string(),
                    },
                    buffer: default_source,
                    origin: input.origin,
                });
                return Some(Focus::Input);
            }
            InputKind::CreateWorktreeSource { repo, name } => {
                let source_branch = input.buffer.trim();
                if let Err(message) = bbq::validate_branch_name(source_branch) {
                    self.set_error(message);
                    self.input = Some(InputState {
                        kind: InputKind::CreateWorktreeSource { repo, name },
                        buffer: input.buffer,
                        origin: input.origin,
                    });
                    return Some(Focus::Input);
                }

                let default_branch = default_branch_name(&name);
                let default_source = default_source_branch(&repo);
                let default_branch = if source_branch == default_source {
                    default_branch
                } else {
                    source_branch.to_string()
                };
                self.input = Some(InputState {
                    kind: InputKind::CreateWorktreeBranch {
                        repo,
                        name,
                        source_branch: source_branch.to_string(),
                    },
                    buffer: default_branch,
                    origin: input.origin,
                });
                return Some(Focus::Input);
            }
            InputKind::CreateWorktreeBranch {
                repo,
                name,
                source_branch,
            } => {
                let branch = input.buffer.trim();
                if let Err(message) = bbq::validate_branch_name(branch) {
                    self.set_error(message);
                    self.input = Some(InputState {
                        kind: InputKind::CreateWorktreeBranch {
                            repo,
                            name,
                            source_branch,
                        },
                        buffer: input.buffer,
                        origin: input.origin,
                    });
                    return Some(Focus::Input);
                }

                let label = self.format_worktree_label(&repo.name, &name);
                self.set_loading(
                    LoadingGroup::Action,
                    format!("Creating worktree {}", label),
                    LoadingPriority::Action,
                );
                let _ = self.worker_tx.send(WorkerRequest::CreateWorktree {
                    repo,
                    name,
                    branch: branch.to_string(),
                    source_branch,
                });
            }
            InputKind::DeleteRepo { name } => {
                if !delete_confirmed(&input.buffer) {
                    self.set_status("Delete canceled");
                    return None;
                }

                let label = self.display_repo_name(&name).to_string();
                self.set_loading(
                    LoadingGroup::Action,
                    format!("Deleting repo {}", label),
                    LoadingPriority::Action,
                );
                let _ = self.worker_tx.send(WorkerRequest::DeleteRepo { name });
            }
            InputKind::DeleteWorktree { repo, name } => {
                if !delete_confirmed(&input.buffer) {
                    self.set_status("Delete canceled");
                    return None;
                }

                let change_count = self.worktree_change_count(&repo, &name).unwrap_or(0);
                if change_count > 0 {
                    let label = self.format_worktree_label(&repo.name, &name);
                    let file_label = if change_count == 1 {
                        "1 changed file".to_string()
                    } else {
                        format!("{change_count} changed files")
                    };
                    self.set_error(format!(
                        "{} has {}. Type 'discard' to delete and lose those changes.",
                        label, file_label
                    ));
                    self.input = Some(InputState {
                        kind: InputKind::DeleteWorktreeForce { repo, name },
                        buffer: String::new(),
                        origin: input.origin,
                    });
                    return Some(Focus::Input);
                }

                let label = self.format_worktree_label(&repo.name, &name);
                self.set_loading(
                    LoadingGroup::Action,
                    format!("Deleting worktree {}", label),
                    LoadingPriority::Action,
                );
                let _ = self.worker_tx.send(WorkerRequest::DeleteWorktree {
                    repo,
                    name,
                    force: false,
                });
            }
            InputKind::DeleteWorktreeForce { repo, name } => {
                if !discard_confirmed(&input.buffer) {
                    self.set_status("Delete canceled");
                    return None;
                }

                let label = self.format_worktree_label(&repo.name, &name);
                self.set_loading(
                    LoadingGroup::Action,
                    format!("Deleting worktree {}", label),
                    LoadingPriority::Action,
                );
                let _ = self.worker_tx.send(WorkerRequest::DeleteWorktree {
                    repo,
                    name,
                    force: true,
                });
            }
        }
        None
    }

    fn next_request_id(&mut self) -> u64 {
        self.request_seq = self.request_seq.wrapping_add(1);
        self.request_seq
    }

    fn request_env_info(&mut self) {
        self.set_loading(
            LoadingGroup::EnvInfo,
            "Loading environment",
            LoadingPriority::Background,
        );
        let _ = self.worker_tx.send(WorkerRequest::LoadEnvInfo);
    }

    fn request_update_check(&mut self) {
        if !check_updates_enabled() {
            return;
        }
        if !force_upgrade_prompt_enabled() && !update::is_homebrew_install() {
            return;
        }
        let _ = self.worker_tx.send(WorkerRequest::CheckForUpdate);
    }

    fn request_all_data(&mut self, silent: bool) {
        let request_id = self.next_request_id();
        self.pending_all_request = Some(request_id);
        self.needs_reload = false;
        if !silent {
            self.set_loading(
                LoadingGroup::Repos,
                "Loading repos",
                LoadingPriority::Background,
            );
            self.set_loading(
                LoadingGroup::Worktrees,
                "Loading worktrees",
                LoadingPriority::Background,
            );
        }
        let _ = self.worker_tx.send(WorkerRequest::LoadAll { request_id });
    }

    pub(crate) fn handle_worker_events(&mut self) {
        while let Ok(event) = self.worker_rx.try_recv() {
            match event {
                WorkerEvent::AllDataLoaded { request_id, result } => {
                    if self.pending_all_request != Some(request_id) {
                        continue;
                    }
                    self.pending_all_request = None;
                    self.clear_loading(LoadingGroup::Repos);
                    self.clear_loading(LoadingGroup::Worktrees);
                    match result {
                        Ok(data) => {
                            let prev_key = self.selected_tree_key();
                            self.repos = data.repos;
                            self.repo_worktrees = data.repo_worktrees;
                            self.repo_display = data.repo_display;
                            self.expanded_repos
                                .retain(|name| self.repos.iter().any(|repo| repo.name == *name));
                            let mut preferred = None;
                            if let Some((repo_name, worktree_name)) =
                                self.desired_worktree_selection.take()
                            {
                                self.expanded_repos.insert(repo_name.clone());
                                preferred = Some(TreeKey::Worktree {
                                    repo: repo_name,
                                    name: worktree_name,
                                });
                            } else if let Some(repo_name) = self.desired_repo_selection.take() {
                                preferred = Some(TreeKey::Repo(repo_name));
                            } else if let Some(prev_key) = prev_key {
                                preferred = Some(prev_key);
                            }
                            self.rebuild_tree_items(preferred);
                            if let Some(err) = data.error {
                                self.set_error(err);
                            }
                        }
                        Err(err) => {
                            self.repos = Vec::new();
                            self.repo_worktrees.clear();
                            self.repo_display.clear();
                            self.tree_items.clear();
                            self.tree_state.select(None);
                            self.expanded_repos.clear();
                            self.set_error(err);
                        }
                    }
                    if self.needs_reload {
                        self.needs_reload = false;
                        self.request_all_data(true);
                    }
                }
                WorkerEvent::UpdateCheckResult { latest } => {
                    if let Some(latest) = latest {
                        if is_newer_version(&latest, BBQ_VERSION) {
                            let existing = known_latest_version();
                            if existing.as_deref() != Some(latest.as_str()) {
                                let _ = save_known_latest_version(&latest);
                            }
                        }
                    }
                }
                WorkerEvent::UpgradeResult { result } => {
                    if let Some(prompt) = self.update_prompt.as_mut() {
                        prompt.running = false;
                        if result.is_ok() {
                            prompt.completed = true;
                        }
                    }
                    match result {
                        Ok(()) => {
                            self.set_status("Upgrade complete. Press Enter to quit and relaunch.");
                        }
                        Err(err) => {
                            self.set_error(format!("Upgrade failed: {err}"));
                        }
                    }
                }
                WorkerEvent::FsChanged => {
                    if self.pending_all_request.is_some() {
                        self.needs_reload = true;
                    } else {
                        self.request_all_data(true);
                    }
                }
                WorkerEvent::EnvInfoLoaded {
                    home_dir,
                    git_version,
                    gh_version,
                } => {
                    self.clear_loading(LoadingGroup::EnvInfo);
                    self.env_info = EnvInfo {
                        home_dir,
                        git_version,
                        gh_version,
                    };
                }
                WorkerEvent::CheckoutRepoResult { result } => match result {
                    Ok(repo) => {
                        self.clear_loading(LoadingGroup::Action);
                        let label = self.display_repo_name(&repo.name).to_string();
                        self.set_status(format!("Checked out {}", label));
                        self.desired_repo_selection = Some(repo.name);
                        self.request_all_data(false);
                    }
                    Err(err) => {
                        self.clear_loading(LoadingGroup::Action);
                        self.set_error(err);
                    }
                },
                WorkerEvent::WorktreeScriptStarted { kind, path } => {
                    self.set_loading(
                        LoadingGroup::Action,
                        format!("Running {kind} script {path}"),
                        LoadingPriority::Action,
                    );
                }
                WorkerEvent::CreateWorktreeResult { repo_name, result } => match result {
                    Ok(worktree) => {
                        let worktree_name = worktree.display_name();
                        let selection_key = worktree
                            .branch
                            .clone()
                            .unwrap_or_else(|| worktree_name.clone());
                        self.clear_loading(LoadingGroup::Action);
                        let label = self.format_worktree_label(&repo_name, &worktree_name);
                        self.set_status(format!("Created worktree {}", label));
                        self.desired_worktree_selection = Some((repo_name, selection_key));
                        self.request_all_data(false);
                    }
                    Err(err) => {
                        self.clear_loading(LoadingGroup::Action);
                        self.set_error(err);
                    }
                },
                WorkerEvent::DeleteRepoResult { name, result } => match result {
                    Ok(()) => {
                        self.clear_loading(LoadingGroup::Action);
                        let label = self.display_repo_name(&name).to_string();
                        self.set_status(format!("Deleted repo {}", label));
                        self.request_all_data(false);
                    }
                    Err(err) => {
                        self.clear_loading(LoadingGroup::Action);
                        self.set_error(err);
                    }
                },
                WorkerEvent::DeleteWorktreeResult {
                    repo_name,
                    worktree_name,
                    result,
                } => match result {
                    Ok(()) => {
                        self.clear_loading(LoadingGroup::Action);
                        let label = self.format_worktree_label(&repo_name, &worktree_name);
                        self.set_status(format!("Deleted worktree {}", label));
                        self.request_all_data(false);
                    }
                    Err(err) => {
                        self.clear_loading(LoadingGroup::Action);
                        self.set_error(err);
                    }
                },
            }
        }
    }

    fn clamp_selection(state: &mut ListState, len: usize) {
        if len == 0 {
            state.select(None);
            return;
        }

        let selected = state.selected().unwrap_or(0).min(len - 1);
        state.select(Some(selected));
    }

    fn move_selection(&mut self, delta: i32) {
        match self.focus {
            Focus::List => {
                move_state(&mut self.tree_state, self.tree_items.len(), delta);
            }
            Focus::Input => {}
        }
    }

    fn rebuild_tree_items(&mut self, preferred: Option<TreeKey>) {
        self.tree_items = build_tree_items(
            &self.repos,
            &self.repo_worktrees,
            &self.repo_display,
            &self.expanded_repos,
        );
        Self::clamp_selection(&mut self.tree_state, self.tree_items.len());
        if let Some(key) = preferred {
            self.select_tree_key(&key);
        }
    }

    fn selected_tree_key(&self) -> Option<TreeKey> {
        self.selected_tree_item().map(tree_item_key)
    }

    fn select_tree_key(&mut self, key: &TreeKey) -> bool {
        if let Some((idx, _)) = self
            .tree_items
            .iter()
            .enumerate()
            .find(|(_, item)| tree_item_matches_key(item, key))
        {
            self.tree_state.select(Some(idx));
            return true;
        }
        false
    }

    pub(crate) fn selected_tree_item(&self) -> Option<&TreeItem> {
        self.tree_state
            .selected()
            .and_then(|idx| self.tree_items.get(idx))
    }

    fn selected_repo_name(&self) -> Option<&str> {
        match self.selected_tree_item()?.kind {
            TreeItemKind::Repo { ref name, .. } => Some(name.as_str()),
            TreeItemKind::Worktree { ref repo, .. } => Some(repo.as_str()),
        }
    }

    pub(crate) fn selected_repo(&self) -> Option<&Repo> {
        let name = self.selected_repo_name()?;
        self.repos.iter().find(|repo| repo.name == name)
    }

    pub(crate) fn display_repo_name<'a>(&'a self, repo_name: &'a str) -> &'a str {
        self.repo_display
            .get(repo_name)
            .map(|name| name.as_str())
            .unwrap_or(repo_name)
    }

    fn format_worktree_label(&self, repo_name: &str, worktree_name: &str) -> String {
        format!("{}/{}", self.display_repo_name(repo_name), worktree_name)
    }

    fn worktree_label_for_repo(&self, repo: Option<&Repo>, worktree: &Worktree) -> String {
        let name = worktree.display_name();
        match repo {
            Some(repo) => self.format_worktree_label(&repo.name, &name),
            None => name,
        }
    }

    fn selected_worktree(&self) -> Option<&Worktree> {
        self.selected_worktree_entry().map(|entry| &entry.worktree)
    }

    pub(crate) fn selected_worktree_entry(&self) -> Option<&WorktreeEntry> {
        match self.selected_tree_item()?.kind {
            TreeItemKind::Worktree { ref entry, .. } => Some(entry),
            _ => None,
        }
    }

    pub(crate) fn is_setup_mode(&self) -> bool {
        self.setup.is_some()
    }

    pub(crate) fn is_update_prompt_mode(&self) -> bool {
        self.update_prompt.is_some()
    }

    pub(crate) fn setup_state(&self) -> Option<&SetupState> {
        self.setup.as_ref()
    }

    pub(crate) fn update_prompt_state(&self) -> Option<&UpdatePromptState> {
        self.update_prompt.as_ref()
    }

    pub(crate) fn effective_focus(&self) -> Focus {
        if let (Focus::Input, Some(input)) = (self.focus, self.input.as_ref()) {
            input.origin
        } else {
            self.focus
        }
    }

    pub(crate) fn is_input_mode(&self) -> bool {
        matches!(self.focus, Focus::Input)
    }

    fn current_theme(&self) -> Theme {
        *THEMES.get(self.theme_index).unwrap_or(&THEMES[0])
    }

    pub(crate) fn theme_color(&self) -> ratatui::style::Color {
        self.current_theme().color()
    }

    pub(crate) fn theme_name(&self) -> &'static str {
        self.current_theme().name
    }

    fn cycle_theme(&mut self, delta: i32) {
        let len = THEMES.len() as i32;
        let next = (self.theme_index as i32 + delta).rem_euclid(len);
        self.theme_index = next as usize;
        if let Some(theme) = THEMES.get(self.theme_index) {
            if let Err(err) = save_theme_name(theme.name) {
                self.set_error(format!("Failed to save theme: {}", err));
            }
        }
    }

    pub(crate) fn set_status(&mut self, message: impl Into<String>) {
        self.set_status_tone(message, StatusTone::Success);
    }

    fn set_error(&mut self, message: impl Into<String>) {
        self.set_status_tone(message, StatusTone::Error);
    }

    fn set_status_tone(&mut self, message: impl Into<String>, tone: StatusTone) {
        let message = message.into();
        if message.is_empty() {
            self.clear_status();
            return;
        }
        let deadline = Instant::now() + status_duration(&message);
        self.status = Some(StatusMessage {
            text: message,
            tone,
            deadline,
        });
    }

    fn set_loading(
        &mut self,
        group: LoadingGroup,
        message: impl Into<String>,
        priority: LoadingPriority,
    ) {
        let message = message.into();
        if message.is_empty() {
            self.clear_loading(group);
            return;
        }
        if priority == LoadingPriority::Action {
            self.clear_status();
        }
        self.loading.retain(|item| item.group != group);
        self.loading.push(LoadingMessage {
            group,
            text: message,
            started_at: Instant::now(),
            priority,
        });
    }

    fn clear_status(&mut self) {
        self.status = None;
    }

    fn clear_loading(&mut self, group: LoadingGroup) {
        self.loading.retain(|item| item.group != group);
    }

    pub(crate) fn current_loading(&self) -> Option<&LoadingMessage> {
        let mut best: Option<&LoadingMessage> = None;
        for item in &self.loading {
            best = match best {
                None => Some(item),
                Some(existing) => {
                    let item_rank = item.priority.rank();
                    let existing_rank = existing.priority.rank();
                    if item_rank > existing_rank
                        || (item_rank == existing_rank && item.started_at < existing.started_at)
                    {
                        Some(item)
                    } else {
                        Some(existing)
                    }
                }
            };
        }
        best
    }

    pub(crate) fn loading_message(&self, group: LoadingGroup) -> Option<&LoadingMessage> {
        self.loading.iter().find(|item| item.group == group)
    }

    pub(crate) fn update_status(&mut self) {
        let Some(deadline) = self.status.as_ref().map(|status| status.deadline) else {
            return;
        };
        if Instant::now() >= deadline {
            self.status = None;
        }
    }

    pub(crate) fn persist_restore_state(&self) {
        let mut expanded: Vec<String> = self.expanded_repos.iter().cloned().collect();
        expanded.sort();
        let mut state = RestoreState {
            expanded_repos: expanded,
            selected_repo: None,
            selected_worktree_repo: None,
            selected_worktree_name: None,
        };

        if let Some(key) = self.selected_tree_key() {
            match key {
                TreeKey::Repo(name) => state.selected_repo = Some(name),
                TreeKey::Worktree { repo, name } => {
                    state.selected_worktree_repo = Some(repo);
                    state.selected_worktree_name = Some(name);
                }
            }
        }

        let _ = save_restore_state(&state);
    }

    fn apply_setup_selection(&mut self) {
        let (step, choice) = match self.setup.as_ref() {
            Some(setup) => {
                let Some(choice) = setup.options.get(setup.selected).cloned() else {
                    return;
                };
                (setup.step, choice)
            }
            None => return,
        };

        match step {
            SetupStep::DefaultWorktreeName => {
                let value = choice.value.unwrap_or_default();
                let mode = if value.trim().eq_ignore_ascii_case("cities") {
                    Some(DefaultWorktreeNameMode::Cities)
                } else {
                    None
                };
                if let Err(err) = save_default_worktree_name_mode(mode) {
                    self.set_error(format!(
                        "Failed to save default worktree names preference: {err}"
                    ));
                    return;
                }
                self.default_worktree_name_mode = mode;
            }
            SetupStep::Editor => {
                if let Some(value) = choice.value {
                    if let Err(err) = save_editor_command(&value) {
                        self.set_error(format!("Failed to save editor: {err}"));
                        return;
                    }
                    self.editor_command = Some(value);
                }
            }
            SetupStep::Terminal => {
                if let Some(value) = choice.value {
                    if let Err(err) = save_terminal_command(&value) {
                        self.set_error(format!("Failed to save terminal: {err}"));
                        return;
                    }
                    self.terminal_command = Some(value);
                }
            }
        }

        if !self.setup_steps.is_empty() {
            self.setup_steps.remove(0);
        }
        self.start_setup_step();
    }

    fn apply_update_selection(&mut self) {
        let Some(prompt) = self.update_prompt.as_mut() else {
            return;
        };

        match prompt.selected {
            0 => {
                prompt.running = true;
                prompt.completed = false;
                let _ = self.worker_tx.send(WorkerRequest::RunUpgrade);
            }
            1 => {
                self.update_prompt = None;
            }
            2 => {
                if let Err(err) = save_check_updates(false) {
                    self.set_error(format!("Failed to update config: {err}"));
                    return;
                }
                self.update_prompt = None;
            }
            _ => {}
        }
    }
}

fn move_state(state: &mut ListState, len: usize, delta: i32) {
    if len == 0 {
        state.select(None);
        return;
    }

    let current = state.selected().unwrap_or(0) as i32;
    let next = if delta < 0 {
        if current == 0 { len as i32 - 1 } else { current - 1 }
    } else {
        if current as usize >= len - 1 { 0 } else { current + 1 }
    };

    state.select(Some(next as usize));
}

fn status_duration(message: &str) -> Duration {
    let chars = message.chars().count() as u64;
    let millis = STATUS_MIN_MS.saturating_add(STATUS_PER_CHAR_MS.saturating_mul(chars));
    Duration::from_millis(millis.min(STATUS_MAX_MS))
}

fn delete_confirmed(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }

    let normalized = trimmed.to_ascii_lowercase();
    "yes".starts_with(normalized.as_str())
}

fn discard_confirmed(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }

    let normalized = trimmed.to_ascii_lowercase();
    "discard".starts_with(normalized.as_str())
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    match (Version::parse(latest), Version::parse(current)) {
        (Ok(latest), Ok(current)) => latest > current,
        _ => latest != current,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SetupStep {
    DefaultWorktreeName,
    Editor,
    Terminal,
}

#[derive(Debug, Clone)]
pub(crate) struct SetupOption {
    pub(crate) label: String,
    pub(crate) value: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SetupState {
    pub(crate) step: SetupStep,
    pub(crate) options: Vec<SetupOption>,
    pub(crate) selected: usize,
}

const UPDATE_PROMPT_OPTIONS: [&str; 3] = [
    "run: brew upgrade bbq",
    "not right now",
    "never ask again",
];

#[derive(Debug, Clone)]
pub(crate) struct UpdatePromptState {
    pub(crate) current_version: String,
    pub(crate) latest_version: String,
    pub(crate) selected: usize,
    pub(crate) running: bool,
    pub(crate) completed: bool,
}

impl SetupState {
    fn from_step(step: SetupStep) -> Self {
        let mut options = match step {
            SetupStep::DefaultWorktreeName => default_worktree_name_options(),
            SetupStep::Editor => editor_options(),
            SetupStep::Terminal => terminal_options(),
        };
        if options.is_empty() {
            options.push(SetupOption {
                label: "Configure later".to_string(),
                value: None,
            });
        }
        let selected = 0;
        Self {
            step,
            options,
            selected,
        }
    }

    pub(crate) fn question(&self) -> &'static str {
        match self.step {
            SetupStep::DefaultWorktreeName => {
                "Would you like to use default worktree names?"
            }
            SetupStep::Editor => "Which editor do you want to use?",
            SetupStep::Terminal => "Which terminal do you want to use?",
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let len = self.options.len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        let current = self.selected as i32;
        let next = if delta < 0 {
            if current == 0 { len as i32 - 1 } else { current - 1 }
        } else {
            if current as usize >= len - 1 { 0 } else { current + 1 }
        };
        self.selected = next as usize;
    }
}

impl UpdatePromptState {
    fn new(current_version: String, latest_version: String) -> Self {
        Self {
            current_version,
            latest_version,
            selected: 0,
            running: false,
            completed: false,
        }
    }

    pub(crate) fn options(&self) -> &'static [&'static str] {
        &UPDATE_PROMPT_OPTIONS
    }

    fn move_selection(&mut self, delta: i32) {
        let len = self.options().len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        let current = self.selected as i32;
        let next = if delta < 0 {
            if current == 0 { len as i32 - 1 } else { current - 1 }
        } else {
            if current as usize >= len - 1 { 0 } else { current + 1 }
        };
        self.selected = next as usize;
    }
}

fn editor_options() -> Vec<SetupOption> {
    let targets = detect_open_targets();
    let mut options: Vec<SetupOption> = targets
        .into_iter()
        .map(|target| SetupOption {
            label: target.label().to_string(),
            value: Some(target.command().to_string()),
        })
        .collect();
    if options.is_empty() {
        options.push(SetupOption {
            label: "Configure later".to_string(),
            value: None,
        });
    }
    options
}

fn default_worktree_name_options() -> Vec<SetupOption> {
    vec![
        SetupOption {
            label: "cities".to_string(),
            value: Some("cities".to_string()),
        },
        SetupOption {
            label: "no defaults".to_string(),
            value: Some(String::new()),
        },
    ]
}

fn terminal_options() -> Vec<SetupOption> {
    let mut options = Vec::new();
    for candidate in terminal_candidates() {
        if candidate.paths.iter().any(|path| Path::new(path).exists()) {
            options.push(SetupOption {
                label: candidate.label.to_string(),
                value: Some(candidate.value.to_string()),
            });
        }
    }
    if options.is_empty() {
        options.push(SetupOption {
            label: "Terminal.app".to_string(),
            value: Some("Terminal".to_string()),
        });
    }
    options
}

struct TerminalCandidate {
    label: &'static str,
    value: &'static str,
    paths: &'static [&'static str],
}

fn terminal_candidates() -> Vec<TerminalCandidate> {
    vec![
        TerminalCandidate {
            label: "Terminal.app",
            value: "Terminal",
            paths: &[
                "/System/Applications/Utilities/Terminal.app",
                "/Applications/Utilities/Terminal.app",
            ],
        },
        TerminalCandidate {
            label: "iTerm",
            value: "iTerm",
            paths: &["/Applications/iTerm.app", "/Applications/iTerm2.app"],
        },
        TerminalCandidate {
            label: "Warp",
            value: "Warp",
            paths: &["/Applications/Warp.app"],
        },
        TerminalCandidate {
            label: "Ghostty",
            value: "Ghostty",
            paths: &["/Applications/Ghostty.app"],
        },
        TerminalCandidate {
            label: "WezTerm",
            value: "WezTerm",
            paths: &["/Applications/WezTerm.app"],
        },
        TerminalCandidate {
            label: "Alacritty",
            value: "Alacritty",
            paths: &["/Applications/Alacritty.app"],
        },
        TerminalCandidate {
            label: "Hyper",
            value: "Hyper",
            paths: &["/Applications/Hyper.app"],
        },
        TerminalCandidate {
            label: "Kitty",
            value: "Kitty",
            paths: &["/Applications/kitty.app", "/Applications/Kitty.app"],
        },
    ]
}

fn default_source_branch(repo: &Repo) -> String {
    bbq::default_branch(repo)
        .ok()
        .flatten()
        .unwrap_or_else(|| DEFAULT_SOURCE_BRANCH.to_string())
}

fn build_tree_items(
    repos: &[Repo],
    repo_worktrees: &HashMap<String, Vec<WorktreeEntry>>,
    repo_display: &HashMap<String, String>,
    expanded_repos: &HashSet<String>,
) -> Vec<TreeItem> {
    let mut items = Vec::new();
    for repo in repos {
        let display_name = repo_display
            .get(&repo.name)
            .cloned()
            .unwrap_or_else(|| repo.name.clone());
        let expanded = expanded_repos.contains(&repo.name);
        let worktree_count = repo_worktrees
            .get(&repo.name)
            .map(|entries| entries.len())
            .unwrap_or(0);
        items.push(TreeItem {
            left: display_name,
            right: String::new(),
            kind: TreeItemKind::Repo {
                name: repo.name.clone(),
                expanded,
                worktree_count,
            },
        });

        if expanded {
            if let Some(entries) = repo_worktrees.get(&repo.name) {
                for entry in entries {
                    let branch = entry.worktree.branch.as_deref().unwrap_or("detached");
                    let display = entry.worktree.display_name();
                    items.push(TreeItem {
                        left: format!("  {}", branch),
                        right: display.to_string(),
                        kind: TreeItemKind::Worktree {
                            repo: repo.name.clone(),
                            entry: entry.clone(),
                        },
                    });
                }
            }
        }
    }
    items
}

fn tree_item_key(item: &TreeItem) -> TreeKey {
    match &item.kind {
        TreeItemKind::Repo { name, .. } => TreeKey::Repo(name.clone()),
        TreeItemKind::Worktree { repo, entry } => TreeKey::Worktree {
            repo: repo.clone(),
            name: entry.worktree.display_name(),
        },
    }
}

fn tree_item_matches_key(item: &TreeItem, key: &TreeKey) -> bool {
    match (&item.kind, key) {
        (TreeItemKind::Repo { name, .. }, TreeKey::Repo(key)) => name == key,
        (TreeItemKind::Worktree { repo, entry }, TreeKey::Worktree { repo: key_repo, name }) => {
            if repo != key_repo {
                return false;
            }
            if entry.worktree.display_name() == *name {
                return true;
            }
            entry
                .worktree
                .branch
                .as_deref()
                .map(|branch| branch == name)
                .unwrap_or(false)
        }
        _ => false,
    }
}
