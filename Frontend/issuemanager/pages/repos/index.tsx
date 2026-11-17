import Link from "next/link";
import { useRouter } from "next/router";
import { useState } from "react";

import Layout from "@/components/Layout";
import Pagination from "@/components/Pagination";
import TrackControls from "@/components/TrackControls";
import { EmptyState, ErrorMessage, LoadingBlock, PageHeader } from "@/components/ui";
import { query } from "@/lib/api-client";
import { relativeTime } from "@/lib/format";
import type { RepoListItem } from "@/lib/types";
import { useApi } from "@/lib/useApi";

const PER_PAGE = 30;

export default function Repositories() {
  const router = useRouter();
  const page = Math.max(1, Number(router.query.page) || 1);
  const { data, meta, error, loading, reload, setData } = useApi<RepoListItem[]>(
    router.isReady ? `repos${query({ page, per_page: PER_PAGE })}` : null,
    { keepPrevious: true },
  );
  const [filter, setFilter] = useState("");

  const repos = (data ?? []).filter((r) => r.full_name.toLowerCase().includes(filter.trim().toLowerCase()));
  const goTo = (p: number) => void router.push({ pathname: "/repos", query: p > 1 ? { page: p } : {} });

  return (
    <Layout title="Repositories">
      <PageHeader title="Repositories" subtitle="Repositories you can access on GitHub, most recently updated first. Track a repository to include it in the dashboard." />

      <div className="mb-4">
        <label htmlFor="repo-filter" className="sr-only">
          Filter repositories on this page
        </label>
        <input
          id="repo-filter"
          className="input md:max-w-sm"
          type="search"
          placeholder="Filter this page by name…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>

      {error ? <ErrorMessage error={error} onRetry={reload} /> : null}
      {loading && !data ? <LoadingBlock label="Loading repositories" /> : null}
      {data && data.length === 0 ? <EmptyState title="No repositories found">Your GitHub account has no accessible repositories.</EmptyState> : null}
      {data && data.length > 0 && repos.length === 0 ? <EmptyState title="No repositories on this page match your filter" /> : null}

      <ul className="space-y-3">
        {repos.map((r) => (
          <li key={r.id} className="card flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <Link
                href={`/repos/${encodeURIComponent(r.owner.login)}/${encodeURIComponent(r.name)}`}
                className="break-words font-semibold text-sky-700 hover:underline"
              >
                {r.full_name}
              </Link>
              {r.private ? <span className="ml-2 rounded bg-gray-100 px-1.5 text-xs text-gray-600">private</span> : null}
              {r.archived ? <span className="ml-2 rounded bg-yellow-100 px-1.5 text-xs text-yellow-800">archived</span> : null}
              {r.description ? <p className="mt-1 break-words text-sm text-gray-600">{r.description}</p> : null}
              <p className="mt-1 text-xs text-gray-500">
                {r.has_issues ? `${r.open_issues_count} open issues & PRs` : "Issues disabled"} · updated {relativeTime(r.updated_at)}
              </p>
            </div>
            {r.has_issues ? (
              <TrackControls
                owner={r.owner.login}
                repo={r.name}
                compact
                tracked={
                  r.tracked
                    ? { repo_id: r.id, full_name: r.full_name, private: r.private, created_at: "", last_synced_at: null }
                    : null
                }
                onChange={(t) => setData((list) => list?.map((x) => (x.id === r.id ? { ...x, tracked: t !== null } : x)))}
              />
            ) : null}
          </li>
        ))}
      </ul>
      <Pagination meta={meta} onPage={goTo} disabled={loading} />
    </Layout>
  );
}
