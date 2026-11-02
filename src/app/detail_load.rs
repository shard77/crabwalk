use super::AppEvent;
use crate::github::GitHubClient;
use crate::types::{Comment, CommentKind};
use anyhow::{Context, Result};
use tokio::sync::mpsc;

enum CommentSource {
    IssueComments,
    Reviews,
    ReviewThreads,
    Timeline,
}

// Break timestamp ties by source so request completion order cannot reorder the conversation.
pub(super) fn chronological_key(comment: &Comment) -> (chrono::DateTime<chrono::Utc>, u8) {
    let source = match comment.kind {
        CommentKind::Issue { .. } => 0,
        CommentKind::Review { .. } => 1,
        CommentKind::ReviewComment(_) => 2,
        CommentKind::Activity { .. } => 3,
    };
    (comment.created_at, source)
}

pub(super) async fn fetch(
    github: &GitHubClient,
    tx: &mpsc::Sender<AppEvent>,
    generation: u64,
    number: u64,
) -> Result<()> {
    tokio::try_join!(
        async {
            let body = github.fetch_pr_body(number).await?;
            tx.send(AppEvent::DetailBodyLoaded {
                generation,
                number,
                body,
            })
            .await
            .context("detail receiver closed")
        },
        stream_source(github, tx, generation, number, CommentSource::IssueComments),
        stream_source(github, tx, generation, number, CommentSource::Reviews),
        stream_source(github, tx, generation, number, CommentSource::ReviewThreads),
        stream_source(github, tx, generation, number, CommentSource::Timeline),
    )?;
    Ok(())
}

async fn stream_source(
    github: &GitHubClient,
    tx: &mpsc::Sender<AppEvent>,
    generation: u64,
    number: u64,
    source: CommentSource,
) -> Result<()> {
    let mut cursor = None;
    loop {
        let (page, next) = match source {
            CommentSource::IssueComments => {
                github
                    .fetch_issue_comments_page(number, cursor.as_deref())
                    .await?
            }
            CommentSource::Reviews => github.fetch_reviews_page(number, cursor.as_deref()).await?,
            CommentSource::ReviewThreads => {
                github
                    .fetch_review_threads_page(number, cursor.as_deref())
                    .await?
            }
            CommentSource::Timeline => {
                github
                    .fetch_timeline_page(number, cursor.as_deref())
                    .await?
            }
        };
        tx.send(AppEvent::DetailCommentsPage {
            generation,
            number,
            comments: page,
        })
        .await
        .context("detail receiver closed")?;
        cursor = next;
        if cursor.is_none() {
            break;
        }
    }
    Ok(())
}
