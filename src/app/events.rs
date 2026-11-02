use super::{App, ContextAction, Modal, detail_load};
use crate::types::{Comment, ItemDetail, PrSummary};

#[derive(Debug)]
pub enum AppEvent {
    PrPageLoaded {
        generation: u64,
        prs: Vec<PrSummary>,
    },
    PrsComplete {
        generation: u64,
    },
    PrsFailed {
        generation: u64,
        message: String,
    },
    DetailBodyLoaded {
        generation: u64,
        number: u64,
        body: Option<String>,
    },
    DetailCommentsPage {
        generation: u64,
        number: u64,
        comments: Vec<Comment>,
    },
    DetailComplete {
        generation: u64,
        number: u64,
    },
    DetailFailed {
        generation: u64,
        number: u64,
        message: String,
    },
    LabelCacheLoaded {
        generation: u64,
        names: Vec<String>,
    },
    LabelCacheFailed {
        generation: u64,
        message: String,
    },
}

enum DetailUpdateSource {
    Snapshot,
    ActiveStream,
    FailedStream(Vec<Comment>),
}

struct PendingDetailUpdate {
    selected: Option<super::detail::DetailEntry<'static>>,
    source: DetailUpdateSource,
}

impl App {
    // Batch consecutive detail pages to avoid rebuilding after each page. Flush before other
    // events, which may change the selected PR.
    pub fn handle_app_events(&mut self, events: impl IntoIterator<Item = AppEvent>) {
        let mut pending = None;
        for event in events {
            if !matches!(
                &event,
                AppEvent::DetailBodyLoaded { .. }
                    | AppEvent::DetailCommentsPage { .. }
                    | AppEvent::DetailComplete { .. }
                    | AppEvent::DetailFailed { .. }
            ) {
                self.flush_detail_update(&mut pending);
            }
            self.handle_app_event_inner(event, &mut pending);
        }
        self.flush_detail_update(&mut pending);
    }

    fn flush_detail_update(&mut self, pending: &mut Option<PendingDetailUpdate>) {
        let Some(update) = pending.take() else { return };
        match update.source {
            DetailUpdateSource::Snapshot => {}
            DetailUpdateSource::ActiveStream => {
                if let Some(request) = self.background.detail.progress_mut() {
                    request.comments.sort_by_key(detail_load::chronological_key);
                    self.main.detail.set_streamed_comments(&request.comments);
                }
            }
            DetailUpdateSource::FailedStream(mut comments) => {
                comments.sort_by_key(detail_load::chronological_key);
                self.main.detail.set_streamed_comments(&comments);
            }
        }
        self.recompute_detail_with_selection(update.selected);
    }

    fn handle_app_event_inner(
        &mut self,
        event: AppEvent,
        pending: &mut Option<PendingDetailUpdate>,
    ) {
        match event {
            AppEvent::PrPageLoaded { generation, prs } => {
                if !self.background.prs.accepts(generation) {
                    return;
                }
                self.main.prs.extend(prs);
                self.main.sort_prs();
            }
            AppEvent::PrsComplete { generation } => {
                if !self.background.prs.accepts(generation) {
                    return;
                }
                self.background.prs.finish();
                self.select_pr(0);
            }
            AppEvent::PrsFailed {
                generation,
                message,
            } => {
                if !self.background.prs.accepts(generation) {
                    return;
                }
                self.background.prs.finish();
                if self.main.prs.is_empty() {
                    self.main.loaded_filter = None;
                }
                self.modal = Modal::Error(message);
            }
            AppEvent::DetailBodyLoaded {
                generation,
                number,
                body,
            } => {
                if !self.background.accepts_detail(generation, number) {
                    return;
                }
                let update = pending.get_or_insert_with(|| PendingDetailUpdate {
                    selected: self.selected_detail_entry(),
                    source: DetailUpdateSource::ActiveStream,
                });
                self.background
                    .detail
                    .progress_mut()
                    .expect("accepted request")
                    .body
                    .clone_from(&body);
                self.main.detail.set_streamed_body(body);
                update.source = DetailUpdateSource::ActiveStream;
            }
            AppEvent::DetailCommentsPage {
                generation,
                number,
                comments,
            } => {
                if !self.background.accepts_detail(generation, number) {
                    return;
                }
                let update = pending.get_or_insert_with(|| PendingDetailUpdate {
                    selected: self.selected_detail_entry(),
                    source: DetailUpdateSource::Snapshot,
                });
                update.source = DetailUpdateSource::ActiveStream;
                self.background
                    .detail
                    .progress_mut()
                    .expect("accepted request")
                    .comments
                    .extend(comments);
            }
            AppEvent::DetailComplete { generation, number } => {
                if !self.background.accepts_detail(generation, number) {
                    return;
                }
                let request = self.background.detail.finish().expect("accepted request");
                let mut comments = request.comments;
                comments.sort_by_key(detail_load::chronological_key);
                let detail = ItemDetail {
                    body: request.body,
                    comments,
                };
                // Aborting cannot retract a queued completion; cache it only after checking its generation.
                let detail = self.detail_cache.insert(number, detail);
                let update = pending.get_or_insert_with(|| PendingDetailUpdate {
                    selected: self.selected_detail_entry(),
                    source: DetailUpdateSource::Snapshot,
                });
                update.source = DetailUpdateSource::Snapshot;
                self.main.detail.set_snapshot(Some(detail));
                self.main.detail.loaded_pr = Some(number);
            }
            AppEvent::DetailFailed {
                generation,
                number,
                message,
            } => {
                if !self.background.accepts_detail(generation, number) {
                    return;
                }
                let request = self.background.detail.finish().expect("accepted request");
                if let Some(update) = pending
                    && matches!(update.source, DetailUpdateSource::ActiveStream)
                {
                    update.source = DetailUpdateSource::FailedStream(request.comments);
                }
                // A failed load may stay visible, but must not become a complete cache hit.
                self.modal = Modal::Error(message);
            }
            AppEvent::LabelCacheLoaded { generation, names } => {
                if !self.background.labels.accepts(generation) {
                    return;
                }
                self.background.labels.finish();
                self.labels.set_names(names);
                if let Some(action) = self
                    .context
                    .dropdown
                    .as_ref()
                    .map(|dropdown| dropdown.action)
                    && matches!(
                        action,
                        ContextAction::FilterLabel | ContextAction::EditQuickFilterLabel
                    )
                {
                    let items = self.label_dropdown_items(action);
                    let dropdown = self.context.dropdown.as_mut().unwrap();
                    dropdown.items = items;
                    dropdown.selected = 0;
                }
            }
            AppEvent::LabelCacheFailed {
                generation,
                message,
            } => {
                if !self.background.labels.accepts(generation) {
                    return;
                }
                self.background.labels.finish();
                self.context.dropdown = None;
                self.modal = Modal::Error(message);
            }
        }
    }
}
