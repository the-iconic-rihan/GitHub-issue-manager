import type { NextApiRequest, NextApiResponse } from "next";
import { getToken } from "next-auth/jwt";

import { appOrigin, backendUrl, githubApiUrl } from "@/lib/server/config";
import { isSameOrigin, jsonError } from "@/lib/server/proxy";

/**
 * Disconnects the GitHub account:
 *  1. deletes everything the backend stores for the user (tracked repositories, cached issues);
 *  2. revokes this app's OAuth grant on GitHub (requires the OAuth client secret, so it runs here).
 * The browser then signs out to clear the session cookie.
 */
export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  if (req.method !== "POST") {
    res.setHeader("Allow", "POST");
    return res.status(405).json(jsonError("method_not_allowed", "Use POST."));
  }
  const origin = Array.isArray(req.headers.origin) ? req.headers.origin[0] : req.headers.origin;
  if (!isSameOrigin(origin, appOrigin(req.headers.host))) {
    return res.status(403).json(jsonError("forbidden", "Cross-origin request rejected."));
  }
  const token = await getToken({ req, secret: process.env.NEXTAUTH_SECRET });
  if (!token?.accessToken) {
    return res.status(401).json(jsonError("unauthorized", "You are not signed in."));
  }

  try {
    const deleted = await fetch(`${backendUrl()}/api/v1/me`, {
      method: "DELETE",
      headers: { Authorization: `Bearer ${token.accessToken}` },
      signal: AbortSignal.timeout(15_000),
    });
    if (!deleted.ok && deleted.status !== 401) {
      return res.status(502).json(jsonError("backend_error", `Could not delete your data (HTTP ${deleted.status}).`));
    }
  } catch (err) {
    console.error(`[disconnect] backend unavailable: ${(err as Error).message}`);
    return res.status(502).json(jsonError("backend_unavailable", "The API server is unavailable."));
  }

  let revoked = false;
  const clientId = process.env.GITHUB_ID;
  const clientSecret = process.env.GITHUB_SECRET;
  if (clientId && clientSecret) {
    try {
      const response = await fetch(`${githubApiUrl()}/applications/${encodeURIComponent(clientId)}/grant`, {
        method: "DELETE",
        headers: {
          Authorization: `Basic ${Buffer.from(`${clientId}:${clientSecret}`).toString("base64")}`,
          Accept: "application/vnd.github+json",
          "Content-Type": "application/json",
          "User-Agent": "github-issue-manager",
        },
        body: JSON.stringify({ access_token: token.accessToken }),
        signal: AbortSignal.timeout(15_000),
      });
      // 204: revoked; 404: grant already gone.
      revoked = response.status === 204 || response.status === 404;
      if (!revoked) console.warn(`[disconnect] GitHub grant revocation returned HTTP ${response.status}`);
    } catch (err) {
      console.warn(`[disconnect] GitHub grant revocation failed: ${(err as Error).message}`);
    }
  }

  return res.status(200).json({ data: { data_deleted: true, github_access_revoked: revoked } });
}
