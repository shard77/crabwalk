use crate::app::{App, Modal, View};
use crate::keys::is_plain_key;
use crate::keys::text_input::{TextInput, TextInputResult};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_report_key(app: &mut App, key: KeyEvent) {
    if let Some(index) = super::list_navigation_target(
        key,
        app.report.selected,
        app.report.entries.len(),
        app.navigation.page_rows,
    ) {
        app.report.selected = index;
        return;
    }
    if !is_plain_key(key) {
        return;
    }
    match key.code {
        KeyCode::Tab => app.context.focus(false),
        KeyCode::BackTab => app.context.focus(true),
        KeyCode::Char('c' | 'y') => {
            app.copy_report();
        }
        KeyCode::Char('x') | KeyCode::Delete => app.confirm_report_entry_deletion(),
        KeyCode::Char('e') => app.edit_selected_report_note(),
        KeyCode::Esc => {
            app.context.dismiss();
            app.navigation.view = View::Main;
        }
        _ => {}
    }
}

pub(super) fn handle_note_editor_key(
    app: &mut App,
    key: KeyEvent,
    pr_number: u64,
    mut text: String,
) {
    match TextInput::new(&mut text).handle_key(key) {
        TextInputResult::Cancel => {}
        TextInputResult::Submit => app.save_report_note(pr_number, text),
        _ => app.modal = Modal::NoteEditor { pr_number, text },
    }
}
