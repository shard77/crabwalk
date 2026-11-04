use super::reader::{handle_reader_key, is_copy_key, is_search_key};
use crate::app::App;
use crate::keys::{NavigationAction, is_plain_key, navigation_action};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_detail_fullscreen_key(app: &mut App, key: KeyEvent) {
    if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
        app.scroll_detail(if key.code == KeyCode::Tab { 1 } else { -1 });
        return;
    }
    if matches!(key.code, KeyCode::Down | KeyCode::Up)
        && app.navigate_detail_code_block(key.code == KeyCode::Down)
    {
        return;
    }
    if let Some(action) = navigation_action(key)
        && !matches!(action, NavigationAction::Line(_))
    {
        app.move_detail_section(action.delta(app.main.detail.viewport.height));
        return;
    }
    if is_search_key(key) {
        app.start_detail_search();
        return;
    }
    if !is_plain_key(key) {
        if is_copy_key(key) {
            handle_reader_key(app, key);
        }
        return;
    }
    match key.code {
        KeyCode::Esc if app.main.detail.reader.is_some() => app.unfocus_detail(),
        KeyCode::Esc => app.main.detail.fullscreen = false,
        KeyCode::Left => app.collapse_detail_section(),
        KeyCode::Right if app.main.detail.reader.is_none() => app.enter_reader(),
        KeyCode::Down | KeyCode::Up => app.move_detail_focus(key.code == KeyCode::Up),
        KeyCode::Enter | KeyCode::Char(' ') if app.main.detail.reader.is_none() => {
            app.activate_detail_section(key.code == KeyCode::Char(' '));
        }
        _ if app.main.detail.reader.is_some() => handle_reader_key(app, key),
        _ => {}
    }
}
