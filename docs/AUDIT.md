# Audit report – GitHub Issue Manager

Audit of `master` at `be6a637` (2026-11-17), plus the unmerged branches `origin/Shreyas`
(Rust middleware, gi-cli, Kubernetes manifests) and `origin/rihan` (GitHub sign-in attempt).
Every finding below was verified by reading, building or running the code. All are **Fixed** on
branch `fix/end-to-end-app` unless stated otherwise.

**Starting point:** `master` was a scaffold. The Rust workspace had no source files (no API
existed), the website was a single non-functional login page, and there were no Dockerfiles. The
only backend logic lived on the unmerged `Shreyas` branch. That work was merged and fixed; the
missing API server, website features and deployment assets were built on the existing stack
(Rust/actix-web/sqlx/PostgreSQL, Next.js/next-auth/Tailwind, Kubernetes).

## Critical

| ID | File / component | Problem | Fix |
|----|------------------|---------|-----|
| C-01 | `app/` workspace | `server` and `middleware` crates had no `main.rs`/`lib.rs`; `cargo build` failed and no API existed | Merged middleware from `Shreyas`; implemented the `server` crate (routes → services → middleware) |
| C-02 | Frontend | No functionality beyond a login card; every button linked to `"http://"` | Built dashboard, repositories, issue list/detail/create, Discover and account pages wired to the real API |
| C-03 | Deployment | No Dockerfiles; README built from a non-existent `website/` directory | Added multi-stage non-root Dockerfiles for both services, `docker-compose.yml`, corrected README |
| C-04 | `k8s/database/postgresdb-configmap.yaml` | Init script ran `CREATE DATABASE` for the DB the entrypoint already created (with `ON_ERROR_STOP`), misused `--password`, and called the absent `sem-apply`, so Postgres failed on first start | Removed. The API applies sqlx migrations on startup |
| C-05 | `k8s/*networkpolicy.yaml` | Duplicate `port:` keys (invalid YAML semantics); "client-side" policy targeted the server pods | Rewrote policies: DB ← server, server ← website + ingress, website ← ingress |
| C-06 | `k8s/server/server-deployment.yaml` | API pods mounted the **database's** RWO volume at `/app/target` (data-corruption risk, scheduling failures) | Removed the mount; server is stateless with a read-only root filesystem |
| C-07 | `Frontend/issuemanager` dependencies | `next@13.0.5` and every release ≤ 16.3 carry critical advisories (e.g. CVE-2025-29927 middleware bypass, Image Optimizer RCE) | Upgraded to Next 16.4.6 / React 19; image optimizer disabled; `npm audit`: 0 vulnerabilities |

## High

| ID | File / component | Problem | Fix |
|----|------------------|---------|-----|
| H-01 | `middleware/src/routine.rs` | Logged the full database URL (password) and the GitHub token at debug level | Removed; secrets are redacted from `Debug` output; config log lists only non-secret fields |
| H-02 | `middleware/src/routine.rs` | `remove()` called `Issue::retrieve` (never deleted) | Fixed; batch `remove` |
| H-03 | `middleware/src/search.rs` | Pagination maths wrong (`floor`, last-page logic) and stop condition assumed newest-first results, but the query used GitHub's best-match order | Simple remaining-count loop; query sorts `created-desc`; stops at the newest stored `created_at` |
| H-04 | `middleware/src/issue.rs` | `retrieve_latest_id` ordered ascending (oldest), `update_timestamp` used `assert!` (panic) | Rewritten queries; no panics on data |
| H-05 | `middleware/src/search.rs` | An issue without labels aborted the whole page | Missing labels become an empty list |
| H-06 | `middleware/src/status.rs` | One GraphQL request per stored issue; GraphQL `errors` ignored; deleted issues never pruned | Batched `nodes(ids:)` (100 per request), errors surfaced, unresolvable nodes removed, rate-limit headroom respected |
| H-07 | `pages/_app.tsx` | `session={pageProps.session}` rendered as literal text inside `SessionProvider` | Proper `SessionProvider` + auth guard |
| H-08 | Root `package.json`/`node_modules` | `next-auth` installed at repo root (not the app); 4,772 `node_modules` files committed; duplicate React | Removed root package; `node_modules`/`target`/`.env` git-ignored |
| H-09 | `gi-cli/src/cluster.rs`, `yaml.rs` | Namespace `github_issue_manager` is an invalid Kubernetes name; manifest paths did not exist | Uses `githubissuemanager`; manifest lists match `k8s/`; regression test added |
| H-10 | `k8s` Secret | Deployments referenced `githubissuemanager-secret`, which was never defined or documented | Template `k8s/githubissuemanager-secret.env.example` + README instructions; optional keys marked `optional` |
| H-11 | `app/middleware/Cargo.toml` | `sqlx 0.5` (RUSTSEC-2024-0363), `native-tls` | sqlx 0.8 with rustls; reqwest 0.12 |

## Medium

| ID | File / component | Problem | Fix |
|----|------------------|---------|-----|
| M-01 | `k8s-extras/…persistentvolume.yaml` | `persistentVolumeReclaimPolicy: Recycle` wipes data when the claim is released; hostPath/no-provisioner setup is minikube-only | Moved to optional `k8s-extras/` with `Retain`; PVC uses the default StorageClass |
| M-02 | `k8s` workloads | No probes, resources, security contexts; `postgres:alpine` unpinned major; `imagePullPolicy: Never` | Startup/readiness/liveness probes, requests/limits, non-root, read-only rootfs, `postgres:16-alpine`, `IfNotPresent` |
| M-03 | `styles/globals.css` | Google Fonts `@import` after `@tailwind` rules is ignored by browsers | Moved to the top |
| M-04 | `pages/Login.js` | Component inside `pages/` (also routed at `/Login`); `linkedIn.svg` case mismatch; `<link>` inside `<title>`; stray `;` rendered | Replaced by `pages/login.tsx`; unused assets removed |
| M-05 | `.eslintrc.json` | Extended invalid `next/babel` config | ESLint 9 flat config with `next/core-web-vitals` + TypeScript rules |
| M-06 | `origin/rihan` branch | GitHub provider attempt crashed (`useSession` never imported) | Superseded by the new sign-in implementation |
| M-07 | Ingress | No `ingressClassName` | `ingressClassName: nginx`, body-size limit |

## Low

| ID | File / component | Problem | Fix |
|----|------------------|---------|-----|
| L-01 | Login page | Google, LinkedIn and email/password controls never worked and cannot grant GitHub access | Removed (decision: GitHub-only sign-in) |
| L-02 | `pages/api/hello.ts`, `public/vercel.svg` | Scaffold leftovers | Removed |
| L-03 | `gi-cli` | Compiler dead-code warnings, unused function | Fixed; remaining clippy *style* lints in original gi-cli code left unchanged (**Open**, cosmetic) |
| L-04 | Tests / CI | No tests and no CI | 63 Rust tests, 34 Jest tests, gi-cli test, GitHub Actions workflow |

## Follow-up hardening (production readiness)

| ID | Area | Problem | Fix | Status |
|----|------|---------|-----|--------|
| F-01 | Auth (`server/src/auth.rs`) | Concurrent requests on a cold cache each called `GET /user` (a page load fires several) | Single-flight validation: concurrent callers share one GitHub request; only successes cached | Fixed |
| F-02 | Auth (`server/src/lib.rs`) | A token revoked on GitHub stayed trusted from cache for up to the TTL | Any 401 response drops that token's cache entry immediately | Fixed |
| F-03 | Sync | Dashboard went stale until the user pressed Sync; every sync re-downloaded everything | Automatic sync of stale repositories (`POST /tracked-repos/sync-stale`) driven by the open dashboard, using conditional requests (ETag/304, no rate-limit cost); additive migration `20261118000000` | Fixed |
| F-04 | Testing | The full-stack E2E harness lived outside the repository | `e2e/` suite (fake GitHub, workflow checks, headless browser) with `e2e/run.sh`, run in CI with artifacts | Fixed |

## Security controls added

- GitHub token stored only in the encrypted HttpOnly session cookie; never sent to the browser (verified E2E).
- Backend validates tokens with GitHub (cached by SHA-256 digest); all GitHub calls use the user's token; all local rows are scoped to the user id (cross-user isolation test).
- CSRF: state-changing proxy requests require a matching `Origin`; SameSite cookies; JSON-only bodies.
- Strict input validation (owner/repo names, label names, limits); URL path segments percent-encoded; search qualifiers cannot widen the repository scope.
- Parameterised SQL only; no HTML rendering of user content (plain text); CSP, frame-ancestors none, nosniff headers.
- Operator tokens compared in constant time; internal errors logged but never returned to clients.

## Verification performed

| Area | Result |
|------|--------|
| Rust: fmt, clippy `-D warnings` | clean |
| Rust: 23 middleware unit, 4 Discover DB, 16 server unit, 15 HTTP API, 11 HTTP+DB tests | 69/69 passed (PostgreSQL 16) |
| Website: ESLint, `tsc`, production build, `npm audit` | clean, 0 vulnerabilities |
| Website: Jest | 39/39 passed |
| gi-cli test | 1/1 passed |
| Full stack E2E (`e2e/run.sh`: real website + API + PostgreSQL, fake GitHub) | 22/22 checks: OAuth sign-in, repositories, track/sync, filters/search/sort/pagination, details, create, edit, assign, labels, comments, close/reopen, dashboard, restart persistence, automatic sync, disconnect |
| Headless Firefox UI run (part of `e2e/run.sh`) | 8/8 flows; 0 console errors/warnings, 0 uncaught JS errors; no horizontal overflow at 390 px |
| Real github.com | Sign-in, profile, dashboard and repository listing verified with a real OAuth app |
| Kubernetes YAML | parsed and structurally checked locally; schema validation (kubeconform) runs in CI only |

**Not verified locally:** building the Docker images and running Compose/Kubernetes (no Docker or
cluster on the audit machine; CI builds the images and validates the manifests). Against real
github.com only sign-in and read paths were exercised; write workflows were verified with the
fake GitHub.
