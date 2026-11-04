use super::{App, ReaderState};
use crate::types::{Comment, CommentKind, ConversationIndex, ItemDetail, ReviewThreads};
use ratatui::text::Line;
use std::cell::OnceCell;
use std::collections::HashSet;

mod expansion;
mod navigation;
pub(super) use expansion::DetailEntry;
pub use expansion::DetailExpansion;
use expansion::ExpandedEntry;
mod viewport;
use viewport::DetailViewport;

pub struct FullscreenCache {
    pub focused_row: Option<usize>,
    pub content_width: u16,
    pub lines: Vec<Line<'static>>,
    // Folded children share their visible entry's offset so navigation lands on its placeholder.
    pub section_offsets: Vec<usize>,
    pub comment_offsets: Vec<usize>,
    // Entry ranges exclude separators and collapsed children.
    pub visible_sections: Vec<(usize, std::ops::Range<usize>)>,
}

pub struct CompactDetailCache {
    pub pr_number: u64,
    pub content_width: u16,
    pub content_height: u16,
    pub nerd_font: bool,
    pub lines: Vec<Line<'static>>,
}

#[derive(Default)]
pub struct DetailState {
    snapshot: Option<ItemDetail>,
    conversation_index: OnceCell<ConversationIndex>,
    pub expansion: DetailExpansion,
    pub fullscreen: bool,
    // Section 0 is the description; section N is comments[N - 1].
    pub section: usize,
    pub viewport: DetailViewport,
    pub reader: Option<ReaderState>,
    pub(super) loaded_pr: Option<u64>,
    pub fullscreen_cache: Option<FullscreenCache>,
    pub compact_cache: Option<CompactDetailCache>,
}

impl DetailState {
    pub fn snapshot(&self) -> &Option<ItemDetail> {
        &self.snapshot
    }

    pub(super) fn reset(&mut self) {
        *self = Self {
            fullscreen: self.fullscreen,
            viewport: DetailViewport {
                height: self.viewport.height,
                ..DetailViewport::default()
            },
            ..Self::default()
        };
    }

    pub fn review_threads(&self) -> Option<ReviewThreads<'_>> {
        let detail = self.snapshot.as_ref()?;
        let index = self
            .conversation_index
            .get_or_init(|| ConversationIndex::new(&detail.comments));
        Some(ReviewThreads::with_index(&detail.comments, index))
    }

    pub(super) fn set_snapshot(&mut self, snapshot: Option<ItemDetail>) {
        self.snapshot = snapshot;
    }

    /// Callers must sort comments first, then recompute the detail view after replacing them.
    pub(super) fn set_streamed_comments(&mut self, comments: &[Comment]) {
        let detail = self.snapshot.get_or_insert_default();
        comments.clone_into(&mut detail.comments);
    }

    pub(super) fn set_streamed_body(&mut self, body: Option<String>) {
        self.snapshot.get_or_insert_default().body = body;
    }

    fn invalidate_content(&mut self) {
        self.conversation_index.take();
        self.fullscreen_cache = None;
        self.compact_cache = None;
    }
}

impl App {
    pub fn enter_detail_fullscreen(&mut self) {
        self.main.detail.fullscreen = true;
        self.main.detail.viewport.scroll = 0;
        self.main.detail.viewport.positioned_section = None;
        self.clear_reader();
        self.set_detail_section(0);
    }

    pub(super) fn recompute_detail(&mut self) {
        let selected = self.selected_detail_entry();
        self.recompute_detail_with_selection(selected);
    }

    pub(super) fn selected_detail_entry(&self) -> Option<DetailEntry<'static>> {
        let index = self.main.detail.section.checked_sub(1)?;
        self.main
            .detail
            .snapshot
            .as_ref()?
            .comments
            .get(index)
            .map(|comment| DetailEntry::from_comment(comment).into_owned())
    }

    pub(super) fn recompute_detail_with_selection(
        &mut self,
        selected: Option<DetailEntry<'static>>,
    ) {
        let state = &mut self.main.detail;
        state.invalidate_content();
        if let Some(detail) = &mut state.snapshot {
            detail.group_reviews();
            state.expansion.transfer_hidden_review_expansion(detail);
            if let Some(selected_id) = selected {
                state.section = detail
                    .comments
                    .iter()
                    .position(|comment| DetailEntry::from_comment(comment) == selected_id)
                    .or_else(|| {
                        // Keep selection on the reply when its empty review summary disappears.
                        let DetailEntry::Review(id) = &selected_id else {
                            return None;
                        };
                        detail.comments.iter().position(|comment| {
                            matches!(&comment.kind, CommentKind::ReviewComment(context)
                                if context.review_id.as_deref() == Some(id.as_ref())
                                    && context.reply_to.is_some())
                        })
                    })
                    .map_or(0, |index| index + 1);
            } else {
                state.section = 0;
            }
        }
        state.viewport.positioned_section = None;
        // Streaming can move comments between section indices; discard the old reader before reusing one.
        self.clear_reader();
        if self.main.detail.fullscreen {
            self.main.detail.viewport.pending_scroll = 0;
        }
    }

    pub fn set_detail_section(&mut self, section: usize) {
        self.main.detail.section = section;
        self.main.detail.fullscreen_cache = None;
        self.main.detail.viewport.section_changed();
    }

    pub fn follow_detail_scroll(&mut self) -> Option<usize> {
        let state = &self.main.detail;
        let (section, start) = state.fullscreen_cache.as_ref().and_then(|cache| {
            cache
                .visible_sections
                .iter()
                .find(|(_, rows)| {
                    rows.end > state.viewport.scroll
                        && rows.start < state.viewport.scroll.saturating_add(state.viewport.height)
                })
                .map(|(section, rows)| (*section, rows.start))
        })?;
        if section == state.section {
            return None;
        }
        self.unfocus_detail();
        self.main.detail.section = section;
        self.main.detail.viewport.positioned_section = Some(section);
        Some(start)
    }

    pub fn remember_detail_expansion(&mut self) {
        let state = &mut self.main.detail;
        if !state.fullscreen {
            return;
        }
        if let Some(focus) = &state.reader
            && let Some(id) = DetailEntry::from_section(state.section, &state.snapshot)
        {
            let entry = state
                .expansion
                .entries
                .entry(id.into_owned())
                .or_insert_with(|| ExpandedEntry {
                    code_only: focus.inline_code,
                    ..ExpandedEntry::default()
                });
            entry.code_only &= focus.inline_code;
            if entry.body != focus.markdown.source() {
                entry.body = focus.markdown.source().to_owned();
            }
            if entry.code_blocks != focus.expanded_code_blocks {
                entry.code_blocks.clone_from(&focus.expanded_code_blocks);
            }
        }
    }

    pub fn collapse_detail_section(&mut self) {
        let state = &mut self.main.detail;
        let section = state.section;
        if section == 0 {
            state.expansion.entries.remove(&DetailEntry::Description);
        } else if state.snapshot.is_some() {
            let (group, parents) = {
                let threads = state.review_threads().expect("detail present");
                (
                    threads.expansion_group(section - 1),
                    threads.review_expansion_targets(section - 1),
                )
            };
            let detail = state.snapshot.as_ref().unwrap();
            let visible_reviews: HashSet<_> = detail
                .comments
                .iter()
                .filter_map(|comment| match &comment.kind {
                    CommentKind::Review { id, .. } => Some(id.as_ref()),
                    _ => None,
                })
                .collect();
            for index in group {
                let comment = &detail.comments[index];
                state
                    .expansion
                    .entries
                    .remove(&DetailEntry::from_comment(comment).into_owned());
                // Otherwise the hidden review summary would reopen this discussion on the next page.
                if let CommentKind::ReviewComment(context) = &comment.kind
                    && let Some(id) = &context.review_id
                    && !visible_reviews.contains(id.as_str())
                {
                    state
                        .expansion
                        .entries
                        .remove(&DetailEntry::Review(id.clone().into()));
                }
            }
            // Otherwise the parent expansion would immediately reopen this child.
            for parent in parents {
                state
                    .expansion
                    .entries
                    .remove(&DetailEntry::from_comment(&detail.comments[parent]).into_owned());
            }
        }
        state.reader = None;
        state.fullscreen_cache = None;
        state.viewport.section_changed();
    }
}
