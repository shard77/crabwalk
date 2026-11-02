use crate::types::{Comment, CommentKind, ItemDetail};
use std::{collections::VecDeque, mem::size_of};

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 64;

struct CachedDetail {
    number: u64,
    bytes: usize,
    detail: ItemDetail,
}

#[derive(Default)]
pub(super) struct DetailCache {
    entries: VecDeque<CachedDetail>,
    bytes: usize,
}

impl DetailCache {
    pub(super) fn get(&mut self, number: u64) -> Option<ItemDetail> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.number == number)?;
        let entry = self.entries.remove(index).unwrap();
        let detail = entry.detail.clone();
        self.entries.push_back(entry);
        Some(detail)
    }

    pub(super) fn insert(&mut self, number: u64, detail: ItemDetail) -> ItemDetail {
        let bytes = estimate_detail_bytes(&detail);
        if let Some(index) = self.entries.iter().position(|entry| entry.number == number) {
            self.bytes -= self.entries.remove(index).unwrap().bytes;
        }
        // An oversized replacement invalidates its old entry without evicting other PRs.
        if bytes <= MAX_BYTES {
            while self.entries.len() >= MAX_ENTRIES || self.bytes > MAX_BYTES - bytes {
                self.bytes -= self.entries.pop_front().unwrap().bytes;
            }
            self.entries.push_back(CachedDetail {
                number,
                bytes,
                detail: detail.clone(),
            });
            self.bytes += bytes;
        }
        detail
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

fn optional_string_capacity(value: &Option<String>) -> usize {
    value.as_ref().map_or(0, String::capacity)
}

fn estimate_detail_bytes(detail: &ItemDetail) -> usize {
    // The vector capacity already includes Comment and its enum fields; add only their heap data.
    let mut bytes = size_of::<ItemDetail>()
        .saturating_add(optional_string_capacity(&detail.body))
        .saturating_add(
            detail
                .comments
                .capacity()
                .saturating_mul(size_of::<Comment>()),
        );
    for comment in &detail.comments {
        for capacity in [
            comment.author.capacity(),
            comment.body.capacity(),
            optional_string_capacity(&comment.path),
            optional_string_capacity(&comment.minimized_reason),
        ] {
            bytes = bytes.saturating_add(capacity);
        }
        match &comment.kind {
            CommentKind::Issue { id } => {
                bytes = bytes.saturating_add(id.capacity());
            }
            CommentKind::Activity { id, action } => {
                bytes = bytes
                    .saturating_add(id.capacity())
                    .saturating_add(action.capacity());
            }
            CommentKind::Review { id, .. } => {
                bytes = bytes.saturating_add(id.capacity());
            }
            CommentKind::ReviewComment(context) => {
                for capacity in [
                    context.id.capacity(),
                    optional_string_capacity(&context.review_id),
                    context.thread_id.capacity(),
                    optional_string_capacity(&context.reply_to),
                    context.diff_hunk.capacity(),
                ] {
                    bytes = bytes.saturating_add(capacity);
                }
            }
        }
    }
    bytes
}
