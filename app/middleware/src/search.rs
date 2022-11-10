use graphql_client::{GraphQLQuery, Response};
use log::{debug, info};
use reqwest::RequestBuilder;
use sqlx::PgPool;

use crate::error::{Error, Result};
use crate::graphql::{
    issue_easy::{IssueEasySearch, IssueEasySearchEdgesNode, ResponseData, Variables},
    IssueEasy,
};
use crate::issue::Issue;
use crate::pagination::Pagination;

#[derive(Debug)]
pub struct IssueEasyPage {
    pagination: Pagination,
    issues: Vec<Issue>,
}

impl From<IssueEasySearch> for Result<IssueEasyPage> {
    fn from(search: IssueEasySearch) -> Result<IssueEasyPage> {
        let pagination = Pagination {
            has_next_page: search.page_info.has_next_page,
            end_cursor: search.page_info.end_cursor,
        };

        let edges = search
            .edges
            .ok_or_else(|| Error::Missing(String::from("Search returned no results")))?
            .into_iter()
            // `edges` is `None` or contains a list of `None`,
            // either way, the search returned no results.
            .flatten()
            .map(|edge| edge.node)
            .collect::<Vec<_>>();

        let mut issues = Vec::with_capacity(edges.len());

        for edge in edges {
            if let IssueEasySearchEdgesNode::Issue(issue) = edge
                .ok_or_else(|| Error::Missing(String::from("Search returned no related issues")))?
            {
                // issue
                let id = issue.id;
                let title = issue.title;
                let created_at = issue.created_at;
                let url = issue.url;
                let labels = issue
                    .labels
                    // `labels` is double nested
                    // which can be `flatten` safely
                    .and_then(|labels| labels.nodes)
                    .ok_or_else(|| Error::Missing(String::from("Issue has no labels")))?
                    .into_iter()
                    // filter out `None`
                    .filter_map(|label| label.map(|l| l.name))
                    .collect::<Vec<_>>();

                // repository
                let repository_name = issue.repository.name;
                // `fork_count` and `stargazer_count`
                // are of type `i64`.
                // They cannot be negative, but maybe they are
                // set to `i64` because the conversion
                // corrosponds to `BIGINT` in sql databases.
                let fork_count = issue.repository.fork_count;
                let star_count = issue.repository.stargazer_count;
                let primary_language = issue
                    .repository
                    .primary_language
                    .map(|lang| lang.name)
                    // Either `primary_language` is set or it's a new/unknown language.
                    // primary_language: "lang"
                    // or
                    // primary_language: ""
                    // should convey enough infomation.
                    .unwrap_or_default();

                issues.push(Issue {
                    id,
                    title,
                    created_at,
                    url,
                    labels,
                    repository_name,
                    fork_count,
                    star_count,
                    primary_language,
                })
            }
        }

        Ok(IssueEasyPage { pagination, issues })
    }
}

impl IssueEasyPage {
    pub async fn new(pool: &PgPool, request_builder: &RequestBuilder, issues: u16) -> Result<()> {
        info!("issues: {:#?}", &issues);

        let mut first;
        let mut pages;
        let last_page_issues: u16;

        if issues <= 100 {
            first = issues;
            pages = 1;
            last_page_issues = issues;
        } else {
            first = 100;
            pages = (issues as f32 / first as f32).floor() as i32;
            last_page_issues = issues % first;
        }

        info!("first: {:#?}", &first);
        info!("pages: {:#?}", &pages);

        // If table is empty and now rows are found, set latest_id to empty string.
        let latest_id = Issue::retrieve_latest_id(pool).await.unwrap_or_default();

        debug!("latest_id: {:#?}", latest_id);

        // `Vec`used as a stack.
        let mut end_cursors: Vec<Option<String>> = vec![None];

        'outer: while !end_cursors.is_empty() {
            // Breaks loop when current number of pages traversed
            // exceeds number of pages to be traversed.
            if pages <= 0 {
                break;
            } else {
                pages -= 1;
            }

            // Going to process last page
            if pages == 1 {
                first = last_page_issues;
            }

            let request_body = IssueEasy::build_query(Variables {
                first: first as i64,
                after: end_cursors.pop().flatten(),
            });

            let request_builder = request_builder
                .try_clone()
                .ok_or_else(|| Error::Other("Unable to clone request_builder".to_owned()))?
                .json(&request_body);

            debug!("request_builder: {:#?}", &request_builder);

            let response = request_builder
                .send()
                .await
                .map_err(Error::Reqwest)?
                .json::<Response<ResponseData>>()
                .await
                .map_err(Error::Reqwest)?
                .data
                .ok_or_else(|| Error::Missing(String::from("Response is missing `data` field")))?;

            debug!("response: {:#?}", &response);

            let search: Result<IssueEasyPage> = response.search.into();

            debug!("search: {:#?}", &search);

            let rate_limit = response.rate_limit;

            info!("rate_limit: {:#?}", &rate_limit);

            let IssueEasyPage { pagination, issues } = search?;

            debug!("end_cursor: {:#?}", &pagination.end_cursor);

            end_cursors.push(pagination.end_cursor);

            debug!("end_cursors: {:#?}", &end_cursors);

            for issue in issues {
                debug!("issue: {:#?}", &issue);

                // Break loop if this id exists in database already,
                // because all issues after this should already be
                // in the database.
                // GitHub GraphQL API returns in ascending order
                // (latest issues first).
                if issue.id == latest_id {
                    break 'outer;
                }

                let insertion = Issue::insert(pool, &issue).await?;

                debug!("insertion: {:#?}", &insertion);
            }

            // Breaks the loop if there's no next page.
            // Has to be checked after current page is stored.
            if !pagination.has_next_page {
                break;
            }
        }

        Ok(())
    }
}
