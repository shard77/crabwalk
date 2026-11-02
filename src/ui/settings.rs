use crate::app::{App, SettingsSection};
use crate::ui::theme;
use crate::ui::widgets::panel;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
};

pub(super) fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    app.navigation.page_rows = area.height.saturating_sub(2).into();
    app.settings.label_input_rect = draw_fields(f, app, area);
}

fn draw_fields(f: &mut Frame, app: &App, area: Rect) -> Option<Rect> {
    let section = app.settings.selected_section;
    let selected = app.settings.selected_index();
    let block = panel(section.name());

    let lines: Vec<Line> = match section {
        SettingsSection::GitHub => return draw_github_fields(f, app, area, block, selected),
        SettingsSection::QuickFilters => {
            return draw_quick_filter_fields(f, app, area, block, selected);
        }
        SettingsSection::Display => display_fields(app, selected),
    };

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
    None
}

fn draw_github_fields(
    f: &mut Frame,
    app: &App,
    area: Rect,
    block: Block,
    selected: usize,
) -> Option<Rect> {
    let inner = block.inner(area);
    let mut fields_height = 0;
    let items = github_fields(app, selected)
        .into_iter()
        .map(|line| {
            let rows = wrap_field_line(line, inner.width);
            fields_height += rows.len() as u16;
            ListItem::new(rows)
        })
        .collect::<Vec<_>>();
    let error = app
        .settings
        .token()
        .and_then(|draft| draft.error.as_deref());
    let error_space = if error.is_some() {
        2.min(inner.height.saturating_sub(1))
    } else {
        0
    };
    fields_height = fields_height.min(inner.height - error_space);
    f.render_widget(block, area);
    let fields_area = Rect::new(inner.x, inner.y, inner.width, fields_height);
    let mut state = ListState::default();
    state.select(Some(selected));
    f.render_stateful_widget(List::new(items), fields_area, &mut state);

    if let Some(error) = error {
        let error_y = (inner.y + fields_height + 1).min(inner.bottom());
        f.render_widget(
            Paragraph::new(error)
                .style(theme::BORDER_ERROR)
                .wrap(Wrap { trim: false }),
            Rect::new(
                inner.x + 2.min(inner.width),
                error_y,
                inner.width.saturating_sub(2),
                inner.bottom() - error_y,
            ),
        );
    }
    None
}

fn draw_quick_filter_fields(
    f: &mut Frame,
    app: &App,
    area: Rect,
    block: Block,
    selected: usize,
) -> Option<Rect> {
    let inner = block.inner(area);
    let width = inner.width;
    let mut row_heights = Vec::new();
    let items = quick_filter_fields(app, selected)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let mut rows = wrap_field_line(line, width);
            if app.settings.quick_filter().is_none()
                && index > 0
                && index == app.main.quick_filters.len()
            {
                // Put the gap inside the add action so it cannot become a selectable blank row.
                rows.insert(0, Line::default());
            }
            row_heights.push(rows.len() as u16);
            ListItem::new(rows)
        })
        .collect::<Vec<_>>();
    let list = List::new(items).block(block);
    let mut state = ListState::default();
    state.select(Some(selected));
    f.render_stateful_widget(list, area, &mut state);
    if app.settings.quick_filter().is_some() && state.offset() <= 1 {
        let y = inner.y + row_heights[state.offset()..1].iter().sum::<u16>();
        let height = row_heights[1];
        if width > 0 && y + height <= inner.bottom() {
            return Some(Rect::new(inner.x, y, 8.min(width), height));
        }
    }
    None
}

// Wrap before building list items; Paragraph wrapping would break selection scrolling.
fn wrap_field_line(line: Line<'static>, width: u16) -> Vec<Line<'static>> {
    let mut rows = Vec::new();
    let mut row = Line::default().style(line.style);
    let width = usize::from(width.max(1));
    for span in &line.spans {
        for grapheme in span.styled_graphemes(line.style) {
            let glyph = Span::styled(grapheme.symbol.to_string(), grapheme.style);
            if row.width() + glyph.width() > width && !row.spans.is_empty() {
                rows.push(row);
                row = Line::default().style(line.style);
            }
            row.spans.push(glyph);
        }
    }
    rows.push(row);
    rows
}

fn field_line<'a>(
    label: &'a str,
    value: String,
    selected: bool,
    editing: bool,
    edit_buffer: &str,
) -> Line<'a> {
    let indicator = if selected { "▶ " } else { "  " };
    let indicator_style = if selected {
        theme::SECTION_HEADER_ACTIVE
    } else {
        Style::default()
    };

    let display_value = if selected && editing {
        format!("{edit_buffer}_")
    } else {
        value
    };

    let value_style = if selected && editing {
        theme::CHOICE_SELECTED
    } else if selected {
        theme::CONTEXT_VALUE
    } else {
        Style::default()
    };

    Line::from(vec![
        Span::styled(indicator, indicator_style),
        Span::styled(label.to_string(), theme::METADATA_LABEL),
        Span::styled(display_value, value_style),
    ])
}

fn github_fields(app: &App, selected: usize) -> Vec<Line<'static>> {
    if let Some(draft) = app.settings.token() {
        let input_focused = selected == 0 && app.context.focused.is_none();
        let masked = "•".repeat(draft.text.chars().count().min(24));
        return vec![
            field_line(
                "Token: ",
                masked.clone(),
                selected == 0,
                input_focused,
                &masked,
            ),
            field_line(
                "Save to system keyring: ",
                if draft.save_to_keyring { "[x]" } else { "[ ]" }.into(),
                selected == 1,
                false,
                "",
            ),
            field_line(
                "",
                if app.needs_token() {
                    "Connect"
                } else {
                    "Update token"
                }
                .into(),
                selected == 2,
                false,
                "",
            ),
            field_line("", "Cancel".into(), selected == 3, false, ""),
        ];
    }
    let token_display = if app.github.is_some() {
        "••••••••  (set)".to_string()
    } else {
        "(not set)".to_string()
    };

    vec![
        field_line("Token: ", token_display, selected == 0, false, ""),
        field_line(
            "Repository: ",
            "rust-lang/rust".to_string(),
            selected == 1,
            false,
            "",
        ),
    ]
}

fn quick_filter_fields(app: &App, selected: usize) -> Vec<Line<'static>> {
    if let Some(editor) = app.settings.quick_filter() {
        let draft = &editor.draft;
        let mut fields = vec![
            ("Name: ", draft.name.clone()),
            ("Label: ", draft.filter.label_text().to_owned()),
            ("Author: ", draft.filter.author_text().to_owned()),
            ("Inactive days: ", draft.filter.inactive_days_text()),
            (
                "Inactivity sort: ",
                draft.filter.inactivity_sort.label().into(),
            ),
            (
                "Include drafts: ",
                if draft.filter.show_drafts {
                    "[x]"
                } else {
                    "[ ]"
                }
                .into(),
            ),
            ("", "Save quick filter".into()),
            ("", "Cancel".into()),
        ];
        if editor.source_index.is_some() {
            fields.push(("", "Remove quick filter".into()));
        }
        return fields
            .into_iter()
            .enumerate()
            .map(|(index, (label, value))| {
                field_line(
                    label,
                    value,
                    selected == index,
                    editor.input.is_some() && index < 4,
                    editor.input.as_deref().unwrap_or_default(),
                )
            })
            .collect();
    }

    let mut lines: Vec<_> = app
        .main
        .quick_filters
        .iter()
        .enumerate()
        .map(|(i, qf)| {
            let indicator = if i == selected { "▶ " } else { "  " };
            let style = if i == selected {
                theme::SELECTED_ITEM
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::styled(
                    indicator.to_string(),
                    if i == selected {
                        theme::SECTION_HEADER_ACTIVE
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(format!("[{}] ", qf.name), style),
            ])
        })
        .collect();
    lines.push(field_line(
        "",
        "Add quick filter".into(),
        selected == app.main.quick_filters.len(),
        false,
        "",
    ));
    lines
}

fn display_fields(app: &App, selected: usize) -> Vec<Line<'static>> {
    vec![field_line(
        "Nerd Font icons: ",
        if app.main.nerd_font {
            "[x] Enabled".into()
        } else {
            "[ ] Disabled".into()
        },
        selected == 0,
        false,
        "",
    )]
}
