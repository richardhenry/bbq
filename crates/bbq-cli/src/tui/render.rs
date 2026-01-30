use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use super::constants::{
    SELECTED_SECONDARY, SELECTED_TEXT, SPINNER_FRAMES, SPINNER_INTERVAL_MS,
};
use super::types::{Focus, InputState, TreeItemKind, WorktreeEntry};
use crate::tui::app::App;

const BBQ_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn ui(frame: &mut Frame, app: &mut App) {
    if app.is_update_prompt_mode() {
        render_update_prompt(frame, app);
        return;
    }
    if app.is_setup_mode() {
        render_setup(frame, app);
        return;
    }

    let size = frame.size();
    let inner = size;
    let footer_height = footer_height(app, inner.width).min(inner.height);
    let chunks =
        Layout::vertical([Constraint::Min(0), Constraint::Length(footer_height)]).split(inner);
    let columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    if app.repos.is_empty() {
        if let Some(loading) = app.loading_message(super::types::LoadingGroup::Repos) {
            render_loading_column(
                frame,
                columns[0],
                "Repos & Worktrees",
                app.theme_color(),
                loading.started_at,
            );
        } else {
            render_empty_repos_column(frame, columns[0], app.theme_color());
        }
    } else {
        render_tree_list(frame, columns[0], app);
    }
    let env_height = env_box_height(columns[1].height);
    let right_chunks =
        Layout::vertical([Constraint::Min(0), Constraint::Length(env_height)]).split(columns[1]);
    if let Some(entry) = app.selected_worktree_entry() {
        render_worktree_info(frame, right_chunks[0], entry, app);
    } else {
        render_empty_column(
            frame,
            right_chunks[0],
            "Worktree",
            "No worktree selected",
            app.theme_color(),
        );
    }
    if env_height > 0 {
        render_env_info(frame, right_chunks[1], app);
    }
    if let Some(input) = app.input.as_ref() {
        render_prompt_line(frame, chunks[1], input, app.theme_color());
    } else {
        render_status(frame, chunks[1], app);
    }
}

fn render_setup(frame: &mut Frame, app: &mut App) {
    let Some(setup) = app.setup_state() else {
        return;
    };

    let area = frame.size();
    let color = app.theme_color();
    let dim = Style::default().fg(color).add_modifier(Modifier::DIM);
    let highlight = Style::default().fg(color).add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(color);
    let indent = "  ";

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        format!("{indent}{}", setup.question()),
        highlight,
    )));
    lines.push(Line::from(Span::raw("")));

    for (idx, option) in setup.options.iter().enumerate() {
        let selected = idx == setup.selected;
        let marker = if selected { "◉" } else { "○" };
        let style = if selected { highlight } else { normal };
        lines.push(Line::from(Span::styled(
            format!("{indent}{marker} {}", option.label),
            style,
        )));
    }

    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(
        format!("{indent}Use ↑/↓ to choose, Enter to confirm."),
        dim,
    )));
    lines.push(Line::from(Span::styled(
        format!("{indent}You can edit ~/.bbq/config.toml later."),
        dim,
    )));

    if let Some(status) = app.status.as_ref() {
        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::styled(
            format!("{indent}{}", status.text),
            normal.add_modifier(Modifier::BOLD),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_update_prompt(frame: &mut Frame, app: &mut App) {
    let Some(prompt) = app.update_prompt_state() else {
        return;
    };

    let area = frame.size();
    let color = app.theme_color();
    let dim = Style::default().fg(color).add_modifier(Modifier::DIM);
    let highlight = Style::default().fg(color).add_modifier(Modifier::BOLD);
    let normal = Style::default().fg(color);
    let indent = "  ";

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(
            "{indent}New version of bbq is available ({} → {})!",
            prompt.current_version, prompt.latest_version
        ),
        highlight,
    )));
    lines.push(Line::from(Span::raw("")));

    if prompt.completed {
        lines.push(Line::from(Span::styled(
            format!("{indent}Upgrade complete. Relaunch with bbq."),
            normal,
        )));
        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::styled(
            format!("{indent}Press Enter to quit."),
            dim,
        )));
    } else if prompt.running {
        lines.push(Line::from(Span::styled(
            format!("{indent}Running brew upgrade bbq..."),
            normal,
        )));
    } else {
        for (idx, option) in prompt.options().iter().enumerate() {
            let selected = idx == prompt.selected;
            let marker = if selected { "◉" } else { "○" };
            let style = if selected { highlight } else { normal };
            lines.push(Line::from(Span::styled(
                format!("{indent}{marker} {option}"),
                style,
            )));
        }

        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::styled(
            format!("{indent}Use ↑/↓ to choose, Enter to confirm."),
            dim,
        )));
    }

    if let Some(status) = app.status.as_ref() {
        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::styled(
            format!("{indent}{}", status.text),
            normal.add_modifier(Modifier::BOLD),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_tree_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let color = app.theme_color();
    let highlight = list_highlight(app);
    let repo_right_style = Style::default().fg(color).add_modifier(Modifier::DIM);
    let worktree_left_style = Style::default().fg(color);
    let worktree_right_style = repo_right_style;
    let selected_primary = Style::default().fg(SELECTED_TEXT);
    let selected_secondary = Style::default().fg(SELECTED_SECONDARY);
    let selected_index = app.tree_state.selected();
    let show_selected = matches!(highlight, HighlightMode::Primary);

    let items: Vec<ListItem> = app
        .tree_items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let is_selected = show_selected && selected_index == Some(idx);
            match &item.kind {
                TreeItemKind::Repo {
                    expanded,
                    worktree_count,
                    ..
                } => {
                    let count_text = worktree_count.to_string();
                    let count_style = if is_selected {
                        selected_secondary
                    } else {
                        repo_right_style
                    };
                    let arrow_style = if is_selected {
                        if *expanded {
                            selected_primary
                        } else {
                            selected_secondary
                        }
                    } else if *expanded {
                        Style::default().fg(color)
                    } else {
                        repo_right_style
                    };
                    let mut right_parts = Vec::new();
                    if !*expanded {
                        right_parts.push((count_text, count_style));
                        right_parts.push((" ".to_string(), count_style));
                    }
                    right_parts.push((if *expanded { "↓" } else { "→" }.to_string(), arrow_style));
                    list_item_with_right_parts(
                        &item.left,
                        if is_selected {
                            selected_primary
                        } else {
                            Style::default().fg(color)
                        },
                        right_parts,
                        area.width,
                    )
                }
                TreeItemKind::Worktree { .. } => list_item_with_right_text(
                    &item.left,
                    &item.right,
                    if is_selected {
                        selected_primary
                    } else {
                        worktree_left_style
                    },
                    if is_selected {
                        selected_secondary
                    } else {
                        worktree_right_style
                    },
                    area.width,
                ),
            }
        })
        .collect();

    render_list(
        frame,
        area,
        "Repos & Worktrees",
        items,
        &mut app.tree_state,
        color,
        highlight,
    );
}

fn render_env_info(frame: &mut Frame, area: Rect, app: &App) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let color = app.theme_color();
    let normal = Style::default().fg(color);
    let dim = normal.add_modifier(Modifier::DIM);
    let border_style = Style::default().fg(color);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().fg(color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    render_block_title(frame, area, "Environment", color);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let mut labels = vec!["Root:", "Git:"];
    if app.env_info.gh_version.is_some() {
        labels.push("GitHub:");
    }
    let label_width = label_width(&labels);
    let home = app.env_info.home_dir.as_deref().unwrap_or("unknown");
    let git = format_component("git", app.env_info.git_version.as_deref());
    let gh = app
        .env_info
        .gh_version
        .as_deref()
        .map(|version| format_component("gh", Some(version)));

    let mut lines = Vec::new();
    lines.push(aligned_info_line(
        "Root: ",
        home,
        dim,
        normal,
        label_width,
        inner.width,
    ));
    lines.push(aligned_info_line(
        "Git: ",
        &git,
        dim,
        normal,
        label_width,
        inner.width,
    ));
    if let Some(gh) = gh {
        lines.push(aligned_info_line(
            "GitHub: ",
            &gh,
            dim,
            normal,
            label_width,
            inner.width,
        ));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn render_worktree_info(frame: &mut Frame, area: Rect, entry: &WorktreeEntry, app: &App) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let color = app.theme_color();
    let normal = Style::default().fg(color);
    let dim = normal.add_modifier(Modifier::DIM);
    let border_style = Style::default().fg(color);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().fg(color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    render_block_title(frame, area, "Worktree", color);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let name = entry.worktree.display_name();
    let repo = app
        .selected_repo()
        .map(|repo| app.display_repo_name(&repo.name));
    let branch = entry.worktree.branch.as_deref().unwrap_or("detached");
    let head = entry
        .worktree
        .head
        .as_deref()
        .map(short_git_hash)
        .unwrap_or_else(|| "none".to_string());
    let head_author = entry
        .head_author
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let head_message = entry
        .head_message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let head_line = match head_author {
        Some(author) if head != "none" => format!("{head} - {author}"),
        _ => head.clone(),
    };

    let label_width = label_width(&[
        "Worktree:",
        "Dir:",
        "Repo:",
        "Branch:",
        "Upstream:",
        "Head:",
        "Sync:",
        "Changes:",
    ]);

    let mut lines = Vec::new();
    let value_width = inner.width.saturating_sub(label_width as u16) as usize;
    let repo_value = repo.unwrap_or("none");
    let repo_value = truncate_from_start_with_ellipsis(repo_value, value_width);
    let branch_value = truncate_from_start_with_ellipsis(branch, value_width);
    let dir_value = truncate_after_first_slash(&entry.worktree_path, value_width);
    lines.push(aligned_info_line(
        "Worktree: ",
        &name,
        dim,
        normal,
        label_width,
        inner.width,
    ));
    lines.push(aligned_info_line(
        "Dir: ",
        &dir_value,
        dim,
        normal,
        label_width,
        inner.width,
    ));
    lines.push(aligned_info_line(
        "Repo: ",
        &repo_value,
        dim,
        normal,
        label_width,
        inner.width,
    ));
    lines.push(aligned_info_line(
        "Branch: ",
        &branch_value,
        dim,
        normal,
        label_width,
        inner.width,
    ));
    lines.push(aligned_info_line(
        "Upstream: ",
        entry.upstream.as_deref().unwrap_or("none"),
        dim,
        normal,
        label_width,
        inner.width,
    ));
    lines.push(aligned_info_line(
        "Head: ",
        &head_line,
        dim,
        normal,
        label_width,
        inner.width,
    ));
    if let Some(message) = head_message {
        lines.push(aligned_info_line(
            "",
            message,
            dim,
            normal,
            label_width,
            inner.width,
        ));
    }
    let sync_style = if entry.upstream.is_some() { normal } else { dim };
    lines.extend(aligned_info_lines(
        "Sync: ",
        &entry.sync_status,
        dim,
        sync_style,
        label_width,
        inner.width,
    ));
    let remaining = (inner.height as usize).saturating_sub(lines.len());
    if remaining > 0 {
        let dash_count = inner.width as usize;
        let separator = "─".repeat(dash_count.max(1));
        lines.push(Line::from(Span::styled(separator, dim)));
        let remaining = remaining.saturating_sub(1);
        if remaining == 0 {
            let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
            frame.render_widget(paragraph, inner);
            return;
        }
        let mut items: Vec<(String, String, Style, Style)> = if entry.changed_files.is_empty() {
            vec![("none".to_string(), String::new(), dim, dim)]
        } else {
            entry
                .changed_files
                .iter()
                .map(|file| {
                    (
                        file.path.clone(),
                        format!("+{}/-{}", file.added, file.removed),
                        normal,
                        dim,
                    )
                })
                .collect()
        };

        if !entry.changed_files.is_empty() && items.len() > remaining {
            let visible = remaining.saturating_sub(1);
            let more_count = items.len().saturating_sub(visible);
            items.truncate(visible);
            items.push((format!("(+{} more)", more_count), String::new(), dim, dim));
        }

        let label_text = pad_to_width("Changes: ", label_width);
        let content_width = inner.width.saturating_sub(label_width as u16) as usize;
        let pad = " ".repeat(label_width);
        for (idx, (left, right, left_style, right_style)) in items.into_iter().enumerate() {
            let mut spans = Vec::new();
            if idx == 0 {
                spans.push(Span::styled(label_text.clone(), dim));
            } else {
                spans.push(Span::styled(pad.clone(), dim));
            }
            spans.extend(left_right_spans(
                &left,
                &right,
                left_style,
                right_style,
                content_width,
            ));
            lines.push(Line::from(spans));
        }
    }
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

#[derive(Clone, Copy)]
enum HighlightMode {
    Primary,
    None,
}

fn render_list(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: Vec<ListItem>,
    state: &mut ListState,
    color: Color,
    highlight: HighlightMode,
) {
    let border_style = Style::default().fg(color);
    let (highlight_style, highlight_symbol) = match highlight {
        HighlightMode::Primary => (Style::default().bg(color), ""),
        HighlightMode::None => (Style::default(), ""),
    };

    let list = List::new(items)
        .style(Style::default().fg(color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .style(Style::default().fg(color)),
        )
        .highlight_style(highlight_style)
        .highlight_symbol(highlight_symbol);

    frame.render_stateful_widget(list, area, state);
    render_block_title(frame, area, title, color);
}

fn list_item_with_right_text(
    left: &str,
    right: &str,
    left_style: Style,
    right_style: Style,
    width: u16,
) -> ListItem<'static> {
    let content_width = width.saturating_sub(2) as usize;
    if content_width == 0 {
        return ListItem::new(Line::from(Span::raw(String::new())));
    }

    let right_len = right.chars().count();
    if right_len >= content_width {
        let right_text = truncate_to_width(right, content_width);
        return ListItem::new(Line::from(Span::styled(right_text, right_style)));
    }

    let max_left = content_width.saturating_sub(right_len + 1);
    let left_text = truncate_left_from_start(left, max_left);
    let left_len = left_text.chars().count();
    let padding = content_width.saturating_sub(left_len + right_len);
    let spaces = " ".repeat(padding);

    ListItem::new(Line::from(vec![
        Span::styled(left_text, left_style),
        Span::raw(spaces),
        Span::styled(right.to_string(), right_style),
    ]))
}

fn list_item_with_right_parts(
    left: &str,
    left_style: Style,
    right_parts: Vec<(String, Style)>,
    width: u16,
) -> ListItem<'static> {
    let content_width = width.saturating_sub(2) as usize;
    if content_width == 0 {
        return ListItem::new(Line::from(Span::raw(String::new())));
    }

    let right_len: usize = right_parts
        .iter()
        .map(|(text, _)| text.chars().count())
        .sum();
    let right_text: String = right_parts
        .iter()
        .map(|(text, _)| text.as_str())
        .collect();

    if right_len >= content_width {
        let truncated = truncate_to_width(&right_text, content_width);
        let style = right_parts
            .first()
            .map(|(_, style)| *style)
            .unwrap_or_default();
        return ListItem::new(Line::from(Span::styled(truncated, style)));
    }

    let max_left = content_width.saturating_sub(right_len + 1);
    let left_text = truncate_to_width(left, max_left);
    let left_len = left_text.chars().count();
    let padding = content_width.saturating_sub(left_len + right_len);
    let spaces = " ".repeat(padding);
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(left_text, left_style));
    spans.push(Span::raw(spaces));
    for (text, style) in right_parts {
        spans.push(Span::styled(text, style));
    }
    ListItem::new(Line::from(spans))
}

fn truncate_to_width(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn truncate_from_start_with_ellipsis(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let len = text.chars().count();
    if len <= max {
        return text.to_string();
    }
    if max == 1 {
        return "…".to_string();
    }
    let mut chars: Vec<char> = text.chars().rev().take(max).collect();
    chars.reverse();
    if !chars.is_empty() {
        chars[0] = '…';
    }
    chars.into_iter().collect()
}

fn truncate_left_from_start(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut prefix = String::new();
    let mut remainder = String::new();
    let mut in_prefix = true;
    for ch in text.chars() {
        if in_prefix && ch.is_whitespace() {
            prefix.push(ch);
        } else {
            in_prefix = false;
            remainder.push(ch);
        }
    }
    let prefix_len = prefix.chars().count();
    if prefix_len >= max {
        return prefix.chars().take(max).collect();
    }
    let available = max - prefix_len;
    let tail = truncate_from_start_with_ellipsis(&remainder, available);
    format!("{prefix}{tail}")
}

fn truncate_after_first_slash(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let len = text.chars().count();
    if len <= max {
        return text.to_string();
    }

    let mut iter = text.chars();
    let mut prefix = String::new();
    let mut remainder = String::new();
    let mut found_slash = false;
    while let Some(ch) = iter.next() {
        if !found_slash {
            prefix.push(ch);
            if ch == '/' {
                found_slash = true;
            }
        } else {
            remainder.push(ch);
        }
    }

    let prefix_len = prefix.chars().count();
    if prefix_len >= max {
        return truncate_to_width(&prefix, max);
    }
    let available = max - prefix_len;
    if available == 0 {
        return prefix;
    }
    if remainder.is_empty() {
        return prefix;
    }
    if available == 1 {
        return format!("{prefix}…");
    }
    let tail_len = available - 1;
    let tail = remainder
        .chars()
        .rev()
        .take(tail_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{prefix}…{tail}")
}

fn list_highlight(app: &App) -> HighlightMode {
    match app.effective_focus() {
        Focus::List => HighlightMode::Primary,
        Focus::Input => HighlightMode::None,
    }
}

fn render_empty_column(frame: &mut Frame, area: Rect, title: &str, message: &str, color: Color) {
    let border_style = Style::default().fg(color);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().fg(color));
    let inner = block.inner(area);

    frame.render_widget(block, area);
    render_block_title(frame, area, title, color);

    if inner.height == 0 || message.is_empty() {
        return;
    }

    let y = inner.y + inner.height / 2;
    let text_area = Rect {
        x: inner.x,
        y,
        width: inner.width,
        height: 1,
    };
    let paragraph = Paragraph::new(message)
        .alignment(Alignment::Center)
        .style(Style::default().fg(color).add_modifier(Modifier::DIM));
    frame.render_widget(paragraph, text_area);
}

fn render_loading_column(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    color: Color,
    started_at: std::time::Instant,
) {
    let border_style = Style::default().fg(color);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().fg(color));
    let inner = block.inner(area);

    frame.render_widget(block, area);
    render_block_title(frame, area, title, color);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let normal = Style::default().fg(color);
    let dim = normal.add_modifier(Modifier::DIM);
    let spinner = spinner_frame(started_at);
    let line = Line::from(vec![
        Span::styled(spinner, dim),
        Span::styled(" ", dim),
        Span::styled("Loading…", normal),
    ]);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center).style(normal);

    let text_area = Rect {
        x: inner.x,
        y: inner.y + inner.height / 2,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(paragraph, text_area);
}

fn render_empty_repos_column(frame: &mut Frame, area: Rect, color: Color) {
    let border_style = Style::default().fg(color);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().fg(color));
    let inner = block.inner(area);

    frame.render_widget(block, area);
    render_block_title(frame, area, "Repos & Worktrees", color);

    if inner.height == 0 {
        return;
    }

    let dim = Style::default().fg(color).add_modifier(Modifier::DIM);
    let normal = Style::default().fg(color);
    let lines = vec![
        Line::from(Span::styled("No repos", dim)),
        Line::from(vec![
            Span::styled("Press ", dim),
            Span::styled("c", normal),
            Span::styled(" to clone", dim),
        ]),
    ];

    let height = lines.len() as u16;
    if inner.height < height {
        return;
    }

    let start_y = inner
        .y
        .saturating_add(inner.height / 2)
        .saturating_sub(height / 2);
    let text_area = Rect {
        x: inner.x,
        y: start_y,
        width: inner.width,
        height,
    };

    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .style(Style::default().fg(color));
    frame.render_widget(paragraph, text_area);
}

fn render_block_title(frame: &mut Frame, area: Rect, title: &str, color: Color) {
    if title.is_empty() || area.width < 4 {
        return;
    }

    let x = area.x.saturating_add(2);
    let width = area.width.saturating_sub(3);
    if width == 0 {
        return;
    }

    let title_area = Rect {
        x,
        y: area.y,
        width,
        height: 1,
    };
    let paragraph = Paragraph::new(title).style(Style::default().fg(color));
    frame.render_widget(paragraph, title_area);
}

fn build_help_text(app: &App) -> String {
    let mut items: Vec<&str> = Vec::new();
    let focus = app.effective_focus();
    let has_repos = !app.repos.is_empty();

    if focus == Focus::List || !has_repos {
        items.push("c clone");
    }
    if app.selected_repo().is_some() {
        items.push("n new worktree");
    }
    let delete_available = focus == Focus::List && app.selected_tree_item().is_some();
    if delete_available {
        items.push("d delete");
    }
    if app.selected_worktree_entry().is_some() {
        items.push("t terminal");
        items.push("enter editor");
    }

    items.join(" | ")
}

fn render_status(frame: &mut Frame, area: Rect, app: &App) {
    let area = inset_h(area, 1);
    if area.width == 0 || area.height == 0 {
        return;
    }

    let color = app.theme_color();
    let normal = Style::default().fg(color);
    let dim = normal.add_modifier(Modifier::DIM);
    let error = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let error_dim = Style::default().fg(Color::Red);

    let help = build_help_text(app);

    if let Some(status) = app.status.as_ref() {
        let (prefix_style, message_style) = match status.tone {
            super::types::StatusTone::Success => (dim, normal),
            super::types::StatusTone::Error => (error_dim, error),
        };
        let line = Line::from(vec![
            Span::styled("→ ", prefix_style),
            Span::styled(status.text.clone(), message_style),
        ]);
        let paragraph = Paragraph::new(line).style(message_style).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
        let hide_line = Line::from(vec![
            Span::styled("esc", normal),
            Span::styled(" hide", dim),
        ]);
        let hide_area = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(1),
            width: area.width,
            height: 1,
        };
        let hide_para = Paragraph::new(hide_line)
            .style(normal)
            .alignment(Alignment::Right);
        frame.render_widget(hide_para, hide_area);
        return;
    }

    if let Some(loading) = app.current_loading() {
        let spinner = spinner_frame(loading.started_at);
        let line = Line::from(vec![
            Span::styled(spinner, dim),
            Span::styled(" ", dim),
            Span::styled(loading.text.clone(), normal),
        ]);
        let paragraph = Paragraph::new(line).style(normal).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
        return;
    }

    let (left_line, _left_len) = build_help_line(&help, normal, dim);
    let (right_line, right_len) = build_footer_line(app.theme_name(), normal, dim);

    let right_width = (right_len as u16).min(area.width);
    let columns = Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)])
        .split(area);

    if columns[0].width > 0 {
        let left_para = Paragraph::new(left_line).style(normal);
        frame.render_widget(left_para, columns[0]);
    }

    if columns[1].width > 0 {
        let right_para = Paragraph::new(right_line)
            .style(normal)
            .alignment(Alignment::Right);
        frame.render_widget(right_para, columns[1]);
    }
}

fn render_prompt_line(frame: &mut Frame, area: Rect, input: &InputState, color: Color) {
    let label = format!(" {}", input.label());
    let base_style = Style::default().fg(SELECTED_TEXT).bg(color);
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (content, content_style) = if input.buffer.is_empty() {
        (input.placeholder(), base_style.add_modifier(Modifier::DIM))
    } else {
        (input.buffer.as_str(), base_style)
    };

    let line = Line::from(vec![
        Span::styled(&label, base_style),
        Span::styled(content, content_style),
    ]);
    let paragraph = Paragraph::new(line).style(base_style);
    frame.render_widget(paragraph, area);

    let cursor_x = area.x + label.len() as u16 + input.buffer.len() as u16;
    let cursor_x = cursor_x.min(area.x + area.width.saturating_sub(1));
    frame.set_cursor(cursor_x, area.y);
}

fn inset_h(area: Rect, padding: u16) -> Rect {
    let total = padding.saturating_mul(2);
    if area.width <= total {
        return Rect {
            x: area.x,
            y: area.y,
            width: 0,
            height: area.height,
        };
    }

    Rect {
        x: area.x + padding,
        y: area.y,
        width: area.width - total,
        height: area.height,
    }
}

fn env_box_height(total_height: u16) -> u16 {
    let min_height: u16 = 5;
    if total_height >= min_height.saturating_add(2) {
        min_height
    } else {
        0
    }
}

fn aligned_info_line(
    label: &str,
    value: &str,
    label_style: Style,
    value_style: Style,
    label_width: usize,
    width: u16,
) -> Line<'static> {
    if width == 0 {
        return Line::from(Span::raw(String::new()));
    }

    let label = pad_to_width(label, label_width);
    if width as usize <= label_width {
        let text = truncate_to_width(&label, width as usize);
        return Line::from(Span::styled(text, label_style));
    }

    let max_value = width.saturating_sub(label_width as u16) as usize;
    let value = truncate_to_width(value, max_value);
    Line::from(vec![
        Span::styled(label, label_style),
        Span::styled(value, value_style),
    ])
}

fn aligned_info_lines(
    label: &str,
    value: &str,
    label_style: Style,
    value_style: Style,
    label_width: usize,
    width: u16,
) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::from(Span::raw(String::new()))];
    }

    let label_text = pad_to_width(label, label_width);
    if width as usize <= label_width {
        let text = truncate_to_width(&label_text, width as usize);
        return vec![Line::from(Span::styled(text, label_style))];
    }

    let max_value = width.saturating_sub(label_width as u16) as usize;
    let wrapped = wrap_text(value, max_value);
    let pad = " ".repeat(label_width);
    let mut lines = Vec::new();

    for (idx, chunk) in wrapped.into_iter().enumerate() {
        if idx == 0 {
            lines.push(Line::from(vec![
                Span::styled(label_text.clone(), label_style),
                Span::styled(chunk, value_style),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(pad.clone(), label_style),
                Span::styled(chunk, value_style),
            ]));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(label_text, label_style)));
    }

    lines
}

fn left_right_spans(
    left: &str,
    right: &str,
    left_style: Style,
    right_style: Style,
    width: usize,
) -> Vec<Span<'static>> {
    if width == 0 {
        return vec![Span::raw(String::new())];
    }

    if right.is_empty() {
        let left_text = truncate_to_width(left, width);
        return vec![Span::styled(left_text, left_style)];
    }

    let right_len = right.chars().count();
    if right_len >= width {
        let right_text = truncate_to_width(right, width);
        return vec![Span::styled(right_text, right_style)];
    }

    let max_left = width.saturating_sub(right_len + 1);
    let left_text = truncate_to_width(left, max_left);
    let left_len = left_text.chars().count();
    let padding = width.saturating_sub(left_len + right_len);
    let spaces = " ".repeat(padding);

    vec![
        Span::styled(left_text, left_style),
        Span::raw(spaces),
        Span::styled(right.to_string(), right_style),
    ]
}

fn wrap_text(value: &str, max: usize) -> Vec<String> {
    if max == 0 {
        return vec![String::new()];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in value.split_whitespace() {
        let word_len = word.chars().count();

        if current.is_empty() {
            if word_len > max {
                let mut remainder = word.to_string();
                while remainder.chars().count() > max {
                    let chunk = truncate_to_width(&remainder, max);
                    lines.push(chunk.clone());
                    remainder = remainder.chars().skip(max).collect();
                }
                current = remainder;
            } else {
                current = word.to_string();
            }
            continue;
        }

        let candidate_len = current.chars().count() + 1 + word_len;
        if candidate_len <= max {
            current.push(' ');
            current.push_str(word);
            continue;
        }

        lines.push(current);
        if word_len > max {
            let mut remainder = word.to_string();
            while remainder.chars().count() > max {
                let chunk = truncate_to_width(&remainder, max);
                lines.push(chunk.clone());
                remainder = remainder.chars().skip(max).collect();
            }
            current = remainder;
        } else {
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn label_width(labels: &[&str]) -> usize {
    labels
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn pad_to_width(value: &str, width: usize) -> String {
    let len = value.chars().count();
    if len >= width {
        return value.to_string();
    }
    let mut out = String::with_capacity(width);
    out.push_str(value);
    out.push_str(&" ".repeat(width - len));
    out
}

fn format_component(name: &str, version: Option<&str>) -> String {
    match version {
        Some(version) => format!("{name} v{version}"),
        None => format!("{name} (unknown)"),
    }
}

fn short_git_hash(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 7 {
        trimmed.to_string()
    } else {
        trimmed[..7].to_string()
    }
}

fn build_help_line(help: &str, normal: Style, dim: Style) -> (Line<'static>, usize) {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut len = 0usize;

    for (idx, part) in help.split(" | ").enumerate() {
        if idx > 0 {
            push_span_owned(&mut spans, " | ".to_string(), dim, &mut len);
        }

        if let Some((key, label)) = part.split_once(' ') {
            push_span_owned(&mut spans, key.to_string(), normal, &mut len);
            let label = format!(" {}", label);
            push_span_owned(&mut spans, label, dim, &mut len);
        } else {
            push_span_owned(&mut spans, part.to_string(), dim, &mut len);
        }
    }

    (Line::from(spans), len)
}

fn build_footer_line(theme: &str, normal: Style, dim: Style) -> (Line<'static>, usize) {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut len = 0usize;
    push_span_owned(&mut spans, "h".to_string(), normal, &mut len);
    let theme_text = format!(" theme: {}", theme);
    push_span_owned(&mut spans, theme_text, dim, &mut len);
    push_span_owned(&mut spans, " | ".to_string(), dim, &mut len);
    let footer = format!("bbq v{} - get cookin'", BBQ_VERSION);
    push_span_owned(&mut spans, footer, normal, &mut len);
    (Line::from(spans), len)
}

fn push_span_owned(spans: &mut Vec<Span<'static>>, text: String, style: Style, len: &mut usize) {
    *len += text.chars().count();
    spans.push(Span::styled(text, style));
}

fn footer_height(app: &App, width: u16) -> u16 {
    if app.input.is_some() {
        return 1;
    }

    let available = width.saturating_sub(2).max(1);
    let text = if let Some(status) = app.status.as_ref() {
        format!("→ {}", status.text)
    } else if let Some(loading) = app.current_loading() {
        format!("{} {}", SPINNER_FRAMES[0], loading.text)
    } else {
        return 1;
    };
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: true });
    paragraph.line_count(available) as u16
}

fn spinner_frame(started_at: std::time::Instant) -> &'static str {
    let elapsed = started_at.elapsed().as_millis();
    let idx = (elapsed / SPINNER_INTERVAL_MS) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx]
}
