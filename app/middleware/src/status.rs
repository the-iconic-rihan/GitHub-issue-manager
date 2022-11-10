use log::{debug, info};
use reqwest::RequestBuilder;
use sqlx::PgPool;

use crate::graphql::issue_status::{IssueState, IssueStatusNode, ResponseData, Variables};
use crate::issue::Issue;
use crate::{
    error::{Error, Result},
    graphql::IssueStatus,
};
use graphql_client::{GraphQLQuery, Response};

pub struct IssueStatusPage;

impl IssueStatusPage {
    pub async fn update(
        pool: &PgPool,
        request_builder: &RequestBuilder,
        issues: u16,
    ) -> Result<()> {
        info!("issues: {:#?}", &issues);

        let ids = Issue::retrieve_ids_asc(pool, &(issues as i64)).await?;

        debug!("ids: {:#?}", &ids);

        for id in ids {
            let request_body = IssueStatus::build_query(Variables { id: id.clone() });

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
                .map_err(Error::Reqwest)?;

            info!("response: {:#?}", &response);

            let response_parsed = response
                .data
                .ok_or_else(|| Error::Missing(String::from("Response is missing `data` field")))?;

            debug!("response_parsed: {:#?}", &response_parsed);

            let issue = response_parsed.node.and_then(|n| {
                if let IssueStatusNode::Issue(issue) = n {
                    Some(issue.state)
                } else {
                    None
                }
            });

            let rate_limit = response_parsed.rate_limit;

            info!("rate_limit: {:#?}", &rate_limit);

            if let Some(issue) = issue {
                debug!("issue: {:#?}", &issue);
                match issue {
                    IssueState::CLOSED => {
                        let removal = Issue::remove(pool, &id).await?;

                        debug!("removal: {:#?}", &removal);
                    }
                    _ => {
                        let updation = Issue::update_timestamp(pool, &id).await?;

                        debug!("updation: {:#?}", &updation);
                    }
                }
            }
        }

        Ok(())
    }
}
