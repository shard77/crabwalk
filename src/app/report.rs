use super::{App, DeletionTarget, Modal};
use crate::types::TriageLabel;
use chrono::{Datelike, Duration, Utc};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

#[derive(Debug, Clone)]
pub struct ReportEntry {
    pub pr_number: u64,
    pub title: String,
    pub author: String,
    pub reviewer: Option<String>,
    pub inactive_days: i64,
    pub status_label: Option<TriageLabel>,
    pub note: String,
}

const NO_STATUS_HEADING: &str = "No status label";

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum ReportFormat {
    #[default]
    Zulip,
    Plain,
}

impl ReportFormat {
    pub const ALL: [Self; 2] = [Self::Zulip, Self::Plain];

    pub fn label(&self) -> &str {
        match self {
            Self::Zulip => "Zulip",
            Self::Plain => "Plain",
        }
    }
}

#[derive(Debug, Default)]
pub struct ReportState {
    pub entries: Vec<ReportEntry>,
    pub selected: usize,
    pub format: ReportFormat,
}

impl ReportState {
    pub fn delete_selected(&mut self) {
        if !self.entries.is_empty() && self.selected < self.entries.len() {
            self.entries.remove(self.selected);
            if self.selected > 0 && self.selected >= self.entries.len() {
                self.selected -= 1;
            }
        }
    }

    pub fn add_entry(&mut self, entry: ReportEntry) {
        if !self.entries.iter().any(|e| e.pr_number == entry.pr_number) {
            self.entries.push(entry);
        }
    }

    pub fn selected_entry(&self) -> Option<&ReportEntry> {
        self.entries.get(self.selected)
    }
}

impl App {
    pub fn delete_report_entry(&mut self, number: u64) {
        if let Some(index) = self
            .report
            .entries
            .iter()
            .position(|entry| entry.pr_number == number)
        {
            self.report.selected = index;
            self.report.delete_selected();
            self.set_status_message(format!("Removed #{number} from report"));
        }
    }

    pub fn save_report_note(&mut self, pr_number: u64, text: String) {
        if let Some(entry) = self
            .report
            .entries
            .iter_mut()
            .find(|entry| entry.pr_number == pr_number)
        {
            entry.note = text;
            self.set_status_message(format!("Updated note for #{pr_number}"));
        } else if self
            .main
            .selected_pr()
            .is_some_and(|pr| pr.number == pr_number)
        {
            self.add_to_report(text);
        } else {
            self.modal = Modal::Error("The PR changed while editing; note was not applied.".into());
        }
    }

    pub fn edit_report_note(&mut self, pr_number: u64) {
        let text = self
            .report
            .entries
            .iter()
            .find(|entry| entry.pr_number == pr_number)
            .map(|entry| entry.note.clone())
            .unwrap_or_default();
        self.modal = Modal::NoteEditor { pr_number, text };
    }

    pub fn edit_selected_report_note(&mut self) {
        if let Some(entry) = self.report.selected_entry() {
            self.edit_report_note(entry.pr_number);
        }
    }

    pub fn confirm_report_entry_deletion(&mut self) {
        if let Some(entry) = self.report.selected_entry() {
            self.modal = Modal::ConfirmDelete {
                target: DeletionTarget::ReportEntry(entry.pr_number),
            };
        }
    }

    pub fn copy_report(&mut self) {
        let text = format_report(&self.report.entries, &self.report.format);
        self.copy_to_clipboard(&text, "Copied!");
    }

    pub fn add_to_report(&mut self, note: String) {
        if let Some(pr) = self.main.selected_pr() {
            let reviewer = pr
                .assignees
                .first()
                .cloned()
                .or_else(|| pr.requested_reviewers.first().cloned());
            let entry = ReportEntry {
                pr_number: pr.number,
                title: pr.title.clone(),
                author: pr.author.clone(),
                reviewer,
                inactive_days: pr.updated_days_ago(),
                status_label: pr.triage_label(),
                note,
            };
            let number = pr.number;
            self.report.add_entry(entry);
            self.set_status_message(format!("Added #{number} to report"));
        }
    }
}

fn current_week_range() -> (chrono::NaiveDate, chrono::NaiveDate) {
    let today = Utc::now().date_naive();
    let days_since_monday = today.weekday().num_days_from_monday();
    let monday = today - Duration::days(days_since_monday as i64);
    let sunday = monday + Duration::days(6);
    (monday, sunday)
}

pub fn format_report(entries: &[ReportEntry], format: &ReportFormat) -> String {
    let (monday, sunday) = current_week_range();
    format_report_for_week(entries, format, monday, sunday)
}

fn format_report_for_week(
    entries: &[ReportEntry],
    format: &ReportFormat,
    monday: chrono::NaiveDate,
    sunday: chrono::NaiveDate,
) -> String {
    let (week_prefix, week_suffix, label_marker, indent) = match format {
        ReportFormat::Zulip => ("### ", "", "**", ""),
        ReportFormat::Plain => ("== ", " ==", "", "  "),
    };
    let mut out = format!(
        "{week_prefix}Week {} to {}{week_suffix}\n\n",
        monday.format("%Y-%m-%d"),
        sunday.format("%Y-%m-%d")
    );

    let groups = group_and_sort(entries);
    for (heading, group_entries) in &groups {
        if group_entries.is_empty() {
            continue;
        }
        let count = group_entries.len();
        let pr_word = if count == 1 { "PR" } else { "PRs" };
        writeln!(
            out,
            "{label_marker}{heading}{label_marker} ({count} {pr_word})"
        )
        .expect("writing to a String cannot fail");
        for e in group_entries {
            let reviewer = e
                .reviewer
                .as_deref()
                .map(|r| format!("/@{r}"))
                .unwrap_or_default();
            let note = if e.note.is_empty() {
                String::new()
            } else {
                format!(": {}", e.note)
            };
            writeln!(
                out,
                "{indent}- #{} ({}d) @{}{}{}",
                e.pr_number, e.inactive_days, e.author, reviewer, note
            )
            .expect("writing to a String cannot fail");
        }
        out.push('\n');
    }

    out
}

fn group_and_sort(entries: &[ReportEntry]) -> Vec<(&str, Vec<&ReportEntry>)> {
    static ORDER: [TriageLabel; 3] = TriageLabel::PRIMARY;

    let mut result: Vec<(&str, Vec<&ReportEntry>)> =
        ORDER.iter().map(|l| (l.as_str(), Vec::new())).collect();

    let mut other_groups: BTreeMap<&str, Vec<&ReportEntry>> = BTreeMap::new();
    let mut unlabelled = Vec::new();

    for entry in entries {
        match &entry.status_label {
            Some(label) if let Some(i) = ORDER.iter().position(|l| l == label) => {
                result[i].1.push(entry)
            }
            Some(label) => other_groups.entry(label.as_str()).or_default().push(entry),
            None => unlabelled.push(entry),
        }
    }

    result.extend(other_groups);
    result.push((NO_STATUS_HEADING, unlabelled));

    for (_, group) in &mut result {
        group.sort_by_key(|entry| std::cmp::Reverse(entry.inactive_days));
    }

    result
}

pub struct ReportSummary {
    pub total: usize,
    // Whole days, truncated; zero for an empty report.
    pub avg_inactive_days: i64,
    pub reviewers: usize,
    // Same order as TriageLabel::PRIMARY.
    pub primary: [usize; TriageLabel::PRIMARY.len()],
    pub stale: usize,
}

pub fn report_summary(entries: &[ReportEntry]) -> ReportSummary {
    let total = entries.len();
    let avg_inactive_days = if total > 0 {
        entries.iter().map(|e| e.inactive_days).sum::<i64>() / total as i64
    } else {
        0
    };
    let reviewers: HashSet<&str> = entries
        .iter()
        .filter_map(|e| e.reviewer.as_deref())
        .collect();

    let mut primary = [0; TriageLabel::PRIMARY.len()];
    let mut stale = 0;

    for e in entries {
        let label = e.status_label.as_ref();
        if let Some(i) = label.and_then(|l| TriageLabel::PRIMARY.iter().position(|p| p == l)) {
            primary[i] += 1;
        }
        if e.inactive_days >= TriageLabel::threshold_days(label) as i64 {
            stale += 1;
        }
    }

    ReportSummary {
        total,
        avg_inactive_days,
        reviewers: reviewers.len(),
        primary,
        stale,
    }
}
