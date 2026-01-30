use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use bbq::paths;
use bbq::DefaultWorktreeNameMode;

use crate::open::OpenTarget;
use crate::theme::{default_theme_index, theme_index_by_name};

#[derive(Debug, Default, Clone)]
pub(crate) struct Config {
    pub(crate) theme: Option<String>,
    pub(crate) default_open: Option<String>,
    pub(crate) editor: Option<String>,
    pub(crate) terminal: Option<String>,
    pub(crate) github_prefix: Option<bool>,
    pub(crate) default_worktree_name: Option<DefaultWorktreeNameMode>,
    pub(crate) default_worktree_name_set: bool,
    pub(crate) known_latest_version: Option<String>,
    pub(crate) check_updates: Option<bool>,
    pub(crate) force_upgrade_prompt: Option<bool>,
}

pub(crate) fn load_config() -> Config {
    let Ok(path) = config_path() else {
        return Config::default();
    };

    let Ok(contents) = fs::read_to_string(path) else {
        return Config::default();
    };

    parse_config(&contents)
}

fn parse_config(contents: &str) -> Config {
    let mut config = Config::default();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }

        let mut parts = line.splitn(2, '=');
        let key = parts.next().unwrap_or_default().trim();
        let Some(value) = parts.next() else {
            continue;
        };
        let value = value.trim();

        match key {
            "theme" => config.theme = Some(trim_quotes(value)),
            "default_open" => config.default_open = Some(trim_quotes(value)),
            "default_worktree_name" => {
                let trimmed = trim_quotes(value);
                config.default_worktree_name = DefaultWorktreeNameMode::from_config(&trimmed);
                config.default_worktree_name_set = true;
            }
            "github_prefix" | "github_user_prefix" => {
                if let Some(enabled) = parse_bool(value) {
                    config.github_prefix = Some(enabled);
                }
            }
            "editor" => {
                let editor = trim_quotes(value);
                if !editor.is_empty() {
                    if config.editor.is_none() {
                        config.editor = Some(editor.clone());
                    }
                    if config.default_open.is_none() {
                        config.default_open = Some(editor);
                    }
                }
            }
            "terminal" => {
                let terminal = trim_quotes(value);
                if !terminal.is_empty() {
                    config.terminal = Some(terminal);
                }
            }
            "known_latest_version" => {
                let latest = trim_quotes(value);
                if !latest.is_empty() {
                    config.known_latest_version = Some(latest);
                }
            }
            "check_updates" => {
                if let Some(enabled) = parse_bool(value) {
                    config.check_updates = Some(enabled);
                }
            }
            "force_upgrade_prompt" => {
                if let Some(enabled) = parse_bool(value) {
                    config.force_upgrade_prompt = Some(enabled);
                }
            }
            _ => {}
        }
    }

    config
}

pub(crate) fn load_theme_index() -> usize {
    let config = load_config();
    if let Some(name) = config.theme {
        if let Some(index) = theme_index_by_name(&name) {
            return index;
        }
    }

    default_theme_index()
}

pub(crate) fn load_default_open_target() -> Option<OpenTarget> {
    let config = load_config();
    config
        .default_open
        .as_deref()
        .and_then(OpenTarget::from_config)
}

pub(crate) fn load_default_worktree_name_mode() -> Option<DefaultWorktreeNameMode> {
    load_config().default_worktree_name
}

pub(crate) fn default_worktree_name_is_configured() -> bool {
    load_config().default_worktree_name_set
}

pub(crate) fn load_editor_command() -> Option<String> {
    let config = load_config();
    config
        .editor
        .filter(|value| !value.trim().is_empty())
        .or_else(|| config.default_open.filter(|value| !value.trim().is_empty()))
}

pub(crate) fn load_terminal_command() -> Option<String> {
    let config = load_config();
    config.terminal.filter(|value| !value.trim().is_empty())
}

pub(crate) fn editor_is_configured() -> bool {
    load_config().editor.is_some()
}

pub(crate) fn terminal_is_configured() -> bool {
    load_config().terminal.is_some()
}

pub(crate) fn known_latest_version() -> Option<String> {
    load_config().known_latest_version
}

pub(crate) fn check_updates_enabled() -> bool {
    load_config().check_updates.unwrap_or(true)
}

pub(crate) fn force_upgrade_prompt_enabled() -> bool {
    load_config().force_upgrade_prompt.unwrap_or(false)
}

pub(crate) fn save_editor_command(value: &str) -> io::Result<()> {
    set_config_value("editor", value)
}

pub(crate) fn save_terminal_command(value: &str) -> io::Result<()> {
    set_config_value("terminal", value)
}

pub(crate) fn save_default_worktree_name_mode(
    mode: Option<DefaultWorktreeNameMode>,
) -> io::Result<()> {
    let value = match mode {
        Some(DefaultWorktreeNameMode::Cities) => "cities",
        None => "",
    };
    set_config_value("default_worktree_name", value)
}

pub(crate) fn preload_github_username() {
    let _ = gh_username();
}

pub(crate) fn github_prefix_enabled() -> bool {
    load_config().github_prefix.unwrap_or(true)
}

pub(crate) fn default_branch_name(worktree_name: &str) -> String {
    if !github_prefix_enabled() {
        return worktree_name.to_string();
    }
    let Some(username) = gh_username() else {
        return worktree_name.to_string();
    };
    let candidate = format!("{}/{}", username, worktree_name);
    if bbq::validate_branch_name(&candidate).is_ok() {
        candidate
    } else {
        worktree_name.to_string()
    }
}

pub(crate) fn save_theme_name(name: &str) -> io::Result<()> {
    set_config_value("theme", name)
}

pub(crate) fn save_check_updates(enabled: bool) -> io::Result<()> {
    let value = if enabled { "true" } else { "false" };
    set_config_value("check_updates", value)
}

pub(crate) fn save_known_latest_version(value: &str) -> io::Result<()> {
    set_config_value("known_latest_version", value)
}

#[derive(Debug, Default, Clone)]
pub(crate) struct RestoreState {
    pub(crate) expanded_repos: Vec<String>,
    pub(crate) selected_repo: Option<String>,
    pub(crate) selected_worktree_repo: Option<String>,
    pub(crate) selected_worktree_name: Option<String>,
}

pub(crate) fn load_restore_state() -> RestoreState {
    let Ok(path) = restore_path() else {
        return RestoreState::default();
    };
    let Ok(contents) = fs::read_to_string(path) else {
        return RestoreState::default();
    };
    parse_restore(&contents)
}

pub(crate) fn save_restore_state(state: &RestoreState) -> io::Result<()> {
    let path = restore_path().map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut lines = Vec::new();
    if !state.expanded_repos.is_empty() {
        let items = state
            .expanded_repos
            .iter()
            .map(|value| format!("\"{}\"", escape_toml_string(value)))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("expanded = [{items}]"));
    }
    if let Some(repo) = state.selected_repo.as_ref() {
        lines.push(format!(
            "selected_repo = \"{}\"",
            escape_toml_string(repo)
        ));
    }
    if let Some(repo) = state.selected_worktree_repo.as_ref() {
        lines.push(format!(
            "selected_worktree_repo = \"{}\"",
            escape_toml_string(repo)
        ));
    }
    if let Some(name) = state.selected_worktree_name.as_ref() {
        lines.push(format!(
            "selected_worktree_name = \"{}\"",
            escape_toml_string(name)
        ));
    }

    let mut output = lines.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    fs::write(path, output)
}

fn config_path() -> Result<PathBuf, bbq::BbqError> {
    paths::config_path()
}

fn restore_path() -> Result<PathBuf, bbq::BbqError> {
    Ok(paths::config_root()?.join("restore.toml"))
}

fn set_config_value(key: &str, value: &str) -> io::Result<()> {
    let path = config_path().map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut lines = Vec::new();
    let mut found = false;

    if let Ok(contents) = fs::read_to_string(&path) {
        for line in contents.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with('[') {
                if let Some((existing, _)) = trimmed.split_once('=') {
                    if existing.trim() == key {
                        lines.push(format!("{key} = \"{value}\""));
                        found = true;
                        continue;
                    }
                }
            }
            lines.push(line.to_string());
        }
    }

    if !found {
        lines.push(format!("{key} = \"{value}\""));
    }

    let mut output = lines.join("\n");
    output.push('\n');
    fs::write(path, output)
}

fn trim_quotes(value: &str) -> String {
    let trimmed = value.trim();
    let without = trimmed
        .trim_start_matches('"')
        .trim_end_matches('"')
        .trim_start_matches('\'')
        .trim_end_matches('\'');
    without.to_string()
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn parse_restore(contents: &str) -> RestoreState {
    let mut state = RestoreState::default();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }

        let mut parts = line.splitn(2, '=');
        let key = parts.next().unwrap_or_default().trim();
        let Some(value) = parts.next() else {
            continue;
        };
        let value = value.trim();

        match key {
            "expanded" => state.expanded_repos = parse_string_list(value),
            "selected_repo" => state.selected_repo = Some(trim_quotes(value)),
            "selected_worktree_repo" => state.selected_worktree_repo = Some(trim_quotes(value)),
            "selected_worktree_name" => state.selected_worktree_name = Some(trim_quotes(value)),
            _ => {}
        }
    }

    state
}

fn parse_string_list(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
        return Vec::new();
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    inner
        .split(',')
        .map(|item| trim_quotes(item))
        .filter(|item| !item.is_empty())
        .collect()
}

static GH_USERNAME_CACHE: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn gh_username() -> Option<String> {
    let cache = GH_USERNAME_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if guard.is_some() {
            return guard.clone();
        }
    }
    let username = gh_username_uncached();
    if let Ok(mut guard) = cache.lock() {
        *guard = username.clone();
    }
    username
}

fn gh_username_uncached() -> Option<String> {
    let output = Command::new("gh")
        .args(["api", "user", "-q", ".login"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let username = stdout.lines().next().unwrap_or("").trim();
    if username.is_empty() {
        None
    } else {
        Some(username.to_string())
    }
}

#[cfg(test)]
fn clear_github_username_cache() {
    if let Some(cache) = GH_USERNAME_CACHE.get() {
        if let Ok(mut guard) = cache.lock() {
            *guard = None;
        }
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    let normalized = trim_quotes(value).trim().to_ascii_lowercase();
    match normalized.as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        clear_github_username_cache, default_branch_name,
        default_worktree_name_is_configured, load_default_open_target,
        load_default_worktree_name_mode,
    };
    use crate::open::OpenTarget;
    use bbq::DefaultWorktreeNameMode;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn load_default_open_target_from_config() {
        let _guard = TEST_MUTEX.lock().expect("lock test mutex");
        let root = unique_root("load_default_open_target_from_config");
        let home = root.join("home");
        fs::create_dir_all(&home).expect("create home");
        let _home_env = EnvGuard::set("HOME", &home);

        write_config(&home, "default_open = \"VSCode\"");
        let target = load_default_open_target();
        assert_eq!(target, Some(OpenTarget::VsCode));

        cleanup_root(&root);
    }

    #[test]
    fn default_branch_name_uses_gh_username() {
        let _guard = TEST_MUTEX.lock().expect("lock test mutex");
        clear_github_username_cache();
        let root = unique_root("default_branch_name_uses_gh_username");
        let home = root.join("home");
        fs::create_dir_all(&home).expect("create home");
        let _home_env = EnvGuard::set("HOME", &home);

        let bin_dir = root.join("bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        write_stub_command(&bin_dir, "gh", "echo octocat");
        let path = prepend_path(&bin_dir);
        let _path_env = EnvGuard::set_str("PATH", &path);

        write_config(&home, "github_prefix = true");
        let branch = default_branch_name("feature");
        assert_eq!(branch, "octocat/feature");

        cleanup_root(&root);
    }

    #[test]
    fn default_branch_name_falls_back_when_invalid() {
        let _guard = TEST_MUTEX.lock().expect("lock test mutex");
        clear_github_username_cache();
        let root = unique_root("default_branch_name_falls_back_when_invalid");
        let home = root.join("home");
        fs::create_dir_all(&home).expect("create home");
        let _home_env = EnvGuard::set("HOME", &home);

        let bin_dir = root.join("bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        write_stub_command(&bin_dir, "gh", "echo 'bad name'");
        let path = prepend_path(&bin_dir);
        let _path_env = EnvGuard::set_str("PATH", &path);

        write_config(&home, "github_prefix = true");
        let branch = default_branch_name("feature");
        assert_eq!(branch, "feature");

        cleanup_root(&root);
    }

    #[test]
    fn load_default_worktree_name_mode_from_config() {
        let _guard = TEST_MUTEX.lock().expect("lock test mutex");
        let root = unique_root("load_default_worktree_name_mode_from_config");
        let home = root.join("home");
        fs::create_dir_all(&home).expect("create home");
        let _home_env = EnvGuard::set("HOME", &home);

        write_config(&home, "default_worktree_name = \"cities\"");
        let mode = load_default_worktree_name_mode();
        assert_eq!(mode, Some(DefaultWorktreeNameMode::Cities));

        cleanup_root(&root);
    }

    #[test]
    fn load_default_worktree_name_mode_ignores_unknown() {
        let _guard = TEST_MUTEX.lock().expect("lock test mutex");
        let root = unique_root("load_default_worktree_name_mode_ignores_unknown");
        let home = root.join("home");
        fs::create_dir_all(&home).expect("create home");
        let _home_env = EnvGuard::set("HOME", &home);

        write_config(&home, "default_worktree_name = \"invalid\"");
        let mode = load_default_worktree_name_mode();
        assert_eq!(mode, None);

        cleanup_root(&root);
    }

    #[test]
    fn default_worktree_name_is_configured_when_set() {
        let _guard = TEST_MUTEX.lock().expect("lock test mutex");
        let root = unique_root("default_worktree_name_is_configured_when_set");
        let home = root.join("home");
        fs::create_dir_all(&home).expect("create home");
        let _home_env = EnvGuard::set("HOME", &home);

        write_config(&home, "default_worktree_name = \"cities\"");
        assert!(default_worktree_name_is_configured());

        cleanup_root(&root);
    }

    #[test]
    fn default_worktree_name_is_configured_when_empty() {
        let _guard = TEST_MUTEX.lock().expect("lock test mutex");
        let root = unique_root("default_worktree_name_is_configured_when_empty");
        let home = root.join("home");
        fs::create_dir_all(&home).expect("create home");
        let _home_env = EnvGuard::set("HOME", &home);

        write_config(&home, "default_worktree_name = \"\"");
        assert!(default_worktree_name_is_configured());
        assert_eq!(load_default_worktree_name_mode(), None);

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
            .join(".bbq-cli-config-test")
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

    fn write_config(home: &Path, contents: &str) {
        let config_dir = home.join(".bbq");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(config_dir.join("config.toml"), contents).expect("write config");
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

    fn prepend_path(dir: &Path) -> String {
        let current = std::env::var("PATH").unwrap_or_default();
        format!("{}:{}", dir.display(), current)
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

        fn set_str(key: &'static str, value: &str) -> Self {
            let prev = std::env::var_os(key);
            std::env::set_var(key, value);
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

    // No additional helpers needed beyond set and set_str.
}
