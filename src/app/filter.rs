use super::{App, ContextAction, DeletionTarget, Modal, SettingsSection, View};
use crate::types::QuickFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMenuAction {
    Label,
    Author,
    InactiveDays,
    InactivitySort,
    Drafts,
    Preset(usize),
    SavePreset,
    SetDefault,
    DeletePreset,
}

pub fn filter_menu_items(app: &App) -> Vec<FilterMenuAction> {
    use FilterMenuAction::*;
    let mut items = vec![Label, Author, InactiveDays, InactivitySort, Drafts];
    items.extend((0..app.main.quick_filters.len()).map(Preset));
    items.extend([SavePreset, SetDefault, DeletePreset]);
    items
}

impl App {
    pub fn activate_filter_menu_item(&mut self, selected: usize) {
        let Some(action) = filter_menu_items(self).get(selected).copied() else {
            return;
        };
        match action {
            FilterMenuAction::Label
            | FilterMenuAction::Author
            | FilterMenuAction::InactiveDays
            | FilterMenuAction::InactivitySort => {
                self.main.filter_menu_return = Some(selected);
                let action = match action {
                    FilterMenuAction::Label => ContextAction::FilterLabel,
                    FilterMenuAction::Author => ContextAction::FilterAuthor,
                    FilterMenuAction::InactivitySort => ContextAction::FilterInactivitySort,
                    _ => ContextAction::FilterInactiveDays,
                };
                self.open_dropdown_for(action);
                return;
            }
            FilterMenuAction::Drafts => {
                self.toggle_drafts();
            }
            FilterMenuAction::SavePreset => {
                self.main.filter_menu_return = Some(selected);
                self.modal = Modal::SaveQuickFilter {
                    name_text: String::new(),
                };
                return;
            }
            FilterMenuAction::SetDefault => {
                if !self.save_default_filter() {
                    self.main.filter_menu_return = Some(selected);
                    return;
                }
            }
            FilterMenuAction::DeletePreset => {
                if let Some(index) = self.main.active_qf {
                    self.main.filter_menu_return = Some(selected);
                    self.modal = Modal::ConfirmDelete {
                        target: DeletionTarget::QuickFilter(index),
                    };
                    return;
                }
            }
            FilterMenuAction::Preset(index) => {
                self.apply_quick_filter_and_reload(index);
            }
        }
        self.modal = Modal::FilterMenu { selected };
    }

    pub fn delete_quick_filter(&mut self, index: usize) {
        if index >= self.main.quick_filters.len() {
            return;
        }
        // Saving changes the menu; capture its return row first.
        let return_action = self
            .main
            .filter_menu_return
            .and_then(|selected| filter_menu_items(self).get(selected).copied());
        match self.save_preferences(|prefs| {
            prefs.quick_filters.remove(index);
        }) {
            Ok(()) => {
                self.main.active_qf = match self.main.active_qf {
                    Some(active) if active == index => None,
                    Some(active) if active > index => Some(active - 1),
                    active => active,
                };
                if self.navigation.view == View::Settings
                    && self.settings.selected_section == SettingsSection::QuickFilters
                {
                    self.cancel_settings_quick_filter();
                    self.settings
                        .select_index(index.min(self.main.quick_filters.len()));
                }
                if let Some(action) = return_action {
                    self.main.filter_menu_return = filter_menu_items(self)
                        .iter()
                        .position(|item| *item == action);
                }
                self.set_status_message("Preset removed.");
            }
            Err(error) => self.modal = Modal::Error(format!("Failed to remove preset: {error}")),
        }
    }

    pub fn save_current_quick_filter(&mut self, name_text: String) {
        let name = name_text.trim().to_string();
        if name.is_empty() {
            self.modal = Modal::SaveQuickFilter { name_text };
            return;
        }
        let filter = self.main.filter.clone();
        let new_qf = QuickFilter { name, filter };
        // Saving changes the menu; capture its return row first.
        let return_to_save = self.main.filter_menu_return.is_some_and(|selected| {
            filter_menu_items(self).get(selected) == Some(&FilterMenuAction::SavePreset)
        });
        match self.save_preferences(|prefs| prefs.quick_filters.push(new_qf)) {
            Ok(()) => {
                self.main.active_qf = Some(self.main.quick_filters.len() - 1);
                if return_to_save {
                    self.main.filter_menu_return = filter_menu_items(self)
                        .iter()
                        .position(|item| *item == FilterMenuAction::SavePreset);
                }
                self.set_status_message("Quick filter saved.");
            }
            Err(e) => {
                self.modal = Modal::Error(format!("Failed to save config: {e}"));
            }
        }
    }

    fn save_default_filter(&mut self) -> bool {
        let filter = self.main.filter.clone();
        match self.save_preferences(|prefs| prefs.default_filter = filter) {
            Ok(()) => {
                self.set_status_message("Default filter saved.");
                true
            }
            Err(error) => {
                self.modal = Modal::Error(format!("Failed to save default: {error}"));
                false
            }
        }
    }

    pub fn toggle_drafts(&mut self) {
        self.main.filter.show_drafts = !self.main.filter.show_drafts;
        self.apply_filter_and_reload();
    }
}
