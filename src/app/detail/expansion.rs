use crate::types::{Comment, CommentKind, ItemDetail};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

#[derive(Hash, PartialEq, Eq)]
pub(in crate::app) enum DetailEntry<'a> {
    Description,
    Activity(Cow<'a, str>),
    Review(Cow<'a, str>),
    ReviewComment(Cow<'a, str>),
    Issue(Cow<'a, str>),
}

impl<'a> DetailEntry<'a> {
    pub(super) fn from_comment(comment: &'a Comment) -> Self {
        match &comment.kind {
            CommentKind::Activity { id, .. } => Self::Activity(Cow::Borrowed(id)),
            CommentKind::Review { id, .. } => Self::Review(Cow::Borrowed(id)),
            CommentKind::ReviewComment(context) => Self::ReviewComment(Cow::Borrowed(&context.id)),
            CommentKind::Issue { id } => Self::Issue(Cow::Borrowed(id)),
        }
    }

    pub(super) fn into_owned(self) -> DetailEntry<'static> {
        match self {
            Self::Description => DetailEntry::Description,
            Self::Activity(id) => DetailEntry::Activity(Cow::Owned(id.into_owned())),
            Self::Review(id) => DetailEntry::Review(Cow::Owned(id.into_owned())),
            Self::ReviewComment(id) => DetailEntry::ReviewComment(Cow::Owned(id.into_owned())),
            Self::Issue(id) => DetailEntry::Issue(Cow::Owned(id.into_owned())),
        }
    }

    pub(super) fn from_section(section: usize, detail: &'a Option<ItemDetail>) -> Option<Self> {
        if section == 0 {
            Some(Self::Description)
        } else {
            detail
                .as_ref()?
                .comments
                .get(section - 1)
                .map(Self::from_comment)
        }
    }
}

#[derive(Default)]
pub(super) struct ExpandedEntry {
    // Code-block IDs belong to this source; discard them if it changes.
    pub(super) body: String,
    pub(super) code_blocks: HashSet<usize>,
    // Opening a code block must not also expand the enclosing discussion.
    pub(super) code_only: bool,
}

#[derive(Default)]
pub struct DetailExpansion {
    pub(super) entries: HashMap<DetailEntry<'static>, ExpandedEntry>,
}

impl DetailExpansion {
    pub fn is_section_expanded(&self, section: usize, detail: &Option<ItemDetail>) -> bool {
        DetailEntry::from_section(section, detail)
            .and_then(|entry| self.entries.get(&entry))
            .is_some_and(|entry| !entry.code_only)
    }

    pub fn is_comment_expanded(&self, comment: &Comment) -> bool {
        self.entries
            .get(&DetailEntry::from_comment(comment))
            .is_some_and(|entry| !entry.code_only)
    }

    pub fn expanded_comments(&self, detail: &ItemDetail) -> HashSet<usize> {
        detail
            .comments
            .iter()
            .enumerate()
            .filter(|(_, comment)| self.is_comment_expanded(comment))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn code_blocks(
        &self,
        section: usize,
        detail: &Option<ItemDetail>,
        body: &str,
    ) -> HashSet<usize> {
        DetailEntry::from_section(section, detail)
            .and_then(|id| self.entries.get(&id))
            .filter(|entry| entry.body == body)
            .map(|entry| entry.code_blocks.clone())
            .unwrap_or_default()
    }

    pub fn comment_code_blocks(&self, comment: &Comment) -> HashSet<usize> {
        self.entries
            .get(&DetailEntry::from_comment(comment))
            .filter(|entry| entry.body == comment.reader_source())
            .map(|entry| entry.code_blocks.clone())
            .unwrap_or_default()
    }

    // Streaming can remove an empty review summary; transfer its expansion so replies stay open.
    pub(super) fn transfer_hidden_review_expansion(&mut self, detail: &ItemDetail) {
        if !self
            .entries
            .iter()
            .any(|(id, entry)| !entry.code_only && matches!(id, DetailEntry::Review(_)))
        {
            return;
        }
        let visible_reviews: HashSet<_> = detail
            .comments
            .iter()
            .filter_map(|comment| match &comment.kind {
                CommentKind::Review { id, .. } => Some(id.as_ref()),
                _ => None,
            })
            .collect();
        let hidden_open_reviews: HashSet<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| !entry.code_only)
            .filter_map(|(entry, _)| match entry {
                DetailEntry::Review(id) if !visible_reviews.contains(id.as_ref()) => {
                    Some(id.as_ref())
                }
                _ => None,
            })
            .collect();
        let replies: Vec<_> = detail
            .comments
            .iter()
            .filter(|comment| {
                matches!(&comment.kind, CommentKind::ReviewComment(context)
                if context.review_id.as_deref().is_some_and(|id| hidden_open_reviews.contains(id))
                    && context.reply_to.is_some())
            })
            .collect();
        for reply in replies {
            self.entries
                .entry(DetailEntry::from_comment(reply).into_owned())
                .or_insert_with(|| ExpandedEntry {
                    body: reply.body.clone(),
                    code_blocks: HashSet::new(),
                    code_only: false,
                })
                .code_only = false;
        }
    }
}
