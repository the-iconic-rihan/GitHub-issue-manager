import { useState, type FormEvent } from "react";

import type { Label, SimpleUser } from "@/lib/types";
import { MAX_BODY_CHARS, MAX_TITLE_CHARS, validateIssueForm, type FormErrors, type IssueFormValues } from "@/lib/validation";

import { LabelBadge } from "./ui";

/** Form for creating an issue: title (required), description, labels, assignees. */
export default function IssueForm({
  labels,
  assignees,
  onSubmit,
  submitting,
  serverErrors,
}: {
  labels: Label[];
  assignees: SimpleUser[];
  onSubmit: (values: IssueFormValues) => void | Promise<void>;
  submitting: boolean;
  serverErrors?: FormErrors;
}) {
  const [values, setValues] = useState<IssueFormValues>({ title: "", body: "", labels: [], assignees: [] });
  const [errors, setErrors] = useState<FormErrors>({});
  const shown = { ...serverErrors, ...errors };

  const toggle = (key: "labels" | "assignees", item: string) =>
    setValues((v) => ({
      ...v,
      [key]: v[key].includes(item) ? v[key].filter((x) => x !== item) : [...v[key], item],
    }));

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const found = validateIssueForm(values);
    setErrors(found);
    if (Object.keys(found).length === 0) {
      void onSubmit({ ...values, title: values.title.trim() });
    }
  };

  return (
    <form onSubmit={submit} noValidate className="space-y-5">
      <div>
        <label htmlFor="title" className="mb-1 block text-sm font-semibold text-gray-800">
          Title <span className="text-red-600">*</span>
        </label>
        <input
          id="title"
          className="input"
          value={values.title}
          maxLength={MAX_TITLE_CHARS}
          aria-invalid={shown.title ? true : undefined}
          aria-describedby={shown.title ? "title-error" : undefined}
          onChange={(e) => setValues((v) => ({ ...v, title: e.target.value }))}
          placeholder="Short summary of the issue"
        />
        {shown.title ? (
          <p id="title-error" className="mt-1 text-sm text-red-700">
            {shown.title}
          </p>
        ) : null}
      </div>

      <div>
        <label htmlFor="body" className="mb-1 block text-sm font-semibold text-gray-800">
          Description
        </label>
        <textarea
          id="body"
          className="input min-h-[10rem] font-mono"
          value={values.body}
          maxLength={MAX_BODY_CHARS}
          aria-invalid={shown.body ? true : undefined}
          onChange={(e) => setValues((v) => ({ ...v, body: e.target.value }))}
          placeholder="Steps to reproduce, expected behaviour… (Markdown is rendered on GitHub)"
        />
        {shown.body ? <p className="mt-1 text-sm text-red-700">{shown.body}</p> : null}
      </div>

      {labels.length > 0 ? (
        <fieldset>
          <legend className="mb-2 text-sm font-semibold text-gray-800">Labels</legend>
          <div className="flex flex-wrap gap-3">
            {labels.map((l) => (
              <label key={l.id} className="inline-flex cursor-pointer items-center gap-2 text-sm">
                <input type="checkbox" checked={values.labels.includes(l.name)} onChange={() => toggle("labels", l.name)} />
                <LabelBadge name={l.name} color={l.color} />
              </label>
            ))}
          </div>
        </fieldset>
      ) : null}

      {assignees.length > 0 ? (
        <fieldset>
          <legend className="mb-2 text-sm font-semibold text-gray-800">Assignees</legend>
          <div className="flex flex-wrap gap-3">
            {assignees.map((a) => (
              <label key={a.id} className="inline-flex cursor-pointer items-center gap-2 text-sm">
                <input type="checkbox" checked={values.assignees.includes(a.login)} onChange={() => toggle("assignees", a.login)} />
                {a.login}
              </label>
            ))}
          </div>
          {shown.assignees ? <p className="mt-1 text-sm text-red-700">{shown.assignees}</p> : null}
        </fieldset>
      ) : null}

      <button type="submit" className="btn-primary" disabled={submitting}>
        {submitting ? "Creating…" : "Create issue"}
      </button>
    </form>
  );
}
