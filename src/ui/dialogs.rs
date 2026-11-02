use crate::ui::{
    theme,
    widgets::{Canvas, centered_rect},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
};

fn text_dialog(
    f: &mut Frame,
    size: Rect,
    (width_pct, height_pct): (u16, u16),
    title: &str,
    border: Style,
    text: String,
) {
    let area = centered_rect(width_pct, height_pct, size);
    f.render_widget(Canvas, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border)
        .padding(Padding::horizontal(1));
    let para = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

pub(super) fn draw_error(f: &mut Frame, size: Rect, msg: &str) {
    let text = format!("{msg}\n\nEnter / Esc: dismiss");
    text_dialog(f, size, (60, 30), "Error", theme::BORDER_ERROR, text);
}

pub(super) fn draw_confirmation(f: &mut Frame, size: Rect, msg: &str, action: &str) {
    let text = format!("{msg}\n\nEnter: {action}   Esc: cancel");
    text_dialog(f, size, (50, 20), "Confirm", theme::BORDER_FOCUSED, text);
}

pub(super) fn draw_preset_name(f: &mut Frame, size: Rect, name_text: &str) {
    let title = "Save Quick Filter — enter a short name, then Enter";
    let text = format!("{name_text}_");
    text_dialog(f, size, (50, 20), title, theme::BORDER_SUCCESS, text);
}

pub(super) fn draw_note_editor(f: &mut Frame, size: Rect, pr_number: u64, text: &str) {
    let title = format!("Note for #{pr_number} — Enter: save, Esc: cancel");
    let text = format!("{text}_");
    text_dialog(f, size, (60, 20), &title, theme::BORDER_FOCUSED, text);
}
