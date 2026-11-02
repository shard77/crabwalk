use crate::types::{Comment, CommentKind, DiffSide, ReviewState, ReviewThreads};
use crate::ui::theme;
use ratatui::text::Span;
use std::fmt::Write;

#[derive(Debug, PartialEq)]
pub(super) enum ReviewBadge {
    Author,
    Outdated,
    Resolved,
    Hidden(Option<String>),
}

impl ReviewBadge {
    pub(super) fn label(&self) -> String {
        match self {
            Self::Author => "AUTHOR".into(),
            Self::Outdated => "OUTDATED".into(),
            Self::Resolved => "RESOLVED".into(),
            Self::Hidden(None) => "HIDDEN".into(),
            Self::Hidden(Some(reason)) => format!("HIDDEN · {reason}"),
        }
    }

    pub(super) fn into_span(self, nerd_font: bool) -> Span<'static> {
        let (icon, style) = match &self {
            Self::Author => ('\u{f007}', theme::REVIEW_AUTHOR_BADGE),
            Self::Outdated => ('\u{f1da}', theme::REVIEW_STATUS),
            Self::Resolved => ('\u{f058}', theme::REVIEW_RESOLVED_BADGE),
            Self::Hidden(_) => ('\u{f070}', theme::REVIEW_STATUS),
        };
        let label = if nerd_font {
            format!("{icon} {}", self.label())
        } else {
            self.label()
        };
        Span::styled(format!(" {label} "), style)
    }
}

pub(super) struct ReviewLabels<'a> {
    pub(super) threads: ReviewThreads<'a>,
}

impl<'a> ReviewLabels<'a> {
    pub(super) fn from_threads(threads: ReviewThreads<'a>) -> Self {
        Self { threads }
    }

    pub(super) fn recent_entries<'b>(
        &'b self,
        comments: &'b [Comment],
    ) -> impl Iterator<Item = [usize; 3]> + 'b {
        let mut recent: Vec<usize> = (0..comments.len()).collect();
        recent.sort_by_key(|&index| (comments[index].created_at, index));
        recent.into_iter().rev().map(|index| {
            [
                review_start(comments, index, &self.threads),
                self.threads.thread_start(index),
                index,
            ]
        })
    }

    pub(super) fn heading(&self, comment: &Comment) -> String {
        format!("@{} {}", comment.author, self.action(comment))
    }

    pub(super) fn action<'b>(&self, comment: &'b Comment) -> &'b str {
        match &comment.kind {
            CommentKind::Activity { action, .. } => action,
            CommentKind::Review { state, .. } => match state {
                ReviewState::Approved => "approved",
                ReviewState::ChangesRequested => "requested changes",
                ReviewState::Commented | ReviewState::Other => "reviewed",
                // Keep the submission time; the timeline shows the later dismissal and its actor.
                ReviewState::Dismissed => "reviewed (now dismissed)",
                ReviewState::Pending => "pending review",
            },
            CommentKind::Issue { .. } => "commented",
            CommentKind::ReviewComment(_) => "review · summary unavailable",
        }
    }

    pub(super) fn parent(&self, comment: &Comment) -> Option<&'a Comment> {
        let CommentKind::ReviewComment(context) = &comment.kind else {
            return None;
        };
        self.threads.inline_comment(context.reply_to.as_deref()?)
    }

    pub(super) fn review_label(&self, comment: &Comment) -> Option<String> {
        let id = self.threads.display_review_id(comment)?;
        Some(self.threads.review_summary(id).map_or_else(
            || "Review · summary unavailable".into(),
            |summary| format!("@{}’s review", summary.author),
        ))
    }

    pub(super) fn relationship(&self, comment: &Comment) -> Option<String> {
        let CommentKind::ReviewComment(context) = &comment.kind else {
            return None;
        };
        let mut text = self
            .review_label(comment)
            .unwrap_or_else(|| "Review unavailable".into());
        if context.reply_to.is_some() {
            if let Some(parent) = self.parent(comment) {
                if !self.threads.is_same_thread(parent, comment) {
                    write!(text, " · reply to @{}", parent.author)
                        .expect("writing to a String cannot fail");
                }
            } else {
                text.push_str(" · earlier comment unavailable");
            }
        }
        Some(text)
    }

    pub(super) fn summary(&self, comment: &Comment, nerd_font: bool) -> Option<String> {
        let (comments, discussions) = self.threads.review_counts(comment)?;
        if nerd_font {
            return Some(format!("\u{f0e6} {} · \u{f126} {}", comments, discussions));
        }
        Some(format!(
            "{} {} · {} {}",
            comments,
            if comments == 1 { "comment" } else { "comments" },
            discussions,
            if discussions == 1 {
                "discussion"
            } else {
                "discussions"
            }
        ))
    }
}

pub(super) fn is_resolved(comment: &Comment) -> bool {
    matches!(&comment.kind, CommentKind::ReviewComment(context) if context.resolved)
}

fn review_start(comments: &[Comment], index: usize, threads: &ReviewThreads<'_>) -> usize {
    let mut start = index;
    while start > 0 && threads.is_same_review(&comments[start - 1], &comments[index]) {
        start -= 1;
    }
    start
}

pub(super) fn location(comment: &Comment) -> Option<String> {
    let path = comment.path.as_deref()?;
    let CommentKind::ReviewComment(context) = &comment.kind else {
        return Some(path.into());
    };
    let (start, end) = if comment.outdated || context.line.is_none() {
        (context.original_start_line, context.original_line)
    } else {
        (context.start_line, context.line)
    };
    let cross_side =
        start.is_some() && context.start_side.is_some() && context.start_side != context.side;
    let side_name = |side: Option<DiffSide>| match side {
        Some(DiffSide::Left) => "old",
        Some(DiffSide::Right) => "new",
        None => "unknown",
    };
    let mut text = match (start, end) {
        (Some(start), Some(end)) if cross_side => format!(
            "{path}:{start} ({}) → {end} ({})",
            side_name(context.start_side),
            side_name(context.side)
        ),
        (Some(start), Some(end)) if start != end => format!("{path}:{start}-{end}"),
        (_, Some(line)) => format!("{path}:{line}"),
        _ => format!("{path} (file)"),
    };
    if end.is_some() && !cross_side && context.side == Some(DiffSide::Left) {
        text.push_str(" (old)");
    }
    Some(text)
}

pub(super) fn status_badges(comment: &Comment) -> Vec<ReviewBadge> {
    let mut badges = Vec::new();
    if comment.outdated && !is_resolved(comment) {
        badges.push(ReviewBadge::Outdated);
    }
    if comment.minimized {
        badges.push(ReviewBadge::Hidden(comment.minimized_reason.clone()));
    }
    if is_resolved(comment) {
        badges.push(ReviewBadge::Resolved);
    }
    badges
}
