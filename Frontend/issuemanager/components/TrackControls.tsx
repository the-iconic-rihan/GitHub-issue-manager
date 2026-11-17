import { useState } from "react";
import { FaSync } from "react-icons/fa";

import { apiPath, apiRequest, errorMessage } from "@/lib/api-client";
import { relativeTime } from "@/lib/format";
import type { SyncResult, TrackedRepository } from "@/lib/types";

import { Notice, type NoticeKind } from "./ui";

/**
 * Track / untrack a repository and sync its issue snapshot (which powers the dashboard).
 * Tracking immediately runs a first sync.
 */
export default function TrackControls({
  owner,
  repo,
  tracked,
  onChange,
  compact = false,
}: {
  owner: string;
  repo: string;
  tracked: TrackedRepository | null;
  onChange: (tracked: TrackedRepository | null) => void;
  compact?: boolean;
}) {
  const [busy, setBusy] = useState<"track" | "untrack" | "sync" | null>(null);
  const [notice, setNotice] = useState<{ kind: NoticeKind; text: string }>();
  const base = `tracked-repos/${apiPath(owner, repo)}`;

  async function sync() {
    setBusy("sync");
    setNotice(undefined);
    try {
      const { data } = await apiRequest<SyncResult>(`${base}/sync`, { method: "POST" });
      onChange(data.repository);
      setNotice({
        kind: data.truncated ? "info" : "success",
        text: data.truncated
          ? `Synced the ${data.issues_fetched} most recently updated issues (the repository has more than the sync limit).`
          : `Synced ${data.issues_fetched} issue${data.issues_fetched === 1 ? "" : "s"}.`,
      });
    } catch (err) {
      setNotice({ kind: "error", text: `Sync failed: ${errorMessage(err)}` });
    } finally {
      setBusy(null);
    }
  }

  async function track() {
    setBusy("track");
    setNotice(undefined);
    try {
      const { data } = await apiRequest<TrackedRepository>(base, { method: "PUT" });
      onChange(data);
    } catch (err) {
      setNotice({ kind: "error", text: `Could not track ${owner}/${repo}: ${errorMessage(err)}` });
      setBusy(null);
      return;
    }
    await sync();
  }

  async function untrack() {
    if (!window.confirm(`Stop tracking ${owner}/${repo}? Its cached issue statistics will be removed.`)) return;
    setBusy("untrack");
    setNotice(undefined);
    try {
      await apiRequest<void>(base, { method: "DELETE" });
      onChange(null);
      setNotice({ kind: "success", text: "Repository untracked." });
    } catch (err) {
      setNotice({ kind: "error", text: `Could not untrack: ${errorMessage(err)}` });
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="flex flex-col items-start gap-2 sm:items-end">
      <div className="flex flex-wrap gap-2">
        {tracked ? (
          <>
            <button className="btn-secondary" onClick={sync} disabled={busy !== null} title="Refresh issue statistics from GitHub">
              <FaSync className={busy === "sync" ? "animate-spin" : ""} aria-hidden />
              {busy === "sync" ? "Syncing…" : "Sync"}
            </button>
            <button className="btn-danger" onClick={untrack} disabled={busy !== null}>
              {busy === "untrack" ? "Untracking…" : "Untrack"}
            </button>
          </>
        ) : (
          <button className="btn-primary" onClick={track} disabled={busy !== null}>
            {busy === "track" ? "Tracking…" : "Track"}
          </button>
        )}
      </div>
      {!compact && tracked ? (
        <span className="text-xs text-gray-500">Last synced {relativeTime(tracked.last_synced_at)}</span>
      ) : null}
      {notice ? (
        <div className="max-w-md">
          <Notice kind={notice.kind} onDismiss={() => setNotice(undefined)}>
            {notice.text}
          </Notice>
        </div>
      ) : null}
    </div>
  );
}
