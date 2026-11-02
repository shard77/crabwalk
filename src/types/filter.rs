use super::PrSummary;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InactivitySort {
    #[default]
    Ascending,
    Descending,
}

impl InactivitySort {
    pub const ALL: [Self; 2] = [Self::Ascending, Self::Descending];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    pub fn compare(self, a: &PrSummary, b: &PrSummary) -> std::cmp::Ordering {
        let order = b
            .updated_at
            .cmp(&a.updated_at)
            .then(b.created_at.cmp(&a.created_at));
        match self {
            Self::Ascending => order,
            Self::Descending => order.reverse(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterSpec {
    pub label: String,
    pub author: String,
    pub min_inactive_days: Option<u64>,
    // Draft filtering only applies to GraphQL search; the REST path always includes drafts.
    pub show_drafts: bool,
    pub inactivity_sort: InactivitySort,
}

impl FilterSpec {
    pub fn label_text(&self) -> &str {
        or_any(&self.label)
    }

    pub fn author_text(&self) -> &str {
        or_any(&self.author)
    }

    pub fn inactive_days_text(&self) -> String {
        self.min_inactive_days
            .map_or_else(|| "off".to_owned(), |days| format!("{days}d"))
    }
}

fn or_any(value: &str) -> &str {
    if value.is_empty() { "(any)" } else { value }
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuickFilter {
    pub name: String,
    pub filter: FilterSpec,
}
