# bbq - get cookin'

`bbq` is a small CLI/TUI for managing git worktrees. It keeps bare repositories and worktrees under a single root and provides quick commands to clone, list, open, and remove worktrees. It‘s written in Rust.

## Requirements

- `git` on your PATH
- Optional: `cursor`, `code`, or `zed` on PATH for open in editor
- Optional: `gh` for owner/repo GitHub shorthand

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

`bbq` reads `~/.bbq/config.toml`:

```toml
root_dir = "~/dev/bbq"
theme = "orange"
default_open = "code"
editor = "code"
terminal = "wezterm"
github_prefix = true
default_worktree_name = "cities"
```

When `default_worktree_name = "cities"` is set, new worktrees default to a random city slug (for example `san-francisco` or `shanghai`).

`BBQ_ROOT_DIR` overrides `root_dir`.

### Terminal support (Unix-like)

If no `terminal` is configured, `bbq` tries common terminal emulators in order (`wezterm`, `alacritty`, `kitty`, `gnome-terminal`, `konsole`, `xfce4-terminal`, `x-terminal-emulator`, then `xterm`). Configure `terminal` if you use something else.

## License

MIT. See `LICENSE`.
