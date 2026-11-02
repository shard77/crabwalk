use crate::ui::theme;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Widget},
};

pub struct Canvas;

impl Widget for Canvas {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        buf.set_style(area, theme::APP_BACKGROUND);
    }
}

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(rows[1])[1]
}

pub(super) fn master_detail_split(area: Rect, ratio: (u16, u16)) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(ratio.0),
            Constraint::Percentage(ratio.1),
        ])
        .split(area);
    (chunks[0], chunks[1])
}

pub(super) fn panel(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(theme::BORDER)
        .title(Line::from(vec![
            Span::styled("── ", theme::BORDER),
            Span::styled(title, theme::SECTION_HEADER_ACTIVE),
            Span::styled(" ──", theme::BORDER),
        ]))
}

pub(super) fn clip(text: &str, width: usize, ellipsis: bool) -> String {
    let mut visible = String::new();
    let mut used = 0;
    let mut ellipsis_end = 0;
    for grapheme in Span::raw(text).styled_graphemes(Style::default()) {
        let cells = Span::raw(grapheme.symbol).width();
        if used + cells > width {
            if ellipsis && width > 0 {
                visible.truncate(ellipsis_end);
                visible.push('…');
            }
            return visible;
        }
        visible.push_str(grapheme.symbol);
        used += cells;
        if used < width {
            ellipsis_end = visible.len();
        }
    }
    visible
}
