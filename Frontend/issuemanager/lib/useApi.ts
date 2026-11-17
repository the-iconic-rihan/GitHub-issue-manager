import { useCallback, useEffect, useState } from "react";

import { ApiError, apiRequest } from "./api-client";
import type { PageMeta } from "./types";

export interface ApiState<T> {
  data?: T;
  meta?: PageMeta;
  error?: ApiError | Error;
  loading: boolean;
  reload: () => void;
  /** Replace local data after a successful mutation (no refetch needed). */
  setData: (update: (current: T | undefined) => T | undefined) => void;
}

interface Result<T> {
  /** Request that produced this result (`path#nonce`). */
  key: string | null;
  path: string | null;
  data?: T;
  meta?: PageMeta;
  error?: ApiError | Error;
}

/**
 * Fetches `path` from the backend whenever it changes. Pass `null` to skip fetching.
 *
 * While a reload or a new page of the *same* resource is loading, the previous data stays
 * visible; data for a *different* path is never shown (so a detail page can't act on
 * the previous record). With `keepPrevious`, data from any earlier path stays visible
 * while loading (smooth pagination/filtering of lists).
 */
export function useApi<T>(path: string | null, { keepPrevious = false } = {}): ApiState<T> {
  const [nonce, setNonce] = useState(0);
  const [result, setResult] = useState<Result<T>>({ key: null, path: null });
  const key = path === null ? null : `${path}#${nonce}`;

  useEffect(() => {
    if (path === null || key === null) return;
    const controller = new AbortController();
    apiRequest<T>(path, { signal: controller.signal })
      .then((r) => setResult({ key, path, data: r.data, meta: r.meta }))
      .catch((err: Error) => {
        if (err.name === "AbortError") return;
        setResult({ key, path, error: err });
      });
    return () => controller.abort();
  }, [key, path]);

  const reload = useCallback(() => setNonce((n) => n + 1), []);
  const setData = useCallback(
    (update: (current: T | undefined) => T | undefined) => setResult((r) => ({ ...r, data: update(r.data) })),
    [],
  );

  const current = result.key === key;
  const visible = current || keepPrevious || result.path === path;
  return {
    data: visible ? result.data : undefined,
    meta: visible ? result.meta : undefined,
    error: current ? result.error : undefined,
    loading: key !== null && !current,
    reload,
    setData,
  };
}
