use super::Preferences;
use crate::types::{FilterSpec, InactivitySort, QuickFilter, TriageLabel};

pub(super) fn startup_preferences() -> Preferences {
    Preferences {
        quick_filters: builtin_quick_filters(),
        default_filter: label_filter(&TriageLabel::WaitingOnReview),
        ..Preferences::default()
    }
}

pub fn builtin_quick_filters() -> Vec<QuickFilter> {
    TriageLabel::PRIMARY
        .iter()
        .map(|label| QuickFilter {
            name: label.short_name().to_string(),
            filter: label_filter(label),
        })
        .collect()
}

fn label_filter(label: &TriageLabel) -> FilterSpec {
    FilterSpec {
        label: label.as_str().to_string(),
        author: String::new(),
        min_inactive_days: Some(TriageLabel::threshold_days(Some(label))),
        inactivity_sort: InactivitySort::default(),
        show_drafts: false,
    }
}
