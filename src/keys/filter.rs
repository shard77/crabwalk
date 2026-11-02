use crate::app::{
    App, ContextAction, DeletionTarget, FilterMenuAction, Modal, View, filter_menu_items,
};
use crate::keys::cycle;
use crate::keys::text_input::{TextInput, TextInputResult};
use crate::types::QuickFilter;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_filter_menu_key(app: &mut App, key: KeyEvent, mut selected: usize) {
    let items = filter_menu_items(app);
    selected = selected.min(items.len() - 1);
    if let Some(index) =
        super::list_navigation_target(key, selected, items.len(), app.navigation.page_rows)
    {
        selected = index;
    } else {
        match key.code {
            KeyCode::Esc => return,
            KeyCode::Tab | KeyCode::BackTab => {
                selected = cycle(selected, items.len(), key.code == KeyCode::BackTab)
            }
            KeyCode::Enter => match items[selected] {
                FilterMenuAction::Label
                | FilterMenuAction::Author
                | FilterMenuAction::InactiveDays
                | FilterMenuAction::InactivitySort => {
                    app.main.filter_menu_return = Some(selected);
                    let action = match items[selected] {
                        FilterMenuAction::Label => ContextAction::FilterLabel,
                        FilterMenuAction::Author => ContextAction::FilterAuthor,
                        FilterMenuAction::InactivitySort => ContextAction::FilterInactivitySort,
                        _ => ContextAction::FilterInactiveDays,
                    };
                    super::open_dropdown_for(app, action);
                    return;
                }
                FilterMenuAction::Drafts => {
                    app.main.filter.show_drafts = !app.main.filter.show_drafts;
                    app.apply_filter_and_reload();
                }
                FilterMenuAction::SavePreset => {
                    app.main.filter_menu_return = Some(selected);
                    app.modal = Modal::SaveQuickFilter {
                        name_text: String::new(),
                    };
                    return;
                }
                FilterMenuAction::SetDefault => {
                    let filter = app.main.filter.clone();
                    match app.save_preferences(|prefs| prefs.default_filter = filter) {
                        Ok(()) => app.set_status_message("Default filter saved."),
                        Err(error) => {
                            app.main.filter_menu_return = Some(selected);
                            app.modal = Modal::Error(format!("Failed to save default: {error}"));
                            return;
                        }
                    }
                }
                FilterMenuAction::DeletePreset => {
                    if let Some(index) = app.main.active_qf {
                        app.main.filter_menu_return = Some(selected);
                        app.modal = Modal::ConfirmDelete {
                            target: DeletionTarget::QuickFilter(index),
                        };
                        return;
                    }
                }
                FilterMenuAction::Preset(index) => {
                    app.apply_quick_filter_and_reload(index);
                }
            },
            KeyCode::Char(' ') if items[selected] == FilterMenuAction::Drafts => {
                app.main.filter.show_drafts = !app.main.filter.show_drafts;
                app.apply_filter_and_reload();
            }
            _ => {}
        }
    }
    app.modal = Modal::FilterMenu { selected };
}

pub fn delete_quick_filter(app: &mut App, index: usize) {
    if index >= app.main.quick_filters.len() {
        return;
    }
    // Saving changes the menu; capture its return row first.
    let return_action = app
        .main
        .filter_menu_return
        .and_then(|selected| filter_menu_items(app).get(selected).copied());
    match app.save_preferences(|prefs| {
        prefs.quick_filters.remove(index);
    }) {
        Ok(()) => {
            app.main.active_qf = match app.main.active_qf {
                Some(active) if active == index => None,
                Some(active) if active > index => Some(active - 1),
                active => active,
            };
            if app.navigation.view == View::Settings
                && app.settings.selected_section == crate::app::SettingsSection::QuickFilters
            {
                app.cancel_settings_quick_filter();
                app.settings
                    .select_index(index.min(app.main.quick_filters.len()));
            }
            if let Some(action) = return_action {
                app.main.filter_menu_return = filter_menu_items(app)
                    .iter()
                    .position(|item| *item == action);
            }
            app.set_status_message("Preset removed.");
        }
        Err(error) => app.modal = Modal::Error(format!("Failed to remove preset: {error}")),
    }
}

pub fn handle_save_quick_filter_key(app: &mut App, key: KeyEvent, mut name_text: String) {
    let result = TextInput::new(&mut name_text).handle_key(key);
    match result {
        TextInputResult::Cancel => {
            app.modal = Modal::None;
        }
        TextInputResult::Changed => {
            app.modal = Modal::SaveQuickFilter { name_text };
        }
        TextInputResult::Submit => {
            let name = name_text.trim().to_string();
            if name.is_empty() {
                app.modal = Modal::SaveQuickFilter { name_text };
                return;
            }
            let filter = app.main.filter.clone();
            let new_qf = QuickFilter { name, filter };
            // Saving changes the menu; capture its return row first.
            let return_to_save = app.main.filter_menu_return.is_some_and(|selected| {
                filter_menu_items(app).get(selected) == Some(&FilterMenuAction::SavePreset)
            });
            match app.save_preferences(|prefs| prefs.quick_filters.push(new_qf)) {
                Ok(()) => {
                    app.main.active_qf = Some(app.main.quick_filters.len() - 1);
                    if return_to_save {
                        app.main.filter_menu_return = filter_menu_items(app)
                            .iter()
                            .position(|item| *item == FilterMenuAction::SavePreset);
                    }
                    app.set_status_message("Quick filter saved.");
                }
                Err(e) => {
                    app.modal = Modal::Error(format!("Failed to save config: {e}"));
                }
            }
        }
        _ => app.modal = Modal::SaveQuickFilter { name_text },
    }
}
