use super::{App, DetailState};
use crate::types::{FilterSpec, PrSummary, QuickFilter};

pub struct MainState {
    pub nerd_font: bool,
    pub quick_filters: Vec<QuickFilter>,
    pub active_qf: Option<usize>,
    pub filter: FilterSpec,
    pub default_filter: FilterSpec,
    pub prs: Vec<PrSummary>,
    pub selected: usize,
    // Keep the loaded query separate from filter edits that have not been applied yet.
    pub(super) loaded_filter: Option<FilterSpec>,
    pub detail: DetailState,
    pub filter_menu_return: Option<usize>,
}

impl MainState {
    pub fn new(
        quick_filters: Vec<QuickFilter>,
        default_filter: FilterSpec,
        nerd_font: bool,
    ) -> Self {
        let mut state = Self {
            nerd_font,
            quick_filters,
            active_qf: None,
            filter: default_filter.clone(),
            default_filter,
            prs: Vec::new(),
            selected: 0,
            loaded_filter: None,
            detail: DetailState::default(),
            filter_menu_return: None,
        };
        state.set_custom_filter();
        state
    }

    pub(super) fn select_pr(&mut self, index: usize) -> bool {
        if index >= self.prs.len() {
            return false;
        }
        let previous = self.selected_pr().map(|pr| pr.number);
        self.update_selection(index, previous)
    }

    pub(super) fn sort_prs(&mut self) {
        let selected = self.selected_pr().map(|pr| pr.number);
        self.prs
            .sort_by(|a, b| self.filter.inactivity_sort.compare(a, b));
        self.selected = selected
            .and_then(|number| self.prs.iter().position(|pr| pr.number == number))
            .unwrap_or(0);
    }

    fn update_selection(&mut self, index: usize, previous: Option<u64>) -> bool {
        self.selected = index;
        let changed = previous != self.selected_pr().map(|pr| pr.number);
        if changed {
            self.detail.reset();
        }
        changed
    }

    pub fn selected_pr(&self) -> Option<&PrSummary> {
        self.prs.get(self.selected)
    }

    fn set_quick_filter(&mut self, idx: usize) {
        if let Some(qf) = self.quick_filters.get(idx) {
            self.filter.clone_from(&qf.filter);
            self.active_qf = Some(idx);
        }
    }

    pub fn set_custom_filter(&mut self) {
        self.active_qf = self
            .quick_filters
            .iter()
            .position(|quick_filter| quick_filter.filter == self.filter);
    }
}

impl App {
    pub fn open_selected_pr_detail(&mut self) {
        if let Some(pr) = self.main.selected_pr() {
            let number = pr.number;
            if self.main.detail.snapshot().is_none() {
                self.load_detail(number);
            }
            self.enter_detail_fullscreen();
        }
    }

    pub fn edit_selected_pr_note(&mut self) {
        if let Some(pr) = self.main.selected_pr() {
            self.edit_report_note(pr.number);
        }
    }

    pub fn open_selected_pr_in_browser(&self) {
        if let Some(pr) = self.main.selected_pr() {
            let _ = open::that(&pr.html_url);
        }
    }

    pub fn apply_filter_and_reload(&mut self) {
        self.main.set_custom_filter();
        self.apply_pr_filter();
    }

    pub fn apply_quick_filter_and_reload(&mut self, index: usize) {
        if index < self.main.quick_filters.len() {
            self.main.set_quick_filter(index);
            self.apply_pr_filter();
        }
    }

    fn apply_pr_filter(&mut self) {
        let same_query = self.main.loaded_filter.as_ref().is_some_and(|loaded| {
            let mut requested = self.main.filter.clone();
            requested.inactivity_sort = loaded.inactivity_sort;
            requested == *loaded
        });
        if same_query {
            if self.main.loaded_filter.as_ref() != Some(&self.main.filter) {
                self.main.sort_prs();
            }
            self.main.loaded_filter = Some(self.main.filter.clone());
        } else {
            self.main.detail.fullscreen = false;
            self.load_prs();
        }
    }

    pub fn refresh_prs(&mut self) {
        self.remember_detail_expansion();
        self.detail_cache.clear();
        self.load_prs();
    }

    pub fn select_pr(&mut self, index: usize) {
        if self.main.select_pr(index) {
            self.clear_reader();
        }
        if let Some(pr) = self.main.selected_pr() {
            self.load_detail(pr.number);
        }
    }
}
