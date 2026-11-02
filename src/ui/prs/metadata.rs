use super::text::wrap_conversation_line;
use crate::{types::PrSummary, ui::theme};
use ratatui::text::{Line, Span};

pub(super) fn lines(pr: &PrSummary, content_width: u16) -> Vec<Line<'static>> {
    let mut lines = wrap_conversation_line(
        Line::from(vec![
            Span::styled(format!("#{} ", pr.number), theme::PR_NUMBER),
            Span::styled(pr.title.clone(), theme::PR_TITLE),
        ]),
        content_width,
    );
    if pr.draft {
        let badge = Span::styled(" DRAFT ", theme::PR_METADATA_CHIP);
        if let Some(last) = lines.last_mut()
            && last.width() + 2 + badge.width() <= usize::from(content_width)
        {
            last.spans.push(Span::raw("  "));
            last.spans.push(badge);
        } else {
            lines.extend(wrap_conversation_line(Line::from(badge), content_width));
        }
    }
    lines.extend(wrap_conversation_line(
        Line::from(vec![
            Span::styled("by ", theme::METADATA_LABEL),
            Span::styled(format!("@{}", pr.author), theme::AUTHOR),
            Span::styled(
                format!(" · updated {}d ago", pr.updated_days_ago()),
                theme::METADATA_LABEL,
            ),
        ]),
        content_width,
    ));
    lines.push(Line::from(""));
    let people = |values: &[String]| {
        values
            .iter()
            .map(|value| format!("@{value}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for (label, values, chips) in [
        (
            "Requested reviews",
            vec![people(&pr.requested_reviewers)],
            false,
        ),
        ("Milestone", pr.milestone.iter().cloned().collect(), true),
        ("Assigned", vec![people(&pr.assignees)], false),
        ("Labels", pr.labels.clone(), true),
    ] {
        let values: Vec<_> = values
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect();
        let stacked = content_width < 30;
        let indent = if stacked {
            content_width.saturating_sub(1).min(2)
        } else {
            19
        };
        let value_width = content_width.saturating_sub(indent).max(1);
        if stacked {
            lines.extend(wrap_conversation_line(
                Line::styled(format!("{label}:"), theme::METADATA_LABEL),
                content_width,
            ));
        }
        let wrapped = if values.is_empty() {
            vec![Line::from("—")]
        } else if chips {
            chip_lines(&values, value_width)
        } else {
            wrap_conversation_line(Line::from(values.join(", ")), value_width)
        };
        for (index, mut line) in wrapped.into_iter().enumerate() {
            let prefix = if index == 0 && !stacked {
                format!("{label:<17}  ")
            } else {
                " ".repeat(usize::from(indent))
            };
            line.spans
                .insert(0, Span::styled(prefix, theme::METADATA_LABEL));
            lines.push(line);
        }
    }
    lines.push(Line::from(""));
    lines
}

fn chip_lines(values: &[String], width: u16) -> Vec<Line<'static>> {
    let padding = usize::from(width > 2);
    let mut lines = Vec::new();
    let mut row = Line::default();
    for value in values {
        for part in wrap_conversation_line(
            Line::from(value.clone()),
            width.saturating_sub((padding * 2) as u16).max(1),
        ) {
            let text: String = part
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            let chip = Span::styled(
                format!("{pad}{text}{pad}", pad = " ".repeat(padding)),
                theme::PR_METADATA_CHIP,
            );
            if !row.spans.is_empty() && row.width() + 1 + chip.width() > usize::from(width) {
                lines.push(row);
                row = Line::default();
            }
            if !row.spans.is_empty() {
                row.spans.push(Span::raw(" "));
            }
            row.spans.push(chip);
        }
    }
    if !row.spans.is_empty() {
        lines.push(row);
    }
    lines
}
