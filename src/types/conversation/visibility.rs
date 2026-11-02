use super::{CollapseReason, Comment, CommentKind, ConversationVisibility, ReviewThreads};
use std::collections::HashSet;

fn thread_hidden_reason(comment: &Comment) -> Option<CollapseReason> {
    if matches!(&comment.kind, CommentKind::ReviewComment(context) if context.resolved) {
        Some(CollapseReason::ResolvedThread)
    } else {
        None
    }
}

impl ReviewThreads<'_> {
    // Approval alone does not resolve a discussion; only resolved inline comments count.
    pub fn is_review_resolved(&self, comment: &Comment) -> bool {
        self.display_review_id(comment)
            .is_some_and(|review_id| self.index.resolved_reviews.contains(review_id))
    }

    pub fn review_expansion_targets(&self, index: usize) -> Vec<usize> {
        self.review_expansion_targets_in(index, 0..self.comments.len())
    }

    fn review_expansion_targets_in(
        &self,
        index: usize,
        mut range: std::ops::Range<usize>,
    ) -> Vec<usize> {
        let Some(comment) = self.comments.get(index) else {
            return Vec::new();
        };
        if matches!(
            comment.kind,
            CommentKind::Issue { .. } | CommentKind::Activity { .. }
        ) {
            return Vec::new();
        }
        let summaries: Vec<_> = range
            .clone()
            .filter(|&i| {
                let other = &self.comments[i];
                matches!(other.kind, CommentKind::Review { .. })
                    && (i == index || self.is_same_review(comment, other))
            })
            .collect();
        if !summaries.is_empty() {
            return summaries;
        }
        if !self.is_review_resolved(comment) {
            return Vec::new();
        }
        range
            .find(|&i| {
                let other = &self.comments[i];
                matches!(other.kind, CommentKind::ReviewComment(_))
                    && self.is_same_review(comment, other)
            })
            .into_iter()
            .collect()
    }

    pub fn expansion_group(&self, index: usize) -> Vec<usize> {
        let Some(comment) = self.comments.get(index) else {
            return Vec::new();
        };
        let is_review_target = self.review_expansion_targets(index).contains(&index);
        self.comments
            .iter()
            .enumerate()
            .filter_map(|(i, other)| {
                (i == index
                    || if is_review_target {
                        self.is_same_review(comment, other)
                    } else {
                        self.is_same_thread(comment, other)
                    })
                .then_some(i)
            })
            .collect()
    }

    pub fn default_visibility(&self) -> ConversationVisibility {
        self.visibility_for(&HashSet::new())
    }

    pub fn visibility_for(&self, expanded: &HashSet<usize>) -> ConversationVisibility {
        let mut opened_reviews = HashSet::new();
        let mut opened_threads = HashSet::new();
        for &index in expanded {
            let Some(comment) = self.comments.get(index) else {
                continue;
            };
            if let Some(review_id) = self.display_review_id(comment) {
                opened_reviews.insert(review_id);
            }
            if let Some(key) = self.thread_membership(index) {
                opened_threads.insert(key);
            }
        }
        let review_opened = |index: usize| match self.display_review_id(&self.comments[index]) {
            Some(review_id) => opened_reviews.contains(review_id),
            None => self
                .thread_membership(index)
                .is_some_and(|key| opened_threads.contains(&key)),
        };
        let thread_opened = |index: usize| {
            self.thread_membership(index)
                .is_some_and(|key| opened_threads.contains(&key))
        };
        let mut visibility = ConversationVisibility {
            representatives: (0..self.comments.len()).collect(),
            reasons: vec![None; self.comments.len()],
            expanded: vec![false; self.comments.len()],
        };
        let mut start = 0;
        while start < self.comments.len() {
            let comment = &self.comments[start];
            let mut end = start + 1;
            while end < self.comments.len() && self.is_same_review(comment, &self.comments[end]) {
                end += 1;
            }
            let open_review = self
                .review_expansion_targets_in(start, start..end)
                .iter()
                .any(|index| expanded.contains(index));
            let open_child = review_opened(start);
            if self.is_review_resolved(comment) && !open_child {
                visibility.representatives[start..end].fill(start);
                visibility.reasons[start] = Some(CollapseReason::ResolvedReview);
            } else {
                if open_review {
                    visibility.expanded[start..end].fill(true);
                }
                if self.is_review_resolved(comment)
                    && !open_review
                    && matches!(comment.kind, CommentKind::Review { .. })
                {
                    visibility.reasons[start] = Some(CollapseReason::ResolvedReview);
                }
                let mut thread = start;
                while thread < end {
                    let mut thread_end = thread + 1;
                    while thread_end < end
                        && self.is_same_thread(&self.comments[thread], &self.comments[thread_end])
                    {
                        thread_end += 1;
                    }
                    let open_thread = expanded.contains(&thread) || thread_opened(thread);
                    if open_thread {
                        visibility.expanded[thread..thread_end].fill(true);
                    }
                    let reason = self.comments[thread..thread_end]
                        .iter()
                        .find_map(thread_hidden_reason);
                    if !open_review {
                        if let Some(reason) = reason
                            && !open_thread
                        {
                            visibility.representatives[thread..thread_end].fill(thread);
                            visibility.reasons[thread] = Some(reason);
                        } else if reason.is_none() {
                            for (index, comment) in self
                                .comments
                                .iter()
                                .enumerate()
                                .take(thread_end)
                                .skip(thread)
                            {
                                if !expanded.contains(&index) && comment.minimized {
                                    visibility.reasons[index] = Some(CollapseReason::Minimized);
                                }
                            }
                        }
                    }
                    thread = thread_end;
                }
            }
            start = end;
        }
        visibility
    }

    // Include the display review so conflicting relationships cannot open an unrelated thread.
    fn thread_membership(&self, index: usize) -> Option<(usize, Option<&str>)> {
        let comment = &self.comments[index];
        matches!(comment.kind, CommentKind::ReviewComment(_)).then(|| {
            (
                self.index.components[index],
                self.display_review_id(comment),
            )
        })
    }
}
