mod clipboard;
mod context;
mod detail;
mod detail_cache;
mod detail_load;
mod events;
mod feedback;
mod loading;
mod prs;
pub(crate) mod reader;
pub(crate) mod report;
mod settings;

pub use context::{ContextAction, ContextBarState, LabelState};
pub use detail::{CompactDetailCache, DetailExpansion, DetailState, FullscreenCache};
pub use events::AppEvent;
pub use feedback::{FeedbackState, HelpState};
pub use loading::BackgroundWork;
pub use prs::MainState;
pub use prs::{FilterMenuAction, filter_menu_items};
pub use reader::{ReaderMode, ReaderState};
pub use report::ReportState;
pub use settings::{QuickFilterField, TokenField};
pub use settings::{SettingsSection, SettingsState};

use crate::config::Preferences;
use crate::github::GitHubClient;
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq)]
pub enum View {
    Main,
    Report,
    Settings,
}

pub struct NavigationState {
    pub view: View,
    pub pending_prefix: bool,
    pub should_quit: bool,
    pub page_rows: usize,
}

impl NavigationState {
    fn new(view: View) -> Self {
        Self {
            view,
            pending_prefix: false,
            should_quit: false,
            page_rows: 20,
        }
    }
}

#[derive(Debug)]
pub enum Modal {
    None,
    NoteEditor {
        // Capture the PR when opening the editor so the note cannot follow a later selection.
        pr_number: u64,
        text: String,
    },
    Help,
    Error(String),
    FilterMenu {
        selected: usize,
    },
    ConfirmDelete {
        target: DeletionTarget,
    },
    SaveQuickFilter {
        name_text: String,
    },
}

#[derive(Debug)]
pub enum DeletionTarget {
    ReportEntry(u64),
    QuickFilter(usize),
}

pub const APP_EVENT_CAPACITY: usize = 64;

pub struct App {
    pub navigation: NavigationState,
    pub main: MainState,
    pub report: ReportState,
    pub settings: SettingsState,
    pub modal: Modal,
    pub help: HelpState,
    pub context: ContextBarState,
    pub feedback: FeedbackState,
    pub background: BackgroundWork,
    pub labels: LabelState,
    pub github: Option<Arc<GitHubClient>>,
    detail_cache: detail_cache::DetailCache,
    pub highlights: crate::markdown::HighlightCache,
}

impl App {
    pub fn new(github: Option<Arc<GitHubClient>>, prefs: Preferences) -> Self {
        let needs_token = github.is_none();

        let view = if needs_token {
            View::Settings
        } else {
            View::Main
        };

        Self {
            navigation: NavigationState::new(view),
            main: MainState::new(prefs.quick_filters, prefs.default_filter, prefs.nerd_font),
            report: ReportState::default(),
            settings: SettingsState::new(needs_token),
            modal: Modal::None,
            help: HelpState::default(),
            context: ContextBarState::default(),
            feedback: FeedbackState::default(),
            background: BackgroundWork::new(),
            labels: LabelState::default(),
            github,
            detail_cache: detail_cache::DetailCache::default(),
            highlights: crate::markdown::HighlightCache::default(),
        }
    }

    pub fn needs_token(&self) -> bool {
        self.github.is_none()
    }

    pub fn load_initial_prs(&mut self) {
        if !self.needs_token() {
            self.load_prs();
        }
    }

    pub fn is_reader_editing(&self) -> bool {
        self.main
            .detail
            .reader
            .as_ref()
            .is_some_and(|reader| reader.mode != ReaderMode::Normal)
    }

    pub fn is_text_entry_focused(&self) -> bool {
        matches!(
            self.modal,
            Modal::NoteEditor { .. } | Modal::SaveQuickFilter { .. }
        ) || self
            .context
            .dropdown
            .as_ref()
            .is_some_and(|dropdown| dropdown.action.has_search())
            || self.is_settings_text_input_focused()
            || self
                .main
                .detail
                .reader
                .as_ref()
                .is_some_and(|reader| reader.mode == ReaderMode::Search)
    }

    pub fn quit(&mut self) {
        self.cancel_background_loads();
        self.navigation.should_quit = true;
    }

    pub fn show_help(&mut self) {
        self.help.scroll = 0;
        if let Some(focus) = self.main.detail.reader.as_mut() {
            focus.pending_prefix = None;
        }
        self.modal = Modal::Help;
    }

    pub fn copy_to_clipboard(&mut self, text: &str, confirmation: &str) {
        match clipboard::copy_to_clipboard(text) {
            Ok(()) => self.set_status_message(confirmation),
            Err(error) => self.modal = Modal::Error(format!("Clipboard: {error}")),
        }
    }
}
