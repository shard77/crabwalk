use super::report::ReportFormat;
use super::{App, Modal, SettingsSection};
use crate::types::{InactivitySort, TriageLabel};
use ratatui::layout::Rect;

const INACTIVE_DAYS: [(&str, Option<u64>); 4] = [
    ("off", None),
    ("7d", Some(7)),
    ("15d", Some(15)),
    ("30d", Some(30)),
];

#[derive(Default)]
pub struct ContextBarState {
    pub rect: Option<Rect>,
    // Columns relative to rect, with an exclusive end.
    pub regions: Vec<(u16, u16, ContextAction)>,
    pub focused: Option<usize>,
    pub dropdown: Option<ContextDropdown>,
}

impl ContextBarState {
    pub fn record_geometry(&mut self, rect: Rect, regions: Vec<(u16, u16, ContextAction)>) {
        self.rect = Some(rect);
        self.regions = regions;
        if self
            .focused
            .is_some_and(|index| index >= self.regions.len())
        {
            self.focused = None;
        }
    }

    pub fn dismiss(&mut self) {
        self.dropdown = None;
        self.focused = None;
    }

    pub fn focus(&mut self, backwards: bool) {
        self.focused =
            (!self.regions.is_empty()).then(|| if backwards { self.regions.len() - 1 } else { 0 });
    }
}

#[derive(Default)]
pub struct LabelState {
    // Cache empty results too, so opening the picker does not fetch them again.
    names: Option<Vec<String>>,
}

impl LabelState {
    pub fn names(&self) -> &[String] {
        self.names.as_deref().unwrap_or_default()
    }

    pub(super) fn is_loaded(&self) -> bool {
        self.names.is_some()
    }

    pub(super) fn set_names(&mut self, names: Vec<String>) {
        self.names = Some(names);
    }

    pub(super) fn clear(&mut self) {
        self.names = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    FilterLabel,
    EditQuickFilterLabel,
    FilterAuthor,
    FilterInactiveDays,
    FilterInactivitySort,
    ToggleDrafts,
    CycleReportFormat,
    SettingsSection,
}

impl ContextAction {
    pub fn has_search(self) -> bool {
        matches!(
            self,
            Self::FilterLabel | Self::EditQuickFilterLabel | Self::FilterAuthor
        )
    }
}

pub struct ContextDropdown {
    pub action: ContextAction,
    pub anchor_col: u16,
    pub anchor_width: u16,
    pub items: Vec<String>,
    // Index into the filtered items, not the full list.
    pub selected: usize,
    pub search_query: String,
    pub viewport_height: u16,
}

impl App {
    pub fn resume_filter_menu(&mut self) {
        if matches!(self.modal, Modal::None)
            && self.context.dropdown.is_none()
            && let Some(selected) = self.main.filter_menu_return.take()
        {
            self.modal = Modal::FilterMenu { selected };
        }
    }

    pub fn open_context_dropdown(
        &mut self,
        action: ContextAction,
        anchor_start: u16,
        anchor_end: u16,
    ) {
        let (items, search_query) = match &action {
            ContextAction::FilterLabel => {
                self.ensure_label_cache();
                (
                    self.label_dropdown_items(action),
                    self.main.filter.label.clone(),
                )
            }
            ContextAction::EditQuickFilterLabel => {
                self.ensure_label_cache();
                (
                    self.label_dropdown_items(action),
                    self.settings.text_input().unwrap_or_default().to_string(),
                )
            }
            ContextAction::FilterAuthor => (vec![], self.main.filter.author.clone()),
            ContextAction::FilterInactiveDays => (
                INACTIVE_DAYS
                    .iter()
                    .map(|(label, _)| (*label).into())
                    .collect(),
                String::new(),
            ),
            ContextAction::FilterInactivitySort => (
                InactivitySort::ALL
                    .iter()
                    .map(|sort| sort.label().into())
                    .collect(),
                String::new(),
            ),
            ContextAction::ToggleDrafts => return,
            ContextAction::CycleReportFormat => (
                ReportFormat::ALL
                    .iter()
                    .map(|format| format.label().into())
                    .collect(),
                String::new(),
            ),
            ContextAction::SettingsSection => (
                SettingsSection::ALL
                    .iter()
                    .map(|section| section.name().into())
                    .collect(),
                String::new(),
            ),
        };
        let selected = match &action {
            ContextAction::FilterInactiveDays => INACTIVE_DAYS
                .iter()
                .position(|(_, days)| *days == self.main.filter.min_inactive_days)
                .unwrap_or(0),
            ContextAction::FilterInactivitySort => InactivitySort::ALL
                .iter()
                .position(|sort| *sort == self.main.filter.inactivity_sort)
                .unwrap_or(0),
            ContextAction::CycleReportFormat => ReportFormat::ALL
                .iter()
                .position(|format| *format == self.report.format)
                .unwrap_or(0),
            ContextAction::SettingsSection => self.settings.selected_section.index(),
            _ => 0,
        };
        self.context.dropdown = Some(ContextDropdown {
            action,
            anchor_col: anchor_start,
            anchor_width: anchor_end.saturating_sub(anchor_start),
            items,
            selected,
            search_query,
            viewport_height: 0,
        });
    }

    pub(super) fn label_dropdown_items(&self, action: ContextAction) -> Vec<String> {
        let names = self.labels.names().iter().cloned();
        if action == ContextAction::EditQuickFilterLabel {
            std::iter::once(String::new()).chain(names).collect()
        } else {
            names.collect()
        }
    }

    pub fn dropdown_filtered_items(&self) -> Vec<String> {
        let Some(ref dropdown) = self.context.dropdown else {
            return vec![];
        };
        if !dropdown.action.has_search() || dropdown.search_query.is_empty() {
            return dropdown.items.clone();
        }
        let q = dropdown.search_query.to_lowercase();
        dropdown
            .items
            .iter()
            .filter(|item| item.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    pub fn apply_dropdown_selection(&mut self) {
        let filtered = self.dropdown_filtered_items();
        let Some(dropdown) = self.context.dropdown.take() else {
            return;
        };
        match dropdown.action {
            ContextAction::FilterLabel | ContextAction::EditQuickFilterLabel => {
                let label = filtered
                    .get(dropdown.selected)
                    .unwrap_or(&dropdown.search_query)
                    .clone();
                if matches!(dropdown.action, ContextAction::EditQuickFilterLabel) {
                    self.apply_settings_label_selection(&label);
                    return;
                }
                let status = TriageLabel::from_label_name(&label);
                self.main.filter.min_inactive_days =
                    Some(TriageLabel::threshold_days(status.as_ref()));
                self.main.filter.label = label;
                self.apply_filter_and_reload();
            }
            ContextAction::FilterAuthor => {
                self.main.filter.author = dropdown.search_query;
                self.apply_filter_and_reload();
            }
            ContextAction::FilterInactiveDays => {
                if let Some((_, days)) = INACTIVE_DAYS.get(dropdown.selected) {
                    self.main.filter.min_inactive_days = *days;
                    self.apply_filter_and_reload();
                }
            }
            ContextAction::FilterInactivitySort => {
                if let Some(sort) = InactivitySort::ALL.get(dropdown.selected) {
                    self.main.filter.inactivity_sort = *sort;
                    self.apply_filter_and_reload();
                }
            }
            ContextAction::ToggleDrafts => {}
            ContextAction::CycleReportFormat => {
                if let Some(format) = ReportFormat::ALL.get(dropdown.selected) {
                    self.report.format = *format;
                }
            }
            ContextAction::SettingsSection => {
                if let Some(section) = SettingsSection::ALL.get(dropdown.selected) {
                    self.select_settings_section(*section);
                }
            }
        }
    }
}
