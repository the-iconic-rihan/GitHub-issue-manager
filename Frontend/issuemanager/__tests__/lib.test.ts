import { ApiError, apiPath, apiRequest, errorMessage, query } from "@/lib/api-client";
import { labelColors, relativeTime, safeCallbackUrl, splitFullName } from "@/lib/format";
import { backendTarget, isSameOrigin } from "@/lib/server/proxy";
import { MAX_TITLE_CHARS, validateBody, validateIssueForm, validateTitle } from "@/lib/validation";

describe("validation", () => {
  it("requires a non-blank title within the limit", () => {
    expect(validateTitle("   ")).toBe("Title is required.");
    expect(validateTitle("x".repeat(MAX_TITLE_CHARS + 1))).toMatch(/at most/);
    expect(validateTitle("  Crash on start ")).toBeUndefined();
  });

  it("validates body length and required comments", () => {
    expect(validateBody("")).toBeUndefined();
    expect(validateBody(" ", { required: true })).toBe("This field is required.");
    expect(validateBody("x".repeat(65_537))).toMatch(/at most/);
  });

  it("collects issue form errors", () => {
    expect(validateIssueForm({ title: "", body: "", labels: [], assignees: Array(11).fill("a") })).toEqual({
      title: "Title is required.",
      assignees: "At most 10 assignees.",
    });
    expect(validateIssueForm({ title: "ok", body: "", labels: [], assignees: [] })).toEqual({});
  });
});

describe("format", () => {
  const now = new Date("2026-11-17T12:00:00Z");
  it("formats relative times", () => {
    expect(relativeTime("2026-11-17T11:59:30Z", now)).toBe("just now");
    expect(relativeTime("2026-11-17T10:00:00Z", now)).toBe("2 hours ago");
    expect(relativeTime("2026-11-14T12:00:00Z", now)).toBe("3 days ago");
    expect(relativeTime(null, now)).toBe("never");
    expect(relativeTime("garbage", now)).toBe("unknown");
  });

  it("picks readable label text colours and rejects invalid colours", () => {
    expect(labelColors("ffffff")).toEqual({ background: "#ffffff", color: "#111827" });
    expect(labelColors("000000")).toEqual({ background: "#000000", color: "#ffffff" });
    expect(labelColors("red;background:url(x)").background).toBe("#e5e7eb");
  });

  it("only allows same-site callback URLs", () => {
    expect(safeCallbackUrl("/repos/a/b?x=1")).toBe("/repos/a/b?x=1");
    expect(safeCallbackUrl("//evil.example")).toBe("/");
    expect(safeCallbackUrl("/\\evil.example")).toBe("/");
    expect(safeCallbackUrl("https://evil.example")).toBe("/");
    expect(safeCallbackUrl(["/a"])).toBe("/");
  });

  it("splits full names", () => {
    expect(splitFullName("acme/app")).toEqual({ owner: "acme", repo: "app" });
  });
});

describe("api-client helpers", () => {
  it("encodes path segments and builds queries", () => {
    expect(apiPath("acme", "my repo", 5)).toBe("acme/my%20repo/5");
    expect(apiPath("a/b")).toBe("a%2Fb");
    expect(query({ a: "x y", b: undefined, c: "", d: 2 })).toBe("?a=x+y&d=2");
    expect(query({})).toBe("");
  });

  it("formats validation errors", () => {
    const err = new ApiError(422, "validation_failed", "request validation failed", [{ field: "title", message: "is required" }]);
    expect(errorMessage(err)).toBe("request validation failed: title is required");
    expect(errorMessage(new Error("boom"))).toBe("boom");
    expect(errorMessage(42)).toBe("Something went wrong.");
  });
});

describe("apiRequest", () => {
  const originalFetch = global.fetch;
  afterEach(() => {
    global.fetch = originalFetch;
  });

  function mockFetch(status: number, body: unknown, headers: Record<string, string> = {}) {
    const fn = jest.fn().mockResolvedValue({
      status,
      ok: status >= 200 && status < 300,
      headers: { get: (k: string) => headers[k.toLowerCase()] ?? null },
      json: async () => {
        if (body === undefined) throw new Error("no body");
        return body;
      },
    });
    global.fetch = fn as unknown as typeof fetch;
    return fn;
  }

  it("sends JSON to the proxy and returns data/meta", async () => {
    const fn = mockFetch(200, { data: [1], meta: { page: 1, per_page: 30, has_next: false } });
    const result = await apiRequest<number[]>("repos/a/b/issues", { method: "POST", body: { title: "x" } });
    expect(result).toEqual({ data: [1], meta: { page: 1, per_page: 30, has_next: false } });
    expect(fn).toHaveBeenCalledWith(
      "/api/backend/repos/a/b/issues",
      expect.objectContaining({ method: "POST", body: '{"title":"x"}', headers: { "Content-Type": "application/json" } }),
    );
  });

  it("handles 204 No Content", async () => {
    mockFetch(204, undefined);
    await expect(apiRequest("tracked-repos/a/b", { method: "DELETE" })).resolves.toEqual({ data: undefined });
  });

  it("raises ApiError with code, details and retry-after", async () => {
    mockFetch(429, { error: { code: "rate_limited", message: "slow down" } }, { "retry-after": "30" });
    await expect(apiRequest("x")).rejects.toMatchObject({ status: 429, code: "rate_limited", retryAfter: 30 });
  });

  it("raises a friendly error on network failure and non-JSON errors", async () => {
    global.fetch = jest.fn().mockRejectedValue(new TypeError("Failed to fetch")) as unknown as typeof fetch;
    await expect(apiRequest("x")).rejects.toMatchObject({ code: "network_error" });
    mockFetch(500, undefined);
    await expect(apiRequest("x")).rejects.toMatchObject({ status: 500, message: "Request failed (HTTP 500)." });
  });
});

describe("proxy helpers", () => {
  it("compares origins strictly", () => {
    expect(isSameOrigin("http://localhost:3000", "http://localhost:3000")).toBe(true);
    expect(isSameOrigin("http://localhost:3000", "http://localhost:3000/")).toBe(true);
    expect(isSameOrigin("https://evil.example", "http://localhost:3000")).toBe(false);
    expect(isSameOrigin("http://localhost:3001", "http://localhost:3000")).toBe(false);
    expect(isSameOrigin(undefined, "http://localhost:3000")).toBe(false);
    expect(isSameOrigin("null", "http://localhost:3000")).toBe(false);
  });

  it("builds backend URLs safely", () => {
    expect(backendTarget("http://server:8080", ["repos", "a", "b", "issues"], "/api/backend/repos/a/b/issues?state=all")).toBe(
      "http://server:8080/api/v1/repos/a/b/issues?state=all",
    );
    expect(backendTarget("http://s", ["repos", "a b"], undefined)).toBe("http://s/api/v1/repos/a%20b");
    expect(backendTarget("http://s", ["repos", "..", "x"], undefined)).toBeNull();
    expect(backendTarget("http://s", [], undefined)).toBeNull();
    expect(backendTarget("http://s", ["command"], undefined)).toBeNull();
    expect(backendTarget("http://s", ["health", "live"], undefined)).toBeNull();
  });
});
