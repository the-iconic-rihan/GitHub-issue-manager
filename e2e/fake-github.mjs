// Stateful fake of the GitHub OAuth + REST endpoints used by GitHub Issue Manager.
// For end-to-end tests only (see e2e/README.md). Implements the OAuth web flow, the REST
// endpoints the app calls, pagination via Link headers, and ETag / If-None-Match (304).
import crypto from "node:crypto";
import http from "node:http";

const PORT = Number(process.env.FAKE_GH_PORT || 4010);
const BASE = `http://127.0.0.1:${PORT}`;
const AVATAR = "data:image/svg+xml;base64," + Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#38bdf8"/></svg>`).toString("base64");
const TOKEN = "gho_e2e_token_abcdefghijklmnop";
const me = { id: 1001, login: "e2e-user", name: "E2E User", email: "e2e@example.test", avatar_url: AVATAR, html_url: "https://github.com/e2e-user" };
const helper = { id: 1002, login: "helper", avatar_url: AVATAR, html_url: "https://github.com/helper" };
const simple = (u) => ({ id: u.id, login: u.login, avatar_url: u.avatar_url, html_url: u.html_url });

const labels = [
  { id: 1, name: "bug", color: "d73a4a", description: "Something isn't working" },
  { id: 2, name: "enhancement", color: "a2eeef", description: null },
  { id: 3, name: "good first issue", color: "7057ff", description: null },
];
const repos = [
  { id: 501, name: "demo", full_name: "e2e-user/demo", owner: simple(me), private: false, description: "Demo repo", html_url: "https://github.com/e2e-user/demo", open_issues_count: 3, has_issues: true, archived: false, updated_at: "2026-11-16T00:00:00Z", permissions: { admin: true, maintain: true, push: true, triage: true, pull: true } },
  { id: 502, name: "second", full_name: "e2e-user/second", owner: simple(me), private: true, description: null, html_url: "https://github.com/e2e-user/second", open_issues_count: 0, has_issues: true, archived: false, updated_at: "2026-11-15T00:00:00Z", permissions: { admin: true, maintain: true, push: true, triage: true, pull: true } },
];
let clock = Date.parse("2026-11-10T00:00:00Z");
const tick = () => new Date((clock += 60_000)).toISOString();
const issues = { "e2e-user/demo": [], "e2e-user/second": [] };
const comments = {};
let nextId = 9000;
function addIssue(repo, fields) {
  const list = issues[repo];
  const number = list.length + 1;
  const now = tick();
  const issue = {
    id: nextId++, number, title: fields.title, body: fields.body ?? null, state: fields.state ?? "open", state_reason: null,
    user: simple(me), labels: (fields.labels ?? []).map((n) => labels.find((l) => l.name === n)).filter(Boolean),
    assignees: (fields.assignees ?? []).map((a) => simple(a === "helper" ? helper : me)), comments: 0,
    html_url: `https://github.com/${repo}/issues/${number}`, locked: false, created_at: now, updated_at: now,
    closed_at: fields.state === "closed" ? now : null, ...(fields.pr ? { pull_request: { url: "x" } } : {}),
  };
  list.push(issue);
  comments[`${repo}#${number}`] = [];
  return issue;
}
addIssue("e2e-user/demo", { title: "App crashes on start", body: "Stack trace...", labels: ["bug"], assignees: ["e2e-user"] });
addIssue("e2e-user/demo", { title: "Add dark mode", labels: ["enhancement"] });
addIssue("e2e-user/demo", { title: "Old closed bug", labels: ["bug"], state: "closed" });
addIssue("e2e-user/demo", { title: "A pull request", pr: true });
addIssue("e2e-user/demo", { title: "Typo in README", labels: ["good first issue"] });

const calls = [];
function send(res, status, body, headers = {}) {
  res.writeHead(status, { "content-type": "application/json", ...headers });
  res.end(body === undefined ? "" : JSON.stringify(body));
}
function readBody(req) {
  return new Promise((resolve) => {
    let data = "";
    req.on("data", (c) => (data += c));
    req.on("end", () => resolve(data));
  });
}
function paginate(req, res, url, items) {
  const page = Number(url.searchParams.get("page") || 1);
  const perPage = Number(url.searchParams.get("per_page") || 30);
  const slice = items.slice((page - 1) * perPage, page * perPage);
  const headers = { etag: `W/"${crypto.createHash("sha1").update(JSON.stringify(slice)).digest("hex")}"` };
  if (req.headers["if-none-match"] === headers.etag) {
    calls.push(`304 ${url.pathname}`);
    res.writeHead(304, headers);
    return res.end();
  }
  if (page * perPage < items.length) {
    const next = new URL(url);
    next.searchParams.set("page", String(page + 1));
    headers.link = `<${next}>; rel="next"`;
  }
  send(res, 200, slice, headers);
}
function sortIssues(list, sort, direction) {
  const key = sort === "updated" ? "updated_at" : sort === "comments" ? "comments" : "created_at";
  const sorted = [...list].sort((a, b) => (a[key] < b[key] ? -1 : a[key] > b[key] ? 1 : a.number - b.number));
  return direction === "asc" ? sorted : sorted.reverse();
}
function filterIssues(list, { state, labelNames, assignee }) {
  return list.filter((i) => {
    if (state && state !== "all" && i.state !== state) return false;
    if (labelNames.some((l) => !i.labels.some((x) => x.name === l))) return false;
    if (assignee === "none" && i.assignees.length) return false;
    if (assignee === "*" && !i.assignees.length) return false;
    if (assignee && assignee !== "none" && assignee !== "*" && !i.assignees.some((a) => a.login === assignee)) return false;
    return true;
  });
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, BASE);
  const path = decodeURIComponent(url.pathname);
  calls.push(`${req.method} ${url.pathname}`);

  // ---- OAuth (web) ----
  if (path === "/login/oauth/authorize") {
    const redirect = new URL(url.searchParams.get("redirect_uri"));
    redirect.searchParams.set("code", "e2e-code");
    if (url.searchParams.get("state")) redirect.searchParams.set("state", url.searchParams.get("state"));
    res.writeHead(302, { location: redirect.toString() });
    return res.end();
  }
  if (path === "/login/oauth/access_token" && req.method === "POST") {
    const body = new URLSearchParams(await readBody(req));
    if (body.get("code") !== "e2e-code") return send(res, 400, { error: "bad_verification_code" });
    return send(res, 200, { access_token: TOKEN, token_type: "bearer", scope: "read:user,user:email,repo" });
  }
  if (path === "/__calls") return send(res, 200, calls);
  if (path === "/avatar.png") {
    res.writeHead(200, { "content-type": "image/png" });
    return res.end(Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==", "base64"));
  }
  if (path.startsWith("/applications/") && req.method === "DELETE") {
    const auth = req.headers.authorization ?? "";
    return send(res, auth.startsWith("Basic ") ? 204 : 401, undefined);
  }

  // ---- REST API: requires the token ----
  if (req.headers.authorization !== `Bearer ${TOKEN}`) return send(res, 401, { message: "Bad credentials" });

  let m;
  if (path === "/user") return send(res, 200, me);
  if (path === "/user/emails") return send(res, 200, [{ email: me.email, primary: true }]);
  if (path === "/user/repos") return paginate(req, res, url, repos);
  if (path === "/search/issues") {
    const q = url.searchParams.get("q") ?? "";
    const repoName = /repo:(\S+)/.exec(q)?.[1];
    const state = /is:(open|closed)/.exec(q)?.[1];
    const labelNames = [...q.matchAll(/label:"([^"]+)"/g)].map((x) => x[1]);
    const assignee = /-no:assignee/.test(q) ? "*" : /no:assignee/.test(q) ? "none" : /assignee:(\S+)/.exec(q)?.[1];
    const words = q.replace(/(-?\w+:("[^"]*"|\S+))/g, "").trim().toLowerCase().split(/\s+/).filter(Boolean);
    let list = filterIssues((issues[repoName] ?? []).filter((i) => !i.pull_request), { state, labelNames, assignee });
    list = list.filter((i) => words.every((w) => `${i.title} ${i.body ?? ""}`.toLowerCase().includes(w)));
    list = sortIssues(list, url.searchParams.get("sort"), url.searchParams.get("order"));
    const page = Number(url.searchParams.get("page") || 1);
    const perPage = Number(url.searchParams.get("per_page") || 30);
    return send(res, 200, { total_count: list.length, incomplete_results: false, items: list.slice((page - 1) * perPage, page * perPage) });
  }
  if ((m = /^\/repos\/([^/]+)\/([^/]+)(\/.*)?$/.exec(path))) {
    const full = `${m[1]}/${m[2]}`;
    const repo = repos.find((r) => r.full_name === full);
    if (!repo) return send(res, 404, { message: "Not Found" });
    const rest = m[3] ?? "";
    if (rest === "") return send(res, 200, repo);
    if (rest === "/labels") return paginate(req, res, url, labels);
    if (rest === "/assignees") return paginate(req, res, url, [simple(me), simple(helper)]);
    if (rest === "/issues" && req.method === "GET") {
      const labelNames = (url.searchParams.get("labels") ?? "").split(",").filter(Boolean);
      const list = filterIssues(issues[full], { state: url.searchParams.get("state") ?? "open", labelNames, assignee: url.searchParams.get("assignee") });
      return paginate(req, res, url, sortIssues(list, url.searchParams.get("sort"), url.searchParams.get("direction")));
    }
    if (rest === "/issues" && req.method === "POST") {
      const body = JSON.parse(await readBody(req));
      if (!body.title) return send(res, 422, { message: "Validation Failed", errors: [{ resource: "Issue", field: "title", code: "missing_field" }] });
      return send(res, 201, addIssue(full, body));
    }
    if ((m = /^\/issues\/(\d+)(\/.*)?$/.exec(rest))) {
      const issue = issues[full][Number(m[1]) - 1];
      if (!issue) return send(res, 404, { message: "Not Found" });
      const sub = m[2] ?? "";
      const key = `${full}#${issue.number}`;
      if (sub === "" && req.method === "GET") return send(res, 200, issue);
      if (sub === "" && req.method === "PATCH") {
        const body = JSON.parse(await readBody(req));
        if (body.title !== undefined) issue.title = body.title;
        if (body.body !== undefined) issue.body = body.body;
        if (body.state) {
          issue.state = body.state;
          issue.state_reason = body.state_reason ?? (body.state === "closed" ? "completed" : "reopened");
          issue.closed_at = body.state === "closed" ? tick() : null;
        }
        if (body.assignees) issue.assignees = body.assignees.map((a) => simple(a === "helper" ? helper : me));
        if (body.labels) issue.labels = body.labels.map((n) => labels.find((l) => l.name === n)).filter(Boolean);
        issue.updated_at = tick();
        return send(res, 200, issue);
      }
      if (sub === "/comments" && req.method === "GET") return paginate(req, res, url, comments[key]);
      if (sub === "/comments" && req.method === "POST") {
        const body = JSON.parse(await readBody(req));
        const now = tick();
        const comment = { id: nextId++, body: body.body, user: simple(me), html_url: `${issue.html_url}#c`, author_association: "OWNER", created_at: now, updated_at: now };
        comments[key].push(comment);
        issue.comments += 1;
        issue.updated_at = now;
        return send(res, 201, comment);
      }
      if (sub === "/labels" && req.method === "POST") {
        const body = JSON.parse(await readBody(req));
        for (const n of body.labels) {
          const l = labels.find((x) => x.name === n);
          if (l && !issue.labels.some((x) => x.name === n)) issue.labels.push(l);
        }
        issue.updated_at = tick();
        return send(res, 200, issue.labels);
      }
      if (sub.startsWith("/labels/") && req.method === "DELETE") {
        const name = sub.slice("/labels/".length);
        if (!issue.labels.some((l) => l.name === name)) return send(res, 404, { message: "Label does not exist" });
        issue.labels = issue.labels.filter((l) => l.name !== name);
        issue.updated_at = tick();
        return send(res, 200, issue.labels);
      }
    }
  }
  send(res, 404, { message: "Not Found (fake)" });
});

server.listen(PORT, "127.0.0.1", () => console.log(`fake GitHub on ${BASE}`));
