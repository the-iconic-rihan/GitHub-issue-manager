// Inline editors on the issue detail page. Each one performs its own request, shows its own
// pending/error state, and reports the updated issue back to the page.

import { useState, type FormEvent } from "react";

import { errorMessage } from "@/lib/api-client";
import type { Issue, Label, SimpleUser } from "@/lib/types";
import { MAX_ASSIGNEES, MAX_BODY_CHARS, MAX_TITLE_CHARS, validateBody, validateTitle } from "@/lib/validation";

import { Avatar, LabelBadge } from "./ui";

export type PatchIssue = (body: Record<string, unknown>) => Promise<Issue>;

function useAction() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  async function run(action: () => Promise<unknown>): Promise<boolean> {
    setBusy(true);
    setError(undefined);
    try {
      await action();
      return true;
    } catch (err) {
      setError(errorMessage(err));
      return false;
    } finally {
      setBusy(false);
    }
  }
  return { busy, error, run, setError };
}

function InlineError({ message }: { message?: string }) {
  return message ? (
    <p role="alert" className="mt-2 text-sm text-red-700">
      {message}
    </p>
  ) : null;
}

export function TitleEditor({ issue, patch, canEdit }: { issue: Issue; patch: PatchIssue; canEdit: boolean }) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(issue.title);
  const { busy, error, run, setError } = useAction();

  const save = async (e: FormEvent) => {
    e.preventDefault();
    const invalid = validateTitle(title);
    if (invalid) return setError(invalid);
    if (await run(() => patch({ title: title.trim() }))) setEditing(false);
  };

  if (!editing) {
    return (
      <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
        <h1 className="break-words text-2xl font-bold text-gray-900">
          {issue.title} <span className="font-normal text-gray-400">#{issue.number}</span>
        </h1>
        {canEdit ? (
          <button
            className="btn-secondary shrink-0"
            onClick={() => {
              setTitle(issue.title);
              setEditing(true);
            }}
          >
            Edit title
          </button>
        ) : null}
      </div>
    );
  }
  return (
    <form onSubmit={save} className="flex flex-col gap-2 sm:flex-row">
      <label htmlFor="edit-title" className="sr-only">
        Title
      </label>
      <input id="edit-title" className="input" value={title} maxLength={MAX_TITLE_CHARS} onChange={(e) => setTitle(e.target.value)} autoFocus />
      <div className="flex gap-2">
        <button className="btn-primary" type="submit" disabled={busy}>
          {busy ? "Saving…" : "Save"}
        </button>
        <button className="btn-secondary" type="button" onClick={() => setEditing(false)} disabled={busy}>
          Cancel
        </button>
      </div>
      <InlineError message={error} />
    </form>
  );
}

export function BodyEditor({ issue, patch, canEdit }: { issue: Issue; patch: PatchIssue; canEdit: boolean }) {
  const [editing, setEditing] = useState(false);
  const [body, setBody] = useState(issue.body ?? "");
  const { busy, error, run, setError } = useAction();

  const save = async (e: FormEvent) => {
    e.preventDefault();
    const invalid = validateBody(body);
    if (invalid) return setError(invalid);
    if (await run(() => patch({ body }))) setEditing(false);
  };

  return (
    <section className="card" aria-label="Description">
      <div className="mb-3 flex items-center justify-between gap-2">
        <span className="flex items-center gap-2 text-sm text-gray-600">
          <Avatar src={issue.user?.avatar_url} alt="" size={20} />
          <strong>{issue.user?.login ?? "ghost"}</strong> opened this issue
        </span>
        {canEdit && !editing ? (
          <button
            className="btn-secondary"
            onClick={() => {
              setBody(issue.body ?? "");
              setEditing(true);
            }}
          >
            Edit
          </button>
        ) : null}
      </div>
      {editing ? (
        <form onSubmit={save} className="space-y-2">
          <label htmlFor="edit-body" className="sr-only">
            Description
          </label>
          <textarea
            id="edit-body"
            className="input min-h-[12rem] font-mono"
            value={body}
            maxLength={MAX_BODY_CHARS}
            onChange={(e) => setBody(e.target.value)}
            autoFocus
          />
          <div className="flex gap-2">
            <button className="btn-primary" type="submit" disabled={busy}>
              {busy ? "Saving…" : "Save"}
            </button>
            <button className="btn-secondary" type="button" onClick={() => setEditing(false)} disabled={busy}>
              Cancel
            </button>
          </div>
          <InlineError message={error} />
        </form>
      ) : issue.body?.trim() ? (
        // Plain text (React escapes it): no HTML from GitHub is ever injected into the page.
        <div className="whitespace-pre-wrap break-words text-sm text-gray-800">{issue.body}</div>
      ) : (
        <p className="text-sm italic text-gray-400">No description provided.</p>
      )}
    </section>
  );
}

export function StateControls({ issue, patch, canEdit }: { issue: Issue; patch: PatchIssue; canEdit: boolean }) {
  const { busy, error, run } = useAction();
  if (!canEdit) return null;
  return (
    <div>
      <div className="flex flex-wrap gap-2">
        {issue.state === "open" ? (
          <>
            <button className="btn-secondary" disabled={busy} onClick={() => run(() => patch({ state: "closed", state_reason: "completed" }))}>
              {busy ? "Updating…" : "Close as completed"}
            </button>
            <button className="btn-secondary" disabled={busy} onClick={() => run(() => patch({ state: "closed", state_reason: "not_planned" }))}>
              Close as not planned
            </button>
          </>
        ) : (
          <button className="btn-primary" disabled={busy} onClick={() => run(() => patch({ state: "open", state_reason: "reopened" }))}>
            {busy ? "Updating…" : "Reopen issue"}
          </button>
        )}
      </div>
      <InlineError message={error} />
    </div>
  );
}

export function AssigneeEditor({
  issue,
  candidates,
  patch,
  canEdit,
}: {
  issue: Issue;
  candidates: SimpleUser[];
  patch: PatchIssue;
  canEdit: boolean;
}) {
  const [editing, setEditing] = useState(false);
  const [selected, setSelected] = useState<string[]>([]);
  const { busy, error, run, setError } = useAction();
  const options = [...candidates, ...issue.assignees.filter((a) => !candidates.some((c) => c.login === a.login))];

  const start = () => {
    setSelected(issue.assignees.map((a) => a.login));
    setEditing(true);
  };
  const save = async () => {
    if (selected.length > MAX_ASSIGNEES) return setError(`At most ${MAX_ASSIGNEES} assignees.`);
    if (await run(() => patch({ assignees: selected }))) setEditing(false);
  };

  return (
    <section className="card" aria-label="Assignees">
      <div className="mb-2 flex items-center justify-between">
        <h2 className="text-sm font-semibold text-gray-800">Assignees</h2>
        {canEdit && !editing ? (
          <button className="text-xs font-semibold text-sky-700 hover:underline" onClick={start}>
            Edit
          </button>
        ) : null}
      </div>
      {editing ? (
        <div className="space-y-2">
          {options.length === 0 ? <p className="text-sm text-gray-500">No one can be assigned in this repository.</p> : null}
          <div className="max-h-56 space-y-1 overflow-y-auto">
            {options.map((user) => (
              <label key={user.login} className="flex cursor-pointer items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={selected.includes(user.login)}
                  onChange={() =>
                    setSelected((s) => (s.includes(user.login) ? s.filter((x) => x !== user.login) : [...s, user.login]))
                  }
                />
                <Avatar src={user.avatar_url} alt="" size={18} />
                {user.login}
              </label>
            ))}
          </div>
          <div className="flex gap-2">
            <button className="btn-primary" onClick={save} disabled={busy}>
              {busy ? "Saving…" : "Save"}
            </button>
            <button className="btn-secondary" onClick={() => setEditing(false)} disabled={busy}>
              Cancel
            </button>
          </div>
        </div>
      ) : issue.assignees.length === 0 ? (
        <p className="text-sm text-gray-500">No one assigned</p>
      ) : (
        <ul className="space-y-1">
          {issue.assignees.map((a) => (
            <li key={a.login} className="flex items-center gap-2 text-sm">
              <Avatar src={a.avatar_url} alt="" size={20} />
              {a.login}
            </li>
          ))}
        </ul>
      )}
      <InlineError message={error} />
    </section>
  );
}

export function LabelEditor({
  issue,
  repoLabels,
  onLabels,
  addLabel,
  removeLabel,
  canEdit,
}: {
  issue: Issue;
  repoLabels: Label[];
  onLabels: (labels: Label[]) => void;
  addLabel: (name: string) => Promise<Label[]>;
  removeLabel: (name: string) => Promise<Label[]>;
  canEdit: boolean;
}) {
  const [choice, setChoice] = useState("");
  const { busy, error, run } = useAction();
  const available = repoLabels.filter((l) => !issue.labels.some((x) => x.name === l.name));

  return (
    <section className="card" aria-label="Labels">
      <h2 className="mb-2 text-sm font-semibold text-gray-800">Labels</h2>
      {issue.labels.length === 0 ? <p className="text-sm text-gray-500">None yet</p> : null}
      <div className="flex flex-wrap gap-1">
        {issue.labels.map((l) => (
          <LabelBadge
            key={l.name}
            name={l.name}
            color={l.color}
            onRemove={canEdit && !busy ? () => void run(async () => onLabels(await removeLabel(l.name))) : undefined}
          />
        ))}
      </div>
      {canEdit && available.length > 0 ? (
        <form
          className="mt-3 flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (!choice) return;
            void run(async () => {
              onLabels(await addLabel(choice));
              setChoice("");
            });
          }}
        >
          <label htmlFor="add-label" className="sr-only">
            Add label
          </label>
          <select id="add-label" className="input" value={choice} onChange={(e) => setChoice(e.target.value)}>
            <option value="">Add a label…</option>
            {available.map((l) => (
              <option key={l.id} value={l.name}>
                {l.name}
              </option>
            ))}
          </select>
          <button className="btn-secondary" type="submit" disabled={busy || !choice}>
            Add
          </button>
        </form>
      ) : null}
      <InlineError message={error} />
    </section>
  );
}
