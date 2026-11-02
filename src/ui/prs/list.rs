use crate::app::App;
use crate::types::{PrSummary, TriageLabel};
use crate::ui::{
    theme,
    widgets::{clip, panel},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};

pub(super) fn draw(f: &mut Frame, app: &App, area: Rect) {
    let prs = &app.main.prs;
    let selected = app.main.selected;
    let loading_hint = if app.background.is_loading_prs() {
        if prs.is_empty() {
            " (loading…)".to_string()
        } else {
            format!(" ({} loaded…)", prs.len())
        }
    } else {
        String::new()
    };
    let count = prs.len();
    let pos = if count > 0 {
        format!("[{}/{}]", selected + 1, count)
    } else {
        String::new()
    };
    let rows = usize::from(area.height.saturating_sub(2));
    // Match the existing list viewport: keep the selected row visible at the bottom
    // once it moves beyond the first screen. Each PR occupies exactly one row.
    let start = selected.saturating_add(1).saturating_sub(rows);
    let items: Vec<ListItem> = prs
        .iter()
        .enumerate()
        .skip(start)
        .take(rows)
        .map(|(i, pr)| item(pr, i == selected))
        .collect();
    let title = format!("PRs ({count}){pos}{loading_hint}");
    let list = List::new(items).block(panel(&title));
    let mut state = ListState::default();
    state.select(if count > 0 {
        Some(selected.saturating_sub(start))
    } else {
        None
    });
    f.render_stateful_widget(list, area, &mut state);
}

fn item(pr: &PrSummary, selected: bool) -> ListItem<'static> {
    let updated = pr.updated_days_ago();
    let threshold = TriageLabel::threshold_days(pr.triage_label().as_ref()) as i64;
    let base_style = if selected {
        theme::SELECTED_ITEM
    } else {
        Style::default()
    };
    let draft_span = if pr.draft {
        Span::styled("D ", theme::DRAFT_MARKER.patch(base_style))
    } else {
        Span::styled("  ", base_style)
    };
    let upd_style = if updated >= 2 * threshold {
        theme::INACTIVE_OVERDUE
    } else if updated >= threshold {
        theme::INACTIVE_STALE
    } else {
        theme::INACTIVE_RECENT
    };
    let sep = Span::styled(" │ ", theme::MUTED_TEXT.patch(base_style));
    let spans = vec![
        Span::styled(
            format!("#{:<6}", pr.number),
            theme::PR_NUMBER.patch(base_style),
        ),
        Span::raw(" "),
        draft_span,
        Span::styled(format!("{:>3}d", updated), upd_style.patch(base_style)),
        sep.clone(),
        Span::styled(
            format!("@{:<15}", clip(&pr.author, 15, false)),
            theme::AUTHOR.patch(base_style),
        ),
        sep,
        Span::styled(clip(&pr.title, 40, false), base_style),
    ];
    ListItem::new(Line::from(spans)).style(base_style)
}
