import Link from "next/link";
import { useRouter } from "next/router";
import { useCallback, useState } from "react";

import Comments from "@/components/Comments";
import { AssigneeEditor, BodyEditor, LabelEditor, StateControls, TitleEditor } from "@/components/IssueEditors";
import Layout from "@/components/Layout";
import { useMe } from "@/components/MeContext";
import { ErrorMessage, LoadingBlock, Notice, StateBadge } from "@/components/ui";
import { apiPath, apiRequest } from "@/lib/api-client";
import { relativeTime } from "@/lib/format";
import type { Issue, Label, RepoDetail, SimpleUser } from "@/lib/types";
import { useApi } from "@/lib/useApi";

export default function IssueDetail() {
  const router = useRouter();
  const { me } = useMe();
  const owner = typeof router.query.owner === "string" ? router.query.owner : "";
  const repo = typeof router.query.repo === "string" ? router.query.repo : "";
  const number = typeof router.query.number === "string" && /^\d+$/.test(router.query.number) ? router.query.number : "";
  const ready = router.isReady && owner !== "" && repo !== "" && number !== "";
  const repoBase = `repos/${apiPath(owner, repo)}`;
  const issueBase = `${repoBase}/issues/${apiPath(number)}`;

  const issueState = useApi<Issue>(ready ? issueBase : null);
  const repoState = useApi<RepoDetail>(ready ? repoBase : null);
  const labels = useApi<Label[]>(ready ? `${repoBase}/labels?per_page=100` : null);
  const assignees = useApi<SimpleUser[]>(ready ? `${repoBase}/assignees?per_page=100` : null);
  const [saved, setSaved] = useState<string>();
  const justCreated = router.isReady && router.query.created === "1";
  const notice = saved ?? (justCreated ? "Issue created." : undefined);
  const dismissNotice = () => {
    setSaved(undefined);
    if (justCreated) void router.replace(`/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/issues/${number}`, undefined, { shallow: true });
  };

  const { setData } = issueState;
  const patch = useCallback(
    async (body: Record<string, unknown>) => {
      const { data } = await apiRequest<Issue>(issueBase, { method: "PATCH", body });
      setData(() => data);
      setSaved("Changes saved to GitHub.");
      return data;
    },
    [issueBase, setData],
  );
  const addLabel = async (name: string) =>
    (await apiRequest<Label[]>(`${issueBase}/labels`, { method: "POST", body: { labels: [name] } })).data;
  const removeLabel = async (name: string) =>
    (await apiRequest<Label[]>(`${issueBase}/labels?name=${encodeURIComponent(name)}`, { method: "DELETE" })).data;

  const issue = issueState.data;
  const perms = repoState.data?.permissions;
  const canTriage = Boolean(perms && (perms.admin || perms.maintain || perms.push || perms.triage));
  const isAuthor = Boolean(issue?.user && me && issue.user.login === me.user.login);
  // GitHub enforces permissions; these flags only hide controls that would certainly fail.
  const canEditText = canTriage || isAuthor;

  return (
    <Layout title={issue ? `${issue.title} · ${owner}/${repo}#${number}` : `${owner}/${repo}#${number}`}>
      <div className="mb-4 text-sm">
        <Link href={`/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`} className="text-sky-700 hover:underline">
          ← {owner}/{repo} issues
        </Link>
      </div>

      {issueState.error ? <ErrorMessage error={issueState.error} onRetry={issueState.reload} /> : null}
      {issueState.loading && !issue ? <LoadingBlock label="Loading issue" /> : null}

      {issue ? (
        <div className="space-y-4">
          <TitleEditor key={`title-${issue.updated_at}`} issue={issue} patch={patch} canEdit={canEditText} />
          <div className="flex flex-wrap items-center gap-3 text-sm text-gray-600">
            <StateBadge state={issue.state} reason={issue.state_reason} />
            <span>
              opened {relativeTime(issue.created_at)} by {issue.user?.login ?? "ghost"} · {issue.comments} comment{issue.comments === 1 ? "" : "s"}
            </span>
            <a href={issue.html_url} target="_blank" rel="noopener noreferrer" className="text-sky-700 hover:underline">
              View on GitHub ↗
            </a>
          </div>
          {notice ? (
            <Notice kind="success" onDismiss={dismissNotice}>
              {notice}
            </Notice>
          ) : null}

          <div className="grid gap-6 lg:grid-cols-[1fr_18rem]">
            <div className="min-w-0 space-y-6">
              <BodyEditor key={`body-${issue.updated_at}`} issue={issue} patch={patch} canEdit={canEditText} />
              <StateControls issue={issue} patch={patch} canEdit={canEditText} />
              <Comments
                key={issueBase}
                basePath={issueBase}
                locked={issue.locked}
                onCommentAdded={() => setData((i) => (i ? { ...i, comments: i.comments + 1 } : i))}
              />
            </div>
            <aside className="space-y-4">
              <AssigneeEditor issue={issue} candidates={assignees.data ?? []} patch={patch} canEdit={canTriage} />
              <LabelEditor
                issue={issue}
                repoLabels={labels.data ?? []}
                onLabels={(next) => {
                  setData((i) => (i ? { ...i, labels: next } : i));
                  setSaved("Labels updated on GitHub.");
                }}
                addLabel={addLabel}
                removeLabel={removeLabel}
                canEdit={canTriage}
              />
              {!canTriage && repoState.data ? (
                <p className="text-xs text-gray-500">You need triage or write access to change labels and assignees.</p>
              ) : null}
            </aside>
          </div>
        </div>
      ) : null}
    </Layout>
  );
}
