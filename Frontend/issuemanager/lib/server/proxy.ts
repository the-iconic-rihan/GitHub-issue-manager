// Helpers for the `/api/backend/*` proxy. Kept free of Next.js types so they are easy to test.

export const MUTATING_METHODS = new Set(["POST", "PUT", "PATCH", "DELETE"]);
export const ALLOWED_METHODS = new Set(["GET", ...MUTATING_METHODS]);

/** Backend paths the browser may not reach through the proxy. */
const BLOCKED_ROOTS = new Set(["command", "health"]);

/**
 * CSRF protection for state-changing requests: the browser always sends `Origin` on
 * cross-origin and same-origin POST/PUT/PATCH/DELETE `fetch` calls, so it must match ours.
 * (Session cookies are also SameSite=Lax, and the JSON content type forces a CORS preflight.)
 */
export function isSameOrigin(originHeader: string | undefined, expectedOrigin: string | undefined): boolean {
  if (!originHeader || !expectedOrigin) return false;
  try {
    return new URL(originHeader).origin === new URL(expectedOrigin).origin;
  } catch {
    return false;
  }
}

/**
 * Builds the backend URL for a proxied request. Returns `null` for paths that must not be
 * proxied (empty or traversal segments, operator-only endpoints).
 */
export function backendTarget(base: string, segments: string[], rawUrl: string | undefined): string | null {
  if (segments.length === 0) return null;
  if (segments.some((s) => s === "" || s === "." || s === "..")) return null;
  if (BLOCKED_ROOTS.has(segments[0])) return null;
  const queryIndex = rawUrl?.indexOf("?") ?? -1;
  const search = rawUrl && queryIndex >= 0 ? rawUrl.slice(queryIndex) : "";
  return `${base}/api/v1/${segments.map(encodeURIComponent).join("/")}${search}`;
}

export function jsonError(code: string, message: string) {
  return { error: { code, message } };
}
