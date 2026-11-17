import { useCallback, useEffect, useState, type FormEvent } from "react";

import { apiRequest, errorMessage, query } from "@/lib/api-client";
import { relativeTime } from "@/lib/format";
import type { Comment, PageMeta } from "@/lib/types";
import { MAX_BODY_CHARS, validateBody } from "@/lib/validation";

import { Avatar, ErrorMessage, Spinner } from "./ui";

const PER_PAGE = 30;

/** Paginated ("load more") comment list plus a form to add a comment. */
export default function Comments({
  basePath,
  locked,
  onCommentAdded,
}: {
  basePath: string;
  locked: boolean;
  onCommentAdded?: () => void;
}) {
  const [comments, setComments] = useState<Comment[]>([]);
  const [meta, setMeta] = useState<PageMeta>();
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<unknown>();
  const [body, setBody] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string>();

  // State is only updated when the request settles (never synchronously inside an effect).
  const fetchPage = useCallback(
    (page: number, signal?: AbortSignal) =>
      apiRequest<Comment[]>(`${basePath}/comments${query({ page, per_page: PER_PAGE })}`, { signal }).then(
        (result) => {
          setComments((prev) => (page === 1 ? result.data : [...prev, ...result.data]));
          setMeta(result.meta);
          setLoadError(undefined);
          setLoading(false);
        },
        (err: Error) => {
          if (err.name === "AbortError") return;
          setLoadError(err);
          setLoading(false);
        },
      ),
    [basePath],
  );

  // The parent remounts this component (via `key`) when the issue changes.
  useEffect(() => {
    const controller = new AbortController();
    void fetchPage(1, controller.signal);
    return () => controller.abort();
  }, [fetchPage]);

  function load(page: number) {
    setLoading(true);
    setLoadError(undefined);
    void fetchPage(page);
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    const invalid = validateBody(body, { required: true });
    if (invalid) return setSubmitError(invalid === "This field is required." ? "Write a comment first." : invalid);
    setSubmitting(true);
    setSubmitError(undefined);
    try {
      const { data } = await apiRequest<Comment>(`${basePath}/comments`, { method: "POST", body: { body } });
      setBody("");
      // Only append when every earlier page is loaded; otherwise it appears when paging.
      if (!meta?.has_next) setComments((prev) => [...prev, data]);
      onCommentAdded?.();
    } catch (err) {
      setSubmitError(errorMessage(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <section aria-label="Comments" className="space-y-3">
      <h2 className="text-lg font-semibold text-gray-900">Comments</h2>
      {loadError ? <ErrorMessage error={loadError} onRetry={() => load(meta ? meta.page + 1 : 1)} /> : null}
      {!loading && !loadError && comments.length === 0 ? <p className="text-sm text-gray-500">No comments yet.</p> : null}
      <ul className="space-y-3">
        {comments.map((c) => (
          <li key={c.id} className="card">
            <div className="mb-2 flex items-center gap-2 text-sm text-gray-600">
              <Avatar src={c.user?.avatar_url} alt="" size={20} />
              <strong>{c.user?.login ?? "ghost"}</strong>
              <a href={c.html_url} target="_blank" rel="noopener noreferrer" className="hover:underline">
                {relativeTime(c.created_at)}
              </a>
            </div>
            <div className="whitespace-pre-wrap break-words text-sm text-gray-800">{c.body}</div>
          </li>
        ))}
      </ul>
      {loading ? <Spinner label="Loading comments" /> : null}
      {meta?.has_next && !loading ? (
        <button className="btn-secondary" onClick={() => load(meta.page + 1)}>
          Load more comments
        </button>
      ) : null}

      {locked ? (
        <p className="text-sm text-gray-500">This conversation is locked on GitHub.</p>
      ) : (
        <form onSubmit={submit} className="card space-y-2" noValidate>
          <label htmlFor="new-comment" className="block text-sm font-semibold text-gray-800">
            Add a comment
          </label>
          <textarea
            id="new-comment"
            className="input min-h-[6rem]"
            value={body}
            maxLength={MAX_BODY_CHARS}
            onChange={(e) => setBody(e.target.value)}
            placeholder="Leave a comment"
            aria-invalid={submitError ? true : undefined}
          />
          {submitError ? (
            <p role="alert" className="text-sm text-red-700">
              {submitError}
            </p>
          ) : null}
          <button className="btn-primary" type="submit" disabled={submitting}>
            {submitting ? "Posting…" : "Comment"}
          </button>
        </form>
      )}
    </section>
  );
}
