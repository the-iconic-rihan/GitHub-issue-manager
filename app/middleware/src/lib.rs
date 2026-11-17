//! Shared backend library for GitHub Issue Manager.
//!
//! * [`github`] – REST client used on behalf of signed-in users (their own OAuth token).
//! * [`store`] – persistence for users, tracked repositories and synced issue snapshots.
//! * [`routine`] – the "Discover" aggregator which collects beginner-friendly issues from
//!   all of GitHub using a server-wide token (GraphQL API), stored in the `issues` table.
//! * [`db`] – connection pool and embedded migrations.

pub mod db;
pub mod error;
pub mod github;
mod graphql;
pub mod issue;
mod pagination;
pub mod routine;
mod search;
mod status;
pub mod store;
