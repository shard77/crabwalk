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
        KeyCode::Enter => {
            if let Some(pr) = app.main.selected_pr() {
                let number = pr.number;
                if app.main.detail.snapshot().is_none() {
                    app.load_detail(number);
                }
                app.enter_detail_fullscreen();
            }
        }
        _ => {}
    }
}

fn handle_pr_action(app: &mut App, key: KeyEvent) -> bool {
    if !is_plain_key(key) {
        return false;
    }
    match key.code {
        KeyCode::Char('f') => app.modal = Modal::FilterMenu { selected: 0 },
        KeyCode::Char('e') => {
            if let Some(pr) = app.main.selected_pr() {
                let pr_number = pr.number;
                let text = app
                    .report
                    .entries
                    .iter()
                    .find(|entry| entry.pr_number == pr_number)
                    .map(|entry| entry.note.clone())
                    .unwrap_or_default();
                app.modal = Modal::NoteEditor { pr_number, text };
            }
        }
        KeyCode::Char('a') => app.add_to_report(String::new()),
        KeyCode::Char('o') => {
            if let Some(pr) = app.main.selected_pr() {
                let _ = open::that(&pr.html_url);
            }
        }
        KeyCode::Char('r') => app.refresh_prs(),
        _ => return false,
    }
    true
}
