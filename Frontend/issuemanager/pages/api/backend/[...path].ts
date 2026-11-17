import type { NextApiRequest, NextApiResponse } from "next";
import { getToken } from "next-auth/jwt";

import { appOrigin, backendUrl } from "@/lib/server/config";
import { ALLOWED_METHODS, MUTATING_METHODS, backendTarget, isSameOrigin, jsonError } from "@/lib/server/proxy";

export const config = { api: { bodyParser: { sizeLimit: "512kb" } } };

const TIMEOUT_MS = 60_000;

/**
 * Authenticated proxy to the Rust API: `/api/backend/<path>` → `${BACKEND_URL}/api/v1/<path>`,
 * adding `Authorization: Bearer <GitHub token>` from the encrypted session cookie.
 */
export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  const method = (req.method ?? "GET").toUpperCase();
  if (!ALLOWED_METHODS.has(method)) {
    res.setHeader("Allow", Array.from(ALLOWED_METHODS).join(", "));
    return res.status(405).json(jsonError("method_not_allowed", `${method} is not allowed.`));
  }

  if (MUTATING_METHODS.has(method)) {
    const origin = Array.isArray(req.headers.origin) ? req.headers.origin[0] : req.headers.origin;
    if (!isSameOrigin(origin, appOrigin(req.headers.host))) {
      return res.status(403).json(jsonError("forbidden", "Cross-origin request rejected."));
    }
  }

  const token = await getToken({ req, secret: process.env.NEXTAUTH_SECRET });
  if (!token?.accessToken) {
    return res.status(401).json(jsonError("unauthorized", "You are not signed in."));
  }

  const segments = ([] as string[]).concat(req.query.path ?? []);
  const target = backendTarget(backendUrl(), segments, req.url);
  if (!target) {
    return res.status(404).json(jsonError("not_found", "No such API endpoint."));
  }

  const hasBody = MUTATING_METHODS.has(method) && req.body !== undefined && req.body !== "";
  let upstream: Response;
  try {
    upstream = await fetch(target, {
      method,
      headers: {
        Authorization: `Bearer ${token.accessToken}`,
        Accept: "application/json",
        ...(hasBody ? { "Content-Type": "application/json" } : {}),
      },
      body: hasBody ? JSON.stringify(req.body) : undefined,
      signal: AbortSignal.timeout(TIMEOUT_MS),
      redirect: "manual",
    });
  } catch (err) {
    const timedOut = (err as Error).name === "TimeoutError";
    console.error(`[proxy] ${method} ${segments.join("/")} failed: ${(err as Error).message}`);
    return res
      .status(timedOut ? 504 : 502)
      .json(jsonError("backend_unavailable", timedOut ? "The server took too long to respond." : "The API server is unavailable."));
  }

  res.status(upstream.status);
  for (const header of ["content-type", "retry-after"]) {
    const value = upstream.headers.get(header);
    if (value) res.setHeader(header, value);
  }
  res.setHeader("Cache-Control", "no-store");
  if (upstream.status === 204 || upstream.status === 304) return res.end();
  return res.send(Buffer.from(await upstream.arrayBuffer()));
}
