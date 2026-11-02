use crate::app::{App, ContextAction, View};
use crate::ui::theme;
use ratatui::text::{Line, Span};

/// Returns column ranges for keyboard focus and dropdown placement, with exclusive ends.
pub(super) fn build(app: &App) -> (Line<'_>, Vec<(u16, u16, ContextAction)>) {
    let mut entries: Vec<(Span<'_>, Option<ContextAction>)> = Vec::new();

    match app.navigation.view {
        View::Main => {
            let label_val = app.main.filter.label_text();
            let author_val = app.main.filter.author_text();
            let inactive_val = app.main.filter.inactive_days_text();
            let drafts_val = if app.main.filter.show_drafts {
                "on"
            } else {
                "off"
            };

            entries.extend([
                (
                    Span::styled(" filter: ", theme::CONTEXT_LINE),
                    Some(ContextAction::FilterLabel),
                ),
                (
                    Span::styled(label_val.to_string(), theme::CONTEXT_VALUE),
                    Some(ContextAction::FilterLabel),
                ),
                (Span::styled(" | ", theme::CONTEXT_LINE), None),
                (
                    Span::styled("author: ", theme::CONTEXT_LINE),
                    Some(ContextAction::FilterAuthor),
                ),
                (
                    Span::styled(author_val.to_string(), theme::CONTEXT_VALUE),
                    Some(ContextAction::FilterAuthor),
                ),
                (Span::styled(" | ", theme::CONTEXT_LINE), None),
                (
                    Span::styled("inactive: ", theme::CONTEXT_LINE),
                    Some(ContextAction::FilterInactiveDays),
                ),
                (
                    Span::styled(inactive_val, theme::CONTEXT_VALUE),
                    Some(ContextAction::FilterInactiveDays),
                ),
                (Span::styled(" | ", theme::CONTEXT_LINE), None),
                (
                    Span::styled("sort: ", theme::CONTEXT_LINE),
                    Some(ContextAction::FilterInactivitySort),
                ),
                (
                    Span::styled(
                        app.main.filter.inactivity_sort.label(),
                        theme::CONTEXT_VALUE,
                    ),
                    Some(ContextAction::FilterInactivitySort),
                ),
                (Span::styled(" | ", theme::CONTEXT_LINE), None),
                (
                    Span::styled("drafts: ", theme::CONTEXT_LINE),
                    Some(ContextAction::ToggleDrafts),
                ),
                (
                    Span::styled(drafts_val.to_string(), theme::CONTEXT_VALUE),
                    Some(ContextAction::ToggleDrafts),
                ),
            ]);
        }
        View::Report => {
            let count = app.report.entries.len();
            let fmt = app.report.format.label();
            entries.extend([
                (Span::styled(" ", theme::CONTEXT_LINE), None),
                (
                    Span::styled(format!("{count} entries"), theme::CONTEXT_VALUE),
                    None,
                ),
                (Span::styled(" | ", theme::CONTEXT_LINE), None),
                (
                    Span::styled("format: ", theme::CONTEXT_LINE),
                    Some(ContextAction::CycleReportFormat),
                ),
                (
                    Span::styled(fmt.to_string(), theme::CONTEXT_VALUE),
                    Some(ContextAction::CycleReportFormat),
                ),
            ]);
        }
        View::Settings => {
            entries.extend([
                (
                    Span::styled(" section: ", theme::CONTEXT_LINE),
                    Some(ContextAction::SettingsSection),
                ),
                (
                    Span::styled(app.settings.selected_section.name(), theme::CONTEXT_VALUE),
                    Some(ContextAction::SettingsSection),
                ),
            ]);
        }
    }

    let mut regions = Vec::new();
    let mut span_region_indices: Vec<Option<usize>> = Vec::new();
    let mut col: u16 = 0;
    for (span, action) in &entries {
        let width = span.width().min(u16::MAX as usize) as u16;
        if let Some(act) = action {
            if let Some(last) = regions.last_mut() {
                let (_, ref mut end, ref last_act): (u16, u16, ContextAction) = *last;
                if last_act == act && *end == col {
                    *end = col + width;
                    span_region_indices.push(Some(regions.len() - 1));
                    col += width;
                    continue;
                }
            }
            span_region_indices.push(Some(regions.len()));
            regions.push((col, col + width, *act));
        } else {
            span_region_indices.push(None);
        }
        col += width;
    }

    if let Some(focus_index) = app.context.focused
        && focus_index < regions.len()
    {
        for ((span, _), region_index) in entries.iter_mut().zip(span_region_indices.iter()) {
            if *region_index == Some(focus_index) {
                span.style = theme::CHOICE_SELECTED;
            }
        }
    }
    (
        Line::from(
            entries
                .into_iter()
                .map(|(span, _)| span)
                .collect::<Vec<_>>(),
        ),
        regions,
    )
}
