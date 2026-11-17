import Link from "next/link";
import { useRouter } from "next/router";
import { FaRegComment } from "react-icons/fa";

import IssueFilters, { filtersFromQuery, filtersToParams, type IssueFilterValues } from "@/components/IssueFilters";
import Layout from "@/components/Layout";
import { useMe } from "@/components/MeContext";
import Pagination from "@/components/Pagination";
import TrackControls from "@/components/TrackControls";
import { Avatar, EmptyState, ErrorMessage, LabelBadge, LoadingBlock, PageHeader, StateBadge } from "@/components/ui";
import { apiPath, query } from "@/lib/api-client";
import { relativeTime } from "@/lib/format";
import type { Issue, Label, RepoDetail, SimpleUser } from "@/lib/types";
import { useApi } from "@/lib/useApi";

const PER_PAGE = 25;

export default function RepositoryIssues() {
  const router = useRouter();
  const { me } = useMe();
  const owner = typeof router.query.owner === "string" ? router.query.owner : "";
  const repo = typeof router.query.repo === "string" ? router.query.repo : "";
  const ready = router.isReady && owner !== "" && repo !== "";
  const base = `repos/${apiPath(owner, repo)}`;

  const filters = filtersFromQuery(router.query);
  const page = Math.max(1, Number(router.query.page) || 1);

  const repoState = useApi<RepoDetail>(ready ? base : null);
  const labels = useApi<Label[]>(ready ? `${base}/labels?per_page=100` : null);
  const assignees = useApi<SimpleUser[]>(ready ? `${base}/assignees?per_page=100` : null);
  const issues = useApi<Issue[]>(
    ready ? `${base}/issues${query({ ...filtersToParams(filters), page, per_page: PER_PAGE })}` : null,
    { keepPrevious: true },
  );

  const updateQuery = (next: IssueFilterValues, nextPage = 1) => {
    const q: Record<string, string> = { owner, repo };
    if (next.q.trim()) q.q = next.q.trim();
    if (next.state !== "open") q.state = next.state;
    if (next.label) q.label = next.label;
    if (next.assignee) q.assignee = next.assignee;
    if (next.sort !== "created-desc") q.sort = next.sort;
    if (nextPage > 1) q.page = String(nextPage);
    void router.replace({ pathname: router.pathname, query: q }, undefined, { shallow: true, scroll: false });
  };

  const repoInfo = repoState.data;
  const issuesHref = (n: number) => `/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/issues/${n}`;

  return (
    <Layout title={`${owner}/${repo} issues`}>
      <PageHeader
        title={
          <>
            <span className="text-gray-500">{owner}/</span>
            {repo}
          </>
        }
        subtitle={
          repoInfo ? (
            <a href={repoInfo.html_url} target="_blank" rel="noopener noreferrer" className="hover:underline">
              View on GitHub ↗
            </a>
          ) : null
        }
        actions={
          <>
            {repoInfo ? (
              <TrackControls
                owner={owner}
                repo={repo}
                tracked={repoInfo.tracked}
                onChange={(t) => repoState.setData((r) => (r ? { ...r, tracked: t } : r))}
              />
            ) : null}
            {repoInfo?.has_issues !== false ? (
              <Link href={`/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/issues/new`} className="btn-primary self-start">
                New issue
              </Link>
            ) : null}
          </>
        }
      />

      {repoState.error ? <ErrorMessage error={repoState.error} onRetry={repoState.reload} /> : null}

      <IssueFilters
        value={filters}
        onChange={(next) => updateQuery(next)}
        labels={labels.data ?? []}
        assignees={assignees.data ?? []}
        myLogin={me?.user.login}
      />

      {issues.error ? <ErrorMessage error={issues.error} onRetry={issues.reload} /> : null}
      {issues.loading && !issues.data ? <LoadingBlock label="Loading issues" /> : null}
      {issues.meta?.total_count !== undefined ? (
        <p className="mb-2 text-sm text-gray-500">{issues.meta.total_count.toLocaleString()} matching issues</p>
      ) : null}

      {issues.data && issues.data.length === 0 && !issues.loading ? (
        <EmptyState title="No issues match these filters">
          <button className="btn-secondary mt-3" onClick={() => updateQuery({ q: "", state: "all", label: "", assignee: "", sort: "created-desc" })}>
            Show all issues
          </button>
        </EmptyState>
      ) : null}

      {issues.data && issues.data.length > 0 ? (
        <ul className={`divide-y rounded-2xl bg-white shadow ${issues.loading ? "opacity-60" : ""}`} aria-busy={issues.loading}>
          {issues.data.map((issue) => (
            <li key={issue.id} className="flex flex-col gap-2 p-4 sm:flex-row sm:items-start sm:justify-between">
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <StateBadge state={issue.state} reason={issue.state_reason} />
                  <Link href={issuesHref(issue.number)} className="break-words font-semibold text-gray-900 hover:text-sky-700 hover:underline">
                    {issue.title}
                  </Link>
                  {issue.labels.map((l) => (
                    <LabelBadge key={l.id} name={l.name} color={l.color} />
                  ))}
                </div>
                <p className="mt-1 text-xs text-gray-500">
                  #{issue.number} opened {relativeTime(issue.created_at)} by {issue.user?.login ?? "ghost"} · updated {relativeTime(issue.updated_at)}
                </p>
              </div>
              <div className="flex items-center gap-3 text-sm text-gray-500">
                <span className="flex -space-x-1">
                  {issue.assignees.map((a) => (
                    <Avatar key={a.login} src={a.avatar_url} alt={`Assigned to ${a.login}`} size={22} />
                  ))}
                </span>
                {issue.comments > 0 ? (
                  <span className="inline-flex items-center gap-1" aria-label={`${issue.comments} comments`}>
                    <FaRegComment aria-hidden /> {issue.comments}
                  </span>
                ) : null}
              </div>
            </li>
          ))}
        </ul>
      ) : null}

      <Pagination meta={issues.meta} onPage={(p) => updateQuery(filters, p)} disabled={issues.loading} />
    </Layout>
  );
}
