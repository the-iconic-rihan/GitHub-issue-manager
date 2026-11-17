//! Collects new beginner-friendly issues from GitHub search into the `issues` table.

use log::{debug, info, warn};
use sqlx::PgPool;

use crate::error::Result;
use crate::graphql::{
    issue_easy::{IssueEasySearch, IssueEasySearchEdgesNode, Variables},
    IssueEasy,
};
use crate::issue::Issue;
use crate::pagination::Pagination;
use crate::routine::{GraphQLTransport, InsertReport, StopReason, MIN_RATE_LIMIT_REMAINING};

/// GitHub caps `first` at 100 per request.
const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug)]
pub(crate) struct IssueEasyPage {
    pub(crate) pagination: Pagination,
    pub(crate) issues: Vec<Issue>,
}

impl From<IssueEasySearch> for IssueEasyPage {
    fn from(search: IssueEasySearch) -> Self {
        let pagination = Pagination {
            has_next_page: search.page_info.has_next_page,
            end_cursor: search.page_info.end_cursor,
        };

        let issues = search
            .edges
            .unwrap_or_default()
            .into_iter()
            // A `None` edge or node means GitHub could not resolve that result
            // (e.g. it was deleted between indexing and querying): skip it.
            .flatten()
            .filter_map(|edge| edge.node)
            .filter_map(|node| match node {
                IssueEasySearchEdgesNode::Issue(issue) => Some(issue),
                _ => None,
            })
            .map(|issue| Issue {
                id: issue.id,
                title: issue.title,
                created_at: issue.created_at,
                url: issue.url,
                // Issues without labels are valid; the label filter in the query makes
                // this rare, but it must not abort the whole page.
                labels: issue
                    .labels
                    .and_then(|labels| labels.nodes)
                    .unwrap_or_default()
                    .into_iter()
                    .flatten()
                    .map(|label| label.name)
                    .collect(),
                repository_name: issue.repository.name_with_owner,
                fork_count: issue.repository.fork_count,
                star_count: issue.repository.stargazer_count,
                // Empty string conveys "unknown language".
                primary_language: issue
                    .repository
                    .primary_language
                    .map(|lang| lang.name)
                    .unwrap_or_default(),
            })
            .collect();

        IssueEasyPage { pagination, issues }
    }
}

/// Fetches up to `limit` of the newest matching issues, newest first, and upserts them.
///
/// Stops early once it reaches issues older than the newest one already stored
/// (everything older was collected by a previous run), when search results run out,
/// or when the GitHub rate limit is nearly exhausted.
pub(crate) async fn collect(pool: &PgPool, transport: &GraphQLTransport, limit: u16) -> Result<InsertReport> {
    let newest_known = Issue::latest_created_at(pool).await?;
    debug!("discover: newest stored issue created at {newest_known:?}");

    let mut report = InsertReport::default();
    let mut remaining = i64::from(limit);
    let mut after: Option<String> = None;

    while remaining > 0 {
        let first = remaining.min(MAX_PAGE_SIZE);
        let data = transport
            .post::<IssueEasy>(Variables {
                first,
                after: after.take(),
            })
            .await?;
        report.pages += 1;

        let rate_remaining = data.rate_limit.as_ref().map(|r| r.remaining);
        let page = IssueEasyPage::from(data.search);

        let mut caught_up = false;
        for issue in page.issues {
            report.fetched += 1;
            if newest_known.is_some_and(|known| issue.created_at < known) {
                caught_up = true;
                break;
            }
            Issue::insert(pool, &issue).await?;
            report.stored += 1;
        }

        remaining -= first;

        if caught_up {
            report.stop_reason = StopReason::CaughtUp;
            break;
        }
        if !page.pagination.has_next_page {
            report.stop_reason = StopReason::NoMoreResults;
            break;
        }
        if rate_remaining.is_some_and(|r| r < MIN_RATE_LIMIT_REMAINING) {
            warn!("discover: stopping collection, GitHub rate limit nearly exhausted ({rate_remaining:?} left)");
            report.stop_reason = StopReason::RateLimited;
            break;
        }
        after = page.pagination.end_cursor;
        if after.is_none() {
            report.stop_reason = StopReason::NoMoreResults;
            break;
        }
    }

    info!(
        "discover: collected {} issues ({} fetched over {} pages, stop: {:?})",
        report.stored, report.fetched, report.pages, report.stop_reason
    );
    Ok(report)
}
