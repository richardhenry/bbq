use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenTarget {
    Zed,
    Cursor,
    VsCode,
}

impl OpenTarget {
    pub(crate) fn all() -> [OpenTarget; 3] {
        [OpenTarget::Zed, OpenTarget::Cursor, OpenTarget::VsCode]
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            OpenTarget::Zed => "Zed",
            OpenTarget::Cursor => "Cursor",
            OpenTarget::VsCode => "VSCode",
        }
    }

    pub(crate) fn command(self) -> &'static str {
        match self {
            OpenTarget::Zed => "zed",
            OpenTarget::Cursor => "cursor",
            OpenTarget::VsCode => "code",
        }
    }

    pub(crate) fn from_config(value: &str) -> Option<Self> {
        let normalized = normalize_target(value);
        match normalized.as_str() {
            "zed" => Some(OpenTarget::Zed),
            "cursor" => Some(OpenTarget::Cursor),
            "vscode" | "code" | "visualstudiocode" => Some(OpenTarget::VsCode),
            _ => None,
        }
    }
}

pub(crate) fn detect_open_targets() -> Vec<OpenTarget> {
    OpenTarget::all()
        .into_iter()
        .filter(|target| command_available(target.command()))
        .collect()
}

fn command_available(program: &str) -> bool {
    Command::new("sh")
        .args(["-lc", &format!("command -v {}", program)])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(crate) fn normalize_target(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

pub(crate) fn open_in_target(target: OpenTarget, path: &Path) -> io::Result<()> {
    let mut command = Command::new(target.command());
    command.arg(path);
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    command.spawn()?;
    Ok(())
}

pub(crate) fn open_in_editor(command: &str, path: &Path) -> io::Result<()> {
    run_command_with_path(command, path)
}

pub(crate) fn open_terminal_at_path_with_config(
    path: &Path,
    terminal_command: Option<&str>,
) -> io::Result<()> {
    let Some(command) = terminal_command.map(str::trim).filter(|value| !value.is_empty()) else {
        return open_terminal_at_path(path);
    };

    if open_app_with_path(command, path)? {
        return Ok(());
    }

    run_command_with_path(command, path)
}

#[cfg(target_os = "macos")]
fn open_terminal_at_path(path: &Path) -> io::Result<()> {
    let command_line = format!("cd {}", shell_escape(&path.to_string_lossy()));
    open_terminal_command_line(&command_line)
}

#[cfg(not(target_os = "macos"))]
fn open_terminal_at_path(path: &Path) -> io::Result<()> {
    open_terminal_at_path_unix(path)
}

#[cfg(target_os = "macos")]
fn open_terminal_command_line(command_line: &str) -> io::Result<()> {
    let script = format!(
        "tell application \"Terminal\"\n  activate\n  set newWindow to do script \"\"\n  do script \"{}\" in newWindow\nend tell",
        escape_applescript(&command_line)
    );
    let output = Command::new("osascript").args(["-e", &script]).output()?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(io::Error::new(
        io::ErrorKind::Other,
        format!("osascript failed: {}", stderr.trim()),
    ))
}

#[cfg(not(target_os = "macos"))]
fn open_terminal_at_path_unix(path: &Path) -> io::Result<()> {
    let candidates: &[(&str, &[&str])] = &[
        ("wezterm", &["start", "--cwd"]),
        ("alacritty", &["--working-directory"]),
        ("kitty", &["--directory"]),
        ("gnome-terminal", &["--working-directory"]),
        ("konsole", &["--workdir"]),
        ("xfce4-terminal", &["--working-directory"]),
        ("x-terminal-emulator", &["--working-directory"]),
    ];

    for (command, args) in candidates {
        if command_available(command) {
            let mut cmd = Command::new(command);
            cmd.args(*args);
            cmd.arg(path);
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            cmd.spawn()?;
            return Ok(());
        }
    }

    if command_available("xterm") {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
        let command_line = format!(
            "cd {} && exec {}",
            shell_escape(&path.to_string_lossy()),
            shell
        );
        Command::new("xterm")
            .args(["-e", "sh", "-lc", &command_line])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no terminal emulator found; configure terminal in ~/.bbq/config.toml",
    ))
}

fn run_command_with_path(command: &str, path: &Path) -> io::Result<()> {
    let command = command.trim();
    if command.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command is empty",
        ));
    }

    if command.chars().any(|ch| ch.is_whitespace()) {
        if open_app_with_path(command, path)? {
            return Ok(());
        }
        return run_shell_command_with_path(command, path);
    }

    Command::new(command)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_app_with_path(app: &str, path: &Path) -> io::Result<bool> {
    let status = Command::new("open")
        .arg("-a")
        .arg(app)
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(status.success())
}

#[cfg(not(target_os = "macos"))]
fn open_app_with_path(_app: &str, _path: &Path) -> io::Result<bool> {
    Ok(false)
}

fn run_shell_command_with_path(command: &str, path: &Path) -> io::Result<()> {
    let full = format!("{} {}", command, shell_escape(&path.to_string_lossy()));
    Command::new("sh")
        .args(["-lc", &full])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let is_safe = value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '@' | '=')
    });
    if is_safe {
        return value.to_string();
    }
    let mut escaped = String::new();
    escaped.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            escaped.push_str("'\\''");
        } else {
            escaped.push(ch);
        }
    }
    escaped.push('\'');
    escaped
}

fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
