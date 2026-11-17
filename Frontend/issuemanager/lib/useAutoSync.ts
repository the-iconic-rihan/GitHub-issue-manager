import { useEffect, useRef, useState } from "react";

import { apiRequest, errorMessage } from "./api-client";
import type { StaleSyncReport } from "./types";

/** How often the dashboard asks the server to refresh stale repositories while visible. */
export const AUTO_SYNC_INTERVAL_MS = 5 * 60 * 1000;
/** Upper bound on back-to-back runs when the server reports more stale repositories. */
const MAX_CONSECUTIVE_RUNS = 5;

export interface AutoSyncState {
  refreshing: boolean;
  /** Repositories that could not be refreshed in the last run. */
  failures: StaleSyncReport["failed"];
  error?: string;
}

/**
 * Keeps tracked repositories fresh while the page is open: asks the server to conditionally
 * re-sync stale snapshots (GitHub answers 304 for unchanged repositories, which costs no
 * rate limit) on mount and every few minutes while the tab is visible, and calls `onUpdated`
 * when any repository was refreshed.
 */
export function useAutoSync(onUpdated: () => void, enabled = true): AutoSyncState {
  const [state, setState] = useState<AutoSyncState>({ refreshing: false, failures: [] });
  const onUpdatedRef = useRef(onUpdated);
  useEffect(() => {
    onUpdatedRef.current = onUpdated;
  }, [onUpdated]);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    let running = false;

    async function run() {
      if (running || cancelled || document.visibilityState === "hidden") return;
      running = true;
      setState((s) => ({ ...s, refreshing: true }));
      let changed = false;
      let failures: StaleSyncReport["failed"] = [];
      let error: string | undefined;
      try {
        for (let i = 0; i < MAX_CONSECUTIVE_RUNS && !cancelled; i++) {
          const { data } = await apiRequest<StaleSyncReport>("tracked-repos/sync-stale", { method: "POST" });
          // Unchanged repositories still get a new "last synced" time worth showing.
          changed ||= data.updated.length > 0 || data.unchanged.length > 0;
          failures = data.failed;
          const rateLimited = data.failed.some((f) => f.code === "rate_limited");
          if (!data.more_pending || rateLimited) break;
        }
      } catch (err) {
        error = errorMessage(err);
      } finally {
        running = false;
      }
      if (cancelled) return;
      setState({ refreshing: false, failures, error });
      if (changed) onUpdatedRef.current();
    }

    const first = setTimeout(run, 0);
    const interval = setInterval(run, AUTO_SYNC_INTERVAL_MS);
    const onVisible = () => {
      if (document.visibilityState === "visible") void run();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      cancelled = true;
      clearTimeout(first);
      clearInterval(interval);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [enabled]);

  return state;
}
