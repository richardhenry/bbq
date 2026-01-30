use std::fs;
use std::path::PathBuf;

use crate::error::{BbqError, Result};

pub fn config_root() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or(BbqError::HomeDirMissing)?;
    Ok(home.join(".bbq"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_root()?.join("config.toml"))
}

pub fn bbq_root() -> Result<PathBuf> {
    if let Some(root) = std::env::var_os("BBQ_ROOT_DIR") {
        if !root.is_empty() {
            return Ok(PathBuf::from(root));
        }
    }

    if let Some(root) = root_dir_from_config()? {
        return Ok(root);
    }

    Ok(config_root()?)
}

pub fn repos_root() -> Result<PathBuf> {
    Ok(bbq_root()?.join("repos"))
}

pub fn worktrees_root() -> Result<PathBuf> {
    Ok(bbq_root()?.join("worktrees"))
}

pub fn ensure_root_dirs() -> Result<()> {
    fs::create_dir_all(repos_root()?)?;
    fs::create_dir_all(worktrees_root()?)?;
    Ok(())
}

fn root_dir_from_config() -> Result<Option<PathBuf>> {
    let path = config_path()?;
    let Ok(contents) = fs::read_to_string(path) else {
        return Ok(None);
    };

    let value = parse_config_value(&contents, "root_dir");
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    if value == "~" || value.starts_with("~/") {
        let home = dirs::home_dir().ok_or(BbqError::HomeDirMissing)?;
        let suffix = value.strip_prefix("~/").unwrap_or("");
        return Ok(Some(home.join(suffix)));
    }

    Ok(Some(PathBuf::from(value)))
}

fn parse_config_value(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }

        let mut parts = line.splitn(2, '=');
        let name = parts.next().unwrap_or_default().trim();
        let Some(value) = parts.next() else {
            continue;
        };

        if name == key {
            return Some(trim_quotes(value));
        }
    }
    None
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
