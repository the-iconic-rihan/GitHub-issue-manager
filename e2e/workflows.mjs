// End-to-end workflow checks through the real website (next-auth + proxy) → real API server →
// real PostgreSQL, with a fake GitHub (e2e/fake-github.mjs). Normally run by e2e/run.sh.
//
//   node e2e/workflows.mjs <main|after-restart|disconnect>
//
// Env: E2E_WEB_URL, E2E_API_URL, E2E_STATE_DIR (cookie jar shared between phases).
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const WEB = process.env.E2E_WEB_URL || "http://localhost:13000";
const API = process.env.E2E_API_URL || "http://127.0.0.1:18080";
const JAR_FILE = path.join(process.env.E2E_STATE_DIR || os.tmpdir(), "gim-e2e-cookies.json");
const jar = new Map(fs.existsSync(JAR_FILE) ? JSON.parse(fs.readFileSync(JAR_FILE, "utf8")) : []);
const saveJar = () => fs.writeFileSync(JAR_FILE, JSON.stringify([...jar]));
let passed = 0;

function storeCookies(res) {
  for (const c of res.headers.getSetCookie()) {
    const [pair] = c.split(";");
    const i = pair.indexOf("=");
    const name = pair.slice(0, i), value = pair.slice(i + 1);
    if (/max-age=0|expires=thu, 01 jan 1970/i.test(c) || value === "") jar.delete(name);
    else jar.set(name, value);
  }
}
const cookieHeader = () => [...jar].map(([k, v]) => `${k}=${v}`).join("; ");

async function web(path, init = {}) {
  const res = await fetch(path.startsWith("http") ? path : `${WEB}${path}`, {
    redirect: "manual",
    ...init,
    headers: { cookie: cookieHeader(), ...(init.headers ?? {}) },
  });
  storeCookies(res);
  return res;
}

async function api(method, path, body, { origin = WEB } = {}) {
  const headers = {};
  if (origin) headers.origin = origin;
  if (body !== undefined) headers["content-type"] = "application/json";
  const res = await web(`/api/backend/${path}`, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
  const text = await res.text();
  return { status: res.status, json: text ? JSON.parse(text) : null, headers: res.headers };
}

async function step(name, fn) {
  try {
    await fn();
    passed++;
    console.log(`  ✓ ${name}`);
  } catch (err) {
    console.log(`  ✗ ${name}\n    ${err.stack?.split("\n").slice(0, 3).join("\n    ")}`);
    process.exitCode = 1;
  }
}

async function signIn() {
  jar.clear();
  const csrf = await (await web("/api/auth/csrf")).json();
  const start = await web("/api/auth/signin/github", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ csrfToken: csrf.csrfToken, callbackUrl: `${WEB}/`, json: "true" }),
  });
  const { url: authorizeUrl } = await start.json();
  assert.match(authorizeUrl, /\/login\/oauth\/authorize\?/);
  assert.match(authorizeUrl, /scope=read%3Auser\+user%3Aemail\+repo|scope=read%3Auser%20user%3Aemail%20repo/);
  const authorize = await fetch(authorizeUrl, { redirect: "manual" });
  const callback = authorize.headers.get("location");
  assert.match(callback, /\/api\/auth\/callback\/github\?code=/);
  const done = await web(callback);
  assert.equal(done.status, 302, "callback should redirect into the app");
  assert.ok([...jar.keys()].some((k) => k.includes("session-token")), "session cookie set");
  saveJar();
}

async function main() {
  console.log("Phase: main");
  await step("1. services are healthy (website + API readiness)", async () => {
    assert.equal((await fetch(`${WEB}/api/health`)).status, 200);
    assert.equal((await fetch(`${API}/api/v1/health/ready`)).status, 200);
  });
  await step("unauthenticated requests are rejected and pages redirect client-side", async () => {
    jar.clear();
    const r = await api("GET", "dashboard");
    assert.equal(r.status, 401);
    assert.equal((await web("/login")).status, 200);
  });
  await step("2. connect GitHub account via OAuth (next-auth → fake GitHub)", signIn);
  await step("session exposes the login but never the GitHub token", async () => {
    const session = await (await web("/api/auth/session")).json();
    assert.equal(session.user.login, "e2e-user");
    assert.ok(!JSON.stringify(session).includes("gho_"), "token leaked into session JSON");
  });
  await step("GET /me records the user", async () => {
    const r = await api("GET", "me");
    assert.equal(r.status, 200);
    assert.equal(r.json.data.user.login, "e2e-user");
  });
  await step("CSRF: mutations without/with foreign Origin are rejected", async () => {
    assert.equal((await api("PUT", "tracked-repos/e2e-user/demo", undefined, { origin: null })).status, 403);
    assert.equal((await api("PUT", "tracked-repos/e2e-user/demo", undefined, { origin: "https://evil.example" })).status, 403);
  });
  await step("3. load repositories", async () => {
    const r = await api("GET", "repos?per_page=30");
    assert.equal(r.status, 200);
    assert.deepEqual(r.json.data.map((x) => [x.full_name, x.tracked]), [["e2e-user/demo", false], ["e2e-user/second", false]]);
  });
  await step("4. select + track + sync repository", async () => {
    let r = await api("GET", "repos/e2e-user/demo");
    assert.equal(r.json.data.tracked, null);
    r = await api("PUT", "tracked-repos/e2e-user/demo");
    assert.equal(r.status, 200, JSON.stringify(r.json));
    r = await api("POST", "tracked-repos/e2e-user/demo/sync");
    assert.equal(r.status, 200, JSON.stringify(r.json));
    assert.equal(r.json.data.issues_fetched, 4, "4 issues (PR excluded)");
    r = await api("GET", "repos?per_page=30");
    assert.equal(r.json.data[0].tracked, true);
  });
  await step("5. fetch issues with filters, search, sort and pagination", async () => {
    let r = await api("GET", "repos/e2e-user/demo/issues");
    assert.deepEqual(r.json.data.map((i) => i.number), [5, 2, 1], "open issues, newest first, PR excluded");
    r = await api("GET", "repos/e2e-user/demo/issues?state=closed");
    assert.deepEqual(r.json.data.map((i) => i.number), [3]);
    r = await api("GET", "repos/e2e-user/demo/issues?state=all&labels=bug");
    assert.deepEqual(r.json.data.map((i) => i.number), [3, 1]);
    r = await api("GET", "repos/e2e-user/demo/issues?assignee=none");
    assert.deepEqual(r.json.data.map((i) => i.number), [5, 2]);
    r = await api("GET", "repos/e2e-user/demo/issues?sort=created&direction=asc&per_page=2");
    assert.deepEqual(r.json.data.map((i) => i.number), [1, 2]);
    assert.equal(r.json.meta.has_next, true);
    r = await api("GET", "repos/e2e-user/demo/issues?sort=created&direction=asc&per_page=2&page=2");
    assert.deepEqual(r.json.data.map((i) => i.number), [5]);
    assert.equal(r.json.meta.has_next, false);
    r = await api("GET", "repos/e2e-user/demo/issues?q=dark%20mode");
    assert.deepEqual(r.json.data.map((i) => i.number), [2]);
    assert.equal(r.json.meta.total_count, 1);
    r = await api("GET", "repos/e2e-user/demo/issues?per_page=1000");
    assert.equal(r.status, 422);
  });
  await step("6. open issue details and comments", async () => {
    let r = await api("GET", "repos/e2e-user/demo/issues/1");
    assert.equal(r.json.data.title, "App crashes on start");
    r = await api("GET", "repos/e2e-user/demo/issues/1/comments");
    assert.deepEqual(r.json.data, []);
    r = await api("GET", "repos/e2e-user/demo/issues/4");
    assert.equal(r.status, 404, "pull requests are not issues");
  });
  let created;
  await step("7. create an issue (and reject an invalid one)", async () => {
    let r = await api("POST", "repos/e2e-user/demo/issues", { title: "   " });
    assert.equal(r.status, 422);
    r = await api("POST", "repos/e2e-user/demo/issues", { title: "E2E created issue", body: "Created by e2e", labels: ["bug"] });
    assert.equal(r.status, 201, JSON.stringify(r.json));
    created = r.json.data.number;
    assert.equal(created, 6);
  });
  await step("8. update title and description", async () => {
    const r = await api("PATCH", `repos/e2e-user/demo/issues/${created}`, { title: "E2E edited title", body: "Edited body" });
    assert.equal(r.status, 200);
    assert.equal(r.json.data.title, "E2E edited title");
    assert.equal(r.json.data.body, "Edited body");
  });
  await step("9. assign the issue", async () => {
    const r = await api("PATCH", `repos/e2e-user/demo/issues/${created}`, { assignees: ["helper", "e2e-user"] });
    assert.deepEqual(r.json.data.assignees.map((a) => a.login), ["helper", "e2e-user"]);
  });
  await step("10. add/remove labels and add a comment", async () => {
    let r = await api("POST", `repos/e2e-user/demo/issues/${created}/labels`, { labels: ["good first issue"] });
    assert.deepEqual(r.json.data.map((l) => l.name), ["bug", "good first issue"]);
    r = await api("DELETE", `repos/e2e-user/demo/issues/${created}/labels?name=bug`);
    assert.deepEqual(r.json.data.map((l) => l.name), ["good first issue"]);
    r = await api("POST", `repos/e2e-user/demo/issues/${created}/comments`, { body: "Working on it" });
    assert.equal(r.status, 201);
    r = await api("GET", `repos/e2e-user/demo/issues/${created}/comments`);
    assert.deepEqual(r.json.data.map((c) => c.body), ["Working on it"]);
  });
  await step("11. close (not planned) and reopen", async () => {
    let r = await api("PATCH", `repos/e2e-user/demo/issues/${created}`, { state: "closed", state_reason: "not_planned" });
    assert.equal(r.json.data.state, "closed");
    assert.equal(r.json.data.state_reason, "not_planned");
    r = await api("GET", "repos/e2e-user/demo/issues?state=closed");
    assert.ok(r.json.data.some((i) => i.number === created));
    r = await api("PATCH", `repos/e2e-user/demo/issues/${created}`, { state: "open", state_reason: "reopened" });
    assert.equal(r.json.data.state, "open");
  });
  await step("12. changes are reflected in detail view and dashboard (write-through)", async () => {
    let r = await api("GET", `repos/e2e-user/demo/issues/${created}`);
    assert.equal(r.json.data.title, "E2E edited title");
    assert.equal(r.json.data.comments, 1);
    r = await api("GET", "dashboard");
    const t = r.json.data.totals;
    assert.deepEqual(
      { tracked: t.tracked_repositories, open: t.open_issues, closed: t.closed_issues, mine: t.assigned_to_me_open, unassigned: t.unassigned_open },
      { tracked: 1, open: 4, closed: 1, mine: 2, unassigned: 2 },
    );
    assert.equal(r.json.data.recently_updated[0].number, created);
    assert.ok(r.json.data.top_labels.some((l) => l.label === "good first issue" && l.count === 2), JSON.stringify(r.json.data.top_labels));
  });
  await step("pages render (HTTP 200) for all routes", async () => {
    for (const p of ["/", "/repos", "/repos/e2e-user/demo", "/repos/e2e-user/demo/issues/1", "/repos/e2e-user/demo/issues/new", "/account", "/discover", "/login"]) {
      const res = await web(p);
      assert.equal(res.status, 200, p);
      assert.match(res.headers.get("content-security-policy") ?? "", /frame-ancestors 'none'/);
    }
  });
  await step("operator-only and unknown backend paths are not proxied", async () => {
    assert.equal((await api("POST", "command", { action: "insert", target: "issues", limit: 1 })).status, 404);
    assert.equal((await api("GET", "nope")).status, 404);
  });
  saveJar();
}

async function afterRestart() {
  console.log("Phase: after API restart");
  await step("13. tracked repository and synced statistics persist across restart", async () => {
    let r = await api("GET", "tracked-repos");
    assert.equal(r.status, 200, JSON.stringify(r.json));
    assert.deepEqual(r.json.data.map((t) => t.full_name), ["e2e-user/demo"]);
    assert.ok(r.json.data[0].last_synced_at);
    r = await api("GET", "dashboard");
    assert.equal(r.json.data.totals.open_issues, 4);
  });
  await step("automatic sync refreshes never-synced repositories only", async () => {
    let r = await api("PUT", "tracked-repos/e2e-user/second");
    assert.equal(r.status, 200);
    r = await api("POST", "tracked-repos/sync-stale");
    assert.equal(r.status, 200, JSON.stringify(r.json));
    assert.deepEqual(r.json.data.updated, ["e2e-user/second"], "demo was synced recently and must be skipped");
    r = await api("POST", "tracked-repos/sync-stale");
    assert.deepEqual(r.json.data, { updated: [], unchanged: [], failed: [], more_pending: false });
    assert.equal((await api("DELETE", "tracked-repos/e2e-user/second")).status, 204);
  });
  await step("untrack and re-track second repo round trip", async () => {
    assert.equal((await api("PUT", "tracked-repos/e2e-user/second")).status, 200);
    assert.equal((await api("DELETE", "tracked-repos/e2e-user/second")).status, 204);
  });
}

async function disconnect() {
  console.log("Phase: disconnect");
  await step("disconnect deletes user data and revokes the grant", async () => {
    const res = await web("/api/account/disconnect", { method: "POST", headers: { origin: WEB } });
    const body = await res.json();
    assert.equal(res.status, 200, JSON.stringify(body));
    assert.deepEqual(body.data, { data_deleted: true, github_access_revoked: true });
  });
}

const phase = process.argv[2] ?? "main";
await ({ main, "after-restart": afterRestart, disconnect }[phase])();
console.log(`${phase}: ${passed} checks passed${process.exitCode ? ", SOME FAILED" : ""}`);
