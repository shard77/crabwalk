use super::App;
use crate::markdown::{self, RenderedDocument};
use std::collections::HashSet;

pub(crate) mod navigation;
pub use navigation::SearchMatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReaderMode {
    Normal,
    Visual,
    VisualLine,
    Search,
}

pub struct ReaderState {
    pub inline_code: bool,
    pub markdown: markdown::MarkdownDocument,
    pub rendered: RenderedDocument,
    pub expanded_code_blocks: HashSet<usize>,
    pub focused_element: Option<usize>,
    // (line, character column), matching rendered.plain_lines rather than terminal cells.
    pub cursor: (usize, usize),
    pub visual_start: Option<(usize, usize)>,
    pub mode: ReaderMode,
    pub pending_prefix: Option<char>,
    pub search_query: String,
    pub search_matches: Vec<SearchMatch>,
    pub search_match_index: usize,
}

impl ReaderState {
    pub fn start_search(&mut self) {
        self.pending_prefix = None;
        self.mode = ReaderMode::Search;
        self.search_query.clear();
    }

    pub fn cursor_line_text(&self) -> Option<String> {
        self.rendered.plain_lines.get(self.cursor.0).cloned()
    }

    pub fn advance_search_match(&mut self, forward: bool, update_focus: bool) {
        if self.search_matches.is_empty() {
            return;
        }
        let count = self.search_matches.len();
        self.search_match_index = if forward {
            (self.search_match_index + 1) % count
        } else {
            (self.search_match_index + count - 1) % count
        };
        self.cursor = self.search_matches[self.search_match_index].start;
        if update_focus {
            self.focused_element =
                navigation::interactive_element_at_line(&self.rendered.interactive, self.cursor.0);
        }
    }
}

impl App {
    pub(super) fn clear_reader(&mut self) {
        self.main.detail.reader = None;
    }

    pub fn unfocus_detail(&mut self) {
        self.remember_detail_expansion();
        self.clear_reader();
        self.main.detail.fullscreen_cache = None;
    }

    pub fn enter_reader(&mut self) {
        if let Some(reader) = self.build_reader_state() {
            self.open_reader(reader);
        }
    }

    pub fn focus_detail_code_block(&mut self) -> bool {
        if let Some(index) = self.main.detail.section.checked_sub(1) {
            let Some(detail) = self.main.detail.snapshot().as_ref() else {
                return false;
            };
            let visibility = self
                .main
                .detail
                .review_threads()
                .expect("detail present")
                .visibility_for(&self.main.detail.expansion.expanded_comments(detail));
            if visibility.reason(index).is_some() || visibility.representative(index) != index {
                return false;
            }
        }
        let Some(mut reader) = self.build_reader_state() else {
            return false;
        };
        let Some((index, element)) =
            reader
                .rendered
                .interactive
                .iter()
                .enumerate()
                .find(|(_, element)| {
                    matches!(element.kind, markdown::InteractiveKind::CodeBlock { .. })
                })
        else {
            return false;
        };
        reader.focused_element = Some(index);
        reader.cursor = (element.line_start, 0);
        reader.inline_code = true;
        self.open_reader(reader);
        true
    }

    fn build_reader_state(&self) -> Option<ReaderState> {
        let state = &self.main.detail;
        let section = state.section;
        let body = if section == 0 {
            state
                .snapshot()
                .as_ref()
                .and_then(|detail| detail.body.as_deref())
                .unwrap_or("")
                .to_string()
        } else {
            state
                .snapshot()
                .as_ref()
                .and_then(|detail| detail.comments.get(section - 1))
                .map(|comment| {
                    // Even an empty summary needs a reader so it can open its resolved discussions.
                    if comment.body.trim().is_empty()
                        && (state
                            .review_threads()
                            .expect("detail present")
                            .default_visibility()
                            .reason(section - 1)
                            .is_some()
                            || match &comment.kind {
                                crate::types::CommentKind::Review { .. } => true,
                                crate::types::CommentKind::ReviewComment(context) => {
                                    context.resolved
                                        || comment.outdated
                                        || !context.diff_hunk.is_empty()
                                }
                                crate::types::CommentKind::Issue { .. }
                                | crate::types::CommentKind::Activity { .. } => false,
                            })
                    {
                        "(empty comment)".to_string()
                    } else {
                        comment.reader_source().into_owned()
                    }
                })
                .unwrap_or_default()
        };
        if body.is_empty() {
            return None;
        }
        let expanded_code_blocks = state
            .expansion
            .code_blocks(section, state.snapshot(), &body);
        let markdown = markdown::MarkdownDocument::new(body);
        let rendered = markdown.render(&expanded_code_blocks, &self.highlights);
        if rendered.plain_lines.is_empty() {
            return None;
        }
        let focused_element = (!rendered.interactive.is_empty()).then_some(0);
        let cursor = focused_element
            .and_then(|index| rendered.interactive.get(index))
            .map(|element| (element.line_start, 0))
            .unwrap_or((0, 0));
        Some(ReaderState {
            inline_code: false,
            markdown,
            rendered,
            expanded_code_blocks,
            focused_element,
            cursor,
            visual_start: None,
            mode: ReaderMode::Normal,
            pending_prefix: None,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_index: 0,
        })
    }

    fn open_reader(&mut self, reader: ReaderState) {
        self.main.detail.reader = Some(reader);
        self.remember_detail_expansion();
        self.main.detail.fullscreen_cache = None;
        self.main.detail.viewport.positioned_section = None;
    }
}
