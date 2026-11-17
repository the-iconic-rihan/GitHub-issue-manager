//! Request DTOs and their validation.

use middleware::github::{
    models::{IssuePatch, NewIssue},
    params::{
        is_valid_label, is_valid_login, is_valid_repo_name, AssigneeFilter, Direction, IssueQuery, IssueSort,
        IssueState,
    },
    MAX_PER_PAGE,
};
use serde::Deserialize;

use crate::error::{ApiError, FieldError};

pub const MAX_TITLE_CHARS: usize = 256;
pub const MAX_BODY_CHARS: usize = 65_536;
pub const MAX_ASSIGNEES: usize = 10;
pub const MAX_LABELS: usize = 100;
pub const MAX_FILTER_LABELS: usize = 20;
pub const MAX_SEARCH_CHARS: usize = 256;
/// GitHub's search API returns at most 1000 results.
pub const MAX_SEARCH_RESULTS: u32 = 1000;
pub const DEFAULT_PER_PAGE: u32 = 30;

/// Collects field errors and turns them into a single 422 response.
#[derive(Default)]
struct Errors(Vec<FieldError>);

impl Errors {
    fn add(&mut self, field: &str, message: impl Into<String>) {
        self.0.push(FieldError::new(field, message));
    }

    fn finish(self) -> Result<(), ApiError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(ApiError::Validation(self.0))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RepoPath {
    pub owner: String,
    pub repo: String,
}

impl RepoPath {
    pub fn validate(&self) -> Result<(), ApiError> {
        let mut e = Errors::default();
        if !is_valid_login(&self.owner) {
            e.add("owner", "is not a valid GitHub owner name");
        }
        if !is_valid_repo_name(&self.repo) {
            e.add("repo", "is not a valid GitHub repository name");
        }
        e.finish()
    }

    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

#[derive(Debug, Deserialize)]
pub struct IssuePath {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl IssuePath {
    pub fn validate(&self) -> Result<RepoPath, ApiError> {
        let repo = RepoPath {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
        };
        repo.validate()?;
        if self.number == 0 || self.number > i32::MAX as u64 {
            return Err(ApiError::validation("number", "must be a positive issue number"));
        }
        Ok(repo)
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct PageQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl PageQuery {
    /// Returns `(page, per_page)` with defaults applied.
    pub fn resolve(&self) -> Result<(u32, u32), ApiError> {
        let page = self.page.unwrap_or(1);
        let per_page = self.per_page.unwrap_or(DEFAULT_PER_PAGE);
        let mut e = Errors::default();
        if page == 0 || page > 10_000 {
            e.add("page", "must be between 1 and 10000");
        }
        if per_page == 0 || per_page > MAX_PER_PAGE {
            e.add("per_page", format!("must be between 1 and {MAX_PER_PAGE}"));
        }
        e.finish().map(|_| (page, per_page))
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct IssueListQuery {
    pub state: Option<IssueState>,
    /// Comma-separated label names.
    pub labels: Option<String>,
    /// A login, `none` (unassigned) or `*` (assigned to anyone).
    pub assignee: Option<String>,
    pub sort: Option<IssueSort>,
    pub direction: Option<Direction>,
    pub q: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

impl IssueListQuery {
    pub fn into_query(self) -> Result<IssueQuery, ApiError> {
        let (page, per_page) = PageQuery {
            page: self.page,
            per_page: self.per_page,
        }
        .resolve()?;
        let mut e = Errors::default();

        let labels: Vec<String> = self
            .labels
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect();
        if labels.len() > MAX_FILTER_LABELS {
            e.add("labels", format!("at most {MAX_FILTER_LABELS} labels can be combined"));
        }
        if labels.iter().any(|l| !is_valid_label(l)) {
            e.add("labels", "contains an invalid label name");
        }

        let assignee = match self.assignee.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
            None => None,
            Some(a) => match AssigneeFilter::parse(a) {
                Some(f) => Some(f),
                None => {
                    e.add("assignee", "must be a GitHub login, 'none' or '*'");
                    None
                }
            },
        };

        let text = self.q.map(|q| q.trim().to_owned()).filter(|q| !q.is_empty());
        if let Some(t) = &text {
            if t.chars().count() > MAX_SEARCH_CHARS {
                e.add("q", format!("must be at most {MAX_SEARCH_CHARS} characters"));
            }
            if t.chars().any(char::is_control) {
                e.add("q", "must not contain control characters");
            }
            if page.saturating_mul(per_page) > MAX_SEARCH_RESULTS {
                e.add(
                    "page",
                    format!("search results are limited to the first {MAX_SEARCH_RESULTS} matches"),
                );
            }
        }
        e.finish()?;

        Ok(IssueQuery {
            state: self.state.unwrap_or_default(),
            labels,
            assignee,
            sort: self.sort.unwrap_or_default(),
            direction: self.direction.unwrap_or_default(),
            text,
            page,
            per_page,
        })
    }
}

fn check_title(e: &mut Errors, title: &str) {
    let len = title.trim().chars().count();
    if len == 0 {
        e.add("title", "is required");
    } else if len > MAX_TITLE_CHARS {
        e.add("title", format!("must be at most {MAX_TITLE_CHARS} characters"));
    }
}

fn check_body(e: &mut Errors, field: &str, body: &str) {
    if body.chars().count() > MAX_BODY_CHARS {
        e.add(field, format!("must be at most {MAX_BODY_CHARS} characters"));
    }
}

fn check_labels(e: &mut Errors, labels: &[String]) {
    if labels.len() > MAX_LABELS {
        e.add("labels", format!("at most {MAX_LABELS} labels are allowed"));
    }
    if labels.iter().any(|l| !is_valid_label(l)) {
        e.add(
            "labels",
            "label names must be 1-50 characters without control characters",
        );
    }
}

fn check_assignees(e: &mut Errors, assignees: &[String]) {
    if assignees.len() > MAX_ASSIGNEES {
        e.add("assignees", format!("at most {MAX_ASSIGNEES} assignees are allowed"));
    }
    if assignees.iter().any(|a| !is_valid_login(a)) {
        e.add("assignees", "contains an invalid GitHub login");
    }
}

/// Trims, drops blanks and removes duplicates (case-insensitively) while keeping order.
fn normalise(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    values
        .into_iter()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty() && seen.insert(v.to_lowercase()))
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateIssueBody {
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub assignees: Vec<String>,
}

impl CreateIssueBody {
    pub fn into_new_issue(self) -> Result<NewIssue, ApiError> {
        let labels = normalise(self.labels);
        let assignees = normalise(self.assignees);
        let mut e = Errors::default();
        check_title(&mut e, &self.title);
        if let Some(b) = &self.body {
            check_body(&mut e, "body", b);
        }
        check_labels(&mut e, &labels);
        check_assignees(&mut e, &assignees);
        e.finish()?;
        Ok(NewIssue {
            title: self.title.trim().to_owned(),
            body: self.body.filter(|b| !b.trim().is_empty()),
            labels,
            assignees,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NewState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateReason {
    Completed,
    NotPlanned,
    Reopened,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateIssueBody {
    pub title: Option<String>,
    /// An empty string clears the description.
    pub body: Option<String>,
    pub state: Option<NewState>,
    pub state_reason: Option<StateReason>,
    /// Replaces all assignees (empty list unassigns everyone).
    pub assignees: Option<Vec<String>>,
    /// Replaces all labels (empty list removes all labels).
    pub labels: Option<Vec<String>>,
}

impl UpdateIssueBody {
    pub fn into_patch(self) -> Result<IssuePatch, ApiError> {
        let mut e = Errors::default();
        if self.title.is_none()
            && self.body.is_none()
            && self.state.is_none()
            && self.assignees.is_none()
            && self.labels.is_none()
        {
            e.add(
                "body",
                "provide at least one of title, body, state, assignees or labels",
            );
        }
        if let Some(t) = &self.title {
            check_title(&mut e, t);
        }
        if let Some(b) = &self.body {
            check_body(&mut e, "body", b);
        }
        let assignees = self.assignees.map(normalise);
        if let Some(a) = &assignees {
            check_assignees(&mut e, a);
        }
        let labels = self.labels.map(normalise);
        if let Some(l) = &labels {
            check_labels(&mut e, l);
        }
        let state_reason = match (self.state, self.state_reason) {
            (_, None) => None,
            (Some(NewState::Closed), Some(r @ (StateReason::Completed | StateReason::NotPlanned))) => Some(r),
            (Some(NewState::Open), Some(StateReason::Reopened)) => Some(StateReason::Reopened),
            (None, Some(_)) => {
                e.add("state_reason", "requires state");
                None
            }
            _ => {
                e.add(
                    "state_reason",
                    "closing allows 'completed' or 'not_planned'; reopening allows 'reopened'",
                );
                None
            }
        };
        e.finish()?;

        Ok(IssuePatch {
            title: self.title.map(|t| t.trim().to_owned()),
            body: self.body,
            state: self.state.map(|s| match s {
                NewState::Open => "open".to_owned(),
                NewState::Closed => "closed".to_owned(),
            }),
            state_reason: state_reason.map(|r| {
                match r {
                    StateReason::Completed => "completed",
                    StateReason::NotPlanned => "not_planned",
                    StateReason::Reopened => "reopened",
                }
                .to_owned()
            }),
            assignees,
            labels,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommentBody {
    pub body: String,
}

impl CommentBody {
    pub fn validate(&self) -> Result<(), ApiError> {
        let mut e = Errors::default();
        if self.body.trim().is_empty() {
            e.add("body", "is required");
        }
        check_body(&mut e, "body", &self.body);
        e.finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelsBody {
    pub labels: Vec<String>,
}

impl LabelsBody {
    pub fn into_labels(self) -> Result<Vec<String>, ApiError> {
        let labels = normalise(self.labels);
        let mut e = Errors::default();
        if labels.is_empty() {
            e.add("labels", "provide at least one label");
        }
        check_labels(&mut e, &labels);
        e.finish().map(|_| labels)
    }
}

#[derive(Debug, Deserialize)]
pub struct LabelNameQuery {
    pub name: String,
}

impl LabelNameQuery {
    pub fn validate(&self) -> Result<(), ApiError> {
        if is_valid_label(&self.name) {
            Ok(())
        } else {
            Err(ApiError::validation("name", "is not a valid label name"))
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct DiscoverQuery {
    pub language: Option<String>,
    pub label: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(err: ApiError) -> Vec<String> {
        match err {
            ApiError::Validation(f) => f.into_iter().map(|f| f.field).collect(),
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn create_issue_requires_title_and_normalises() {
        let err = CreateIssueBody {
            title: "  ".into(),
            body: None,
            labels: vec![],
            assignees: vec![],
        }
        .into_new_issue()
        .unwrap_err();
        assert_eq!(fields(err), vec!["title"]);

        let ok = CreateIssueBody {
            title: "  Crash on start ".into(),
            body: Some("   ".into()),
            labels: vec!["bug".into(), "Bug".into(), " ".into()],
            assignees: vec!["octocat".into()],
        }
        .into_new_issue()
        .unwrap();
        assert_eq!(ok.title, "Crash on start");
        assert_eq!(ok.body, None);
        assert_eq!(ok.labels, vec!["bug"]);
    }

    #[test]
    fn create_issue_limits() {
        let err = CreateIssueBody {
            title: "x".repeat(MAX_TITLE_CHARS + 1),
            body: Some("x".repeat(MAX_BODY_CHARS + 1)),
            labels: vec![],
            assignees: (0..11).map(|i| format!("user{i}")).collect(),
        }
        .into_new_issue()
        .unwrap_err();
        assert_eq!(fields(err), vec!["title", "body", "assignees"]);
    }

    #[test]
    fn update_issue_requires_a_field() {
        assert_eq!(
            fields(UpdateIssueBody::default().into_patch().unwrap_err()),
            vec!["body"]
        );
    }

    #[test]
    fn update_issue_state_reason_rules() {
        let close = UpdateIssueBody {
            state: Some(NewState::Closed),
            state_reason: Some(StateReason::NotPlanned),
            ..Default::default()
        }
        .into_patch()
        .unwrap();
        assert_eq!(close.state.as_deref(), Some("closed"));
        assert_eq!(close.state_reason.as_deref(), Some("not_planned"));

        let bad = UpdateIssueBody {
            state: Some(NewState::Open),
            state_reason: Some(StateReason::Completed),
            ..Default::default()
        };
        assert_eq!(fields(bad.into_patch().unwrap_err()), vec!["state_reason"]);

        let orphan = UpdateIssueBody {
            state_reason: Some(StateReason::Completed),
            title: Some("t".into()),
            ..Default::default()
        };
        assert_eq!(fields(orphan.into_patch().unwrap_err()), vec!["state_reason"]);
    }

    #[test]
    fn update_allows_clearing_assignees_and_labels() {
        let patch = UpdateIssueBody {
            assignees: Some(vec![]),
            labels: Some(vec![]),
            ..Default::default()
        }
        .into_patch()
        .unwrap();
        assert_eq!(patch.assignees, Some(vec![]));
        assert_eq!(patch.labels, Some(vec![]));
    }

    #[test]
    fn list_query_defaults_and_errors() {
        let q = IssueListQuery::default().into_query().unwrap();
        assert_eq!((q.page, q.per_page, q.state), (1, DEFAULT_PER_PAGE, IssueState::Open));

        let q = IssueListQuery {
            labels: Some("bug, ui ,".into()),
            assignee: Some("none".into()),
            ..Default::default()
        }
        .into_query()
        .unwrap();
        assert_eq!(q.labels, vec!["bug", "ui"]);
        assert_eq!(q.assignee, Some(AssigneeFilter::None));

        let err = IssueListQuery {
            per_page: Some(500),
            assignee: Some("no spaces".into()),
            ..Default::default()
        }
        .into_query()
        .unwrap_err();
        assert_eq!(fields(err), vec!["per_page"]);

        let err = IssueListQuery {
            assignee: Some("bad name".into()),
            ..Default::default()
        }
        .into_query()
        .unwrap_err();
        assert_eq!(fields(err), vec!["assignee"]);

        let err = IssueListQuery {
            q: Some("x".into()),
            page: Some(11),
            per_page: Some(100),
            ..Default::default()
        }
        .into_query()
        .unwrap_err();
        assert_eq!(fields(err), vec!["page"]);
    }

    #[test]
    fn repo_path_validation() {
        assert!(RepoPath {
            owner: "acme".into(),
            repo: "app.rs".into()
        }
        .validate()
        .is_ok());
        let err = RepoPath {
            owner: "../x".into(),
            repo: "a b".into(),
        }
        .validate()
        .unwrap_err();
        assert_eq!(fields(err), vec!["owner", "repo"]);
        assert!(IssuePath {
            owner: "a".into(),
            repo: "b".into(),
            number: 0
        }
        .validate()
        .is_err());
    }

    #[test]
    fn comment_and_labels() {
        assert!(CommentBody { body: " \n".into() }.validate().is_err());
        assert!(CommentBody { body: "LGTM".into() }.validate().is_ok());
        assert!(LabelsBody {
            labels: vec![" ".into()]
        }
        .into_labels()
        .is_err());
        assert_eq!(
            LabelsBody {
                labels: vec!["bug".into(), "BUG".into()]
            }
            .into_labels()
            .unwrap(),
            vec!["bug"]
        );
    }
}
