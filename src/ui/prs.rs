mod conversation;
mod detail;
mod list;
mod metadata;
mod review_diff;
mod reviews;
mod text;

use crate::app::App;
use crate::ui::widgets::master_detail_split;
use ratatui::{Frame, layout::Rect};

pub(super) use detail::fullscreen_detail_context as detail_context;

pub(super) fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    if app.main.detail.fullscreen {
        detail::draw_fullscreen(f, app, area);
        return;
    }
    let (list_area, detail_area) = master_detail_split(area, (45, 55));
    app.navigation.page_rows = list_area.height.saturating_sub(2).into();
    list::draw(f, app, list_area);
    detail::draw_preview(f, app, detail_area);
}
