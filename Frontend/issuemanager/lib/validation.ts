// Client-side validation mirroring the backend limits (app/server/src/validate.rs).
// The backend re-validates everything; this only gives immediate feedback.

export const MAX_TITLE_CHARS = 256;
export const MAX_BODY_CHARS = 65_536;
export const MAX_ASSIGNEES = 10;

export type FormErrors = Partial<Record<"title" | "body" | "assignees" | "labels", string>>;

const length = (value: string) => Array.from(value).length;

export function validateTitle(title: string): string | undefined {
  const trimmed = title.trim();
  if (trimmed.length === 0) return "Title is required.";
  if (length(trimmed) > MAX_TITLE_CHARS) return `Title must be at most ${MAX_TITLE_CHARS} characters.`;
  return undefined;
}

export function validateBody(body: string, { required = false } = {}): string | undefined {
  if (required && body.trim().length === 0) return "This field is required.";
  if (length(body) > MAX_BODY_CHARS) return `Must be at most ${MAX_BODY_CHARS.toLocaleString()} characters.`;
  return undefined;
}

export interface IssueFormValues {
  title: string;
  body: string;
  labels: string[];
  assignees: string[];
}

export function validateIssueForm(values: IssueFormValues): FormErrors {
  const errors: FormErrors = {};
  const title = validateTitle(values.title);
  if (title) errors.title = title;
  const body = validateBody(values.body);
  if (body) errors.body = body;
  if (values.assignees.length > MAX_ASSIGNEES) errors.assignees = `At most ${MAX_ASSIGNEES} assignees.`;
  return errors;
}
