use super::App;
use std::time::Duration;
use tokio::time::Instant;

const STATUS_LIFETIME: Duration = Duration::from_secs(3);

struct StatusMessage {
    text: String,
    expires_at: Instant,
}

#[derive(Default)]
pub struct FeedbackState {
    status: Option<StatusMessage>,
    pub spinner_tick: usize,
}

pub struct HelpState {
    pub scroll: u16,
    pub viewport_height: u16,
}

impl Default for HelpState {
    fn default() -> Self {
        Self {
            scroll: 0,
            viewport_height: 8,
        }
    }
}

impl FeedbackState {
    fn status_message(&self) -> Option<&str> {
        self.status.as_ref().map(|status| status.text.as_str())
    }

    fn set_status_message(&mut self, text: impl Into<String>) {
        self.status = Some(StatusMessage {
            text: text.into(),
            expires_at: Instant::now() + STATUS_LIFETIME,
        });
    }

    fn tick(&mut self, loading: bool) -> bool {
        let mut changed = false;
        if loading {
            self.spinner_tick = self.spinner_tick.wrapping_add(1);
            changed = true;
        }
        if self
            .status
            .as_ref()
            .is_some_and(|status| status.expires_at <= Instant::now())
        {
            self.status = None;
            changed = true;
        }
        changed
    }
}

impl App {
    pub fn status_message(&self) -> Option<&str> {
        self.feedback.status_message()
    }

    pub fn set_status_message(&mut self, text: impl Into<String>) {
        self.feedback.set_status_message(text);
    }

    pub fn tick(&mut self) -> bool {
        self.feedback.tick(self.is_loading())
    }
}
