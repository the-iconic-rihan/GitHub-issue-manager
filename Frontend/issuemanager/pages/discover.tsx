import { useRouter } from "next/router";
import { useState, type FormEvent } from "react";
import { FaCodeBranch, FaStar } from "react-icons/fa";

import Layout from "@/components/Layout";
import { useMe } from "@/components/MeContext";
import Pagination from "@/components/Pagination";
import { EmptyState, ErrorMessage, LabelBadge, LoadingBlock, PageHeader } from "@/components/ui";
import { query } from "@/lib/api-client";
import { relativeTime } from "@/lib/format";
import type { DiscoverIssue, LanguageCount } from "@/lib/types";
import { useApi } from "@/lib/useApi";

const PER_PAGE = 30;

export default function Discover() {
  const router = useRouter();
  const { me } = useMe();
  const language = typeof router.query.language === "string" ? router.query.language : "";
  const label = typeof router.query.label === "string" ? router.query.label : "";
  const page = Math.max(1, Number(router.query.page) || 1);
  const [labelInput, setLabelInput] = useState(label);
  const [syncedLabel, setSyncedLabel] = useState(label);
  if (syncedLabel !== label) {
    setSyncedLabel(label);
    setLabelInput(label);
  }

  const languages = useApi<LanguageCount[]>("discover/languages");
  const issues = useApi<DiscoverIssue[]>(
    router.isReady ? `discover/issues${query({ language, label, page, per_page: PER_PAGE })}` : null,
    { keepPrevious: true },
  );

  const update = (next: { language?: string; label?: string; page?: number }) => {
    const q: Record<string, string> = {};
    const lang = next.language ?? language;
    const lab = next.label ?? label;
    if (lang) q.language = lang;
    if (lab) q.label = lab;
    if (next.page && next.page > 1) q.page = String(next.page);
    void router.replace({ pathname: "/discover", query: q }, undefined, { shallow: true });
  };

  return (
    <Layout title="Discover">
      <PageHeader
        title="Discover"
        subtitle="Beginner-friendly open issues (good first issue, help wanted, …) collected from across GitHub."
      />
      {me && !me.discover_enabled ? (
        <div className="mb-4">
          <EmptyState title="Discover is not enabled on this server">
            An administrator can enable it by setting GITHUB_ACCESS_TOKEN for the API server.
          </EmptyState>
        </div>
      ) : null}

      <div className="card mb-4 flex flex-col gap-3 md:flex-row md:items-end">
        <label className="flex flex-col text-xs font-medium text-gray-600">
          Language
          <select className="input mt-1 md:w-56" value={language} onChange={(e) => update({ language: e.target.value, page: 1 })}>
            <option value="">All languages</option>
            {(languages.data ?? []).map((l) => (
              <option key={l.language} value={l.language}>
                {l.language} ({l.count})
              </option>
            ))}
          </select>
        </label>
        <form
          className="flex flex-1 gap-2"
          onSubmit={(e: FormEvent) => {
            e.preventDefault();
            update({ label: labelInput.trim(), page: 1 });
          }}
        >
          <label className="flex flex-1 flex-col text-xs font-medium text-gray-600">
            Label
            <input className="input mt-1" value={labelInput} maxLength={100} placeholder="e.g. good first issue" onChange={(e) => setLabelInput(e.target.value)} />
          </label>
          <button className="btn-primary self-end" type="submit">
            Apply
          </button>
        </form>
      </div>

      {issues.error ? <ErrorMessage error={issues.error} onRetry={issues.reload} /> : null}
      {issues.loading && !issues.data ? <LoadingBlock label="Loading issues" /> : null}
      {issues.data && issues.data.length === 0 ? (
        <EmptyState title="No issues found">Try another language or label, or wait for the next collection run.</EmptyState>
      ) : null}

      <ul className="space-y-3">
        {(issues.data ?? []).map((i) => (
          <li key={i.id} className="card">
            <a href={i.url} target="_blank" rel="noopener noreferrer" className="break-words font-semibold text-gray-900 hover:text-sky-700 hover:underline">
              {i.title}
            </a>
            <div className="mt-1 flex flex-wrap items-center gap-3 text-xs text-gray-500">
              <span className="font-medium text-gray-700">{i.repository_name}</span>
              {i.primary_language ? <span>{i.primary_language}</span> : null}
              <span className="inline-flex items-center gap-1">
                <FaStar aria-hidden /> {i.star_count.toLocaleString()}
              </span>
              <span className="inline-flex items-center gap-1">
                <FaCodeBranch aria-hidden /> {i.fork_count.toLocaleString()}
              </span>
              <span>opened {relativeTime(i.created_at)}</span>
            </div>
            <div className="mt-2 flex flex-wrap gap-1">
              {i.labels.map((l) => (
                <LabelBadge key={l} name={l} />
              ))}
            </div>
          </li>
        ))}
      </ul>
      <Pagination meta={issues.meta} onPage={(p) => update({ page: p })} disabled={issues.loading} />
    </Layout>
  );
}
