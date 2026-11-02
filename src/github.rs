use crate::types::{Comment, CommentKind, DiffSide, ReviewCommentContext, ReviewState};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::{StreamExt, TryStreamExt, stream};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

mod prs;
mod timeline;

const OWNER: &str = "rust-lang";
const REPO: &str = "rust";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONCURRENT_REQUESTS: usize = 4;

// concat! needs literals, so shared query fragments are macros rather than constants.

macro_rules! comment_fields_fragment {
    () => {
        r#"
fragment CommentFields on Comment {
  author { login } body createdAt
  ... on Minimizable { isMinimized minimizedReason }
}
"#
    };
}

macro_rules! review_comment_page_fragment {
    () => {
        r#"
fragment ReviewCommentPage on PullRequestReviewCommentConnection {
  pageInfo { hasNextPage endCursor }
  nodes {
    id ...CommentFields publishedAt
    pullRequestReview { id } replyTo { id }
    path line startLine originalLine originalStartLine diffHunk
  }
}
"#
    };
}

const ISSUE_COMMENTS_QUERY: &str = concat!(
    r#"
query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      page: comments(first: 100, after: $cursor) {
        pageInfo { hasNextPage endCursor }
        nodes { id ...CommentFields }
      }
    }
  }
}
"#,
    comment_fields_fragment!()
);

const REVIEWS_QUERY: &str = concat!(
    r#"
query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      page: reviews(first: 100, after: $cursor) {
        pageInfo { hasNextPage endCursor }
        nodes { id state submittedAt ...CommentFields }
      }
    }
  }
}
"#,
    comment_fields_fragment!()
);

const REVIEW_THREADS_QUERY: &str = concat!(
    r#"
query($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      page: reviewThreads(first: 100, after: $cursor) {
        pageInfo { hasNextPage endCursor }
        nodes {
          id isOutdated isResolved diffSide startDiffSide
          comments(first: 100) { ...ReviewCommentPage }
        }
      }
    }
  }
}
"#,
    review_comment_page_fragment!(),
    comment_fields_fragment!()
);

const THREAD_COMMENTS_QUERY: &str = concat!(
    r#"
query($id: ID!, $cursor: String!) {
  node(id: $id) {
    ... on PullRequestReviewThread {
      comments(first: 100, after: $cursor) { ...ReviewCommentPage }
    }
  }
}
"#,
    review_comment_page_fragment!(),
    comment_fields_fragment!()
);

#[derive(Deserialize)]
struct PrPageData<T> {
    repository: PrPageRepository<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrPageRepository<T> {
    pull_request: Option<PrPage<T>>,
}

#[derive(Deserialize)]
struct PrPage<T> {
    page: Connection<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Connection<T> {
    page_info: PageInfo,
    nodes: Vec<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

impl PageInfo {
    // A broken cursor must fail the load; treating it as the last page would cache partial detail.
    fn next_cursor(self, previous: Option<&str>) -> Result<Option<String>> {
        if !self.has_next_page {
            return Ok(None);
        }
        let cursor = self
            .end_cursor
            .context("missing endCursor in graphql response")?;
        anyhow::ensure!(
            !cursor.is_empty() && Some(cursor.as_str()) != previous,
            "graphql pagination cursor did not advance"
        );
        Ok(Some(cursor))
    }
}

#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Option<Vec<Option<T>>>,
}

impl<T> Nodes<T> {
    fn items(nodes: Option<Self>) -> impl Iterator<Item = T> {
        nodes.and_then(|n| n.nodes).into_iter().flatten().flatten()
    }
}

#[derive(Deserialize)]
struct Actor {
    login: String,
}

fn login(actor: Option<Actor>) -> String {
    actor.map_or_else(|| "unknown".to_owned(), |actor| actor.login)
}

#[derive(Deserialize)]
struct NodeId {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentFields {
    author: Option<Actor>,
    body: String,
    created_at: DateTime<Utc>,
    is_minimized: bool,
    minimized_reason: Option<String>,
}

#[derive(Deserialize)]
struct IssueCommentNode {
    id: String,
    #[serde(flatten)]
    fields: CommentFields,
}

impl CommentFields {
    fn into_comment(self, kind: CommentKind) -> Comment {
        Comment {
            kind,
            author: login(self.author),
            created_at: self.created_at,
            body: self.body,
            path: None,
            outdated: false,
            minimized: self.is_minimized,
            minimized_reason: self
                .minimized_reason
                .filter(|reason| !reason.is_empty())
                .map(|reason| reason.to_uppercase()),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewNode {
    id: String,
    state: ReviewState,
    submitted_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    fields: CommentFields,
}

#[derive(Deserialize)]
struct ReviewThreadNode {
    #[serde(flatten)]
    thread: ReviewThread,
    comments: Connection<ReviewCommentNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewThread {
    id: String,
    is_outdated: bool,
    is_resolved: bool,
    diff_side: Option<DiffSide>,
    start_diff_side: Option<DiffSide>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewCommentNode {
    id: String,
    published_at: Option<DateTime<Utc>>,
    pull_request_review: Option<NodeId>,
    reply_to: Option<NodeId>,
    path: String,
    line: Option<u64>,
    start_line: Option<u64>,
    original_line: Option<u64>,
    original_start_line: Option<u64>,
    diff_hunk: String,
    #[serde(flatten)]
    fields: CommentFields,
}

#[derive(Deserialize)]
struct ThreadCommentsData {
    node: Option<ThreadComments>,
}

#[derive(Deserialize)]
struct ThreadComments {
    comments: Connection<ReviewCommentNode>,
}

impl ReviewThread {
    fn build_comment(&self, node: ReviewCommentNode) -> Comment {
        let kind = CommentKind::ReviewComment(ReviewCommentContext {
            id: node.id,
            review_id: node.pull_request_review.map(|review| review.id),
            thread_id: self.id.clone(),
            reply_to: node.reply_to.map(|reply| reply.id),
            line: node.line,
            start_line: node.start_line,
            original_line: node.original_line,
            original_start_line: node.original_start_line,
            start_side: self.start_diff_side,
            side: self.diff_side,
            diff_hunk: node.diff_hunk,
            resolved: self.is_resolved,
        });
        let mut comment = node.fields.into_comment(kind);
        if let Some(published) = node.published_at {
            comment.created_at = published;
        }
        comment.path = Some(node.path);
        comment.outdated = self.is_outdated;
        comment
    }
}

/// App-specific GitHub operations backed by Octocrab.
///
/// Keeps REST/GraphQL queries, pagination rules, and conversion into app types out
/// of the UI. Sharing this client also gives all requests one concurrency limit
/// and a consistent timeout (including time spent waiting for a request slot),
/// instead of requiring each caller to enforce those policies around Octocrab.
pub struct GitHubClient {
    inner: Arc<octocrab::Octocrab>,
    request_slots: tokio::sync::Semaphore,
}

impl GitHubClient {
    /// Builds the client without checking the token against GitHub.
    pub fn new(token: &str) -> Result<Self> {
        let octocrab = octocrab::OctocrabBuilder::new()
            .personal_token(token.to_string())
            .build()
            .context("building octocrab client")?;
        Ok(Self {
            inner: Arc::new(octocrab),
            request_slots: tokio::sync::Semaphore::new(MAX_CONCURRENT_REQUESTS),
        })
    }

    async fn request<T>(
        &self,
        future: impl std::future::Future<Output = octocrab::Result<T>>,
    ) -> Result<T> {
        tokio::time::timeout(REQUEST_TIMEOUT, async {
            let _permit = self
                .request_slots
                .acquire()
                .await
                .expect("request slots open");
            future.await
        })
        .await
        .with_context(|| format!("GitHub request timed out after {REQUEST_TIMEOUT:?}"))?
        .context("GitHub request failed")
    }

    async fn graphql<T: DeserializeOwned>(&self, query: &str, variables: Value) -> Result<T> {
        let payload = serde_json::json!({ "query": query, "variables": variables });
        let data: Value = self.request(self.inner.graphql(&payload)).await?;
        // Decode separately so a bad field is reported instead of an untagged-enum error.
        serde_json::from_value(data).context("unexpected graphql response")
    }

    async fn pr_connection_page<T: DeserializeOwned>(
        &self,
        query: &str,
        number: u64,
        cursor: Option<&str>,
    ) -> Result<(Vec<T>, Option<String>)> {
        let variables = serde_json::json!({
            "owner": OWNER, "repo": REPO, "number": number, "cursor": cursor
        });
        let data: PrPageData<T> = self.graphql(query, variables).await?;
        let page = data
            .repository
            .pull_request
            .with_context(|| format!("pull request #{number} not found"))?
            .page;
        Ok((page.nodes, page.page_info.next_cursor(cursor)?))
    }

    pub async fn fetch_issue_comments_page(
        &self,
        number: u64,
        cursor: Option<&str>,
    ) -> Result<(Vec<Comment>, Option<String>)> {
        let (nodes, next) = self
            .pr_connection_page::<IssueCommentNode>(ISSUE_COMMENTS_QUERY, number, cursor)
            .await
            .with_context(|| format!("fetching comments (graphql) for #{number}"))?;
        let comments = nodes
            .into_iter()
            .map(|node| node.fields.into_comment(CommentKind::Issue { id: node.id }))
            .collect();
        Ok((comments, next))
    }

    pub async fn fetch_reviews_page(
        &self,
        number: u64,
        cursor: Option<&str>,
    ) -> Result<(Vec<Comment>, Option<String>)> {
        let (nodes, next) = self
            .pr_connection_page::<ReviewNode>(REVIEWS_QUERY, number, cursor)
            .await
            .with_context(|| format!("fetching reviews for #{number}"))?;
        let comments = nodes
            .into_iter()
            .map(|node| {
                let kind = CommentKind::Review {
                    id: node.id,
                    state: node.state,
                };
                let mut comment = node.fields.into_comment(kind);
                if let Some(submitted) = node.submitted_at {
                    comment.created_at = submitted;
                }
                comment
            })
            .collect();
        Ok((comments, next))
    }

    pub async fn fetch_review_threads_page(
        &self,
        number: u64,
        cursor: Option<&str>,
    ) -> Result<(Vec<Comment>, Option<String>)> {
        let (threads, next) = self
            .pr_connection_page::<ReviewThreadNode>(REVIEW_THREADS_QUERY, number, cursor)
            .await
            .with_context(|| format!("fetching review threads for #{number}"))?;
        // Collect first to keep the buffered stream Send.
        let pending = threads
            .into_iter()
            .enumerate()
            .map(
                |(
                    index,
                    ReviewThreadNode {
                        thread,
                        comments: mut page,
                    },
                )| async move {
                    let mut comments = Vec::new();
                    let mut comment_cursor = None;
                    loop {
                        comments.extend(
                            page.nodes
                                .into_iter()
                                .map(|node| thread.build_comment(node)),
                        );
                        let Some(next) = page.page_info.next_cursor(comment_cursor.as_deref())?
                        else {
                            break;
                        };
                        let variables = serde_json::json!({ "id": thread.id, "cursor": next });
                        let data: ThreadCommentsData = self
                            .graphql(THREAD_COMMENTS_QUERY, variables)
                            .await
                            .with_context(|| {
                                format!("fetching comments for review thread {}", thread.id)
                            })?;
                        page = data
                            .node
                            .with_context(|| format!("review thread {} not found", thread.id))?
                            .comments;
                        comment_cursor = Some(next);
                    }
                    Ok::<_, anyhow::Error>((index, comments))
                },
            )
            .collect::<Vec<_>>();
        let mut threads: Vec<(usize, Vec<Comment>)> = stream::iter(pending)
            .buffer_unordered(MAX_CONCURRENT_REQUESTS)
            .try_collect()
            .await?;
        threads.sort_unstable_by_key(|(index, _)| *index);
        let comments = threads
            .into_iter()
            .flat_map(|(_, comments)| comments)
            .collect();
        Ok((comments, next))
    }
}
