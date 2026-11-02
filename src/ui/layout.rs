mod context_bar;
mod frame;
mod status_bar;

use crate::app::App;
use ratatui::{Frame, layout::Rect};

pub(super) use status_bar::draw as draw_status;

pub(super) fn draw(f: &mut Frame, app: &mut App) -> (Rect, Rect) {
    let (context_line, regions) = context_bar::build(app);
    let frame = frame::AppFrame::new(&app.navigation.view, context_line);
    let (content, status, context) = frame.render_and_get_areas(f.area(), f.buffer_mut());
    app.context.record_geometry(context, regions);
    (content, status)
}
