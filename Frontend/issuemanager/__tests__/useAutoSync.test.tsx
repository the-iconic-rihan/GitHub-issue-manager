import { act, renderHook, waitFor } from "@testing-library/react";

import { AUTO_SYNC_INTERVAL_MS, useAutoSync } from "@/lib/useAutoSync";

type Report = { updated: string[]; unchanged: string[]; failed: { full_name: string; code: string; message: string }[]; more_pending: boolean };

const originalFetch = global.fetch;
let reports: Report[];
let calls: string[];

beforeEach(() => {
  calls = [];
  reports = [];
  global.fetch = jest.fn(async (url: string, init?: RequestInit) => {
    calls.push(`${init?.method ?? "GET"} ${url}`);
    const report = reports.shift() ?? { updated: [], unchanged: [], failed: [], more_pending: false };
    return { status: 200, ok: true, headers: { get: () => null }, json: async () => ({ data: report }) };
  }) as unknown as typeof fetch;
  Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
});

afterEach(() => {
  global.fetch = originalFetch;
  jest.useRealTimers();
});

const empty = { updated: [], unchanged: [], failed: [], more_pending: false };

it("refreshes on mount and reloads the dashboard when something was refreshed", async () => {
  reports = [{ ...empty, updated: ["acme/app"] }];
  const onUpdated = jest.fn();
  const { result } = renderHook(() => useAutoSync(onUpdated));
  await waitFor(() => expect(onUpdated).toHaveBeenCalledTimes(1));
  expect(calls).toEqual(["POST /api/backend/tracked-repos/sync-stale"]);
  expect(result.current.refreshing).toBe(false);
});

it("does nothing when disabled and does not reload when nothing was stale", async () => {
  const onUpdated = jest.fn();
  renderHook(() => useAutoSync(onUpdated, false));
  await act(async () => new Promise((r) => setTimeout(r, 20)));
  expect(calls).toEqual([]);

  renderHook(() => useAutoSync(onUpdated));
  await waitFor(() => expect(calls).toHaveLength(1));
  expect(onUpdated).not.toHaveBeenCalled();
});

it("keeps going while more repositories are pending and reports failures", async () => {
  reports = [
    { ...empty, unchanged: ["a/1"], more_pending: true },
    { ...empty, failed: [{ full_name: "a/2", code: "github_unavailable", message: "down" }], more_pending: false },
  ];
  const onUpdated = jest.fn();
  const { result } = renderHook(() => useAutoSync(onUpdated));
  await waitFor(() => expect(result.current.failures).toHaveLength(1));
  expect(calls).toHaveLength(2);
  expect(result.current.failures[0].full_name).toBe("a/2");
  expect(onUpdated).toHaveBeenCalledTimes(1);
});

it("stops on rate limiting even if more are pending", async () => {
  reports = [{ ...empty, failed: [{ full_name: "a/1", code: "rate_limited", message: "wait" }], more_pending: true }];
  const { result } = renderHook(() => useAutoSync(jest.fn()));
  await waitFor(() => expect(result.current.failures).toHaveLength(1));
  expect(calls).toHaveLength(1);
});

it("repeats on an interval only while the tab is visible", async () => {
  jest.useFakeTimers();
  renderHook(() => useAutoSync(jest.fn()));
  await act(async () => {
    await jest.advanceTimersByTimeAsync(0);
  });
  expect(calls).toHaveLength(1);

  Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
  await act(async () => {
    await jest.advanceTimersByTimeAsync(AUTO_SYNC_INTERVAL_MS);
  });
  expect(calls).toHaveLength(1);

  Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
  await act(async () => {
    document.dispatchEvent(new Event("visibilitychange"));
    await jest.advanceTimersByTimeAsync(0);
  });
  expect(calls).toHaveLength(2);
});
