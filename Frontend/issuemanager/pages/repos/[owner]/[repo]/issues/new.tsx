import Link from "next/link";
import { useRouter } from "next/router";
import { useState } from "react";

import IssueForm from "@/components/IssueForm";
import Layout from "@/components/Layout";
import { ErrorMessage, LoadingBlock, PageHeader } from "@/components/ui";
import { ApiError, apiPath, apiRequest } from "@/lib/api-client";
import type { Issue, Label, SimpleUser } from "@/lib/types";
import { useApi } from "@/lib/useApi";
import type { FormErrors, IssueFormValues } from "@/lib/validation";

export default function NewIssue() {
  const router = useRouter();
  const owner = typeof router.query.owner === "string" ? router.query.owner : "";
  const repo = typeof router.query.repo === "string" ? router.query.repo : "";
  const ready = router.isReady && owner !== "" && repo !== "";
  const base = `repos/${apiPath(owner, repo)}`;

  const labels = useApi<Label[]>(ready ? `${base}/labels?per_page=100` : null);
  const assignees = useApi<SimpleUser[]>(ready ? `${base}/assignees?per_page=100` : null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<unknown>();
  const [fieldErrors, setFieldErrors] = useState<FormErrors>();

  async function create(values: IssueFormValues) {
    setSubmitting(true);
    setError(undefined);
    setFieldErrors(undefined);
    try {
      const { data } = await apiRequest<Issue>(`${base}/issues`, {
        method: "POST",
        body: { title: values.title, body: values.body || undefined, labels: values.labels, assignees: values.assignees },
      });
      await router.push(`/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/issues/${data.number}?created=1`);
    } catch (err) {
      if (err instanceof ApiError && err.details.length > 0) {
        setFieldErrors(Object.fromEntries(err.details.map((d) => [d.field, d.message])) as FormErrors);
      }
      setError(err);
      setSubmitting(false);
    }
  }

  const loadingOptions = labels.loading || assignees.loading;

  return (
    <Layout title={`New issue · ${owner}/${repo}`}>
      <PageHeader
        title="New issue"
        subtitle={
          <Link href={`/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`} className="hover:underline">
            ← {owner}/{repo}
          </Link>
        }
      />
      <div className="card max-w-3xl">
        {error ? (
          <div className="mb-4">
            <ErrorMessage error={error} />
          </div>
        ) : null}
        {labels.error || assignees.error ? (
          <p className="mb-4 text-sm text-gray-500">Labels or assignees could not be loaded; you can still create the issue.</p>
        ) : null}
        {loadingOptions ? (
          <LoadingBlock label="Loading form" />
        ) : (
          <IssueForm
            labels={labels.data ?? []}
            assignees={assignees.data ?? []}
            onSubmit={create}
            submitting={submitting}
            serverErrors={fieldErrors}
          />
        )}
      </div>
    </Layout>
  );
}
