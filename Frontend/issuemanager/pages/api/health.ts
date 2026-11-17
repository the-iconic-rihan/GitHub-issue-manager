import type { NextApiRequest, NextApiResponse } from "next";

/** Liveness probe for Docker/Kubernetes. */
export default function handler(_req: NextApiRequest, res: NextApiResponse) {
  res.setHeader("Cache-Control", "no-store");
  res.status(200).json({ data: { status: "ok" } });
}
