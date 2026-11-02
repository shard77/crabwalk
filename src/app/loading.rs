use super::{APP_EVENT_CAPACITY, App, AppEvent, detail_load};
use crate::github::GitHubClient;
use crate::types::{Comment, FilterSpec};
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::mpsc;

// Dropping a plain JoinHandle detaches the task; replaced loads need to stop instead.
struct LoadTask(tokio::task::JoinHandle<()>);

impl Drop for LoadTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct ActiveLoad<T> {
    _task: LoadTask,
    progress: T,
}

// Generations reject results already queued by a cancelled task.
pub(super) struct LoadSlot<T> {
    generation: u64,
    active: Option<ActiveLoad<T>>,
}

impl<T> Default for LoadSlot<T> {
    fn default() -> Self {
        Self {
            generation: 0,
            active: None,
        }
    }
}

impl<T> LoadSlot<T> {
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.active = None;
    }

    /// Cancel the previous load and read its new generation before spawning this task.
    pub(super) fn start(&mut self, task: tokio::task::JoinHandle<()>, progress: T) {
        self.active = Some(ActiveLoad {
            _task: LoadTask(task),
            progress,
        });
    }

    pub(super) fn accepts(&self, generation: u64) -> bool {
        self.generation == generation && self.active.is_some()
    }

    pub(super) fn progress(&self) -> Option<&T> {
        self.active.as_ref().map(|load| &load.progress)
    }

    pub(super) fn progress_mut(&mut self) -> Option<&mut T> {
        self.active.as_mut().map(|load| &mut load.progress)
    }

    pub(super) fn finish(&mut self) -> Option<T> {
        self.active.take().map(|load| load.progress)
    }
}

async fn fetch_prs(
    github: &GitHubClient,
    tx: &mpsc::Sender<AppEvent>,
    generation: u64,
    filter: FilterSpec,
) -> Result<()> {
    let FilterSpec {
        label,
        author,
        min_inactive_days,
        show_drafts,
        inactivity_sort,
    } = filter;
    if let Some(days) = min_inactive_days {
        // Use the same cutoff on every page so the query does not shift during a load.
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let mut cursor = None;
        loop {
            let (prs, next) = github
                .fetch_pr_search_page(
                    &label,
                    &author,
                    cutoff,
                    show_drafts,
                    inactivity_sort,
                    cursor.as_deref(),
                )
                .await?;
            tx.send(AppEvent::PrPageLoaded { generation, prs })
                .await
                .context("PR receiver closed")?;
            cursor = next;
            if cursor.is_none() {
                break;
            }
        }
    } else {
        let mut page = 1;
        loop {
            let (prs, has_more) = github
                .fetch_pr_page(&label, &author, inactivity_sort, page)
                .await?;
            tx.send(AppEvent::PrPageLoaded { generation, prs })
                .await
                .context("PR receiver closed")?;
            if !has_more {
                break;
            }
            page += 1;
        }
    }
    Ok(())
}

pub(super) struct DetailRequest {
    pub(super) number: u64,
    pub(super) body: Option<String>,
    pub(super) comments: Vec<Comment>,
}

pub struct BackgroundWork {
    pub events: mpsc::Receiver<AppEvent>,
    tx: mpsc::Sender<AppEvent>,
    pub(super) prs: LoadSlot<()>,
    pub(super) detail: LoadSlot<DetailRequest>,
    pub(super) labels: LoadSlot<()>,
}

impl BackgroundWork {
    pub(super) fn new() -> Self {
        let (tx, events) = mpsc::channel(APP_EVENT_CAPACITY);
        Self {
            events,
            tx,
            prs: LoadSlot::default(),
            detail: LoadSlot::default(),
            labels: LoadSlot::default(),
        }
    }

    pub fn is_loading_prs(&self) -> bool {
        self.prs.progress().is_some()
    }

    pub fn is_loading_labels(&self) -> bool {
        self.labels.progress().is_some()
    }

    pub fn detail_progress(&self) -> Option<(u64, usize)> {
        self.detail
            .progress()
            .map(|request| (request.number, request.comments.len()))
    }

    fn is_loading(&self) -> bool {
        self.is_loading_prs() || self.is_loading_labels() || self.detail.progress().is_some()
    }

    pub(super) fn accepts_detail(&self, generation: u64, number: u64) -> bool {
        self.detail.accepts(generation)
            && self
                .detail
                .progress()
                .is_some_and(|request| request.number == number)
    }
}

impl App {
    pub fn set_github_client(&mut self, client: Arc<GitHubClient>) {
        self.background.labels.cancel();
        self.labels.clear();
        self.detail_cache.clear();
        self.github = Some(client);
        self.load_prs();
    }

    pub(super) fn cancel_pr_load(&mut self) {
        self.background.prs.cancel();
    }

    pub(super) fn cancel_detail_load(&mut self) {
        self.background.detail.cancel();
    }

    pub(super) fn cancel_background_loads(&mut self) {
        self.cancel_pr_load();
        self.cancel_detail_load();
        self.background.labels.cancel();
    }

    pub fn is_loading(&self) -> bool {
        self.background.is_loading()
    }

    pub fn load_prs(&mut self) {
        self.cancel_pr_load();
        self.cancel_detail_load();
        let Some(gh) = self.github.as_ref().map(Arc::clone) else {
            return;
        };
        self.main.prs.clear();
        self.main.selected = 0;
        self.main.detail.reset();
        self.main.loaded_filter = Some(self.main.filter.clone());
        self.clear_reader();
        let tx = self.background.tx.clone();
        let filter = self.main.filter.clone();
        let load_gen = self.background.prs.generation();

        self.background.prs.start(
            tokio::spawn(async move {
                let event = match fetch_prs(&gh, &tx, load_gen, filter).await {
                    Ok(()) => AppEvent::PrsComplete {
                        generation: load_gen,
                    },
                    Err(error) => AppEvent::PrsFailed {
                        generation: load_gen,
                        message: format!("Failed to load PRs: {error:#}"),
                    },
                };
                let _ = tx.send(event).await;
            }),
            (),
        );
    }

    pub fn load_detail(&mut self, number: u64) {
        if self.main.detail.loaded_pr == Some(number) {
            return;
        }
        if self
            .background
            .detail
            .progress()
            .is_some_and(|request| request.number == number)
        {
            return;
        }
        self.cancel_detail_load();
        let Some(gh) = self.github.as_ref().map(Arc::clone) else {
            return;
        };
        self.main.detail.loaded_pr = None;

        if let Some(d) = self.detail_cache.get(number) {
            self.main.detail.set_snapshot(Some(d));
            self.main.detail.loaded_pr = Some(number);
            self.recompute_detail();
            return;
        }

        let tx = self.background.tx.clone();
        let generation = self.background.detail.generation();
        self.main.detail.set_snapshot(None);
        self.recompute_detail();

        self.background.detail.start(
            tokio::spawn(async move {
                let event = match detail_load::fetch(&gh, &tx, generation, number).await {
                    Ok(()) => AppEvent::DetailComplete { generation, number },
                    Err(error) => AppEvent::DetailFailed {
                        generation,
                        number,
                        message: format!("#{number}: {error:#}"),
                    },
                };
                let _ = tx.send(event).await;
            }),
            DetailRequest {
                number,
                body: None,
                comments: Vec::new(),
            },
        );
    }

    pub fn ensure_label_cache(&mut self) {
        if self.labels.is_loaded() || self.background.is_loading_labels() {
            return;
        }
        let Some(gh) = self.github.as_ref().map(Arc::clone) else {
            return;
        };
        let tx = self.background.tx.clone();
        self.background.labels.cancel();
        let generation = self.background.labels.generation();

        self.background.labels.start(
            tokio::spawn(async move {
                match gh.fetch_all_label_names().await {
                    Ok(names) => {
                        let _ = tx
                            .send(AppEvent::LabelCacheLoaded { generation, names })
                            .await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(AppEvent::LabelCacheFailed {
                                generation,
                                message: format!("Failed to load labels: {e:#}"),
                            })
                            .await;
                    }
                }
            }),
            (),
        );
    }
}
