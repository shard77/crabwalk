use super::{Actor, Connection, GitHubClient, Nodes, OWNER, REPO, login};
use crate::types::{InactivitySort, PrSummary};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use octocrab::{models::issues::Issue, params};
use serde::Deserialize;
use serde_json::Value;
use std::fmt::Write;

const SEARCH_QUERY: &str = r#"
query($query: String!, $cursor: String) {
  search(query: $query, type: ISSUE, first: 100, after: $cursor) {
    pageInfo { hasNextPage endCursor }
    nodes {
      ... on PullRequest {
        number title author { login }
        labels(first: 30) { nodes { name } }
        assignees(first: 10) { nodes { login } }
        reviewRequests(first: 10) {
          nodes { requestedReviewer { ... on User { login } } }
        }
        updatedAt createdAt url isDraft milestone { title }
      }
    }
  }
}
"#;

#[derive(Deserialize)]
struct SearchData {
    search: Connection<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPr {
    number: u64,
    title: String,
    author: Option<Actor>,
    labels: Option<Nodes<LabelNode>>,
    assignees: Option<Nodes<Actor>>,
    review_requests: Option<Nodes<ReviewRequest>>,
    updated_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    url: String,
    is_draft: bool,
    milestone: Option<Milestone>,
}

#[derive(Deserialize)]
struct LabelNode {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewRequest {
    requested_reviewer: Option<Reviewer>,
}

#[derive(Deserialize)]
struct Reviewer {
    login: Option<String>,
}

#[derive(Deserialize)]
struct Milestone {
    title: String,
}

impl GitHubClient {
    pub async fn fetch_all_label_names(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        let mut page_num: u32 = 1;
        loop {
            let page = self
                .request(
                    self.inner
                        .issues(OWNER, REPO)
                        .list_labels_for_repo()
                        .per_page(100)
                        .page(page_num)
                        .send(),
                )
                .await
                .context("fetching labels")?;
            let items = page.items;
            let has_more = items.len() == 100;
            names.extend(items.into_iter().map(|label| label.name));
            if !has_more {
                break;
            }
            page_num += 1;
        }
        names.sort();
        Ok(names)
    }

    /// A short PR page can still have a next page: the issues endpoint also returns non-PRs.
    pub async fn fetch_pr_page(
        &self,
        label: &str,
        author: &str,
        inactivity_sort: InactivitySort,
        page_num: u32,
    ) -> Result<(Vec<PrSummary>, bool)> {
        let labels = (!label.is_empty()).then(|| vec![label.to_string()]);
        let issue_handler = self.inner.issues(OWNER, REPO);
        let mut builder = issue_handler
            .list()
            .state(params::State::Open)
            .sort(params::issues::Sort::Updated)
            .direction(match inactivity_sort {
                InactivitySort::Ascending => params::Direction::Descending,
                InactivitySort::Descending => params::Direction::Ascending,
            })
            .per_page(100)
            .page(page_num);
        if let Some(labels) = &labels {
            builder = builder.labels(labels);
        }
        if !author.is_empty() {
            builder = builder.creator(author);
        }
        let items: Vec<Issue> = self
            .request(builder.send())
            .await
            .context("fetching PRs")?
            .items;
        let has_more = items.len() == 100;
        let prs = items
            .into_iter()
            .filter(|issue| issue.pull_request.is_some())
            .map(issue_to_pr_summary)
            .collect();
        Ok((prs, has_more))
    }

    pub async fn fetch_pr_body(&self, number: u64) -> Result<Option<String>> {
        let issue = self
            .request(self.inner.issues(OWNER, REPO).get(number))
            .await
            .with_context(|| format!("fetching issue #{number}"))?;
        Ok(issue.body)
    }

    // Search uses a date-only cutoff; check exact timestamps before accepting a result.
    pub async fn fetch_pr_search_page(
        &self,
        label: &str,
        author: &str,
        updated_before: DateTime<Utc>,
        show_drafts: bool,
        inactivity_sort: InactivitySort,
        cursor: Option<&str>,
    ) -> Result<(Vec<PrSummary>, Option<String>)> {
        let date = updated_before.format("%Y-%m-%d");
        let mut query = format!("repo:{OWNER}/{REPO} is:pr is:open updated:<={date}");
        if !label.is_empty() {
            write!(query, " label:{label}").expect("writing to a String cannot fail");
        }
        if !author.is_empty() {
            write!(query, " author:{author}").expect("writing to a String cannot fail");
        }
        if !show_drafts {
            query.push_str(" draft:false");
        }
        query.push_str(match inactivity_sort {
            InactivitySort::Ascending => " sort:updated-desc",
            InactivitySort::Descending => " sort:updated-asc",
        });

        let variables = serde_json::json!({ "query": query, "cursor": cursor });
        let data: SearchData = self
            .graphql(SEARCH_QUERY, variables)
            .await
            .context("fetching PRs via GraphQL search")?;
        let prs = data
            .search
            .nodes
            .into_iter()
            .filter_map(|node| serde_json::from_value(node).ok())
            .map(search_pr_to_summary)
            .filter(|pr| pr.updated_at <= updated_before)
            .collect();
        Ok((prs, data.search.page_info.next_cursor(cursor)?))
    }
}

fn search_pr_to_summary(pr: SearchPr) -> PrSummary {
    PrSummary {
        number: pr.number,
        title: pr.title,
        author: login(pr.author),
        labels: Nodes::items(pr.labels).map(|label| label.name).collect(),
        assignees: Nodes::items(pr.assignees)
            .map(|assignee| assignee.login)
            .collect(),
        requested_reviewers: Nodes::items(pr.review_requests)
            .filter_map(|request| request.requested_reviewer?.login)
            .collect(),
        updated_at: pr.updated_at,
        created_at: pr.created_at,
        html_url: pr.url,
        draft: pr.is_draft,
        milestone: pr.milestone.map(|milestone| milestone.title),
    }
}

// The issues endpoint omits draft status and review requests; use the draft label as a fallback.
fn issue_to_pr_summary(issue: Issue) -> PrSummary {
    let labels: Vec<_> = issue.labels.into_iter().map(|label| label.name).collect();
    let draft = labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case("draft"));
    PrSummary {
        number: issue.number,
        title: issue.title,
        author: issue.user.login,
        labels,
        assignees: issue
            .assignees
            .into_iter()
            .map(|assignee| assignee.login)
            .collect(),
        requested_reviewers: Vec::new(),
        updated_at: issue.updated_at,
        created_at: issue.created_at,
        html_url: issue.html_url.to_string(),
        draft,
        milestone: issue.milestone.map(|milestone| milestone.title),
    }
}
