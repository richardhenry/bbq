# bbq - get cookin'

`bbq` is a small CLI/TUI for managing git worktrees. It keeps bare repositories and worktrees under a single root and provides quick commands to clone, list, open, and remove worktrees. It‘s written in Rust.

## Requirements

- `git` on your PATH
- Optional: `cursor`, `code`, or `zed` on PATH for open in editor
- Optional: `gh` for owner/repo GitHub shorthand

## Install (Homebrew)

```sh
brew tap richardhenry/bbq
brew install bbq
```

Upgrade later with:

```sh
brew upgrade bbq
```

## Install (from source)

```sh
cargo build --release
```

The binary is `target/release/bbq` and can be copied to your path.

## Usage

Run `bbq` with no arguments to launch the TUI.

Alternatively, use directly through the CLI:

```sh
bbq repo clone <url-or-path> [name]
bbq repo list
bbq repo rm <name>

bbq worktree create <repo> [--branch <branch>]
bbq worktree list <repo>
bbq worktree open <repo> <name> [--target zed|cursor|vscode|terminal]
bbq worktree rm <repo> <name>
```

### Default branch behavior

If `--branch` is omitted, `bbq` tries to use the repo's default branch in this order:

1. `origin/HEAD` (if set)
2. the bare repo's `HEAD` branch
3. `origin/main`, `origin/master`, `main`, `master`

If none exist, it falls back to `main`.

## Configuration

`bbq` reads `~/.bbq/config.toml`. Example with macOS defaults:

```toml
root_dir = "~/.bbq"
theme = "orange"
github_user_prefix = true
check_updates = true
```

All configuration options:

| Option | Default (macOS) | Description |
| --- | --- | --- |
| `root_dir` | `~/.bbq` | Base directory for repos/worktrees. `BBQ_ROOT_DIR` overrides. |
| `theme` | `orange` | TUI accent color. |
| `editor` | unset (auto-detect `zed`, `cursor`, `code`) | Command/app to open worktrees. Used by TUI and CLI when no `--target` is provided. |
| `terminal` | unset (uses Terminal.app) | Command/app to open a terminal at a worktree path. On Linux, auto-detects common terminals. |
| `github_user_prefix` | `true` | Prefix new branch names with your GitHub username (requires `gh`). |
| `default_worktree_name` | unset | If set to `cities`, new worktrees default to a random city slug (for example `san-francisco`). |
| `check_updates` | `true` | Check for Homebrew updates and show the upgrade prompt. |
| `known_latest_version` | unset (internal) | Last version seen by the background update check; managed by `bbq`. |

The environment variable `BBQ_ROOT_DIR` overrides `root_dir`.

### Terminal support (Unix-like)

If no `terminal` is configured, `bbq` tries common terminal emulators in order (`wezterm`, `alacritty`, `kitty`, `gnome-terminal`, `konsole`, `xfce4-terminal`, `x-terminal-emulator`, then `xterm`). Configure `terminal` if you use something else.

## License

MIT. See `LICENSE`.
