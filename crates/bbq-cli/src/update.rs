use std::process::Command;

use serde_json::Value;

pub(crate) struct UpdateInfo {
    pub(crate) latest: String,
}

pub(crate) fn is_homebrew_install() -> bool {
    let Ok(path) = std::env::current_exe() else {
        return false;
    };
    let path = path.to_string_lossy();
    path.contains("/Cellar/bbq/")
}

pub(crate) fn check_homebrew_update() -> Option<UpdateInfo> {
    let output = Command::new("brew")
        .args(["outdated", "--json=v2"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let json: Value = serde_json::from_slice(&output.stdout).ok()?;
    let formulae = json.get("formulae")?.as_array()?;
    let entry = formulae
        .iter()
        .find(|formula| formula.get("name").and_then(Value::as_str) == Some("bbq"))?;
    let latest = entry
        .get("current_version")
        .and_then(Value::as_str)?
        .trim();
    if latest.is_empty() {
        return None;
    }

    Some(UpdateInfo {
        latest: latest.to_string(),
    })
}

pub(crate) fn run_homebrew_upgrade() -> Result<(), String> {
    let output = Command::new("brew")
        .args(["upgrade", "bbq"])
        .output()
        .map_err(|err| format!("Failed to run brew: {err}"))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = stderr.trim();
    if message.is_empty() {
        Err("brew upgrade failed".to_string())
    } else {
        Err(message.to_string())
    }
}
