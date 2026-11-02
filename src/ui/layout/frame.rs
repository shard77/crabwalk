use crate::app::View;
use crate::ui::theme;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct AppFrame<'a> {
    active_view: &'a View,
    context_line: Line<'a>,
}

impl<'a> AppFrame<'a> {
    pub fn new(active_view: &'a View, context_line: Line<'a>) -> Self {
        Self {
            active_view,
            context_line,
        }
    }

    /// Returns (content_area, status_area, context_line_area).
    pub fn render_and_get_areas(self, area: Rect, buf: &mut Buffer) -> (Rect, Rect, Rect) {
        let tab_title = self.build_tab_title();

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::FRAME_BORDER)
            .title(tab_title);

        let inner = outer_block.inner(area);
        outer_block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // context line
                Constraint::Length(1), // separator
                Constraint::Min(0),    // content area
                Constraint::Length(1), // status bar
            ])
            .split(inner);

        let ctx_area = chunks[0];
        Paragraph::new(self.context_line).render(ctx_area, buf);

        render_separator(area, chunks[1], buf);

        (chunks[2], chunks[3], ctx_area)
    }

    fn build_tab_title(&self) -> Line<'a> {
        let views = [
            ("PRs", View::Main),
            ("Report", View::Report),
            ("Settings", View::Settings),
        ];

        let mut spans = vec![Span::styled(" crabwalk ", theme::TAB_ACTIVE)];
        spans.push(Span::styled("── ", theme::FRAME_BORDER));

        for (name, view) in &views {
            if self.active_view == view {
                spans.push(Span::styled("[", theme::TAB_ACTIVE));
                spans.push(Span::styled(*name, theme::TAB_ACTIVE));
                spans.push(Span::styled("] ", theme::TAB_ACTIVE));
            } else {
                spans.push(Span::styled(format!("{name} "), theme::TAB_INACTIVE));
            }
        }

        Line::from(spans)
    }
}

fn render_separator(outer: Rect, sep_area: Rect, buf: &mut Buffer) {
    if sep_area.width == 0 || sep_area.height == 0 {
        return;
    }
    let y = sep_area.y;
    let left_x = outer.x;
    let right_x = outer.x + outer.width.saturating_sub(1);

    buf[(left_x, y)]
        .set_symbol("├")
        .set_style(theme::FRAME_BORDER);

    for x in (left_x + 1)..right_x {
        buf[(x, y)].set_symbol("─").set_style(theme::FRAME_BORDER);
    }

    if right_x > left_x {
        buf[(right_x, y)]
            .set_symbol("┤")
            .set_style(theme::FRAME_BORDER);
    }
}
