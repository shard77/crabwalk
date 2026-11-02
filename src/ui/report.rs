use crate::app::App;
use crate::app::report::{format_report, report_summary};
use crate::types::TriageLabel;
use crate::ui::theme;
use crate::ui::widgets::master_detail_split;
use crate::ui::widgets::panel;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{List, ListItem, ListState, Paragraph, Wrap},
};

pub(super) fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let (list_area, preview_area) = master_detail_split(area, (35, 65));
    let inner_rows = usize::from(list_area.height.saturating_sub(2));
    app.navigation.page_rows = (inner_rows / ENTRY_ROWS).max(1);
    draw_entry_list(f, app, list_area);
    draw_preview(f, app, preview_area);
}

const ENTRY_ROWS: usize = 2;

fn draw_entry_list(f: &mut Frame, app: &App, area: Rect) {
    let entries = &app.report.entries;
    let selected = app.report.selected;

    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let reviewer = entry
                .reviewer
                .as_deref()
                .map(|r| format!("/@{r}"))
                .unwrap_or_default();
            let line = format!(
                "#{} ({}d) @{}{} [{}]",
                entry.pr_number,
                entry.inactive_days,
                entry.author,
                reviewer,
                entry
                    .status_label
                    .as_ref()
                    .map_or("no status", TriageLabel::as_str),
            );
            let title = Line::styled(format!("  {}", entry.title), theme::MUTED_TEXT);
            let style = if i == selected {
                theme::SELECTED_ITEM
            } else {
                Style::default()
            };
            ListItem::new(vec![Line::from(line), title]).style(style)
        })
        .collect();

    let count = entries.len();
    let title = format!("Report Entries ({count})");

    let list = List::new(items).block(panel(&title));

    let mut state = ListState::default();
    state.select(if count > 0 { Some(selected) } else { None });

    f.render_stateful_widget(list, area, &mut state);
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    let fmt = &app.report.format;
    let entries = &app.report.entries;

    let content: Text = if entries.is_empty() {
        Text::from(
            "No entries yet.\n\nSelect a PR in PRs (1) and press `a` to add it, or `e` to add a note.",
        )
    } else {
        let summary = report_summary(entries);

        let stale_style = if summary.stale > 0 {
            theme::INACTIVE_OVERDUE
        } else {
            theme::STALE_NONE
        };

        let mut summary_line1 = vec![
            Span::styled("PRs: ", theme::METADATA_LABEL),
            Span::styled(format!("{}", summary.total), theme::PR_NUMBER),
            Span::styled("  │", theme::MUTED_TEXT),
        ];
        for (label, count) in TriageLabel::PRIMARY.iter().zip(summary.primary) {
            summary_line1.push(Span::styled(
                format!("  {}: ", label.short_name()),
                theme::METADATA_LABEL,
            ));
            summary_line1.push(Span::styled(format!("{count}"), theme::PR_NUMBER));
        }
        summary_line1.extend([
            Span::styled("  │  ", theme::MUTED_TEXT),
            Span::styled("Stale: ", theme::METADATA_LABEL),
            Span::styled(format!("{}", summary.stale), stale_style),
        ]);

        let summary_line2 = Line::from(vec![
            Span::styled("Reviewers: ", theme::METADATA_LABEL),
            Span::styled(format!("{}", summary.reviewers), theme::PR_NUMBER),
            Span::styled("  │  ", theme::MUTED_TEXT),
            Span::styled("Avg inactive: ", theme::METADATA_LABEL),
            Span::styled(format!("{}d", summary.avg_inactive_days), theme::PR_NUMBER),
        ]);

        let body = format_report(entries, fmt);

        let mut lines = vec![Line::from(summary_line1), summary_line2, Line::from("")];
        for line in body.lines() {
            lines.push(Line::from(line.to_string()));
        }
        Text::from(lines)
    };

    let title = format!("Preview [{}]", fmt.label());

    let para = Paragraph::new(content)
        .block(panel(&title))
        .wrap(Wrap { trim: false });

    f.render_widget(para, area);
}
