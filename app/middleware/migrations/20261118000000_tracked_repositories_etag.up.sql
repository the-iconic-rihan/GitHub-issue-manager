-- ETag of the first page of a repository's issue listing at the last complete sync.
-- Lets automatic syncs use conditional requests (304 Not Modified is free of rate limit).
ALTER TABLE tracked_repositories ADD COLUMN issues_etag TEXT;

-- Finding repositories whose snapshot is stale.
CREATE INDEX tracked_repositories_user_synced_idx ON tracked_repositories (user_id, last_synced_at NULLS FIRST);
