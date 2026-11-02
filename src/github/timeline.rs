use super::GitHubClient;
use crate::types::{Comment, CommentKind};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;

// Comments and reviews load separately; including them here would duplicate conversation entries.
const TIMELINE_QUERY: &str = r#"
query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      page: timelineItems(first: 100, after: $cursor, itemTypes: [
        HEAD_REF_FORCE_PUSHED_EVENT, BASE_REF_FORCE_PUSHED_EVENT, PULL_REQUEST_COMMIT,
        REVIEW_REQUESTED_EVENT, REVIEW_REQUEST_REMOVED_EVENT, REVIEW_DISMISSED_EVENT,
        READY_FOR_REVIEW_EVENT, CONVERT_TO_DRAFT_EVENT, CLOSED_EVENT, REOPENED_EVENT,
        MERGED_EVENT, HEAD_REF_DELETED_EVENT, HEAD_REF_RESTORED_EVENT, BASE_REF_CHANGED_EVENT,
        BASE_REF_DELETED_EVENT, RENAMED_TITLE_EVENT, LABELED_EVENT, UNLABELED_EVENT,
        ASSIGNED_EVENT, UNASSIGNED_EVENT, MILESTONED_EVENT, DEMILESTONED_EVENT,
        LOCKED_EVENT, UNLOCKED_EVENT, AUTO_MERGE_ENABLED_EVENT, AUTO_MERGE_DISABLED_EVENT,
        ADDED_TO_MERGE_QUEUE_EVENT, REMOVED_FROM_MERGE_QUEUE_EVENT,
        CROSS_REFERENCED_EVENT, CONNECTED_EVENT, DISCONNECTED_EVENT
      ]) {
        pageInfo { hasNextPage endCursor }
        nodes {
          __typename
          ... on HeadRefForcePushedEvent {
            id actor { login } createdAt ref { name }
            beforeCommit { ...ActivityCommit } afterCommit { ...ActivityCommit }
          }
          ... on BaseRefForcePushedEvent {
            id actor { login } createdAt ref { name }
            beforeCommit { ...ActivityCommit } afterCommit { ...ActivityCommit }
          }
          ... on PullRequestCommit {
            id commit {
              ...ActivityCommit messageHeadline committedDate author { name user { login } }
            }
          }
          ... on ReviewRequestedEvent {
            id actor { login } createdAt
            requestedReviewer {
              ... on User { login }
              ... on Team { slug organization { login } }
              ... on Mannequin { login }
              ... on Bot { login }
            }
          }
          ... on ReviewRequestRemovedEvent {
            id actor { login } createdAt
            requestedReviewer {
              ... on User { login }
              ... on Team { slug organization { login } }
              ... on Mannequin { login }
              ... on Bot { login }
            }
          }
          ... on ReviewDismissedEvent {
            id actor { login } createdAt previousReviewState dismissalMessage
            review { author { login } }
          }
          ... on ReadyForReviewEvent { id actor { login } createdAt }
          ... on ConvertToDraftEvent { id actor { login } createdAt }
          ... on ClosedEvent { id actor { login } createdAt }
          ... on ReopenedEvent { id actor { login } createdAt }
          ... on MergedEvent {
            id actor { login } createdAt mergeRefName mergeCommit: commit { ...ActivityCommit }
          }
          ... on HeadRefDeletedEvent { id actor { login } createdAt headRefName }
          ... on HeadRefRestoredEvent { id actor { login } createdAt }
          ... on BaseRefChangedEvent {
            id actor { login } createdAt previousRefName currentRefName
          }
          ... on BaseRefDeletedEvent { id actor { login } createdAt baseRefName }
          ... on RenamedTitleEvent {
            id actor { login } createdAt previousTitle currentTitle
          }
          ... on LabeledEvent { id actor { login } createdAt label { name } }
          ... on UnlabeledEvent { id actor { login } createdAt label { name } }
          ... on AssignedEvent {
            id actor { login } createdAt
            assignee {
              ... on User { login } ... on Mannequin { login }
              ... on Bot { login } ... on Organization { login }
            }
          }
          ... on UnassignedEvent {
            id actor { login } createdAt
            assignee {
              ... on User { login } ... on Mannequin { login }
              ... on Bot { login } ... on Organization { login }
            }
          }
          ... on MilestonedEvent { id actor { login } createdAt milestoneTitle }
          ... on DemilestonedEvent { id actor { login } createdAt milestoneTitle }
          ... on LockedEvent { id actor { login } createdAt lockReason }
          ... on UnlockedEvent { id actor { login } createdAt }
          ... on AutoMergeEnabledEvent { id actor { login } createdAt }
          ... on AutoMergeDisabledEvent { id actor { login } createdAt reason }
          ... on AddedToMergeQueueEvent { id actor { login } createdAt }
          ... on RemovedFromMergeQueueEvent { id actor { login } createdAt reason }
          ... on CrossReferencedEvent {
            id actor { login } referencedAt willCloseTarget source { ...ActivitySubject }
          }
          ... on ConnectedEvent {
            id actor { login } createdAt source { ...ActivitySubject }
          }
          ... on DisconnectedEvent {
            id actor { login } createdAt source { ...ActivitySubject }
          }
        }
      }
    }
  }
}
fragment ActivityCommit on Commit { oid abbreviatedOid url }
fragment ActivitySubject on ReferencedSubject {
  ... on Issue { number title url repository { nameWithOwner } }
  ... on PullRequest { number title url repository { nameWithOwner } }
}
"#;

impl GitHubClient {
    pub async fn fetch_timeline_page(
        &self,
        number: u64,
        cursor: Option<&str>,
    ) -> Result<(Vec<Comment>, Option<String>)> {
        let (nodes, next) = self
            .pr_connection_page::<Value>(TIMELINE_QUERY, number, cursor)
            .await
            .with_context(|| format!("fetching activity for #{number}"))?;
        let comments = nodes
            .iter()
            .map(activity)
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>>>()?;
        Ok((comments, next))
    }
}

fn required_string(node: &Value, field: &str) -> Result<String> {
    node[field]
        .as_str()
        .map(str::to_owned)
        .with_context(|| format!("missing {field} in graphql response"))
}

fn text<'a>(node: &'a Value, field: &str, fallback: &'a str) -> &'a str {
    node[field]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
}

fn reviewer(node: &Value) -> String {
    let reviewer = &node["requestedReviewer"];
    if let Some(login) = reviewer["login"].as_str() {
        format!("@{login}")
    } else if let Some(slug) = reviewer["slug"].as_str() {
        match reviewer["organization"]["login"].as_str() {
            Some(org) => format!("@{org}/{slug}"),
            None => slug.to_owned(),
        }
    } else {
        "an unavailable reviewer".to_owned()
    }
}

fn assignee(node: &Value) -> String {
    node["assignee"]["login"].as_str().map_or_else(
        || "an unavailable assignee".to_owned(),
        |login| format!("@{login}"),
    )
}

fn commit_name(commit: &Value) -> &str {
    text(
        commit,
        "abbreviatedOid",
        text(commit, "oid", "unavailable commit"),
    )
}

fn commit_link(commit: &Value) -> String {
    match commit["url"].as_str() {
        Some(url) => format!("[{}]({url})", commit_name(commit)),
        None => commit_name(commit).to_owned(),
    }
}

fn activity(node: &Value) -> Result<Option<Comment>> {
    let kind = text(node, "__typename", "");
    let mut body = String::new();
    let action = match kind {
        "HeadRefForcePushedEvent" | "BaseRefForcePushedEvent" => {
            let branch = if kind == "HeadRefForcePushedEvent" {
                "head"
            } else {
                "base"
            };
            body = format!(
                "{} → {}",
                commit_link(&node["beforeCommit"]),
                commit_link(&node["afterCommit"]),
            );
            format!(
                "force-pushed {branch} branch {} from {} to {}",
                text(&node["ref"], "name", "(deleted)"),
                commit_name(&node["beforeCommit"]),
                commit_name(&node["afterCommit"]),
            )
        }
        "ReviewRequestedEvent" => format!("requested a review from {}", reviewer(node)),
        "ReviewRequestRemovedEvent" => format!("removed the review request for {}", reviewer(node)),
        "ReviewDismissedEvent" => {
            body = text(node, "dismissalMessage", "").to_owned();
            format!(
                "dismissed @{}’s review ({})",
                text(&node["review"]["author"], "login", "unknown"),
                match text(node, "previousReviewState", "") {
                    "APPROVED" => "previously approved",
                    "CHANGES_REQUESTED" => "previously requested changes",
                    _ => "previously reviewed",
                },
            )
        }
        "ReadyForReviewEvent" => "marked this pull request ready for review".to_owned(),
        "ConvertToDraftEvent" => "converted this pull request to draft".to_owned(),
        "ClosedEvent" => "closed this pull request".to_owned(),
        "ReopenedEvent" => "reopened this pull request".to_owned(),
        "MergedEvent" => {
            body = commit_link(&node["mergeCommit"]);
            format!(
                "merged commit {} into {}",
                commit_name(&node["mergeCommit"]),
                text(node, "mergeRefName", "an unavailable branch"),
            )
        }
        "HeadRefDeletedEvent" => format!(
            "deleted branch {}",
            text(node, "headRefName", "(unavailable)")
        ),
        "HeadRefRestoredEvent" => "restored the head branch".to_owned(),
        "BaseRefDeletedEvent" => format!(
            "deleted base branch {}",
            text(node, "baseRefName", "(unavailable)")
        ),
        "BaseRefChangedEvent" => format!(
            "changed the base branch from {} to {}",
            text(node, "previousRefName", "(unavailable)"),
            text(node, "currentRefName", "(unavailable)"),
        ),
        "RenamedTitleEvent" => format!(
            "changed the title from “{}” to “{}”",
            text(node, "previousTitle", "(unavailable)"),
            text(node, "currentTitle", "(unavailable)"),
        ),
        "LabeledEvent" => format!("added label {}", text(&node["label"], "name", "(deleted)")),
        "UnlabeledEvent" => format!(
            "removed label {}",
            text(&node["label"], "name", "(deleted)")
        ),
        "AssignedEvent" => format!("assigned {}", assignee(node)),
        "UnassignedEvent" => format!("unassigned {}", assignee(node)),
        "MilestonedEvent" => format!(
            "added this to milestone {}",
            text(node, "milestoneTitle", "(deleted)")
        ),
        "DemilestonedEvent" => format!(
            "removed this from milestone {}",
            text(node, "milestoneTitle", "(deleted)")
        ),
        "LockedEvent" => format!(
            "locked the conversation ({})",
            text(node, "lockReason", "no reason given")
        ),
        "UnlockedEvent" => "unlocked the conversation".to_owned(),
        "AutoMergeEnabledEvent" => "enabled auto-merge".to_owned(),
        "AutoMergeDisabledEvent" => {
            body = text(node, "reason", "").to_owned();
            "disabled auto-merge".to_owned()
        }
        "AddedToMergeQueueEvent" => "added this pull request to the merge queue".to_owned(),
        "RemovedFromMergeQueueEvent" => {
            body = text(node, "reason", "").to_owned();
            "removed this pull request from the merge queue".to_owned()
        }
        "CrossReferencedEvent" | "ConnectedEvent" | "DisconnectedEvent" => {
            let source = &node["source"];
            let subject = source["number"].as_u64().map_or_else(
                || "an unavailable issue or pull request".to_owned(),
                |number| {
                    format!(
                        "{}#{number}",
                        text(&source["repository"], "nameWithOwner", "")
                    )
                },
            );
            if let Some(url) = source["url"].as_str() {
                body = format!("[{subject}]({url})");
            }
            if let Some(title) = source["title"].as_str().filter(|title| !title.is_empty()) {
                if !body.is_empty() {
                    body.push_str(" — ");
                }
                body.push_str(&crate::types::escape_markdown_text(title));
            }
            match kind {
                "ConnectedEvent" => format!("connected {subject}"),
                "DisconnectedEvent" => format!("disconnected {subject}"),
                _ if node["willCloseTarget"].as_bool() == Some(true) => {
                    format!("referenced this in {subject} (will close this pull request)")
                }
                _ => format!("referenced this in {subject}"),
            }
        }
        "PullRequestCommit" => {
            let commit = &node["commit"];
            body = commit_link(commit);
            format!(
                "committed {}: {}",
                commit_name(commit),
                text(commit, "messageHeadline", "")
            )
        }
        _ => return Ok(None),
    };
    let (author, timestamp) = if kind == "PullRequestCommit" {
        let author = &node["commit"]["author"];
        (
            text(&author["user"], "login", text(author, "name", "unknown")),
            required_string(&node["commit"], "committedDate")?,
        )
    } else {
        (
            text(&node["actor"], "login", "unknown"),
            required_string(
                node,
                if kind == "CrossReferencedEvent" {
                    "referencedAt"
                } else {
                    "createdAt"
                },
            )?,
        )
    };
    Ok(Some(Comment {
        kind: CommentKind::Activity {
            id: required_string(node, "id")?,
            action,
        },
        author: author.to_owned(),
        created_at: DateTime::parse_from_rfc3339(&timestamp)?.with_timezone(&Utc),
        body,
        path: None,
        outdated: false,
        minimized: false,
        minimized_reason: None,
    }))
}
