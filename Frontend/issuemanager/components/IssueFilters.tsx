import { useState, type FormEvent } from "react";

import type { Label, SimpleUser } from "@/lib/types";

export interface IssueFilterValues {
  q: string;
  state: "open" | "closed" | "all";
  label: string;
  assignee: string;
  sort: string;
}

export const DEFAULT_FILTERS: IssueFilterValues = { q: "", state: "open", label: "", assignee: "", sort: "created-desc" };

export const SORT_OPTIONS: { value: string; label: string }[] = [
  { value: "created-desc", label: "Newest" },
  { value: "created-asc", label: "Oldest" },
  { value: "updated-desc", label: "Recently updated" },
  { value: "updated-asc", label: "Least recently updated" },
  { value: "comments-desc", label: "Most commented" },
];

/** Converts filter values into backend query parameters. */
export function filtersToParams(f: IssueFilterValues): Record<string, string | undefined> {
  const [sort, direction] = (SORT_OPTIONS.some((o) => o.value === f.sort) ? f.sort : DEFAULT_FILTERS.sort).split("-");
  return {
    q: f.q.trim() || undefined,
    state: f.state,
    labels: f.label || undefined,
    assignee: f.assignee || undefined,
    sort,
    direction,
  };
}

/** Reads filter values from a URL query object, falling back to defaults for bad values. */
export function filtersFromQuery(query: Record<string, string | string[] | undefined>): IssueFilterValues {
  const one = (key: string) => (typeof query[key] === "string" ? (query[key] as string) : "");
  const state = one("state");
  const sort = one("sort");
  return {
    q: one("q"),
    state: state === "closed" || state === "all" ? state : "open",
    label: one("label"),
    assignee: one("assignee"),
    sort: SORT_OPTIONS.some((o) => o.value === sort) ? sort : DEFAULT_FILTERS.sort,
  };
}

export default function IssueFilters({
  value,
  onChange,
  labels,
  assignees,
  myLogin,
}: {
  value: IssueFilterValues;
  onChange: (next: IssueFilterValues) => void;
  labels: Label[];
  assignees: SimpleUser[];
  myLogin?: string;
}) {
  const [search, setSearch] = useState(value.q);
  // Keep the input in sync when the URL changes (back button, "Clear filters").
  const [syncedQ, setSyncedQ] = useState(value.q);
  if (syncedQ !== value.q) {
    setSyncedQ(value.q);
    setSearch(value.q);
  }

  const submitSearch = (e: FormEvent) => {
    e.preventDefault();
    onChange({ ...value, q: search });
  };
  const isDefault = JSON.stringify(value) === JSON.stringify(DEFAULT_FILTERS);
  const otherAssignees = assignees.filter((a) => a.login !== myLogin);

  return (
    <div className="card mb-4 space-y-3">
      <form onSubmit={submitSearch} className="flex gap-2" role="search">
        <label htmlFor="issue-search" className="sr-only">
          Search issues
        </label>
        <input
          id="issue-search"
          className="input"
          type="search"
          placeholder="Search issues by text…"
          value={search}
          maxLength={256}
          onChange={(e) => setSearch(e.target.value)}
        />
        <button className="btn-primary" type="submit">
          Search
        </button>
      </form>

      <div className="flex flex-col gap-3 md:flex-row md:flex-wrap md:items-end">
        <div className="inline-flex overflow-hidden rounded-xl border-2 border-gray-200" role="group" aria-label="Issue state">
          {(["open", "closed", "all"] as const).map((state) => (
            <button
              key={state}
              type="button"
              aria-pressed={value.state === state}
              onClick={() => onChange({ ...value, state })}
              className={`px-4 py-2 text-sm font-medium capitalize ${value.state === state ? "bg-sky-400 text-black" : "bg-white text-gray-700 hover:bg-gray-50"}`}
            >
              {state}
            </button>
          ))}
        </div>

        <label className="flex flex-col text-xs font-medium text-gray-600">
          Label
          <select className="input mt-1 md:w-48" value={value.label} onChange={(e) => onChange({ ...value, label: e.target.value })}>
            <option value="">Any label</option>
            {labels.map((l) => (
              <option key={l.id} value={l.name}>
                {l.name}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col text-xs font-medium text-gray-600">
          Assignee
          <select className="input mt-1 md:w-48" value={value.assignee} onChange={(e) => onChange({ ...value, assignee: e.target.value })}>
            <option value="">Anyone</option>
            <option value="none">Unassigned</option>
            <option value="*">Assigned to anyone</option>
            {myLogin ? <option value={myLogin}>Assigned to me</option> : null}
            {otherAssignees.map((a) => (
              <option key={a.id} value={a.login}>
                {a.login}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col text-xs font-medium text-gray-600">
          Sort
          <select className="input mt-1 md:w-52" value={value.sort} onChange={(e) => onChange({ ...value, sort: e.target.value })}>
            {SORT_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </label>

        {!isDefault ? (
          <button type="button" className="btn-secondary" onClick={() => onChange(DEFAULT_FILTERS)}>
            Clear filters
          </button>
        ) : null}
      </div>
    </div>
  );
}
