use crate::app::{App, Modal};
use crate::keys::is_plain_key;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_main_key(app: &mut App, key: KeyEvent) {
    if handle_pr_action(app, key) {
        return;
    }
    if app.main.detail.fullscreen {
        super::detail::handle_detail_fullscreen_key(app, key);
        return;
    }
    if let Some(index) = super::list_navigation_target(
        key,
        app.main.selected,
        app.main.prs.len(),
        app.navigation.page_rows,
    ) {
        app.select_pr(index);
        return;
    }
    match key.code {
        KeyCode::Tab => app.context.focus(false),
        KeyCode::BackTab => app.context.focus(true),
        KeyCode::Enter => app.open_selected_pr_detail(),
        _ => {}
    }
}

fn handle_pr_action(app: &mut App, key: KeyEvent) -> bool {
    if !is_plain_key(key) {
        return false;
    }
    match key.code {
        KeyCode::Char('f') => app.modal = Modal::FilterMenu { selected: 0 },
        KeyCode::Char('e') => app.edit_selected_pr_note(),
        KeyCode::Char('a') => app.add_to_report(String::new()),
        KeyCode::Char('o') => app.open_selected_pr_in_browser(),
        KeyCode::Char('r') => app.refresh_prs(),
        _ => return false,
    }
    true
}
