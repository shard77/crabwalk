use chrono::{DateTime, Utc};
use serde::Deserialize;

mod conversation;
pub use conversation::{CollapseReason, ConversationIndex, ConversationVisibility, ReviewThreads};
mod filter;
pub use filter::{FilterSpec, InactivitySort, QuickFilter};
pub mod review_hunk;

#[derive(Debug, Clone, PartialEq)]
pub enum TriageLabel {
    WaitingOnReview,
    WaitingOnAuthor,
    Blocked,
    Other(String),
}

impl TriageLabel {
    pub const PRIMARY: [Self; 3] = [Self::WaitingOnReview, Self::WaitingOnAuthor, Self::Blocked];

    pub fn from_label_name(name: &str) -> Option<Self> {
        match name {
            "S-waiting-on-review" => Some(Self::WaitingOnReview),
            "S-waiting-on-author" => Some(Self::WaitingOnAuthor),
            "S-blocked" => Some(Self::Blocked),
            s if s.starts_with("S-") => Some(Self::Other(s.to_string())),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::WaitingOnReview => "S-waiting-on-review",
            Self::WaitingOnAuthor => "S-waiting-on-author",
            Self::Blocked => "S-blocked",
            Self::Other(s) => s.as_str(),
        }
    }

    pub fn short_name(&self) -> &str {
        match self {
            Self::WaitingOnReview => "WoR",
            Self::WaitingOnAuthor => "WoA",
            Self::Blocked => "Blocked",
            Self::Other(s) => s.as_str(),
        }
    }

    pub fn threshold_days(label: Option<&Self>) -> u64 {
        match label {
            Some(Self::Blocked) => 30,
            _ => 15,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrSummary {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub requested_reviewers: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub html_url: String,
    pub draft: bool,
    pub milestone: Option<String>,
}

impl PrSummary {
    pub fn updated_days_ago(&self) -> i64 {
        (Utc::now() - self.updated_at).num_days()
    }

    pub fn triage_label(&self) -> Option<TriageLabel> {
        self.labels
            .iter()
            .find_map(|l| TriageLabel::from_label_name(l))
    }
}

#[derive(Debug, Clone)]
pub enum CommentKind {
    Issue { id: String },
    Activity { id: String, action: String },
    Review { id: String, state: ReviewState },
    ReviewComment(ReviewCommentContext),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewState {
    Pending,
    Commented,
    Approved,
    ChangesRequested,
    Dismissed,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DiffSide {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct ReviewCommentContext {
    pub id: String,
    pub review_id: Option<String>,
    pub thread_id: String,
    pub reply_to: Option<String>,
    pub line: Option<u64>,
    pub start_line: Option<u64>,
    pub original_line: Option<u64>,
    pub original_start_line: Option<u64>,
    // A multiline range can start and end on different diff sides.
    pub start_side: Option<DiffSide>,
    pub side: Option<DiffSide>,
    pub diff_hunk: String,
    pub resolved: bool,
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub kind: CommentKind,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub body: String,
    pub path: Option<String>,
    pub outdated: bool,
    pub minimized: bool,
    pub minimized_reason: Option<String>,
}

impl Comment {
    /// Saved expansions must use the same source as the reader, including activity text.
    pub fn reader_source(&self) -> std::borrow::Cow<'_, str> {
        let CommentKind::Activity { action, .. } = &self.kind else {
            return std::borrow::Cow::Borrowed(&self.body);
        };
        let mut source = escape_markdown_text(action);
        if !self.body.trim().is_empty() {
            source.push_str("\n\n");
            source.push_str(&self.body);
        }
        std::borrow::Cow::Owned(source)
    }
}

pub fn escape_markdown_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_punctuation() {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

#[derive(Debug, Clone, Default)]
pub struct ItemDetail {
    pub body: Option<String>,
    pub comments: Vec<Comment>,
}
