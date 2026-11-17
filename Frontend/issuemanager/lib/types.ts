// Shapes returned by the backend API (`app/server`). Keep in sync with the Rust models.

export interface PageMeta {
  page: number;
  per_page: number;
  has_next: boolean;
  total_count?: number;
}

export interface GitHubUser {
  id: number;
  login: string;
  name?: string | null;
  avatar_url: string;
  html_url: string;
}

export interface SimpleUser {
  id: number;
  login: string;
  avatar_url: string;
  html_url: string;
}

export interface Label {
  id: number;
  name: string;
  color: string;
  description?: string | null;
}

export interface RepoPermissions {
  admin: boolean;
  maintain: boolean;
  push: boolean;
  triage: boolean;
  pull: boolean;
}

export interface Repository {
  id: number;
  name: string;
  full_name: string;
  owner: SimpleUser;
  private: boolean;
  description?: string | null;
  html_url: string;
  open_issues_count: number;
  has_issues: boolean;
  archived: boolean;
  updated_at?: string | null;
  permissions?: RepoPermissions | null;
}

export interface RepoListItem extends Repository {
  tracked: boolean;
}

export interface TrackedRepository {
  repo_id: number;
  full_name: string;
  private: boolean;
  created_at: string;
  last_synced_at: string | null;
}

export interface RepoDetail extends Repository {
  tracked: TrackedRepository | null;
}

export type IssueStateValue = "open" | "closed";

export interface Issue {
  id: number;
  number: number;
  title: string;
  body?: string | null;
  state: IssueStateValue;
  state_reason?: string | null;
  user?: SimpleUser | null;
  labels: Label[];
  assignees: SimpleUser[];
  comments: number;
  html_url: string;
  locked: boolean;
  created_at: string;
  updated_at: string;
  closed_at?: string | null;
}

export interface Comment {
  id: number;
  body?: string | null;
  user?: SimpleUser | null;
  html_url: string;
  author_association?: string | null;
  created_at: string;
  updated_at: string;
}

export interface Me {
  user: GitHubUser;
  discover_enabled: boolean;
}

export interface SyncResult {
  repository: TrackedRepository;
  issues_fetched: number;
  truncated: boolean;
  synced_at: string;
}

export interface RepoStats {
  repo_id: number;
  full_name: string;
  private: boolean;
  last_synced_at: string | null;
  open_issues: number;
  closed_issues: number;
  unassigned_open: number;
  assigned_to_me_open: number;
  opened_last_30_days: number;
  closed_last_30_days: number;
}

export interface Dashboard {
  totals: {
    tracked_repositories: number;
    open_issues: number;
    closed_issues: number;
    unassigned_open: number;
    assigned_to_me_open: number;
    opened_last_30_days: number;
    closed_last_30_days: number;
  };
  repositories: RepoStats[];
  top_labels: { label: string; count: number }[];
  recently_updated: {
    repo_full_name: string;
    number: number;
    title: string;
    state: IssueStateValue;
    html_url: string;
    labels: string[];
    assignees: string[];
    updated_at: string;
  }[];
}

export interface DiscoverIssue {
  id: string;
  title: string;
  created_at: string;
  url: string;
  labels: string[];
  repository_name: string;
  fork_count: number;
  star_count: number;
  primary_language: string;
}

export interface LanguageCount {
  language: string;
  count: number;
}
