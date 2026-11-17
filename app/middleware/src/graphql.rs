use graphql_client::GraphQLQuery;

// Custom scalars referenced by the queries must be in scope with the schema's names.
#[allow(clippy::upper_case_acronyms)]
type URI = String;
pub(crate) type DateTime = chrono::DateTime<chrono::Utc>;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "query.graphql",
    response_derives = "Debug"
)]
pub struct IssueEasy;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "schema.graphql",
    query_path = "status.graphql",
    response_derives = "Debug"
)]
pub struct IssueStatus;
