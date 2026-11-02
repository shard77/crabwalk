use crate::app::App;
use crate::keys::cycle;
use crate::keys::text_input::{TextInput, TextInputResult};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_dropdown_key(app: &mut App, key: KeyEvent) {
    let Some(ref dropdown) = app.context.dropdown else {
        return;
    };

    let has_search = dropdown.action.has_search();
    let key = if has_search {
        key
    } else {
        let Some(key) = super::navigation_key(app, key) else {
            return;
        };
        key
    };
    let len = app.dropdown_filtered_items().len();
    let dropdown = app.context.dropdown.as_ref().unwrap();
    let selected = dropdown.selected;
    let page = usize::from(dropdown.viewport_height);
    if let Some(index) = super::list_navigation_target(key, selected, len, page) {
        app.context.dropdown.as_mut().unwrap().selected = index;
        return;
    }
    match key.code {
        KeyCode::Esc => {
            app.context.dropdown = None;
        }
        KeyCode::Enter => {
            app.apply_dropdown_selection();
        }
        KeyCode::Tab | KeyCode::BackTab => {
            let dropdown = app.context.dropdown.as_mut().unwrap();
            dropdown.selected = cycle(dropdown.selected, len, key.code == KeyCode::BackTab);
        }
        _ if has_search => {
            let dropdown = app.context.dropdown.as_mut().unwrap();
            if TextInput::new(&mut dropdown.search_query).handle_key(key)
                == TextInputResult::Changed
            {
                dropdown.selected = 0;
            }
        }
        _ => {}
    }
}
