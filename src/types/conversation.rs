use super::{Comment, CommentKind, ItemDetail, ReviewState};
use std::collections::{BTreeMap, HashMap, HashSet};

mod visibility;

// Replies can be submitted in a later review; display them under the original discussion.
// Ambiguous or cyclic relationships keep their submitted review instead of guessing which review to display them under.
pub struct ReviewThreads<'a> {
    comments: &'a [Comment],
    index: &'a ConversationIndex,
}

pub struct ConversationIndex {
    // None marks a duplicate ID, which cannot safely identify a reply parent.
    ids: HashMap<String, Option<usize>>,
    components: Vec<usize>,
    // No root when the thread is missing one or its relationships conflict.
    roots: Vec<Option<usize>>,
    resolved_reviews: HashSet<String>,
    summaries: HashMap<String, usize>,
    inline_comments: HashMap<String, usize>,
    // (inline comments, discussions); discussion counts require grouped comment order.
    counts: HashMap<String, (usize, usize)>,
    following_discussion: Vec<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapseReason {
    ResolvedReview,
    ResolvedThread,
    Minimized,
}

pub struct ConversationVisibility {
    representatives: Vec<usize>,
    reasons: Vec<Option<CollapseReason>>,
    expanded: Vec<bool>,
}

impl ConversationVisibility {
    pub fn representative(&self, index: usize) -> usize {
        self.representatives[index]
    }

    pub fn reason(&self, index: usize) -> Option<CollapseReason> {
        self.reasons[index]
    }

    pub fn is_expanded(&self, index: usize) -> bool {
        self.expanded[index]
    }
}

fn nonempty(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn representative(parents: &mut [usize], mut i: usize) -> usize {
    while parents[i] != i {
        parents[i] = parents[parents[i]];
        i = parents[i];
    }
    i
}

fn join(parents: &mut [usize], sizes: &mut [usize], a: usize, b: usize) {
    let (mut a, mut b) = (representative(parents, a), representative(parents, b));
    if a == b {
        return;
    }
    if sizes[a] < sizes[b] {
        std::mem::swap(&mut a, &mut b);
    }
    parents[b] = a;
    sizes[a] += sizes[b];
}

impl ConversationIndex {
    /// Build after group_reviews: counts and branch connectors depend on that order.
    pub fn new(comments: &[Comment]) -> Self {
        let mut ids = HashMap::new();
        for (i, comment) in comments.iter().enumerate() {
            if let CommentKind::ReviewComment(context) = &comment.kind
                && let Some(id) = nonempty(&context.id)
            {
                ids.entry(id.to_owned())
                    .and_modify(|v| *v = None)
                    .or_insert(Some(i));
            }
        }
        let mut components: Vec<_> = (0..comments.len()).collect();
        let mut sizes = vec![1; comments.len()];
        let mut parents = vec![None; comments.len()];
        let mut threads = HashMap::new();
        for (i, comment) in comments.iter().enumerate() {
            let CommentKind::ReviewComment(context) = &comment.kind else {
                continue;
            };
            if let Some(thread) = nonempty(&context.thread_id) {
                let previous = *threads.entry(thread).or_insert(i);
                join(&mut components, &mut sizes, i, previous);
            }
            if let Some(Some(parent)) = context.reply_to.as_deref().and_then(|id| ids.get(id)) {
                parents[i] = Some(*parent);
                join(&mut components, &mut sizes, i, *parent);
            }
        }
        for i in 0..components.len() {
            components[i] = representative(&mut components, i);
        }
        let mut roots = vec![None; comments.len()];
        let mut invalid = vec![false; comments.len()];
        // Thread IDs can connect a cycle to a root, so check reply cycles separately.
        // Walk iteratively to avoid overflowing the stack on long reply chains.
        let mut visited = vec![false; comments.len()];
        let mut finished = vec![false; comments.len()];
        for start in 0..comments.len() {
            let mut path = Vec::new();
            let mut next = Some(start);
            while let Some(i) = next {
                if visited[i] {
                    if !finished[i] {
                        invalid[components[i]] = true;
                    }
                    break;
                }
                visited[i] = true;
                path.push(i);
                next = parents[i];
            }
            for i in path {
                finished[i] = true;
            }
        }
        let mut thread_ids = vec![None; comments.len()];
        for (i, comment) in comments.iter().enumerate() {
            let CommentKind::ReviewComment(context) = &comment.kind else {
                continue;
            };
            let component = components[i];
            if context.reply_to.as_deref().and_then(nonempty).is_none()
                && roots[component].replace(i).is_some()
            {
                invalid[component] = true;
            }
            if let Some(thread) = nonempty(&context.thread_id)
                && let Some(previous) = thread_ids[component].replace(thread)
                && previous != thread
            {
                invalid[component] = true;
            }
            if ids.get(context.id.as_str()) == Some(&None) {
                invalid[component] = true;
            }
        }
        for i in 0..roots.len() {
            if invalid[i] {
                roots[i] = None;
            }
        }
        let mut index = Self {
            ids,
            components,
            roots,
            resolved_reviews: HashSet::new(),
            summaries: HashMap::new(),
            inline_comments: HashMap::new(),
            counts: HashMap::new(),
            following_discussion: vec![false; comments.len()],
        };
        let threads = ReviewThreads::with_index(comments, &index);
        let mut resolved = HashMap::new();
        let mut summaries = HashMap::new();
        let mut inline_comments = HashMap::new();
        let mut counts = HashMap::new();
        for (i, comment) in comments.iter().enumerate() {
            match &comment.kind {
                CommentKind::Review { id, .. } => {
                    summaries.entry(id.clone()).or_insert(i);
                }
                CommentKind::ReviewComment(context) => {
                    // Display lookup can show the first duplicate; reply linking must reject ambiguous IDs.
                    inline_comments.entry(context.id.clone()).or_insert(i);
                    if let Some(review_id) = threads.display_review_id(comment) {
                        *resolved.entry(review_id.to_owned()).or_insert(true) &= context.resolved;
                        let count = counts.entry(review_id.to_owned()).or_insert((0, 0));
                        count.0 += 1;
                        if i == 0 || !threads.is_same_thread(&comments[i - 1], comment) {
                            count.1 += 1;
                        }
                    }
                }
                CommentKind::Issue { .. } | CommentKind::Activity { .. } => {}
            }
        }
        let mut following = vec![false; comments.len()];
        for i in (0..comments.len().saturating_sub(1)).rev() {
            if threads.is_same_review(&comments[i], &comments[i + 1]) {
                following[i] = following[i + 1]
                    || (matches!(comments[i + 1].kind, CommentKind::ReviewComment(_))
                        && !threads.is_same_thread(&comments[i], &comments[i + 1]));
            }
        }
        index.summaries = summaries;
        index.inline_comments = inline_comments;
        index.counts = counts;
        index.following_discussion = following;
        index.resolved_reviews = resolved
            .into_iter()
            .filter_map(|(review_id, resolved)| resolved.then_some(review_id))
            .collect();
        index
    }
}

impl<'a> ReviewThreads<'a> {
    /// Contents and order must match the snapshot used to build the index.
    pub fn with_index(comments: &'a [Comment], index: &'a ConversationIndex) -> Self {
        assert_eq!(
            comments.len(),
            index.components.len(),
            "snapshot length changed"
        );
        Self { comments, index }
    }

    pub fn review_summary(&self, id: &str) -> Option<&'a Comment> {
        self.comments.get(*self.index.summaries.get(id)?)
    }

    pub fn inline_comment(&self, id: &str) -> Option<&'a Comment> {
        self.comments.get(*self.index.inline_comments.get(id)?)
    }

    pub fn review_counts(&self, comment: &Comment) -> Option<(usize, usize)> {
        self.index
            .counts
            .get(self.display_review_id(comment)?)
            .copied()
    }

    pub fn has_following_discussion(&self, index: usize) -> bool {
        self.index.following_discussion[index]
    }

    fn position(&self, comment: &Comment) -> Option<usize> {
        // The caller may pass a cloned comment. Verify pointer identity before using an address-derived index.
        (comment as *const Comment as usize)
            .checked_sub(self.comments.as_ptr() as usize)
            .map(|offset| offset / std::mem::size_of::<Comment>())
            .filter(|&i| {
                self.comments
                    .get(i)
                    .is_some_and(|c| std::ptr::eq(c, comment))
            })
            .or_else(|| {
                let CommentKind::ReviewComment(context) = &comment.kind else {
                    return None;
                };
                self.index.ids.get(context.id.as_str()).copied().flatten()
            })
    }

    /// Uses the thread root's review so later replies stay with the original discussion.
    pub fn display_review_id<'b>(&'b self, comment: &'b Comment) -> Option<&'b str> {
        match &comment.kind {
            CommentKind::Review { id, .. } => nonempty(id),
            CommentKind::ReviewComment(context) => {
                let root = self
                    .position(comment)
                    .and_then(|i| self.index.roots[self.index.components[i]]);
                let context = root.map_or(context, |i| {
                    let CommentKind::ReviewComment(context) = &self.comments[i].kind else {
                        unreachable!()
                    };
                    context
                });
                context.review_id.as_deref().and_then(nonempty)
            }
            CommentKind::Issue { .. } | CommentKind::Activity { .. } => None,
        }
    }

    pub fn is_same_review(&self, a: &Comment, b: &Comment) -> bool {
        match (self.display_review_id(a), self.display_review_id(b)) {
            (Some(a), Some(b)) => a == b,
            (None, None) => self.is_same_thread(a, b),
            _ => false,
        }
    }

    pub fn thread_start(&self, index: usize) -> usize {
        let mut start = index;
        while start > 0 && self.is_same_thread(&self.comments[start - 1], &self.comments[index]) {
            start -= 1;
        }
        start
    }

    pub fn is_same_thread(&self, a: &Comment, b: &Comment) -> bool {
        if !matches!(a.kind, CommentKind::ReviewComment(_))
            || !matches!(b.kind, CommentKind::ReviewComment(_))
        {
            return false;
        }
        match (self.position(a), self.position(b)) {
            (Some(a_index), Some(b_index)) => {
                self.index.components[a_index] == self.index.components[b_index]
                    && self.display_review_id(a) == self.display_review_id(b)
            }
            _ => false,
        }
    }
}

impl ItemDetail {
    /// Reorders for display; operate on the app's copy so the network cache stays chronological.
    pub fn group_reviews(&mut self) {
        let order_key = |i: usize| (self.comments[i].created_at, i);
        let mut chronological: Vec<_> = (0..self.comments.len()).collect();
        chronological.sort_by_key(|&i| order_key(i));
        let conversation = ConversationIndex::new(&self.comments);
        let index = ReviewThreads::with_index(&self.comments, &conversation);
        let mut reply_only = HashMap::new();
        for (i, comment) in self.comments.iter().enumerate() {
            let CommentKind::ReviewComment(context) = &comment.kind else {
                continue;
            };
            if let Some(submitted) = context.review_id.as_deref().and_then(nonempty) {
                let moved = index.index.roots[index.index.components[i]]
                    .is_some_and(|root| root != i)
                    && index.display_review_id(comment) != Some(submitted);
                *reply_only.entry(submitted).or_insert(true) &= moved;
            }
        }
        let mut reviews = BTreeMap::new();
        let mut threads: BTreeMap<_, Vec<usize>> = BTreeMap::new();
        let mut top_level = Vec::new();
        for i in chronological {
            match &self.comments[i].kind {
                CommentKind::Review { id, state } => {
                    let comment = &self.comments[i];
                    if *state == ReviewState::Commented
                        && comment.body.trim().is_empty()
                        && !comment.minimized
                        && reply_only.get(id.as_str()) == Some(&true)
                    {
                        continue;
                    }
                    reviews.entry(id.clone()).or_insert(i);
                    top_level.push(i);
                }
                CommentKind::ReviewComment(_) => {
                    threads
                        .entry((
                            index
                                .display_review_id(&self.comments[i])
                                .map(str::to_owned),
                            index.index.components[i],
                        ))
                        .or_default()
                        .push(i);
                }
                CommentKind::Issue { .. } | CommentKind::Activity { .. } => top_level.push(i),
            }
        }

        let mut threads_by_review: BTreeMap<String, Vec<Vec<usize>>> = BTreeMap::new();
        let mut groups: Vec<(usize, Vec<usize>)> =
            top_level.into_iter().map(|i| (i, vec![i])).collect();
        for ((review_id, _), mut thread) in threads {
            let earliest = thread[0];
            // A reply can have an earlier API timestamp than its root; keep the root first.
            thread.sort_by_key(|&i| {
                let is_reply = matches!(&self.comments[i].kind,
                    CommentKind::ReviewComment(context)
                        if context.reply_to.as_deref().and_then(nonempty).is_some());
                (is_reply, order_key(i))
            });
            if let Some(review_id) = review_id {
                threads_by_review.entry(review_id).or_default().push(thread);
            } else {
                groups.push((earliest, thread));
            }
        }
        let mut threads_by_summary = BTreeMap::new();
        for (review_id, mut threads) in threads_by_review {
            threads.sort_by_key(|thread| order_key(thread[0]));
            if let Some(&summary) = reviews.get(&review_id) {
                threads_by_summary.insert(summary, threads);
            } else {
                let earliest = threads
                    .iter()
                    .flatten()
                    .copied()
                    .min_by_key(|&i| order_key(i))
                    .unwrap();
                groups.push((earliest, threads.into_iter().flatten().collect()));
            }
        }
        groups.sort_by_key(|(anchor, _)| order_key(*anchor));
        let mut order = Vec::with_capacity(self.comments.len());
        for (anchor, entries) in groups {
            order.extend(entries);
            if let Some(threads) = threads_by_summary.remove(&anchor) {
                order.extend(threads.into_iter().flatten());
            }
        }
        let mut entries: Vec<_> = std::mem::take(&mut self.comments)
            .into_iter()
            .map(Some)
            .collect();
        self.comments = order
            .into_iter()
            .map(|i| entries[i].take().unwrap())
            .collect();
    }
}
