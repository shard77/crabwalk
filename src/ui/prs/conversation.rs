use super::text::{
    append_inline_focus, conversation_header, finish_conversation_body, style_conversation_line,
    wrap_conversation_line, wrap_header_spans,
};
use super::{review_diff, reviews};
use crate::app::ReaderState;
use crate::types::{CollapseReason, Comment, CommentKind, ConversationVisibility, ReviewState};
use crate::ui::theme;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub(super) struct Conversation<'a> {
    pub comments: &'a [Comment],
    pub highlights: &'a crate::markdown::HighlightCache,
    pub pr_author: &'a str,
    pub width: u16,
    pub labels: reviews::ReviewLabels<'a>,
    pub nerd_font: bool,
    pub compact: bool,
    pub visibility: ConversationVisibility,
    pub expansion: &'a crate::app::DetailExpansion,
}

pub(super) struct RenderedEntry {
    pub lines: Vec<Line<'static>>,
    pub focus_row: Option<usize>,
    pub comment_row: usize,
}

impl Conversation<'_> {
    fn comment_badges(comment: &Comment) -> Vec<reviews::ReviewBadge> {
        let inline = matches!(comment.kind, CommentKind::ReviewComment(_));
        reviews::status_badges(comment)
            .into_iter()
            .filter(|badge| {
                // Put code state on the file header so it is not mistaken for the commenter's status.
                !inline
                    || !matches!(
                        badge,
                        reviews::ReviewBadge::Outdated | reviews::ReviewBadge::Resolved
                    )
            })
            .collect()
    }

    fn author_header(
        &self,
        comment: &Comment,
        action: Option<Span<'static>>,
    ) -> Vec<Span<'static>> {
        let mut header = vec![Span::styled(
            format!("@{}", comment.author),
            theme::REVIEW_HEADER,
        )];
        if comment.author.eq_ignore_ascii_case(self.pr_author) {
            header.push(Span::raw(" "));
            header.push(reviews::ReviewBadge::Author.into_span(self.nerd_font));
        }
        if let Some(action) = action {
            header.push(Span::raw(" "));
            header.push(action);
        }
        header
    }

    fn append_status(
        &self,
        header: &mut Vec<Span<'static>>,
        comment: &Comment,
        review_resolved: bool,
    ) {
        let mut badges = Self::comment_badges(comment);
        if review_resolved && !badges.contains(&reviews::ReviewBadge::Resolved) {
            badges.push(reviews::ReviewBadge::Resolved);
        }
        if review_resolved {
            badges.retain(|badge| *badge != reviews::ReviewBadge::Outdated);
        }
        for badge in badges {
            header.push(Span::raw(" "));
            header.push(badge.into_span(self.nerd_font));
        }
    }

    fn header_lines(
        &self,
        headers: Vec<Line<'static>>,
        width: u16,
        background: Style,
    ) -> Vec<Line<'static>> {
        headers
            .into_iter()
            .flat_map(|header| wrap_header_spans(header, width))
            .map(|line| {
                let style = background.patch(line.style);
                line.style(style)
            })
            .collect()
    }

    fn metadata(&self, comment: &Comment, include_counts: bool) -> Vec<Span<'static>> {
        let date = comment.created_at.format("%d %b");
        let time = comment.created_at.format("%H:%M");
        let timestamp = if self.nerd_font {
            format!("\u{f073} {date} · \u{f017} {time}")
        } else {
            format!("{date} · {time}")
        };
        let text =
            if include_counts && let Some(counts) = self.labels.summary(comment, self.nerd_font) {
                format!("{counts} · {timestamp}")
            } else {
                timestamp
            };
        vec![Span::styled(text, theme::COMMENT_META)]
    }

    fn branch_indent(&self) -> u16 {
        if self.width >= 8 { 4 } else { 0 }
    }

    fn branch_span(&self, index: usize, begins_thread: bool) -> Span<'static> {
        Self::branch_marker(
            begins_thread,
            self.labels.threads.has_following_discussion(index),
        )
    }

    fn branch_marker(begins_thread: bool, following: bool) -> Span<'static> {
        let marker = match (begins_thread, following) {
            (true, true) => "├─  ",
            (true, false) => "└─  ",
            (false, true) => "│   ",
            (false, false) => "    ",
        };
        Span::styled(marker, theme::REVIEW_BRANCH)
    }

    pub fn separator(&self, previous: Option<usize>, index: usize) -> Vec<Line<'static>> {
        if previous.is_some_and(|prev| {
            self.labels
                .threads
                .is_same_review(&self.comments[prev], &self.comments[index])
        }) {
            if previous.is_some_and(|prev| {
                index == prev + 1
                    && matches!(self.comments[prev].kind, CommentKind::Review { .. })
                    && self.comments[prev].body.trim().is_empty()
            }) {
                return Vec::new();
            }
            let branch = previous
                .filter(|&prev| matches!(self.comments[prev].kind, CommentKind::ReviewComment(_)));
            let skipped = previous.is_some_and(|prev| {
                (prev + 1..index).any(|i| self.visibility.representative(i) == i)
            });
            let skipped_discussions = skipped
                && previous.is_some_and(|prev| {
                    matches!(self.comments[prev].kind, CommentKind::Review { .. })
                });
            let indent = if branch.is_some() || skipped_discussions {
                self.branch_indent()
            } else {
                0
            };
            let width = self.width - indent;
            let mut lines = vec![Line::styled(
                " ".repeat(width as usize),
                theme::COMMENT_GAP_BACKGROUND,
            )];
            if skipped && !self.compact {
                lines.extend(wrap_conversation_line(
                    Line::styled(
                        "… earlier comments · Enter for full review",
                        theme::REVIEW_META,
                    ),
                    width,
                ));
            }
            if indent > 0 {
                for (row, line) in lines.iter_mut().enumerate() {
                    let marker = if skipped_discussions {
                        Self::branch_marker(row == 1, true)
                    } else {
                        self.branch_span(branch.unwrap(), false)
                    };
                    line.spans.insert(0, marker);
                }
            }
            lines
        } else {
            vec![Line::styled(
                " ".repeat(self.width as usize),
                theme::COMMENT_GAP_BACKGROUND,
            )]
        }
    }

    fn collapsed_entry(
        &self,
        index: usize,
        reason: CollapseReason,
        mut lines: Vec<Line<'static>>,
        indent: u16,
        active: bool,
    ) -> RenderedEntry {
        let comment = &self.comments[index];
        let is_collapsed_review = reason == CollapseReason::ResolvedReview;
        let placeholder_start = lines.len();
        let mut placeholder = if is_collapsed_review {
            let mut heading = self.review_heading(comment);
            self.append_status(&mut heading, comment, true);
            heading
        } else {
            let text = if reason == CollapseReason::Minimized {
                "Minimized comment"
            } else {
                "Hidden comment"
            };
            vec![Span::styled(text, theme::REVIEW_META)]
        };
        if reason == CollapseReason::ResolvedThread {
            placeholder.push(Span::raw(" "));
            placeholder.push(reviews::ReviewBadge::Resolved.into_span(self.nerd_font));
        } else if reason == CollapseReason::Minimized {
            placeholder.push(Span::raw(" "));
            placeholder.push(
                reviews::ReviewBadge::Hidden(comment.minimized_reason.clone())
                    .into_span(self.nerd_font),
            );
        }
        let collapsed_width = self.width - indent;
        lines.extend(self.header_lines(
            conversation_header(
                placeholder,
                if is_collapsed_review {
                    self.metadata(comment, true)
                } else {
                    Vec::new()
                },
                collapsed_width,
            ),
            collapsed_width,
            theme::REVIEW_BACKGROUND,
        ));
        for (row, line) in lines.iter_mut().enumerate() {
            if row >= placeholder_start && indent > 0 {
                line.spans
                    .insert(0, self.branch_span(index, row == placeholder_start));
            }
            style_conversation_line(line, self.width, theme::REVIEW_BACKGROUND, active);
        }
        RenderedEntry {
            lines,
            focus_row: None,
            comment_row: 0,
        }
    }

    fn review_heading(&self, comment: &Comment) -> Vec<Span<'static>> {
        let action = Span::styled(
            self.labels.action(comment).to_owned(),
            review_style(comment),
        );
        self.author_header(comment, Some(action))
    }

    pub fn entry(
        &self,
        index: usize,
        active: bool,
        focus: Option<&ReaderState>,
        body_rows: usize,
        end_group: bool,
    ) -> RenderedEntry {
        let comment = &self.comments[index];
        let inline = matches!(comment.kind, CommentKind::ReviewComment(_));
        let summary = matches!(comment.kind, CommentKind::Review { .. });
        let grouped = inline || summary;
        let expanded = self.visibility.is_expanded(index);
        let previous = index.checked_sub(1).map(|i| &self.comments[i]);
        let begins_group = grouped
            && previous.is_none_or(|prev| !self.labels.threads.is_same_review(prev, comment));
        let begins_thread = inline
            && previous.is_none_or(|prev| !self.labels.threads.is_same_thread(prev, comment));
        let indent = if inline { self.branch_indent() } else { 0 };
        let width = self.width - indent;
        let mut lines = Vec::new();
        let mut focused_row = None;

        let reason = self.visibility.reason(index);
        if begins_group && reason != Some(CollapseReason::ResolvedReview) {
            let mut heading = self.review_heading(comment);
            if summary {
                self.append_status(
                    &mut heading,
                    comment,
                    self.labels.threads.is_review_resolved(comment),
                );
            }
            lines.extend(self.header_lines(
                conversation_header(heading, self.metadata(comment, true), self.width),
                self.width,
                theme::REVIEW_BACKGROUND,
            ));
        }
        if let Some(reason) = reason {
            let indent = if inline && reason != CollapseReason::ResolvedReview {
                indent
            } else {
                0
            };
            return self.collapsed_entry(index, reason, lines, indent, active);
        }

        if inline && begins_group {
            lines.push(Line::styled("", theme::REVIEW_BACKGROUND));
        }
        let discussion_start = lines.len();
        if begins_thread {
            lines.extend(if self.compact {
                review_diff::render_compact(comment, width, self.nerd_font)
            } else {
                review_diff::render(comment, width, expanded, self.nerd_font)
            });
            lines.push(Line::from(""));
        }

        let comment_start = lines.len();
        let header_background = if active {
            theme::COMMENT_HEADER_BACKGROUND.patch(theme::FOCUS_BLOCK_BG)
        } else {
            theme::COMMENT_HEADER_BACKGROUND
        };
        if !summary {
            let action = match &comment.kind {
                CommentKind::Activity { action, .. }
                    if focus.is_none_or(|focus| {
                        focus.inline_code || focus.markdown.source() != comment.reader_source()
                    }) =>
                {
                    Some(Span::styled(action.clone(), theme::REVIEW_HEADER))
                }
                _ => None,
            };
            let mut header = self.author_header(comment, action);
            self.append_status(&mut header, comment, false);
            lines.extend(self.header_lines(
                conversation_header(header, self.metadata(comment, false), width),
                width,
                header_background,
            ));
        }
        if let CommentKind::ReviewComment(context) = &comment.kind
            && context.reply_to.is_some()
        {
            let context = match self.labels.parent(comment) {
                Some(parent) if self.labels.threads.is_same_thread(parent, comment) => None,
                Some(parent) => Some(format!("In reply to @{}", parent.author)),
                None => Some("Reply · earlier comment unavailable".into()),
            };
            if let Some(context) = context {
                lines.extend(wrap_conversation_line(
                    Line::styled(context, theme::REVIEW_META),
                    width,
                ));
            }
        }
        if !grouped && let Some(location) = reviews::location(comment) {
            lines.extend(self.header_lines(
                vec![Line::styled(location, theme::REVIEW_PATH)],
                width,
                header_background,
            ));
        }

        let body_start = lines.len();
        let mut preview_rows = body_rows;
        if let Some(focus) = focus {
            // Skip the action paragraph already shown in the header, keeping parser IDs and reader
            // coordinates unchanged.
            if focus.inline_code && matches!(comment.kind, CommentKind::Activity { .. }) {
                let mut action_only = comment.clone();
                action_only.body.clear();
                let action = crate::markdown::render_once(
                    &action_only.reader_source(),
                    &Default::default(),
                    self.highlights,
                );
                focused_row = append_inline_focus(&mut lines, focus, width, action.lines.len());
            } else {
                focused_row = append_inline_focus(&mut lines, focus, width, 0);
            }
            if focus.inline_code && !expanded {
                let limit =
                    focused_row.map_or(body_rows, |row| body_rows.max(row - body_start + 1));
                preview_rows = code_preview_rows(&focus.rendered, width, limit);
            }
        } else if comment.body.trim().is_empty() {
            if !summary && !matches!(comment.kind, CommentKind::Activity { .. }) {
                lines.push(Line::styled("(empty comment)", theme::REVIEW_META));
            }
        } else {
            let doc = crate::markdown::render_once(
                &comment.body,
                &self.expansion.comment_code_blocks(comment),
                self.highlights,
            );
            if !expanded {
                preview_rows = code_preview_rows(&doc, width, body_rows);
            }
            lines.extend(doc.lines);
        }
        finish_conversation_body(&mut lines, body_start, width);
        if !expanded
            && focus.is_none_or(|focus| focus.inline_code)
            && lines.len() - body_start > preview_rows
        {
            lines.truncate(body_start + preview_rows);
            lines.extend(wrap_conversation_line(
                Line::styled(
                    if self.compact {
                        "…"
                    } else {
                        "… Enter to read more"
                    },
                    theme::REVIEW_META,
                ),
                width,
            ));
        }
        if inline
            && !begins_thread
            && (focus.is_some_and(|focus| !focus.inline_code)
                || self.expansion.is_comment_expanded(comment))
        {
            lines.push(Line::from(""));
            lines.extend(review_diff::render(comment, width, true, self.nerd_font));
        }
        if grouped
            && !self.compact
            && end_group
            && self.comments.get(index + 1).is_some_and(|next| {
                matches!(next.kind, CommentKind::ReviewComment(_))
                    && self.labels.threads.is_same_review(comment, next)
            })
        {
            let hint_indent = if summary { self.branch_indent() } else { 0 };
            let mut hint = wrap_conversation_line(
                Line::styled(
                    "… more comments · Enter for full review",
                    theme::REVIEW_META,
                ),
                width - hint_indent,
            );
            if hint_indent > 0 {
                for (row, line) in hint.iter_mut().enumerate() {
                    line.spans.insert(0, Self::branch_marker(row == 0, false));
                }
            }
            lines.extend(hint);
        }
        for (row, line) in lines.iter_mut().enumerate() {
            let background = if summary || row < comment_start {
                theme::REVIEW_BACKGROUND
            } else {
                theme::COMMENT_BODY_BACKGROUND
            };
            let branched = inline && row >= discussion_start && indent > 0;
            let row_width = if branched { width } else { self.width };
            style_conversation_line(
                line,
                row_width,
                background,
                active && (summary || row >= comment_start),
            );
            if branched {
                line.spans.insert(
                    0,
                    self.branch_span(index, begins_thread && row == discussion_start),
                );
            }
        }
        RenderedEntry {
            lines,
            focus_row: focused_row,
            comment_row: if summary { 0 } else { comment_start },
        }
    }
}

/// Extends the row limit to finish any code preview that starts within it.
pub(super) fn code_preview_rows(
    doc: &crate::markdown::RenderedDocument,
    width: u16,
    limit: usize,
) -> usize {
    let mut rows = 0;
    let mut limit = limit;
    let mut code_end = None;
    for (index, line) in doc
        .lines
        .iter()
        .enumerate()
        .skip_while(|(_, line)| line.spans.iter().all(|span| span.content.trim().is_empty()))
    {
        if rows >= limit && code_end.is_none_or(|end| index > end) {
            break;
        }
        if let Some(el) = doc.interactive.iter().find(|el| {
            el.line_start == index
                && matches!(el.kind, crate::markdown::InteractiveKind::CodeBlock { .. })
        }) {
            code_end = Some(el.line_end);
        }
        rows += wrap_conversation_line(line.clone(), width).len();
        if code_end.is_some_and(|end| index <= end) {
            limit = limit.max(rows);
        }
    }
    limit
}

fn review_style(comment: &Comment) -> Style {
    match &comment.kind {
        CommentKind::Review {
            state: ReviewState::Approved,
            ..
        } => theme::REVIEW_APPROVED,
        CommentKind::Review {
            state: ReviewState::ChangesRequested,
            ..
        } => theme::REVIEW_CHANGES,
        _ => theme::REVIEW_HEADER,
    }
}
