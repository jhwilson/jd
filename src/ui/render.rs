use super::{
    app::{App, Mode, PendingOp},
    keymap,
    rows::Row,
    theme,
};
use crate::model::NodeType;
use crate::plan;
use ratatui::{prelude::*, widgets::*};

pub fn draw(f: &mut Frame, app: &mut App) {
    let [header, _, main, _, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .areas(f.area());
    let panes = |area| {
        Layout::horizontal([
            Constraint::Length(2),
            Constraint::Fill(3),
            Constraint::Length(3),
            Constraint::Fill(2),
            Constraint::Length(2),
        ])
        .split(area)
    };
    let header_panes = panes(header);
    let main_panes = panes(main);
    let tree_pane = main_panes[1];
    let preview_pane = main_panes[3];

    // Tree pane: browse rows, move-destination candidates, or the entries of
    // the current duplicate group.
    let (indices, list_cursor, title): (Vec<usize>, usize, String) = match &app.mode {
        Mode::MovePicker {
            candidates, cursor, ..
        } => (candidates.clone(), *cursor, "Move to".into()),
        Mode::Duplicates { groups, gi, cursor } => (
            groups[*gi].entries.iter().map(|e| e.row_idx).collect(),
            *cursor,
            format!(
                "Duplicate code {} — {}/{}",
                groups[*gi].code,
                gi + 1,
                groups.len()
            ),
        ),
        _ => (app.visible.clone(), app.cursor, "Johnny.Decimal".into()),
    };
    let query = match &app.mode {
        Mode::MovePicker { query, .. } => query.clone(),
        Mode::Duplicates { .. } => String::new(),
        _ => app.query.clone(),
    };
    let lines: Vec<Line> = match &app.mode {
        Mode::Duplicates { groups, gi, .. } => groups[*gi]
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                if i == groups[*gi].recommended {
                    Line::from(vec![
                        Span::raw(e.label.clone()),
                        Span::styled("  ← recommended", theme::OK),
                    ])
                } else {
                    Line::from(e.label.clone())
                }
            })
            .collect(),
        _ => indices.iter().map(|i| row_line(app, *i, &query)).collect(),
    };
    let list = List::new(lines)
        .highlight_symbol(theme::SELECT_MARK)
        .highlight_style(theme::SELECTED);
    let n_lines = match &app.mode {
        Mode::Duplicates { groups, gi, .. } => groups[*gi].entries.len(),
        _ => indices.len(),
    };
    let mut st = ListState::default().with_selected((n_lines > 0).then_some(list_cursor));
    f.render_stateful_widget(list, tree_pane, &mut st);

    let previewed = indices.get(list_cursor).and_then(|i| app.rows.get(*i));
    f.render_widget(
        Paragraph::new(Line::styled(title.to_uppercase(), theme::LABEL)),
        header_panes[1],
    );
    let right_title = if matches!(app.mode, Mode::MetaEdit { .. }) {
        "LOCATIONS & LINKS".to_string()
    } else {
        previewed
            .map(|r| {
                if r.has_notes {
                    format!("{}  ≡ notes", r.display)
                } else {
                    r.display.clone()
                }
            })
            .unwrap_or_default()
    };
    f.render_widget(
        Paragraph::new(Line::styled(right_title, theme::LABEL)),
        header_panes[3],
    );

    // Preview pane: the meta editor while active, else a preview (with the
    // aggregated locations/links section) of the highlighted row.
    if let Mode::MetaEdit { id, cursor } = &app.mode {
        let entries = app.meta_entries(id).map(|(_, e)| e).unwrap_or_default();
        let lines: Vec<Line> = if entries.is_empty() {
            vec![Line::styled(
                "no locations or links yet — press a to add one",
                theme::MUTED,
            )]
        } else {
            entries.iter().map(|e| Line::from(e.display())).collect()
        };
        let list = List::new(lines)
            .highlight_symbol(theme::SELECT_MARK)
            .highlight_style(theme::SELECTED);
        let mut st = ListState::default().with_selected((!entries.is_empty()).then_some(*cursor));
        f.render_stateful_widget(list, preview_pane, &mut st);
    } else {
        let preview = previewed.map(preview_content).unwrap_or_default();
        f.render_widget(
            Paragraph::new(preview).wrap(Wrap { trim: false }),
            preview_pane,
        );
    }

    // Bottom bar: an input/summary line and a hint/status line.
    let bottom_panes = panes(bottom);
    let bottom = Rect::new(
        bottom_panes[1].x,
        bottom.y,
        bottom_panes[3].right().saturating_sub(bottom_panes[1].x),
        bottom.height,
    );
    let [line1_area, line2_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(bottom);
    let (line1, line2) = bottom_lines(app);
    f.render_widget(Paragraph::new(line1), line1_area);
    f.render_widget(Paragraph::new(line2), line2_area);

    // Place the terminal cursor inside an active prompt.
    if let Mode::Prompt { kind, editor } = &app.mode {
        let label_w = prompt_label(kind).chars().count() as u16;
        f.set_cursor_position((line1_area.x + label_w + editor.cursor as u16, line1_area.y));
    }

    if matches!(app.mode, Mode::Help) {
        // Sized to the help text: horizontal padding plus the title/rule rows.
        let w = keymap::HELP
            .lines()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0) as u16
            + 6;
        let h = keymap::HELP.lines().count() as u16 + 4;
        let area = centered(f.area(), w, h);
        f.render_widget(Clear, area);
        let rule = "─".repeat(area.width.saturating_sub(6) as usize);
        let mut help_lines = vec![
            Line::from(vec![
                Span::styled("HELP", theme::LABEL),
                Span::styled("  any key to close", theme::HINT),
            ]),
            Line::styled(rule, theme::RULE),
        ];
        help_lines.extend(keymap::HELP.lines().map(|line| Line::raw(line.to_string())));
        f.render_widget(
            Paragraph::new(Text::from(help_lines))
                .block(Block::new().padding(Padding::new(3, 3, 1, 1))),
            area,
        );
    }
}

/// One tree line: indent, fold glyph, and the display string with query hits
/// highlighted.
fn row_line(app: &mut App, row_idx: usize, query: &str) -> Line<'static> {
    let r = &app.rows[row_idx];
    let glyph = if r.dir_like {
        if app.expanded.expanded.contains(&r.id) {
            "▾ "
        } else {
            "▸ "
        }
    } else {
        "  "
    };
    let prefix = format!("{}{}", "  ".repeat(r.depth), glyph);
    // Ancestors pulled in only to situate matches (browse filter, not the
    // move picker's own candidate list) render dimmed, no hit highlighting.
    if app.context.contains(&row_idx) && !matches!(app.mode, Mode::MovePicker { .. }) {
        return Line::styled(format!("{}{}", prefix, r.display), theme::MUTED);
    }
    if query.is_empty() {
        return Line::from(format!("{}{}", prefix, r.display));
    }
    let hits = app.search.indices(r, query);
    let mut spans = vec![Span::raw(prefix)];
    let mut run = String::new();
    let mut run_hit = false;
    for (ci, ch) in r.display.chars().enumerate() {
        let hit = hits.binary_search(&(ci as u32)).is_ok();
        if hit != run_hit && !run.is_empty() {
            spans.push(styled(std::mem::take(&mut run), run_hit));
        }
        run_hit = hit;
        run.push(ch);
    }
    if !run.is_empty() {
        spans.push(styled(run, run_hit));
    }
    Line::from(spans)
}

fn styled(s: String, hit: bool) -> Span<'static> {
    if hit {
        Span::styled(s, theme::MATCH)
    } else {
        Span::raw(s)
    }
}

pub fn preview_content(r: &Row) -> Text<'static> {
    let p = std::path::Path::new(&r.path);
    let mut lines = Vec::new();
    for line in &r.meta_lines {
        let (glyph, rest) =
            line.split_at(line.char_indices().nth(1).map_or(line.len(), |(i, _)| i));
        lines.push(Line::from(vec![
            Span::styled(glyph.to_string(), theme::ACCENT),
            Span::raw(rest.to_string()),
        ]));
    }
    if !r.meta_lines.is_empty() {
        lines.push(Line::default());
    }
    match r.node_type {
        NodeType::File => {
            let body = crate::preview::preview_file(p)
                .unwrap_or_else(|e| format!("preview unavailable: {e}"));
            let markdown = p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown"));
            if markdown {
                lines.extend(crate::md::render(&body).lines);
            } else {
                lines.extend(body.lines().map(|s| Line::raw(s.to_string())));
            }
        }
        NodeType::Link => {
            let body = crate::preview::preview_link(p)
                .unwrap_or_else(|e| format!("preview unavailable: {e}"));
            lines.extend(body.lines().map(|s| Line::raw(s.to_string())));
        }
        _ => {
            if let Some(notes) = crate::meta::read_notes(p) {
                lines.extend(crate::md::render(&notes).lines);
                if !lines.is_empty() {
                    lines.push(Line::default());
                }
            }
            lines.push(Line::styled("FILES", theme::LABEL));
            let cap = if r.has_notes { 10 } else { 50 };
            match crate::preview::dir_listing(p, cap) {
                Ok(names) => lines.extend(names.into_iter().map(|s| {
                    Line::styled(
                        s,
                        if r.has_notes {
                            theme::MUTED
                        } else {
                            Style::new()
                        },
                    )
                })),
                Err(e) => lines.push(Line::styled(
                    format!("preview unavailable: {e}"),
                    theme::ERR,
                )),
            }
        }
    }
    Text::from(lines)
}

fn prompt_label(kind: &super::app::PromptKind) -> &'static str {
    use super::app::PromptKind::*;
    match kind {
        Create { .. } => "New (code title | name.ext | URL): ",
        Rename { .. } => "Rename to: ",
        LinkUrl { .. } => "URL: ",
        MetaAdd { .. } => "Add location or URL: ",
    }
}

fn bottom_lines(app: &App) -> (Line<'static>, Line<'static>) {
    let hint = |s: &'static str| Line::styled(s, theme::HINT);
    match &app.mode {
        Mode::Browse => {
            let line1 = if app.query.is_empty() {
                hint("type to filter")
            } else {
                Line::from(format!("filter: {}", app.query))
            };
            let line2 = match (&app.status, app.tree.warnings.first()) {
                (Some(s), _) => Line::styled(s.clone(), theme::OK),
                (None, Some(w)) => {
                    let more = app.tree.warnings.len() - 1;
                    let suffix = if more > 0 {
                        format!(" (+{} more)", more)
                    } else {
                        String::new()
                    };
                    Line::styled(
                        format!("⚠ {}{} · ^F to fix", w, suffix),
                        theme::WARN,
                    )
                }
                (None, None) => hint(keymap::HINT),
            };
            (line1, line2)
        }
        Mode::Prompt { kind, editor } => (
            Line::from(format!("{}{}", prompt_label(kind), editor.buffer)),
            hint("enter submit · esc cancel"),
        ),
        Mode::Confirm { pending } => confirm_lines(pending),
        Mode::MovePicker { query, .. } => (
            Line::from(format!("Move to: {}", query)),
            hint("type to filter · ↑/↓ select · enter choose · esc cancel"),
        ),
        Mode::MetaEdit { .. } => (
            match &app.status {
                // e.g. the post-renumber reminder to update external places
                Some(s) => Line::styled(s.clone(), theme::WARN.add_modifier(Modifier::BOLD)),
                None => Line::from("Locations & links"),
            },
            hint("a add · x remove · e notes · ↑/↓ select · esc done"),
        ),
        Mode::Duplicates { .. } => (
            Line::from("Same code, several entries — renumber one, or merge a pointer/file into the folder"),
            hint("↑/↓ select · enter renumber · m merge into folder · s skip group · esc done"),
        ),
        Mode::Message { text, error } => (
            Line::styled(
                text.clone(),
                if *error {
                    theme::ERR
                } else {
                    Style::new()
                },
            ),
            hint("press any key"),
        ),
        Mode::Help => (Line::raw(""), Line::raw("")),
    }
}

fn confirm_lines(pending: &PendingOp) -> (Line<'static>, Line<'static>) {
    match pending {
        PendingOp::Create { plan: p, .. } => {
            let line1 = Line::from(plan::create_summary(p));
            let line2 = if p.warnings.is_empty() {
                Line::styled(
                    "y/enter confirm · n/esc cancel · d/f/l override kind",
                    theme::HINT,
                )
            } else {
                Line::styled(
                    format!("⚠ {} · y/n · d/f/l override", p.warnings.join(" · ")),
                    theme::WARN,
                )
            };
            (line1, line2)
        }
        PendingOp::Move(p) => (
            Line::from(format!(
                "will move {} → {} as {}",
                p.src_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                p.dest_path
                    .parent()
                    .map(|n| n.display().to_string())
                    .unwrap_or_default(),
                p.final_name
            )),
            Line::styled("y/enter confirm · n/esc cancel", theme::HINT),
        ),
        PendingOp::Delete { display, .. } => (
            Line::from(format!("move {} to .jd_trash/?", display)),
            Line::styled("y/enter confirm · n/esc cancel", theme::HINT),
        ),
        PendingOp::MetaRemove { entry, .. } => (
            Line::from(format!("remove {}?", entry.display())),
            Line::styled("y/enter confirm · n/esc cancel", theme::HINT),
        ),
        PendingOp::Merge(p) => (
            Line::from(plan::merge_summary(p)),
            Line::styled("y/enter confirm · n/esc cancel", theme::HINT),
        ),
        PendingOp::Renumber { plan: p, drawers } => (
            Line::from(plan::renumber_summary(p)),
            if *drawers > 0 {
                Line::styled(
                    format!(
                        "⚠ lives in {} other place(s) — you'll be reminded to update them · y/n",
                        drawers
                    ),
                    theme::WARN,
                )
            } else {
                Line::styled("y/enter confirm · n/esc cancel", theme::HINT)
            },
        ),
    }
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}
