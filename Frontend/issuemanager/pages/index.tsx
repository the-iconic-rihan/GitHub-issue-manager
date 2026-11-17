import Link from "next/link";

import Layout from "@/components/Layout";
import TrackControls from "@/components/TrackControls";
import { EmptyState, ErrorMessage, LabelBadge, LoadingBlock, PageHeader, StateBadge } from "@/components/ui";
import { relativeTime, splitFullName } from "@/lib/format";
import type { Dashboard as DashboardData } from "@/lib/types";
import { useApi } from "@/lib/useApi";

function Stat({ label, value, hint }: { label: string; value: number; hint?: string }) {
  return (
    <div className="card">
      <div className="text-xs font-medium uppercase tracking-wide text-gray-500">{label}</div>
      <div className="mt-1 text-3xl font-bold text-gray-900">{value.toLocaleString()}</div>
      {hint ? <div className="mt-1 text-xs text-gray-400">{hint}</div> : null}
    </div>
  );
}

export default function Dashboard() {
  const { data, error, loading, reload } = useApi<DashboardData>("dashboard");

  return (
    <Layout title="Dashboard">
      <PageHeader
        title="Dashboard"
        subtitle="Statistics for your tracked repositories, from their most recent sync."
        actions={
          <Link href="/repos" className="btn-primary">
            Track repositories
          </Link>
        }
      />

      {error ? <ErrorMessage error={error} onRetry={reload} /> : null}
      {loading && !data ? <LoadingBlock label="Loading dashboard" /> : null}

      {data && data.totals.tracked_repositories === 0 ? (
        <EmptyState title="You are not tracking any repositories yet">
          <p>Track a repository to see its issue statistics here.</p>
          <Link href="/repos" className="btn-primary mt-4">
            Choose repositories
          </Link>
        </EmptyState>
      ) : null}

      {data && data.totals.tracked_repositories > 0 ? (
        <div className="space-y-6">
          <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
            <Stat label="Open issues" value={data.totals.open_issues} />
            <Stat label="Closed issues" value={data.totals.closed_issues} />
            <Stat label="Assigned to me" value={data.totals.assigned_to_me_open} hint="open" />
            <Stat label="Unassigned" value={data.totals.unassigned_open} hint="open" />
            <Stat label="Opened" value={data.totals.opened_last_30_days} hint="last 30 days" />
            <Stat label="Closed" value={data.totals.closed_last_30_days} hint="last 30 days" />
            <Stat label="Tracked repositories" value={data.totals.tracked_repositories} />
          </div>

          <section className="card overflow-x-auto" aria-label="Tracked repositories">
            <h2 className="mb-3 text-lg font-semibold">Repositories</h2>
            <table className="w-full min-w-[40rem] text-left text-sm">
              <thead className="text-xs uppercase text-gray-500">
                <tr>
                  <th className="py-2">Repository</th>
                  <th className="py-2 text-right">Open</th>
                  <th className="py-2 text-right">Closed</th>
                  <th className="py-2 text-right">Unassigned</th>
                  <th className="py-2 text-right">Mine</th>
                  <th className="py-2 pl-4">Last synced</th>
                  <th className="py-2" />
                </tr>
              </thead>
              <tbody className="divide-y">
                {data.repositories.map((r) => {
                  const { owner, repo } = splitFullName(r.full_name);
                  return (
                    <tr key={r.repo_id}>
                      <td className="py-2">
                        <Link href={`/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`} className="font-medium text-sky-700 hover:underline">
                          {r.full_name}
                        </Link>
                        {r.private ? <span className="ml-2 rounded bg-gray-100 px-1.5 text-xs text-gray-600">private</span> : null}
                      </td>
                      <td className="py-2 text-right">{r.open_issues}</td>
                      <td className="py-2 text-right">{r.closed_issues}</td>
                      <td className="py-2 text-right">{r.unassigned_open}</td>
                      <td className="py-2 text-right">{r.assigned_to_me_open}</td>
                      <td className="py-2 pl-4 text-gray-500">{r.last_synced_at ? relativeTime(r.last_synced_at) : "never – sync to load"}</td>
                      <td className="py-2">
                        <TrackControls
                          owner={owner}
                          repo={repo}
                          compact
                          tracked={{ repo_id: r.repo_id, full_name: r.full_name, private: r.private, created_at: "", last_synced_at: r.last_synced_at }}
                          onChange={reload}
                        />
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </section>

          <div className="grid gap-6 md:grid-cols-2">
            <section className="card" aria-label="Top labels">
              <h2 className="mb-3 text-lg font-semibold">Top labels on open issues</h2>
              {data.top_labels.length === 0 ? (
                <p className="text-sm text-gray-500">No labelled open issues.</p>
              ) : (
                <ul className="space-y-2">
                  {data.top_labels.map((l) => (
                    <li key={l.label} className="flex items-center justify-between gap-2 text-sm">
                      <LabelBadge name={l.label} />
                      <span className="font-semibold">{l.count}</span>
                    </li>
                  ))}
                </ul>
              )}
            </section>

            <section className="card" aria-label="Recently updated issues">
              <h2 className="mb-3 text-lg font-semibold">Recently updated</h2>
              {data.recently_updated.length === 0 ? (
                <p className="text-sm text-gray-500">Sync a repository to see its issues.</p>
              ) : (
                <ul className="space-y-3">
                  {data.recently_updated.map((i) => {
                    const { owner, repo } = splitFullName(i.repo_full_name);
                    return (
                      <li key={`${i.repo_full_name}#${i.number}`} className="text-sm">
                        <div className="flex items-start gap-2">
                          <StateBadge state={i.state} />
                          <Link
                            href={`/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/issues/${i.number}`}
                            className="min-w-0 flex-1 break-words font-medium text-gray-900 hover:text-sky-700 hover:underline"
                          >
                            {i.title}
                          </Link>
                        </div>
                        <div className="mt-0.5 text-xs text-gray-500">
                          {i.repo_full_name}#{i.number} · updated {relativeTime(i.updated_at)}
                        </div>
                      </li>
                    );
                  })}
                </ul>
              )}
            </section>
          </div>
        </div>
      ) : null}
    </Layout>
  );
}
