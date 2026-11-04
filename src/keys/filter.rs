use crate::app::{App, FilterMenuAction, Modal, filter_menu_items};
use crate::keys::cycle;
use crate::keys::text_input::{TextInput, TextInputResult};
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
            KeyCode::Enter => {
                app.activate_filter_menu_item(selected);
                return;
            }
            KeyCode::Char(' ') if items[selected] == FilterMenuAction::Drafts => {
                app.toggle_drafts();
            }
            _ => {}
        }
    }
    app.modal = Modal::FilterMenu { selected };
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
        TextInputResult::Submit => app.save_current_quick_filter(name_text),
        _ => app.modal = Modal::SaveQuickFilter { name_text },
    }
}
