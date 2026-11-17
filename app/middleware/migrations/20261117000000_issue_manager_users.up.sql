-- Per-user issue manager: users, the repositories they track, and a snapshot of those
-- repositories' issues (refreshed by "sync" and by write-through after edits) that powers
-- the dashboard. GitHub remains the source of truth; nothing here is authoritative.

CREATE TABLE users
(
    id           BIGINT PRIMARY KEY,               -- GitHub user id (stable across renames)
    login        TEXT        NOT NULL,
    name         TEXT,
    avatar_url   TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE tracked_repositories
(
    user_id        BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    repo_id        BIGINT      NOT NULL,           -- GitHub repository id (stable across renames)
    full_name      TEXT        NOT NULL,           -- owner/name as last seen
    private        BOOLEAN     NOT NULL DEFAULT false,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ,
    PRIMARY KEY (user_id, repo_id)
);

CREATE UNIQUE INDEX tracked_repositories_user_full_name_idx
    ON tracked_repositories (user_id, lower(full_name));

CREATE TABLE repo_issues
(
    user_id      BIGINT      NOT NULL,
    repo_id      BIGINT      NOT NULL,
    number       INTEGER     NOT NULL,
    github_id    BIGINT      NOT NULL,
    title        TEXT        NOT NULL,
    state        TEXT        NOT NULL CHECK (state IN ('open', 'closed')),
    author_login TEXT,
    assignees    TEXT[]      NOT NULL DEFAULT '{}',
    labels       TEXT[]      NOT NULL DEFAULT '{}',
    comments     INTEGER     NOT NULL DEFAULT 0 CHECK (comments >= 0),
    html_url     TEXT        NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL,
    updated_at   TIMESTAMPTZ NOT NULL,
    closed_at    TIMESTAMPTZ,
    PRIMARY KEY (user_id, repo_id, number),
    FOREIGN KEY (user_id, repo_id) REFERENCES tracked_repositories (user_id, repo_id) ON DELETE CASCADE
);

CREATE INDEX repo_issues_user_state_idx ON repo_issues (user_id, state);
CREATE INDEX repo_issues_user_updated_idx ON repo_issues (user_id, updated_at DESC);

-- Discover (aggregator) table: support its filters and maintenance queries.
CREATE INDEX issues_created_at_idx ON issues (created_at DESC);
CREATE INDEX issues_updated_at_idx ON issues (updated_at ASC);
CREATE INDEX issues_primary_language_idx ON issues (lower(primary_language));
