use crate::app::{App, TokenField};
use crate::keys::cycle;
use crate::keys::is_plain_key;
use crate::keys::text_input::{TextInput, TextInputResult};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if !is_plain_key(key) {
        return;
    }
    let field = app.settings.token_field().unwrap_or(TokenField::Token);
    let selected = app.settings.selected_index();
    match key.code {
        KeyCode::Esc => app.cancel_settings_token(),
        KeyCode::Up if field == TokenField::Token => app.context.focus(false),
        KeyCode::Left => app.context.focus(false),
        KeyCode::Tab | KeyCode::BackTab => {
            let count = app.settings_field_count();
            app.settings
                .select_index(cycle(selected, count, key.code == KeyCode::BackTab));
        }
        KeyCode::Enter if field == TokenField::Token => app.settings.select_index(1),
        KeyCode::Enter | KeyCode::Char(' ') if field == TokenField::SaveToKeyring => {
            let draft = app.settings.token_mut().unwrap();
            draft.save_to_keyring = !draft.save_to_keyring;
        }
        KeyCode::Enter if field == TokenField::Connect => app.connect_settings_token(),
        KeyCode::Enter if field == TokenField::Cancel => app.cancel_settings_token(),
        _ => {
            if let Some(index) = super::list_navigation_target(
                key,
                selected,
                app.settings_field_count(),
                app.navigation.page_rows,
            ) {
                app.settings.select_index(index);
            } else if field == TokenField::Token {
                let draft = app.settings.token_mut().unwrap();
                if TextInput::new(&mut draft.text).handle_key(key) == TextInputResult::Changed {
                    draft.error = None;
                }
            }
        }
    }
}
