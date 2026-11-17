// Browser-side API client. All calls go to this app's `/api/backend/*` proxy, which attaches
// the user's GitHub token server-side; the token is never exposed to the browser.

import type { PageMeta } from "./types";

export interface FieldError {
  field: string;
  message: string;
}

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details: FieldError[];
  readonly retryAfter?: number;

  constructor(status: number, code: string, message: string, details: FieldError[] = [], retryAfter?: number) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.details = details;
    this.retryAfter = retryAfter;
  }

  get isUnauthorized() {
    return this.status === 401;
  }
}

export interface ApiResult<T> {
  data: T;
  meta?: PageMeta;
}

export interface RequestOptions {
  method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  body?: unknown;
  signal?: AbortSignal;
}

/** Encodes path segments so names containing `/`, `?`, `#` or spaces stay one segment. */
export function apiPath(...segments: (string | number)[]): string {
  return segments.map((s) => encodeURIComponent(String(s))).join("/");
}

/** Builds a query string from defined, non-empty values. */
export function query(params: Record<string, string | number | undefined | null>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") search.set(key, String(value));
  }
  const s = search.toString();
  return s ? `?${s}` : "";
}

export async function apiRequest<T>(path: string, options: RequestOptions = {}): Promise<ApiResult<T>> {
  const { method = "GET", body, signal } = options;
  let response: Response;
  try {
    response = await fetch(`/api/backend/${path}`, {
      method,
      headers: body !== undefined ? { "Content-Type": "application/json" } : undefined,
      body: body !== undefined ? JSON.stringify(body) : undefined,
      credentials: "same-origin",
      signal,
    });
  } catch (err) {
    if ((err as Error).name === "AbortError") throw err;
    throw new ApiError(0, "network_error", "Could not reach the server. Check your connection and try again.");
  }

  if (response.status === 204) return { data: undefined as T };

  let json: unknown = null;
  try {
    json = await response.json();
  } catch {
    json = null;
  }

  if (!response.ok) {
    const err = (json as { error?: { code?: string; message?: string; details?: FieldError[] } } | null)?.error;
    const retryAfter = Number(response.headers.get("retry-after")) || undefined;
    throw new ApiError(
      response.status,
      err?.code ?? "http_error",
      err?.message ?? `Request failed (HTTP ${response.status}).`,
      err?.details ?? [],
      retryAfter,
    );
  }
  return json as ApiResult<T>;
}

/** A readable message for any thrown value. */
export function errorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    if (err.details.length > 0) {
      return `${err.message}: ${err.details.map((d) => `${d.field} ${d.message}`).join("; ")}`;
    }
    return err.message;
  }
  if (err instanceof Error) return err.message;
  return "Something went wrong.";
}
