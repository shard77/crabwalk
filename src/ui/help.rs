use crate::app::{App, View};
use crate::ui::theme;
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

type Binding = (&'static str, &'static str);
type Group = (&'static str, &'static str, Vec<Binding>);

fn groups(app: &App) -> (&'static str, Vec<Group>) {
    let reader = app.navigation.view == View::Main && app.main.detail.fullscreen;
    let context = if app.context.focused.is_some() {
        "Context controls"
    } else if reader {
        "PR detail"
    } else {
        match app.navigation.view {
            View::Main => "Pull requests",
            View::Report => "Report",
            View::Settings => "Settings",
        }
    };
    let mut groups: Vec<Group> = vec![(
        "Navigate",
        "\u{f14e}",
        vec![
            ("Up / Down or k / j", "Move selection or scroll document"),
            ("Left / Right or h / l", "Move between controls"),
            ("Home / gg, End / G", "First / last"),
            ("PgUp / PgDn", "Page up / down"),
            ("Ctrl+U / Ctrl+D", "Half page up / down"),
            ("Tab / Shift+Tab", "Next / previous local control"),
            ("Enter", "Activate selected item"),
            ("Space", "Toggle checkbox or expand / collapse"),
            ("Esc", "Unfocus, back or cancel"),
        ],
    )];
    if reader && app.context.focused.is_none() && !app.is_reader_editing() {
        groups[0].2 = vec![
            ("Down / Up or j / k", "Next / previous highlighted item"),
            ("Tab / Shift+Tab", "Scroll down / up one line"),
            ("PgUp / PgDn", "Jump selection by a page of items"),
            ("Ctrl+U / Ctrl+D", "Jump selection by half a page of items"),
            ("Home / gg, End / G", "First / last item"),
            ("Enter", "Activate selected item"),
            ("Space", "Expand / collapse"),
            ("Esc", "Unfocus or back"),
        ];
    }
    match app.navigation.view {
        View::Main => {
            groups.push((
                "Actions",
                "\u{f0e7}",
                vec![
                    ("f", "Filters, presets, save / default / delete"),
                    ("r", "Refresh"),
                    ("o", "Open PR in browser"),
                    ("a", "Add to report"),
                    ("e", "Add or edit report note"),
                ],
            ));
            if reader {
                groups.push((
                    "Reader",
                    "\u{f086}",
                    vec![
                        ("/ or Ctrl+F", "Search selected section"),
                        ("n / N", "Next / previous match"),
                        ("F3 / Shift+F3", "Next / previous match"),
                        ("Enter / Esc", "Confirm / cancel search"),
                        ("Left / Right or h / l", "Collapse / enter section"),
                    ],
                ));
                groups.push((
                    "Select / Copy",
                    "\u{f0c5}",
                    vec![
                        ("v / V", "Select characters / lines"),
                        ("Arrows or h j k l", "Extend selection"),
                        ("w / b / e", "Next word / previous word / word end"),
                        ("0 / ^ / $", "Line start / first nonblank / line end"),
                        ("gg / G", "First / last line while selecting"),
                        ("Home / End", "Line start / end while selecting"),
                        ("c / y / Ctrl+C", "Copy selection or current line"),
                        ("Enter", "Open focused link"),
                        ("[ / ]", "Previous / next code block"),
                        ("Esc", "Cancel selection"),
                    ],
                ));
            }
        }
        View::Report => groups.push((
            "Entries",
            "\u{f03a}",
            vec![
                ("e", "Edit report note"),
                ("x / Delete", "Remove entry (confirmation)"),
                ("c / y", "Copy report to clipboard"),
                ("Tab, Enter", "Choose export format in context control"),
            ],
        )),
        View::Settings => groups.push((
            "Preferences",
            "\u{f013}",
            vec![
                (
                    "Tab / Shift+Tab / Left / h",
                    "Focus section context control (outside form)",
                ),
                ("Up at first field", "Focus section context control"),
                ("Enter on section", "Open section dropdown"),
                ("Enter", "Edit preset or activate selected field"),
                ("Type", "Edit focused form text input"),
                ("Tab / Shift+Tab", "Next / previous form field"),
                ("Up / Down", "Move between form fields"),
                ("Delete / x", "Remove selected quick filter (confirmation)"),
                ("Space", "Toggle focused preference"),
                (
                    "Esc",
                    "Cancel form; otherwise leave Settings or unfocus context",
                ),
            ],
        )),
    }
    groups.push((
        "Global",
        "\u{f0ac}",
        vec![
            ("1 / 2 / 3", "PRs / Report / Settings"),
            ("F1 / ?", "Keyboard reference"),
            ("q", "Quit (normal navigation only)"),
        ],
    ));
    (context, groups)
}

fn lines(groups: &[Group], width: usize, nerd_font: bool) -> Vec<Line<'static>> {
    let width = width.max(1);
    let columns = columns(width).min(groups.len().max(1));
    let gap = 2;
    let cell_width = (width - (columns - 1) * gap) / columns;
    let mut content = Vec::new();
    for row in groups.chunks(columns) {
        if !content.is_empty() {
            content.push(Line::default());
            content.push(Line::default());
        }
        let cells: Vec<_> = row
            .iter()
            .map(|group| group_lines(group, cell_width, nerd_font))
            .collect();
        let height = cells.iter().map(Vec::len).max().unwrap_or(0);
        for index in 0..height {
            let mut spans = Vec::new();
            for (column, cell) in cells.iter().enumerate() {
                let line = cell.get(index).cloned().unwrap_or_default();
                let padding = cell_width.saturating_sub(line.width());
                spans.extend(line.spans);
                if column + 1 < cells.len() {
                    spans.push(Span::raw(" ".repeat(padding + gap)));
                }
            }
            content.push(Line::from(spans));
        }
    }
    content
}

fn columns(width: usize) -> usize {
    if width >= 108 {
        4
    } else if width >= 58 {
        2
    } else {
        1
    }
}

fn group_lines(
    (heading, icon, bindings): &Group,
    width: usize,
    nerd_font: bool,
) -> Vec<Line<'static>> {
    let heading = if nerd_font {
        format!("{icon} {}", heading.to_uppercase())
    } else {
        heading.to_uppercase()
    };
    let mut lines: Vec<_> = wrap(&heading, width)
        .into_iter()
        .map(|heading| Line::from(Span::styled(heading, theme::COMMENT_META)))
        .collect();
    let key_width = (bindings.iter().map(|(key, _)| key.len()).max().unwrap_or(0) + 1).max(9);
    let key_width = if key_width + 8 <= width { key_width } else { 0 };
    for &(key, description) in bindings {
        if key_width == 0 {
            for chunk in wrap(key, width) {
                lines.push(Line::from(Span::styled(chunk, theme::COMMENT_AUTHOR)));
            }
        }
        for (index, chunk) in wrap(description, width.saturating_sub(key_width).max(1))
            .into_iter()
            .enumerate()
        {
            let mut spans = Vec::new();
            if key_width > 0 {
                spans.push(Span::styled(
                    if index == 0 {
                        format!("{key:<key_width$}")
                    } else {
                        " ".repeat(key_width)
                    },
                    theme::COMMENT_AUTHOR,
                ));
            }
            spans.push(Span::raw(chunk));
            lines.push(Line::from(spans));
        }
    }
    lines
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut line = String::new();
    let mut line_width = 0;
    for word in text.split_whitespace() {
        if !line.is_empty() && line_width + 1 + Span::raw(word).width() > width {
            result.push(std::mem::take(&mut line));
            line_width = 0;
        }
        if !line.is_empty() {
            line.push(' ');
            line_width += 1;
        }
        // Icons can occupy more than one terminal cell; byte lengths give the wrong wrap width.
        for ch in word.chars() {
            let ch_width = Span::raw(ch.to_string()).width();
            if !line.is_empty() && line_width + ch_width > width {
                result.push(std::mem::take(&mut line));
                line_width = 0;
            }
            line.push(ch);
            line_width += ch_width;
        }
    }
    if !line.is_empty() {
        result.push(line);
    }
    result
}

fn drawer_area(size: Rect, content_height: usize) -> Rect {
    let inner = size.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    if inner.is_empty() {
        return Rect::new(size.x, size.y, 0, 0);
    }
    let available = inner.height.saturating_sub(1);
    let height = (content_height.saturating_add(6).min(u16::MAX as usize) as u16)
        .min((size.height / 2).max(6))
        .min(available);
    Rect::new(
        inner.x,
        inner.bottom().saturating_sub(1 + height).max(inner.y),
        inner.width,
        height,
    )
}

pub(super) fn draw(f: &mut Frame, app: &mut App, size: Rect) {
    let (context, groups) = groups(app);
    let content = lines(
        &groups,
        size.width.saturating_sub(6) as usize,
        app.main.nerd_font,
    );
    let area = drawer_area(size, content.len());
    if area.is_empty() {
        app.help.scroll = 0;
        return;
    }
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(theme::BORDER_MODAL)
        .style(theme::COMMENT_BODY_BACKGROUND);
    f.render_widget(block, area);
    let inner = area.inner(Margin {
        horizontal: 2,
        vertical: 0,
    });
    if inner.is_empty() || area.height < 2 {
        app.help.scroll = 0;
        return;
    }
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Keybinds", theme::COMMENT_AUTHOR),
            Span::styled(format!(" / {context}"), theme::COMMENT_META),
        ])),
        Rect::new(inner.x, area.y + 1, inner.width, 1),
    );
    let body = Rect::new(
        inner.x,
        area.y + 3.min(area.height),
        inner.width,
        area.height.saturating_sub(5),
    );
    let max_scroll = (content.len() as u16).saturating_sub(body.height);
    app.help.viewport_height = body.height;
    app.help.scroll = app.help.scroll.min(max_scroll);
    f.render_widget(
        Paragraph::new(content)
            .style(theme::COMMENT_BODY_BACKGROUND)
            .scroll((app.help.scroll, 0)),
        body,
    );
    if area.height >= 3 {
        let scroll_keys = "↑↓ / j/k";
        let hint = if max_scroll == 0 {
            "Esc / ? Close".to_string()
        } else if inner.width < 40 {
            format!("Esc / ? Close  {scroll_keys} Scroll")
        } else {
            format!(
                "Esc / ? Close  ·  {scroll_keys} Scroll  {}/{}",
                app.help.scroll + 1,
                max_scroll + 1
            )
        };
        f.render_widget(
            Paragraph::new(hint).style(theme::COMMENT_META),
            Rect::new(inner.x, area.bottom() - 1, inner.width, 1),
        );
    }
}
