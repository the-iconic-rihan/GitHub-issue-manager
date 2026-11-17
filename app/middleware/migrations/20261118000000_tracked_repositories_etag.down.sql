DROP INDEX IF EXISTS tracked_repositories_user_synced_idx;
ALTER TABLE tracked_repositories DROP COLUMN IF EXISTS issues_etag;
