use crate::app::{App, FilterMenuAction, filter_menu_items};
use crate::ui::{
    theme,
    widgets::{Canvas, clip},
};
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

struct Row {
    line: Line<'static>,
    selected: bool,
}

fn decoration(rows: &mut Vec<Row>, text: impl Into<String>, heading: bool) {
    rows.push(Row {
        line: Line::styled(
            text.into(),
            if heading {
                theme::FILTER_SECTION
            } else {
                theme::BORDER
            },
        ),
        selected: false,
    });
}

fn action_rows(app: &App, width: usize, selected: usize) -> Vec<Row> {
    use FilterMenuAction::*;
    let mut rows = Vec::new();
    let actions = filter_menu_items(app);
    let active = app
        .main
        .active_qf
        .and_then(|i| app.main.quick_filters.get(i));
    decoration(&mut rows, "CURRENT FILTERS", true);
    for (index, action) in actions.into_iter().enumerate() {
        if action == Preset(0) || (action == SavePreset && app.main.quick_filters.is_empty()) {
            decoration(&mut rows, "", false);
            decoration(&mut rows, "─".repeat(width), false);
            decoration(&mut rows, "SAVED PRESETS", true);
            if app.main.quick_filters.is_empty() {
                rows.push(Row {
                    line: Line::styled("   No saved presets", theme::FILTER_META),
                    selected: false,
                });
            }
        }
        if action == SavePreset {
            decoration(&mut rows, "", false);
            decoration(&mut rows, "─".repeat(width), false);
            decoration(&mut rows, "MANAGE", true);
        }
        let focused = index == selected;
        let marker = if focused {
            "›  "
        } else if matches!(action, Preset(i) if Some(i) == app.main.active_qf) {
            "●  "
        } else {
            "   "
        };
        let mut spans = vec![Span::styled(marker, theme::FILTER_ACCENT)];
        let field = match action {
            Label => Some(("Label", app.main.filter.label_text().to_owned())),
            Author => Some(("Author", app.main.filter.author_text().to_owned())),
            InactiveDays => Some(("Min. inactive days", app.main.filter.inactive_days_text())),
            InactivitySort => Some((
                "Inactivity sort",
                app.main.filter.inactivity_sort.label().into(),
            )),
            Drafts => Some((
                "Include drafts",
                if app.main.filter.show_drafts {
                    "[x]"
                } else {
                    "[ ]"
                }
                .into(),
            )),
            _ => None,
        };
        if let Some((label, value)) = field {
            if width >= 44 {
                spans.push(Span::raw(format!("{label:22}")));
            } else {
                spans.push(Span::raw(label));
                rows.push(Row {
                    line: Line::from(spans),
                    selected: focused,
                });
                spans = vec![Span::raw("     ")];
            }
            let used = spans.iter().map(Span::width).sum::<usize>();
            spans.push(Span::styled(
                clip(&value, width.saturating_sub(used), true),
                if focused {
                    theme::FILTER_ACCENT
                } else {
                    Style::default()
                },
            ));
        } else {
            match action {
                Preset(i) => {
                    let suffix = if Some(i) == app.main.active_qf {
                        "  (active)"
                    } else {
                        ""
                    };
                    spans.push(Span::raw(clip(
                        &app.main.quick_filters[i].name,
                        width.saturating_sub(3 + suffix.len()),
                        true,
                    )));
                    spans.push(Span::styled(suffix, theme::FILTER_ACCENT));
                }
                SavePreset => spans.push(Span::raw(if width >= 34 {
                    "Save current filter as preset…"
                } else {
                    "Save as preset…"
                })),
                SetDefault => spans.push(Span::raw(if width >= 34 {
                    "Use current filter as default"
                } else {
                    "Set as default"
                })),
                DeletePreset => {
                    let name = active
                        .map(|preset| preset.name.as_str())
                        .unwrap_or("none selected");
                    if width < 44 {
                        spans.push(Span::raw(if width >= 24 {
                            "Delete active preset…"
                        } else {
                            "Delete preset…"
                        }));
                        rows.push(Row {
                            line: Line::from(spans),
                            selected: focused,
                        });
                        spans = vec![Span::raw("     ")];
                    } else {
                        spans.push(Span::raw("Delete active preset… "));
                    }
                    let used = spans.iter().map(Span::width).sum::<usize>();
                    spans.push(Span::styled(
                        format!("({})", clip(name, width.saturating_sub(used + 2), true)),
                        theme::FILTER_META,
                    ));
                }
                _ => unreachable!("fields are rendered above"),
            }
        }
        rows.push(Row {
            line: Line::from(spans),
            selected: focused,
        });
    }
    rows
}

fn footer(action: FilterMenuAction, width: usize) -> Vec<Line<'static>> {
    use FilterMenuAction::*;
    let enter = match action {
        Label | Author | InactiveDays | InactivitySort => "Edit",
        Drafts => "Toggle",
        Preset(_) => "Choose",
        SavePreset => "Save",
        SetDefault => "Set default",
        DeletePreset => "Delete",
    };
    let mut hints = vec![("↑/↓ k/j", "Move")];
    hints.push(if action == Drafts {
        ("Space", "Toggle")
    } else {
        ("Tab", "Next")
    });
    hints.extend([("Enter", enter), ("Esc", "Close")]);
    if width < 24 {
        hints.drain(..2);
    }
    let hint_lines: Vec<_> = hints
        .into_iter()
        .map(|(key, label)| {
            Line::from(vec![
                Span::styled(format!(" {key} "), theme::FILTER_KEY),
                Span::styled(format!(" {label}"), theme::FILTER_META),
            ])
        })
        .collect();
    let total = hint_lines.iter().map(Line::width).sum::<usize>();
    let gap = if total + 6 <= width {
        (width - total) / 3
    } else {
        2
    };
    let mut lines = vec![Line::default()];
    for hint in hint_lines {
        let line = lines.last_mut().unwrap();
        if line.width() > 0 && line.width() + gap + hint.width() > width {
            lines.push(hint);
        } else {
            if line.width() > 0 {
                line.spans.push(Span::raw(" ".repeat(gap)));
            }
            line.spans.extend(hint.spans);
        }
    }
    lines
}

pub(in crate::ui) fn draw(f: &mut Frame, app: &mut App, size: Rect, selected: usize) {
    let actions = filter_menu_items(app);
    let selected = selected.min(actions.len() - 1);
    let width = 76.min(if size.width >= 24 {
        size.width - 4
    } else {
        size.width
    });
    let padding = if width >= 30 { 2 } else { 0 };
    let content_width = width.saturating_sub(2 + padding * 2);
    let rows = action_rows(app, content_width as usize, selected);
    let mut hints = footer(actions[selected], content_width as usize);
    let wanted_height = (rows.len() + hints.len() + 6).min(u16::MAX as usize) as u16;
    let height = wanted_height.min(if size.height >= 12 {
        size.height - 2
    } else {
        size.height
    });
    let area = Rect::new(
        size.x + (size.width - width) / 2,
        size.y + (size.height - height) / 2,
        width,
        height,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .style(theme::FILTER_SURFACE)
        .border_style(theme::BORDER_MODAL);
    let inner = block.inner(area).inner(Margin::new(padding, 0));
    f.render_widget(Canvas, area);
    f.render_widget(block, area);
    if inner.is_empty() {
        return;
    }
    f.render_widget(
        Paragraph::new("Filters").style(theme::FILTER_TITLE),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    let header_height = if inner.height >= 8 { 2 } else { 1 };
    let footer_spacing = if inner.height >= 8 { 2 } else { 0 };
    let hint_budget = inner
        .height
        .saturating_sub(header_height + footer_spacing + 1);
    hints.drain(..hints.len().saturating_sub(hint_budget as usize));
    let hint_height = hints.len() as u16;
    let footer_height = hint_height + footer_spacing;
    let body = Rect::new(
        inner.x,
        inner.y + header_height,
        inner.width,
        inner.height.saturating_sub(header_height + footer_height),
    );
    app.navigation.page_rows = body.height.into();
    let first_selected = rows.iter().position(|row| row.selected).unwrap_or(0);
    let last_selected = rows.iter().rposition(|row| row.selected).unwrap_or(0);
    let selected_row = if last_selected - first_selected < body.height as usize {
        last_selected
    } else {
        first_selected
    };
    let offset = selected_row
        .saturating_add(1)
        .saturating_sub(body.height as usize);
    for (index, row) in rows
        .iter()
        .skip(offset)
        .take(body.height as usize)
        .enumerate()
    {
        let row_area = Rect::new(body.x, body.y + index as u16, body.width, 1);
        let style = theme::FILTER_SURFACE.patch(if row.selected {
            theme::FILTER_SELECTED
        } else {
            Style::default()
        });
        let highlight = if row.selected && padding > 0 {
            Rect::new(row_area.x - 1, row_area.y, row_area.width + 2, 1)
        } else {
            row_area
        };
        f.render_widget(Block::default().style(style), highlight);
        f.render_widget(Paragraph::new(row.line.clone()).style(style), row_area);
    }
    if !body.is_empty() {
        for (visible, y, symbol) in [
            (offset > 0, body.y, "↑"),
            (
                offset + (body.height as usize) < rows.len(),
                body.bottom() - 1,
                "↓",
            ),
        ] {
            if visible {
                f.render_widget(
                    Paragraph::new(symbol).style(theme::FILTER_META),
                    Rect::new(area.right() - 1, y, 1, 1),
                );
            }
        }
    }
    if footer_height > hint_height {
        f.render_widget(
            Paragraph::new("─".repeat(inner.width as usize)).style(theme::BORDER),
            Rect::new(inner.x, inner.bottom() - hint_height - 1, inner.width, 1),
        );
    }
    for (index, hint) in hints.into_iter().take(hint_height as usize).enumerate() {
        f.render_widget(
            Paragraph::new(hint),
            Rect::new(
                inner.x,
                inner.bottom() - hint_height + index as u16,
                inner.width,
                1,
            ),
        );
    }
}
