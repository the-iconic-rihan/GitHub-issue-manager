//! Persistence for the per-user issue manager. All queries are scoped by `user_id`, so one
//! user can never read another user's tracked repositories or cached (possibly private) issues.

pub mod repo_issues;
pub mod tracked;
pub mod users;
