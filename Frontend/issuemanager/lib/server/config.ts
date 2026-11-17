// Server-only configuration (never import from browser code).

function trimSlash(url: string): string {
  return url.replace(/\/+$/, "");
}

/** Internal URL of the Rust API, e.g. http://server:8080 in Docker/Kubernetes. */
export function backendUrl(): string {
  return trimSlash(process.env.BACKEND_URL || "http://localhost:8080");
}

/** GitHub web URL (OAuth endpoints). Override for GitHub Enterprise Server. */
export function githubWebUrl(): string {
  return trimSlash(process.env.GITHUB_WEB_URL || "https://github.com");
}

/** GitHub REST API URL. For GitHub Enterprise Server: https://<host>/api/v3 */
export function githubApiUrl(): string {
  return trimSlash(process.env.GITHUB_API_URL || "https://api.github.com");
}

/** OAuth scopes. `repo` is needed to manage issues in private repositories;
 * use `public_repo` to restrict the app to public repositories. */
export function githubScopes(): string {
  return process.env.GITHUB_OAUTH_SCOPES || "read:user user:email repo";
}

/** Public origin of this web app (from NEXTAUTH_URL), used for CSRF origin checks. */
export function appOrigin(hostHeader?: string): string | undefined {
  const configured = process.env.NEXTAUTH_URL;
  if (configured) {
    try {
      return new URL(configured).origin;
    } catch {
      return undefined;
    }
  }
  // Development fallback when NEXTAUTH_URL is unset.
  return hostHeader ? `http://${hostHeader}` : undefined;
}
