/**
 * @jest-environment node
 */
import type { NextApiRequest, NextApiResponse } from "next";

jest.mock("next-auth/jwt", () => ({ getToken: jest.fn() }));

import { getToken } from "next-auth/jwt";

import disconnect from "@/pages/api/account/disconnect";
import proxy from "@/pages/api/backend/[...path]";

const mockedGetToken = getToken as jest.MockedFunction<typeof getToken>;

interface MockRes {
  statusCode: number;
  headers: Record<string, string>;
  body: unknown;
  ended: boolean;
}

function call(
  handler: (req: NextApiRequest, res: NextApiResponse) => unknown,
  req: Partial<NextApiRequest>,
): Promise<MockRes> {
  const state: MockRes = { statusCode: 200, headers: {}, body: undefined, ended: false };
  const res = {
    status(code: number) {
      state.statusCode = code;
      return res;
    },
    setHeader(key: string, value: string) {
      state.headers[key.toLowerCase()] = value;
      return res;
    },
    json(body: unknown) {
      state.body = body;
      state.ended = true;
      return res;
    },
    send(body: Buffer) {
      state.body = JSON.parse(body.toString());
      state.ended = true;
      return res;
    },
    end() {
      state.ended = true;
      return res;
    },
  };
  return Promise.resolve(handler({ headers: {}, query: {}, ...req } as NextApiRequest, res as unknown as NextApiResponse)).then(
    () => state,
  );
}

const ORIGIN = "http://localhost:3000";

beforeEach(() => {
  process.env.NEXTAUTH_URL = ORIGIN;
  process.env.BACKEND_URL = "http://backend:8080";
  process.env.GITHUB_API_URL = "https://api.github.test";
  process.env.GITHUB_ID = "client-id";
  process.env.GITHUB_SECRET = "client-secret";
  mockedGetToken.mockReset();
});

const originalFetch = global.fetch;
afterEach(() => {
  global.fetch = originalFetch;
});

describe("/api/backend proxy", () => {
  it("rejects unauthenticated requests", async () => {
    mockedGetToken.mockResolvedValue(null);
    const res = await call(proxy, { method: "GET", query: { path: ["dashboard"] }, url: "/api/backend/dashboard" });
    expect(res.statusCode).toBe(401);
  });

  it("rejects cross-origin and origin-less mutations before reading the session", async () => {
    for (const origin of ["https://evil.example", undefined]) {
      const res = await call(proxy, {
        method: "POST",
        headers: origin ? { origin } : {},
        query: { path: ["repos", "a", "b", "issues"] },
        body: { title: "x" },
      });
      expect(res.statusCode).toBe(403);
    }
    expect(mockedGetToken).not.toHaveBeenCalled();
  });

  it("rejects unsupported methods and blocked paths", async () => {
    let res = await call(proxy, { method: "OPTIONS", query: { path: ["me"] } });
    expect(res.statusCode).toBe(405);
    mockedGetToken.mockResolvedValue({ accessToken: "gho_secret" });
    res = await call(proxy, { method: "POST", headers: { origin: ORIGIN }, query: { path: ["command"] }, body: {} });
    expect(res.statusCode).toBe(404);
  });

  it("forwards requests with the bearer token, query string and JSON body", async () => {
    mockedGetToken.mockResolvedValue({ accessToken: "gho_secret" });
    const fetchMock = jest.fn().mockResolvedValue(
      new Response(JSON.stringify({ data: { number: 1 } }), {
        status: 201,
        headers: { "content-type": "application/json" },
      }),
    );
    global.fetch = fetchMock as unknown as typeof fetch;

    const res = await call(proxy, {
      method: "POST",
      headers: { origin: ORIGIN },
      query: { path: ["repos", "acme", "my app", "issues"], extra: "ignored" },
      url: "/api/backend/repos/acme/my%20app/issues?dry=1",
      body: { title: "Bug" },
    });

    expect(res.statusCode).toBe(201);
    expect(res.body).toEqual({ data: { number: 1 } });
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("http://backend:8080/api/v1/repos/acme/my%20app/issues?dry=1");
    expect(init.method).toBe("POST");
    expect(init.headers.Authorization).toBe("Bearer gho_secret");
    expect(init.body).toBe('{"title":"Bug"}');
  });

  it("passes through errors and Retry-After, and handles 204", async () => {
    mockedGetToken.mockResolvedValue({ accessToken: "t" });
    global.fetch = jest.fn().mockResolvedValue(
      new Response(JSON.stringify({ error: { code: "rate_limited" } }), {
        status: 429,
        headers: { "content-type": "application/json", "retry-after": "30" },
      }),
    ) as unknown as typeof fetch;
    let res = await call(proxy, { method: "GET", query: { path: ["dashboard"] }, url: "/api/backend/dashboard" });
    expect(res.statusCode).toBe(429);
    expect(res.headers["retry-after"]).toBe("30");

    global.fetch = jest.fn().mockResolvedValue(new Response(null, { status: 204 })) as unknown as typeof fetch;
    res = await call(proxy, { method: "DELETE", headers: { origin: ORIGIN }, query: { path: ["me"] } });
    expect(res.statusCode).toBe(204);
    expect(res.ended).toBe(true);
  });

  it("returns 502 when the backend is unreachable", async () => {
    mockedGetToken.mockResolvedValue({ accessToken: "t" });
    global.fetch = jest.fn().mockRejectedValue(new TypeError("fetch failed")) as unknown as typeof fetch;
    jest.spyOn(console, "error").mockImplementation(() => undefined);
    const res = await call(proxy, { method: "GET", query: { path: ["dashboard"] }, url: "/api/backend/dashboard" });
    expect(res.statusCode).toBe(502);
    expect(res.body).toMatchObject({ error: { code: "backend_unavailable" } });
  });
});

describe("/api/account/disconnect", () => {
  it("requires POST, same origin and a session", async () => {
    expect((await call(disconnect, { method: "GET" })).statusCode).toBe(405);
    expect((await call(disconnect, { method: "POST", headers: { origin: "https://evil.example" } })).statusCode).toBe(403);
    mockedGetToken.mockResolvedValue(null);
    expect((await call(disconnect, { method: "POST", headers: { origin: ORIGIN } })).statusCode).toBe(401);
  });

  it("deletes backend data then revokes the GitHub grant", async () => {
    mockedGetToken.mockResolvedValue({ accessToken: "gho_secret" });
    const fetchMock = jest
      .fn()
      .mockResolvedValueOnce(new Response(null, { status: 204 }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    global.fetch = fetchMock as unknown as typeof fetch;

    const res = await call(disconnect, { method: "POST", headers: { origin: ORIGIN } });
    expect(res.statusCode).toBe(200);
    expect(res.body).toEqual({ data: { data_deleted: true, github_access_revoked: true } });

    expect(fetchMock.mock.calls[0][0]).toBe("http://backend:8080/api/v1/me");
    expect(fetchMock.mock.calls[0][1].method).toBe("DELETE");
    const [revokeUrl, revokeInit] = fetchMock.mock.calls[1];
    expect(revokeUrl).toBe("https://api.github.test/applications/client-id/grant");
    expect(revokeInit.headers.Authorization).toBe(`Basic ${Buffer.from("client-id:client-secret").toString("base64")}`);
    expect(JSON.parse(revokeInit.body)).toEqual({ access_token: "gho_secret" });
  });

  it("does not revoke when deleting data fails", async () => {
    mockedGetToken.mockResolvedValue({ accessToken: "t" });
    const fetchMock = jest.fn().mockResolvedValueOnce(new Response(null, { status: 500 }));
    global.fetch = fetchMock as unknown as typeof fetch;
    const res = await call(disconnect, { method: "POST", headers: { origin: ORIGIN } });
    expect(res.statusCode).toBe(502);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
