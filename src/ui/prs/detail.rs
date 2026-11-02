use super::{
    conversation, metadata, reviews,
    text::{
        append_inline_focus, conversation_header, finish_conversation_body,
        style_conversation_line, wrap_conversation_line,
    },
};
use crate::{
    app::{App, DetailState, FullscreenCache},
    types::{CommentKind, PrSummary},
    ui::{theme, widgets::panel},
};
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};
use std::collections::{BTreeSet, HashSet};
use std::fmt::Write;

struct DetailRenderContext<'a> {
    detail: &'a Option<crate::types::ItemDetail>,
    highlights: &'a crate::markdown::HighlightCache,
    labels: Option<reviews::ReviewLabels<'a>>,
}

// Loading hints go outside the cached body so progress stays current without rebuilding it.
pub(super) fn draw_preview(f: &mut Frame, app: &mut App, area: Rect) {
    let block = panel("Detail");
    let Some(pr) = app.main.selected_pr() else {
        let p = Paragraph::new(
            if app.main.prs.is_empty() && !app.background.is_loading_prs() {
                "No PRs loaded. Press r to refresh or f to set a filter."
            } else if app.background.is_loading_prs() {
                "Loading…"
            } else {
                "No PR selected."
            },
        )
        .block(block);
        f.render_widget(p, area);
        return;
    };
    let width = area.width.saturating_sub(2);
    let height = area.height.saturating_sub(2);
    if !app.main.detail.compact_cache.as_ref().is_some_and(|cache| {
        cache.pr_number == pr.number
            && cache.content_width == width
            && cache.content_height == height
            && cache.nerd_font == app.main.nerd_font
    }) {
        app.main.detail.compact_cache = Some(crate::app::CompactDetailCache {
            pr_number: pr.number,
            content_width: width,
            content_height: height,
            nerd_font: app.main.nerd_font,
            lines: build_preview_lines(
                pr,
                DetailRenderContext {
                    detail: app.main.detail.snapshot(),
                    highlights: &app.highlights,
                    labels: app
                        .main
                        .detail
                        .review_threads()
                        .map(reviews::ReviewLabels::from_threads),
                },
                width,
                height,
                app.main.nerd_font,
            ),
        });
    }
    let mut lines = app
        .main
        .detail
        .compact_cache
        .as_ref()
        .unwrap()
        .lines
        .clone();
    if app.main.detail.snapshot().is_none() {
        lines.push(Line::from(""));
        if app.background.detail_progress().is_some() {
            lines.push(Line::from(vec![Span::styled(
                "Loading…",
                theme::PLACEHOLDER,
            )]));
        } else {
            lines.push(Line::from(vec![Span::styled(
                "Press Enter to open full detail view",
                theme::PLACEHOLDER,
            )]));
        }
    } else if let Some((_, n)) = app.background.detail_progress() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!("Loading activity… ({n} so far)"),
            theme::PLACEHOLDER,
        )]));
    } else if app
        .main
        .detail
        .snapshot()
        .as_ref()
        .is_some_and(|d| d.comments.is_empty())
    {
        lines.push(Line::styled("No activity yet.", theme::PLACEHOLDER));
    }
    let para = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn build_preview_lines(
    pr: &PrSummary,
    context: DetailRenderContext<'_>,
    content_width: u16,
    content_height: u16,
    nerd_font: bool,
) -> Vec<Line<'static>> {
    let DetailRenderContext {
        detail,
        labels,
        highlights,
    } = context;
    let mut lines = metadata::lines(pr, content_width);
    if let Some(detail) = detail {
        let description_start = lines.len();
        if let Some(body) = &detail.body
            && !body.trim().is_empty()
        {
            let doc = crate::markdown::render_once(body, &HashSet::new(), highlights);
            let preview_rows = conversation::code_preview_rows(&doc, content_width, 20);
            for line in doc
                .lines
                .into_iter()
                .flat_map(|line| wrap_conversation_line(line, content_width))
                .take(preview_rows)
            {
                lines.push(line);
            }
        }
        finish_conversation_body(&mut lines, description_start, content_width);
        if lines.len() == description_start {
            lines.push(Line::styled("(no description)", theme::PLACEHOLDER));
        }
        lines.push(Line::from(""));
        for mut line in conversation_header(
            vec![Span::styled("Latest activity", theme::PR_TITLE)],
            vec![Span::styled(
                "Press <Enter> for full discussion",
                theme::COMMENT_META,
            )],
            content_width,
        ) {
            style_conversation_line(
                &mut line,
                content_width,
                theme::COMMENT_HEADER_BACKGROUND,
                false,
            );
            lines.push(line);
        }
        if !detail.comments.is_empty() {
            let labels = labels.expect("detail has conversation metadata");
            let visibility = labels.threads.default_visibility();
            let conversation = conversation::Conversation {
                comments: &detail.comments,
                highlights,
                pr_author: &pr.author,
                width: content_width,
                labels,
                nerd_font,
                compact: true,
                visibility,
                expansion: &crate::app::DetailExpansion::default(),
            };
            let remaining = usize::from(content_height).saturating_sub(lines.len());
            lines.extend(preview_activity_lines(&conversation, remaining));
        }
    }
    lines
}

fn preview_activity_lines(
    conversation: &conversation::Conversation<'_>,
    height: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if height == 0 {
        return lines;
    }
    let mut selected = BTreeSet::new();
    for context in conversation.labels.recent_entries(conversation.comments) {
        let previous_len = selected.len();
        selected.extend(context.map(|index| conversation.visibility.representative(index)));
        if selected.len() == previous_len {
            continue;
        }
        let indices: Vec<_> = selected.iter().copied().collect();
        let mut candidate = Vec::new();
        for (position, index) in indices.iter().copied().enumerate() {
            candidate
                .extend(conversation.separator(position.checked_sub(1).map(|p| indices[p]), index));
            let end_group = indices.get(position + 1).is_none_or(|&next| {
                !conversation
                    .labels
                    .threads
                    .is_same_review(&conversation.comments[index], &conversation.comments[next])
            });
            candidate.extend(conversation.entry(index, false, None, 5, end_group).lines);
        }
        if candidate.len() > height {
            if lines.is_empty() {
                // Show part of the newest entry if it cannot fit on its own.
                candidate.truncate(height);
                lines = candidate;
            }
            break;
        }
        lines = candidate;
    }
    lines
}

pub(in crate::ui) fn fullscreen_detail_context(app: &App) -> Option<String> {
    if app.navigation.view != crate::app::View::Main || !app.main.detail.fullscreen {
        return None;
    }
    let pr = app.main.selected_pr()?;
    let section = app.main.detail.section;
    let section_label = if section == 0 {
        "Description"
    } else {
        match app
            .main
            .detail
            .snapshot()
            .as_ref()
            .and_then(|detail| detail.comments.get(section - 1))
            .map(|comment| &comment.kind)
        {
            Some(CommentKind::Review { .. }) => "Review",
            Some(CommentKind::Activity { .. }) => "Activity",
            Some(CommentKind::ReviewComment(context)) if context.reply_to.is_some() => {
                "Review reply"
            }
            Some(CommentKind::ReviewComment(_)) => "Review comment",
            _ => "Comment",
        }
    };
    let selection_context = app.main.detail.snapshot().as_ref().and_then(|detail| {
        let comment = detail.comments.get(section.checked_sub(1)?)?;
        let labels = reviews::ReviewLabels::from_threads(app.main.detail.review_threads()?);
        match &comment.kind {
            CommentKind::Review { .. } => Some(labels.heading(comment)),
            CommentKind::ReviewComment(_) => labels.relationship(comment),
            CommentKind::Issue { .. } | CommentKind::Activity { .. } => None,
        }
    });
    let mut status = format!(
        "#{} · {}",
        pr.number,
        selection_context.as_deref().unwrap_or(section_label),
    );
    if let Some(focus) = &app.main.detail.reader {
        use crate::app::ReaderMode;
        match focus.mode {
            ReaderMode::Search => write!(
                status,
                " · Find in section: {}  Enter search · Esc cancel",
                focus.search_query
            )
            .expect("writing to a String cannot fail"),
            ReaderMode::Visual | ReaderMode::VisualLine => {
                status.push_str(" · Select text · y copy · Esc cancel")
            }
            ReaderMode::Normal if !focus.search_query.is_empty() => {
                if focus.search_matches.is_empty() {
                    status.push_str(" · No matches");
                } else {
                    write!(
                        status,
                        " · Match {}/{} · F3 next · Shift+F3 previous",
                        focus.search_match_index + 1,
                        focus.search_matches.len()
                    )
                    .expect("writing to a String cannot fail");
                }
            }
            _ => {}
        }
    }
    Some(status)
}

pub(super) fn draw_fullscreen(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(pr) = app.main.selected_pr().cloned() else {
        let p = Paragraph::new("No PR selected.").block(panel("Detail"));
        f.render_widget(p, area);
        return;
    };
    let reading_area = area.inner(Margin {
        horizontal: 3,
        vertical: 2,
    });
    let content_width = reading_area.width;
    let resized = app
        .main
        .detail
        .fullscreen_cache
        .as_ref()
        .is_some_and(|cache| cache.content_width != content_width);
    let rebuilt = app.main.detail.fullscreen_cache.is_none() || resized;
    if rebuilt {
        let cache = build_fullscreen(
            &pr,
            &app.main.detail,
            content_width,
            app.main.nerd_font,
            &app.highlights,
        );
        app.main.detail.fullscreen_cache = Some(cache);
    }
    let content_height = reading_area.height as usize;
    app.navigation.page_rows = content_height;
    let scrolled = app.main.detail.viewport.pending_scroll != 0;
    app.main
        .detail
        .place_fullscreen(content_height, rebuilt, resized);
    if scrolled && let Some(previous_start) = app.follow_detail_scroll() {
        let cache = build_fullscreen(
            &pr,
            &app.main.detail,
            content_width,
            app.main.nerd_font,
            &app.highlights,
        );
        let start = cache.section_offsets[app.main.detail.section];
        // Preserve the viewport position if closing the reader changes preceding rows.
        app.main.detail.viewport.scroll = app
            .main
            .detail
            .viewport
            .scroll
            .saturating_add_signed(start as isize - previous_start as isize)
            .min(cache.lines.len().saturating_sub(content_height));
        app.main.detail.fullscreen_cache = Some(cache);
    }
    let section = app.main.detail.section;
    let cache = app.main.detail.fullscreen_cache.as_ref().unwrap();
    let total_lines = cache.lines.len();
    let scroll = app.main.detail.viewport.scroll;
    let max_scroll = total_lines.saturating_sub(content_height);

    let chrome_area = area.inner(Margin {
        horizontal: 3,
        vertical: 0,
    });
    if let Some(comment) = section.checked_sub(1).and_then(|index| {
        app.main
            .detail
            .snapshot()
            .as_ref()
            .and_then(|detail| detail.comments.get(index))
    }) {
        let mut context = vec![Span::styled(
            comment.created_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            theme::COMMENT_META,
        )];
        if let Some(location) = reviews::location(comment) {
            context.push(Span::styled(format!(" · {location}"), theme::COMMENT_PATH));
        }
        f.render_widget(
            Paragraph::new(Line::from(context)),
            Rect::new(
                chrome_area.x,
                chrome_area.bottom().saturating_sub(1),
                chrome_area.width,
                chrome_area.height.min(1),
            ),
        );
    }
    // Slice with usize offsets to let large diffs scroll past Paragraph's u16 limit.
    let visible = cache
        .lines
        .iter()
        .skip(scroll)
        .take(content_height)
        .cloned()
        .collect::<Vec<_>>();
    f.render_widget(Paragraph::new(Text::from(visible)), reading_area);
    if max_scroll == 0 {
        return;
    }
    let mut scrollbar_state = ScrollbarState::new(max_scroll + 1)
        .viewport_content_length(content_height)
        .position(scroll);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_symbol("│")
            .thumb_style(theme::DETAIL_SCROLLBAR)
            .track_symbol(None)
            .begin_symbol(None)
            .end_symbol(None),
        area.inner(Margin {
            horizontal: 0,
            vertical: 2,
        }),
        &mut scrollbar_state,
    );
}

fn build_fullscreen(
    pr: &PrSummary,
    state: &DetailState,
    content_width: u16,
    nerd_font: bool,
    highlights: &crate::markdown::HighlightCache,
) -> FullscreenCache {
    let detail = state.snapshot();
    let labels = state
        .review_threads()
        .map(reviews::ReviewLabels::from_threads);
    let active_section = state.section;
    let focus = state.reader.as_ref();
    let expansion = &state.expansion;
    let mut focused_row = None;
    let mut lines = metadata::lines(pr, content_width);
    let mut section_offsets: Vec<usize> = Vec::new();
    let mut comment_offsets: Vec<usize> = Vec::new();
    let mut visible_sections = Vec::new();
    section_offsets.push(lines.len());
    comment_offsets.push(lines.len());
    lines.extend(conversation_header(
        vec![Span::styled(
            "Description",
            if active_section == 0 {
                theme::COMMENT_HEADER_ACTIVE
            } else {
                theme::PR_TITLE
            },
        )],
        Vec::new(),
        content_width,
    ));
    let description_header = section_offsets[0];
    let description_start = lines.len();
    match detail {
        None => {
            lines.push(Line::from(vec![Span::styled(
                "Loading detail…",
                theme::PLACEHOLDER,
            )]));
            finish_conversation_body(&mut lines, description_start, content_width);
        }
        Some(d) => {
            match d.body.as_deref() {
                None | Some("") => {
                    lines.push(Line::from(vec![Span::styled(
                        "(no description)",
                        theme::PLACEHOLDER,
                    )]));
                }
                Some(body) => {
                    if active_section == 0
                        && let Some(focus) = focus
                    {
                        focused_row = append_inline_focus(&mut lines, focus, content_width, 0);
                    } else {
                        let doc = crate::markdown::render_once(
                            body,
                            &expansion.code_blocks(0, detail, body),
                            highlights,
                        );
                        lines.extend(doc.lines);
                    }
                }
            }
            finish_conversation_body(&mut lines, description_start, content_width);
            visible_sections.push((0, description_header..lines.len()));
            if active_section == 0 {
                for line in &mut lines[description_header..] {
                    style_conversation_line(line, content_width, Style::default(), true);
                }
            }
            let labels = labels.expect("detail has conversation metadata");
            let mut expanded = expansion.expanded_comments(d);
            if focus.is_some_and(|focus| !focus.inline_code)
                && let Some(index) = active_section.checked_sub(1)
            {
                expanded.insert(index);
            }
            let visibility = labels.threads.visibility_for(&expanded);
            let conversation = conversation::Conversation {
                comments: &d.comments,
                highlights,
                pr_author: &pr.author,
                width: content_width,
                labels,
                nerd_font,
                compact: false,
                visibility,
                expansion,
            };
            let mut previous = None;
            for (i, comment) in d.comments.iter().enumerate() {
                let section_index = i + 1;
                let representative = conversation.visibility.representative(i);
                if representative != i {
                    section_offsets.push(section_offsets[representative + 1]);
                    comment_offsets.push(comment_offsets[representative + 1]);
                    continue;
                }
                let is_active = active_section == section_index
                    || active_section.checked_sub(1).is_some_and(|selected| {
                        selected < d.comments.len()
                            && conversation.visibility.representative(selected) == i
                    });
                lines.extend(conversation.separator(previous, i));
                previous = Some(i);
                section_offsets.push(lines.len());
                let entry = conversation.entry(
                    i,
                    is_active,
                    if is_active { focus } else { None },
                    10,
                    d.comments.get(i + 1).is_none_or(|next| {
                        !conversation.labels.threads.is_same_review(comment, next)
                    }),
                );
                if let Some(row) = entry.focus_row {
                    focused_row = Some(lines.len() + row);
                }
                comment_offsets.push(lines.len() + entry.comment_row);
                let start = lines.len();
                lines.extend(entry.lines);
                visible_sections.push((section_index, start..lines.len()));
            }
        }
    }
    if detail.is_none() && active_section == 0 {
        for line in &mut lines[description_header..] {
            style_conversation_line(line, content_width, Style::default(), true);
        }
    }
    if detail.is_none() {
        visible_sections.push((0, description_header..lines.len()));
    }
    FullscreenCache {
        focused_row,
        content_width,
        lines,
        section_offsets,
        comment_offsets,
        visible_sections,
    }
}
