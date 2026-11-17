//! Re-checks stored Discover issues and drops the ones that are no longer open.

use log::{info, warn};
use sqlx::PgPool;

use crate::error::{Error, Result};
use crate::graphql::{
    issue_status::{IssueState, IssueStatusNodes, Variables},
    IssueStatus,
};
use crate::issue::Issue;
use crate::routine::{GraphQLTransport, UpdateReport, MIN_RATE_LIMIT_REMAINING};

/// `nodes(ids:)` accepts at most 100 ids per request.
const BATCH_SIZE: usize = 100;

/// Checks the `limit` least-recently-checked issues. Open issues are marked as checked,
/// closed or no-longer-resolvable (deleted, transferred, made private) issues are removed.
pub(crate) async fn refresh(pool: &PgPool, transport: &GraphQLTransport, limit: u16) -> Result<UpdateReport> {
    let ids = Issue::retrieve_ids_least_recently_checked(pool, i64::from(limit)).await?;
    let mut report = UpdateReport::default();

    for chunk in ids.chunks(BATCH_SIZE) {
        let data = transport.post::<IssueStatus>(Variables { ids: chunk.to_vec() }).await?;

        if data.nodes.len() != chunk.len() {
            return Err(Error::Other(format!(
                "GitHub returned {} nodes for {} ids",
                data.nodes.len(),
                chunk.len()
            )));
        }

        let mut open = Vec::new();
        let mut gone = Vec::new();
        for (id, node) in chunk.iter().zip(data.nodes) {
            match node {
                Some(IssueStatusNodes::Issue(issue)) if matches!(issue.state, IssueState::OPEN) => {
                    open.push(id.clone())
                }
                _ => gone.push(id.clone()),
            }
        }

        report.checked += chunk.len();
        report.still_open += Issue::touch(pool, &open).await?;
        report.removed += Issue::remove(pool, &gone).await?;

        if data
            .rate_limit
            .as_ref()
            .is_some_and(|r| r.remaining < MIN_RATE_LIMIT_REMAINING)
        {
            warn!("discover: stopping status refresh, GitHub rate limit nearly exhausted");
            break;
        }
    }

    info!(
        "discover: checked {} issues, {} still open, {} removed",
        report.checked, report.still_open, report.removed
    );
    Ok(report)
}
