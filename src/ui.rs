mod dialogs;
mod dropdown;
mod filters;
mod help;
mod layout;
mod prs;
mod report;
mod settings;
pub mod theme;
mod widgets;

use crate::app::{App, DeletionTarget, Modal, View};
use ratatui::{Frame, layout::Rect};
use widgets::Canvas;

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    f.render_widget(Canvas, size);
    let (content, status) = layout::draw(f, app);
    match app.navigation.view {
        View::Main => prs::draw(f, app, content),
        View::Report => report::draw(f, app, content),
        View::Settings => settings::draw(f, app, content),
    }
    layout::draw_status(f, app, status, prs::detail_context(app));
    if !matches!(app.modal, Modal::None) {
        draw_modal(f, app, size);
    } else {
        dropdown::draw(f, app, size);
    }
}

fn draw_modal(f: &mut Frame, app: &mut App, size: Rect) {
    match &app.modal {
        Modal::None => {}
        Modal::Help => help::draw(f, app, size),
        Modal::Error(msg) => dialogs::draw_error(f, size, msg),
        Modal::NoteEditor { pr_number, text } => {
            dialogs::draw_note_editor(f, size, *pr_number, text)
        }
        Modal::FilterMenu { selected } => filters::draw_menu(f, app, size, *selected),
        Modal::ConfirmDelete { target } => {
            let label = match target {
                DeletionTarget::ReportEntry(number) => format!("Remove report entry #{number}?"),
                DeletionTarget::QuickFilter(index) => app
                    .main
                    .quick_filters
                    .get(*index)
                    .map(|filter| format!("Remove quick filter '{}'?", filter.name))
                    .unwrap_or_else(|| "Remove quick filter?".to_string()),
            };
            dialogs::draw_confirmation(f, size, &label, "remove");
        }
        Modal::SaveQuickFilter { name_text } => dialogs::draw_preset_name(f, size, name_text),
    }
}
