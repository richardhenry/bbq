use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use crate::error::{BbqError, Result};
use crate::model::Worktree;

pub const POST_CREATE_SCRIPT_RELATIVE: &str = ".bbq/worktree/post-create";

#[derive(Debug, Clone, Copy)]
pub enum ScriptOutput {
    Inherit,
    Capture,
}

pub fn post_create_script_path(worktree: &Worktree) -> PathBuf {
    worktree.path.join(POST_CREATE_SCRIPT_RELATIVE)
}

pub fn find_post_create_script(worktree: &Worktree) -> Option<PathBuf> {
    let path = post_create_script_path(worktree);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

pub fn run_post_create_script(
    worktree: &Worktree,
    output: ScriptOutput,
) -> Result<Option<PathBuf>> {
    let Some(path) = find_post_create_script(worktree) else {
        return Ok(None);
    };
    run_script(worktree, &path, output)?;
    Ok(Some(path))
}

fn run_script(worktree: &Worktree, script: &Path, output: ScriptOutput) -> Result<()> {
    let script_display = script.display().to_string();
    let mut parts = read_shebang(script).map_err(|err| err.with_script(&script_display))?;
    let Some(command) = parts.first().cloned() else {
        return Err(BbqError::ScriptMissingShebang(script_display));
    };
    if !parts.is_empty() {
        parts.remove(0);
    }

    let mut cmd = Command::new(command);
    if !parts.is_empty() {
        cmd.args(parts);
    }
    cmd.arg(script);
    cmd.current_dir(&worktree.path);

    match output {
        ScriptOutput::Inherit => {
            let status = cmd.status().map_err(|err| BbqError::ScriptFailed {
                script: script_display.clone(),
                message: err.to_string(),
            })?;
            if status.success() {
                Ok(())
            } else {
                Err(BbqError::ScriptFailed {
                    script: script_display,
                    message: format_exit_status(status, None),
                })
            }
        }
        ScriptOutput::Capture => {
            cmd.stdin(Stdio::null());
            let output = cmd.output().map_err(|err| BbqError::ScriptFailed {
                script: script_display.clone(),
                message: err.to_string(),
            })?;
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(BbqError::ScriptFailed {
                    script: script_display,
                    message: format_exit_status(output.status, Some(stderr.as_ref())),
                })
            }
        }
    }
}

fn read_shebang(script: &Path) -> Result<Vec<String>> {
    let file = File::open(script).map_err(|err| BbqError::ScriptFailed {
        script: script.display().to_string(),
        message: err.to_string(),
    })?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|err| BbqError::ScriptFailed {
            script: script.display().to_string(),
            message: err.to_string(),
        })?;
    let line = line.trim_end_matches(|ch| ch == '\n' || ch == '\r');
    if !line.starts_with("#!") {
        return Err(BbqError::ScriptMissingShebang(
            script.display().to_string(),
        ));
    }
    let command_line = line[2..].trim();
    if command_line.is_empty() {
        return Err(BbqError::ScriptMissingShebang(
            script.display().to_string(),
        ));
    }
    Ok(command_line
        .split_whitespace()
        .map(|part| part.to_string())
        .collect())
}

fn format_exit_status(status: ExitStatus, stderr: Option<&str>) -> String {
    let mut message = if let Some(code) = status.code() {
        format!("exit status {code}")
    } else {
        "terminated by signal".to_string()
    };

    if let Some(stderr) = stderr {
        let stderr = stderr.trim();
        if !stderr.is_empty() {
            message.push_str("\nstderr: ");
            message.push_str(stderr);
        }
    }

    message
}

trait ScriptErrorExt {
    fn with_script(self, script: &str) -> BbqError;
}

impl ScriptErrorExt for BbqError {
    fn with_script(self, script: &str) -> BbqError {
        match self {
            BbqError::ScriptMissingShebang(_) => {
                BbqError::ScriptMissingShebang(script.to_string())
            }
            BbqError::ScriptFailed { message, .. } => BbqError::ScriptFailed {
                script: script.to_string(),
                message,
            },
            other => other,
        }
    }
}
