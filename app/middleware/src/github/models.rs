//! Subsets of GitHub REST resources used by the application.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    pub avatar_url: String,
    pub html_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimpleUser {
    pub id: i64,
    pub login: String,
    pub avatar_url: String,
    pub html_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoPermissions {
    #[serde(default)]
    pub admin: bool,
    #[serde(default)]
    pub maintain: bool,
    #[serde(default)]
    pub push: bool,
    #[serde(default)]
    pub triage: bool,
    #[serde(default)]
    pub pull: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub owner: SimpleUser,
    pub private: bool,
    #[serde(default)]
    pub description: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub open_issues_count: i64,
    #[serde(default = "default_true")]
    pub has_issues: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub permissions: Option<RepoPermissions>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub id: i64,
    pub number: i64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub state: String,
    #[serde(default)]
    pub state_reason: Option<String>,
    #[serde(default)]
    pub user: Option<SimpleUser>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub assignees: Vec<SimpleUser>,
    #[serde(default)]
    pub comments: i64,
    pub html_url: String,
    #[serde(default)]
    pub locked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub closed_at: Option<DateTime<Utc>>,
    /// Present only when the "issue" is a pull request; never sent to clients.
    #[serde(default, skip_serializing)]
    pub pull_request: Option<serde_json::Value>,
}

impl Issue {
    pub fn is_pull_request(&self) -> bool {
        self.pull_request.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub id: i64,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub user: Option<SimpleUser>,
    pub html_url: String,
    #[serde(default)]
    pub author_association: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SearchResult<T> {
    pub(crate) total_count: u64,
    pub(crate) items: Vec<T>,
}

/// Fields that may be changed with `PATCH /repos/{owner}/{repo}/issues/{number}`.
/// `None` fields are omitted from the request and left unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IssuePatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NewIssue {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assignees: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}
