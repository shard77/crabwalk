use crate::{app::App, ui::theme};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    text::Span,
    widgets::{Paragraph, Widget},
};

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub(in crate::ui) fn draw(f: &mut Frame, app: &App, area: Rect, context: Option<String>) {
    let loading_detail = if let Some((num, comment_count)) = app.background.detail_progress() {
        LoadingDetail::Detail {
            pr_number: num,
            comment_count,
            has_body: app.main.detail.snapshot().is_some(),
        }
    } else if app.background.is_loading_prs() {
        LoadingDetail::Prs {
            count: app.main.prs.len(),
        }
    } else {
        LoadingDetail::Generic
    };
    let widget = StatusBar {
        spinner_tick: app.feedback.spinner_tick,
        loading: app.is_loading(),
        loading_detail,
        status_message: app.status_message().map(str::to_owned).or(context),
        help_available: crate::keys::is_help_available(app),
        nerd_font: app.main.nerd_font,
    };
    f.render_widget(widget, area);
}

fn spinner_frame(tick: usize) -> char {
    SPINNER_FRAMES[tick % SPINNER_FRAMES.len()]
}

struct StatusBar {
    spinner_tick: usize,
    loading: bool,
    loading_detail: LoadingDetail,
    status_message: Option<String>,
    help_available: bool,
    nerd_font: bool,
}

enum LoadingDetail {
    Prs {
        count: usize,
    },
    Detail {
        pr_number: u64,
        comment_count: usize,
        has_body: bool,
    },
    Generic,
}

impl StatusBar {
    fn build_loading_indicator(&self) -> String {
        if !self.loading {
            return String::new();
        }
        let frame = spinner_frame(self.spinner_tick);
        match &self.loading_detail {
            LoadingDetail::Detail {
                pr_number,
                comment_count,
                has_body,
            } => {
                if *comment_count > 0 {
                    format!("{frame} PR #{pr_number} ({comment_count} comments)  ")
                } else if *has_body {
                    format!("{frame} PR #{pr_number} (loading comments)  ")
                } else {
                    format!("{frame} PR #{pr_number} (loading body)  ")
                }
            }
            LoadingDetail::Prs { count } => {
                if *count > 0 {
                    format!("{frame} Loading PRs ({count})  ")
                } else {
                    format!("{frame} Loading PRs  ")
                }
            }
            LoadingDetail::Generic => {
                format!("{frame} Loading  ")
            }
        }
    }
}

impl Widget for StatusBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let loading_indicator = self.build_loading_indicator();

        let msg = if let Some(status) = &self.status_message {
            format!("{loading_indicator}{status}")
        } else {
            loading_indicator
        };

        let style = if self.loading {
            theme::STATUS_BAR_LOADING
        } else {
            theme::STATUS_BAR
        };

        Paragraph::new("").style(style).render(area, buf);
        let full_hint = Span::raw(if self.nerd_font {
            "\u{f02d} F1 / ? Keys"
        } else {
            "F1 / ? Keys"
        });
        let hint = if !self.help_available {
            Span::raw("")
        } else if area.width as usize >= full_hint.width() {
            full_hint
        } else if area.width >= 6 {
            Span::raw("? Keys")
        } else if area.width > 0 {
            Span::raw("?")
        } else {
            Span::raw("")
        };
        let width = hint.width() as u16;
        let message_area = if width == 0 {
            area
        } else {
            Rect::new(
                area.x,
                area.y,
                area.width.saturating_sub(width + 2),
                area.height,
            )
        };
        Paragraph::new(Span::raw(msg))
            .style(style)
            .render(message_area, buf);
        if width > 0 {
            let hint_area = Rect::new(area.right() - width, area.y, width, area.height);
            Paragraph::new(hint).style(style).render(hint_area, buf);
        }
    }
}
