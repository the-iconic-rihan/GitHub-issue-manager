//! Typed query parameters for listing/searching issues, and input validation helpers.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    #[default]
    Open,
    Closed,
    All,
}

impl IssueState {
    pub fn as_str(self) -> &'static str {
        match self {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
            IssueState::All => "all",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueSort {
    #[default]
    Created,
    Updated,
    Comments,
}

impl IssueSort {
    pub fn as_str(self) -> &'static str {
        match self {
            IssueSort::Created => "created",
            IssueSort::Updated => "updated",
            IssueSort::Comments => "comments",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Asc,
    #[default]
    Desc,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Asc => "asc",
            Direction::Desc => "desc",
        }
    }
}

/// Assignee filter: a specific login, issues with no assignee, or issues with any assignee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssigneeFilter {
    User(String),
    None,
    Any,
}

impl AssigneeFilter {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(AssigneeFilter::None),
            "*" => Some(AssigneeFilter::Any),
            login if is_valid_login(login) => Some(AssigneeFilter::User(login.to_owned())),
            _ => None,
        }
    }

    pub(crate) fn as_list_param(&self) -> &str {
        match self {
            AssigneeFilter::User(login) => login,
            AssigneeFilter::None => "none",
            AssigneeFilter::Any => "*",
        }
    }
}

/// Filters shared by list and search requests for a single repository's issues.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IssueQuery {
    pub state: IssueState,
    pub labels: Vec<String>,
    pub assignee: Option<AssigneeFilter>,
    pub sort: IssueSort,
    pub direction: Direction,
    /// Free text. When present the search API is used instead of the list API.
    pub text: Option<String>,
    pub page: u32,
    pub per_page: u32,
}

/// Qualifiers that would widen a search beyond the current repository.
const SCOPE_QUALIFIERS: [&str; 4] = ["repo:", "org:", "user:", "owner:"];

impl IssueQuery {
    /// Builds a GitHub search query restricted to issues in `owner/repo`.
    pub fn search_query(&self, owner: &str, repo: &str) -> String {
        let mut parts = vec![format!("repo:{owner}/{repo}"), "is:issue".to_owned()];
        match self.state {
            IssueState::Open => parts.push("is:open".into()),
            IssueState::Closed => parts.push("is:closed".into()),
            IssueState::All => {}
        }
        for label in &self.labels {
            parts.push(format!("label:\"{}\"", label.replace('"', "")));
        }
        match &self.assignee {
            Some(AssigneeFilter::User(login)) => parts.push(format!("assignee:{login}")),
            Some(AssigneeFilter::None) => parts.push("no:assignee".into()),
            Some(AssigneeFilter::Any) => parts.push("-no:assignee".into()),
            None => {}
        }
        if let Some(text) = &self.text {
            let cleaned = text
                .split_whitespace()
                .filter(|word| {
                    let lower = word.trim_start_matches('-').to_ascii_lowercase();
                    !SCOPE_QUALIFIERS.iter().any(|q| lower.starts_with(q))
                })
                .collect::<Vec<_>>()
                .join(" ");
            if !cleaned.is_empty() {
                parts.push(cleaned);
            }
        }
        parts.join(" ")
    }
}

/// GitHub user/organisation login (also used for repository owners).
pub fn is_valid_login(value: &str) -> bool {
    (1..=100).contains(&value.len())
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !value.starts_with('-')
}

/// GitHub repository name.
pub fn is_valid_repo_name(value: &str) -> bool {
    (1..=100).contains(&value.len())
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Label names: GitHub allows almost anything up to 50 characters, but not control characters.
pub fn is_valid_label(value: &str) -> bool {
    let len = value.chars().count();
    (1..=50).contains(&len) && !value.trim().is_empty() && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_includes_all_filters() {
        let q = IssueQuery {
            state: IssueState::Closed,
            labels: vec!["bug".into(), "good first issue".into()],
            assignee: Some(AssigneeFilter::User("octocat".into())),
            text: Some("crash on start".into()),
            ..Default::default()
        };
        assert_eq!(
            q.search_query("acme", "app"),
            r#"repo:acme/app is:issue is:closed label:"bug" label:"good first issue" assignee:octocat crash on start"#
        );
    }

    #[test]
    fn search_query_cannot_escape_repository_scope() {
        let q = IssueQuery {
            state: IssueState::All,
            text: Some("leak repo:other/secret ORG:evil -user:x keep".into()),
            ..Default::default()
        };
        assert_eq!(q.search_query("acme", "app"), "repo:acme/app is:issue leak keep");
    }

    #[test]
    fn search_query_assignee_none_and_any() {
        let mut q = IssueQuery {
            assignee: Some(AssigneeFilter::None),
            ..Default::default()
        };
        assert!(q.search_query("a", "b").ends_with("is:open no:assignee"));
        q.assignee = Some(AssigneeFilter::Any);
        assert!(q.search_query("a", "b").ends_with("is:open -no:assignee"));
    }

    #[test]
    fn label_quotes_are_stripped_in_search() {
        let q = IssueQuery {
            labels: vec![r#"a"b"#.into()],
            state: IssueState::All,
            ..Default::default()
        };
        assert_eq!(q.search_query("a", "b"), r#"repo:a/b is:issue label:"ab""#);
    }

    #[test]
    fn name_validation() {
        assert!(is_valid_login("the-iconic-rihan"));
        assert!(!is_valid_login("bad/owner"));
        assert!(!is_valid_login("-lead"));
        assert!(!is_valid_login(""));
        assert!(is_valid_repo_name("GitHub-issue-manager"));
        assert!(is_valid_repo_name("my.repo_name"));
        assert!(!is_valid_repo_name(".."));
        assert!(!is_valid_repo_name("a/b"));
        assert!(!is_valid_repo_name("a?b"));
        assert!(is_valid_label("good first issue"));
        assert!(is_valid_label("area/ui"));
        assert!(!is_valid_label("   "));
        assert!(!is_valid_label(&"x".repeat(51)));
        assert!(!is_valid_label("a\nb"));
    }

    #[test]
    fn assignee_filter_parse() {
        assert_eq!(AssigneeFilter::parse("none"), Some(AssigneeFilter::None));
        assert_eq!(AssigneeFilter::parse("*"), Some(AssigneeFilter::Any));
        assert_eq!(
            AssigneeFilter::parse("octocat"),
            Some(AssigneeFilter::User("octocat".into()))
        );
        assert_eq!(AssigneeFilter::parse("bad name"), None);
    }
}
