use crate::app::{App, DeletionTarget, Modal, View};
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
        KeyCode::Char('x') | KeyCode::Delete => {
            if let Some(entry) = app.report.selected_entry() {
                app.modal = Modal::ConfirmDelete {
                    target: DeletionTarget::ReportEntry(entry.pr_number),
                };
            }
        }
        KeyCode::Char('e') => {
            if let Some(entry) = app.report.selected_entry() {
                let pr_number = entry.pr_number;
                let text = entry.note.clone();
                app.modal = Modal::NoteEditor { pr_number, text };
            }
        }
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
        TextInputResult::Submit => {
            if let Some(entry) = app
                .report
                .entries
                .iter_mut()
                .find(|entry| entry.pr_number == pr_number)
            {
                entry.note = text;
                app.set_status_message(format!("Updated note for #{pr_number}"));
            } else if app
                .main
                .selected_pr()
                .is_some_and(|pr| pr.number == pr_number)
            {
                app.add_to_report(text);
            } else {
                app.modal =
                    Modal::Error("The PR changed while editing; note was not applied.".into());
            }
        }
        _ => app.modal = Modal::NoteEditor { pr_number, text },
    }
}

pub(super) fn delete_entry(app: &mut App, number: u64) {
    if let Some(index) = app
        .report
        .entries
        .iter()
        .position(|entry| entry.pr_number == number)
    {
        app.report.selected = index;
        app.report.delete_selected();
        app.set_status_message(format!("Removed #{number} from report"));
    }
}
