# GitHub Issue Manager (GitCore)

Manage GitHub issues across your repositories from one place: sign in with GitHub, pick the
repositories you care about, filter/search/sort their issues, create and edit issues, assign
people, manage labels, comment, close and reopen — and see dashboard statistics for the
repositories you track. An optional **Discover** page lists beginner-friendly open issues
("good first issue", "help wanted", …) collected from across GitHub.

- [Architecture](#architecture)
- [Technology stack](#technology-stack)
- [Prerequisites](#prerequisites)
- [Configuration](#configuration)
- [GitHub OAuth setup](#github-oauth-setup)
- [Local development](#local-development)
- [Docker deployment](#docker-deployment)
- [Kubernetes deployment](#kubernetes-deployment)
- [Running tests](#running-tests)
- [API reference](#api-reference)
- [Troubleshooting](#troubleshooting)
- [Known limitations](#known-limitations)

## Architecture

```
 Browser ──► website (Next.js, :3000) ──────────────► API server (Rust/actix-web, :8080) ──► GitHub REST/GraphQL
               │ next-auth: "Sign in with GitHub"         │ validates the user's token with GitHub
               │ /api/backend/* proxy adds the            │ every GitHub call uses the user's own token
               │ user's GitHub token server-side          │
               └ token lives only in an encrypted,        └──► PostgreSQL (users, tracked repositories,
                 HttpOnly session cookie                        synced issue snapshots, Discover issues)
```

- **GitHub is the source of truth.** Issue lists, details, comments and all edits go to GitHub
  live with the signed-in user's token, so GitHub enforces repository permissions.
- **PostgreSQL** stores per-user data: the user record, *tracked repositories*, and a snapshot of
  their issues that powers the dashboard. Every row is scoped to the user's GitHub id. The
  snapshot is kept current three ways: write-through after each edit made in the app, the
  **Sync** button (full download), and **automatic sync** — while the dashboard is open it asks
  the API to refresh snapshots older than `AUTO_SYNC_STALE_SECS` using conditional requests
  (`If-None-Match`; GitHub answers `304` for unchanged repositories at no rate-limit cost).
- **Discover** (optional) is a background aggregator that uses a server-owned token
  (`GITHUB_ACCESS_TOKEN`) and the GraphQL API to collect beginner-friendly issues into the
  `issues` table, and periodically prunes closed ones.
- **gi-cli** (`gi-cli/`) is an operator CLI that bootstraps the Kubernetes manifests and sends
  Discover maintenance commands (`POST /api/v1/command`).

Repository layout:

| Path | Contents |
|------|----------|
| `app/middleware` | Rust library: GitHub REST client, Discover aggregator (GraphQL), persistence, SQL migrations |
| `app/server` | Rust HTTP API (routes → services → middleware), integration tests |
| `Frontend/issuemanager` | Next.js website (pages router), next-auth, API proxy, Jest tests |
| `gi-cli` | Operator CLI (cluster bootstrap, Discover commands) |
| `k8s/` | Kubernetes manifests (`kubectl apply -R -f k8s/`) |
| `k8s-extras/` | Optional manifests (static hostPath volume for clusters without a provisioner) |
| `docker-compose.yml` | Full stack for local/single-host deployment |
| `e2e/` | Full-stack end-to-end suite (fake GitHub, workflow checks, headless browser test) |
| `docs/AUDIT.md` | Audit report of the fixes made to this repository |

## Technology stack

- **API:** Rust 2021, actix-web 4, sqlx 0.8 (PostgreSQL, rustls), reqwest 0.12, graphql_client, tracing (JSON logs)
- **Website:** Next.js 16 (pages router), React 19, next-auth 4 (GitHub OAuth), Tailwind CSS 3, TypeScript
- **Database:** PostgreSQL 16
- **Tests:** cargo test + wiremock (fake GitHub), Jest + Testing Library
- **Deployment:** Docker (multi-stage, non-root images), Docker Compose, Kubernetes

## Prerequisites

- Rust ≥ 1.80 (`rustup`), Node.js ≥ 20.9 (22 LTS recommended), PostgreSQL 16 (or Docker)
- A GitHub OAuth App (see below)
- For containers: Docker 24+ with Compose v2. For Kubernetes: a cluster with an ingress-nginx controller.

## Configuration

All variables are documented in [`.env.example`](.env.example). Copy it to `.env` (git-ignored)
and fill in the values. Never commit real credentials.

| Variable | Used by | Purpose |
|----------|---------|---------|
| `POSTGRES_DB`, `POSTGRES_USER`, `POSTGRES_PASSWORD` | db, API | Database credentials |
| `DATABASE_URL` *or* `POSTGRES_HOST`/`POSTGRES_PORT` | API | Where the API finds Postgres |
| `NEXTAUTH_URL` | website | Public URL of the website |
| `NEXTAUTH_SECRET` | website | Session encryption key (`openssl rand -base64 32`) |
| `GITHUB_ID`, `GITHUB_SECRET` | website | GitHub OAuth App credentials |
| `BACKEND_URL` | website | Internal URL of the API (`http://server:8080` in Compose/K8s) |
| `GITHUB_API_URL`, `GITHUB_WEB_URL` | both | Override for GitHub Enterprise Server |
| `GITHUB_OAUTH_SCOPES` | website | Default `read:user user:email repo`; `public_repo` restricts to public repos |
| `GITHUB_ACCESS_TOKEN` | API | Optional. Enables Discover (server-owned token, no scopes needed) |
| `DISCOVER_SYNC_INTERVAL_SECS` | API | Discover schedule (default 3600; `0` disables the background job) |
| `COMMANDER_ACCESS_TOKEN`, `ADMIN_ACCESS_TOKEN` | API | Optional operator tokens (≥ 16 chars) for `/api/v1/command` |
| `ISSUE_SYNC_MAX_PAGES` | API | Max pages of 100 issues fetched per repository sync (default 10) |
| `AUTO_SYNC_STALE_SECS`, `AUTO_SYNC_MAX_REPOS` | API | Automatic sync: refresh snapshots older than N seconds (default 900, min 60), at most M repositories per run (default 10) |
| `AUTH_CACHE_TTL_SECS` | API | How long a validated token is trusted (default 120); concurrent validations share one GitHub call, and a token GitHub rejects is dropped immediately |
| `LOG_FORMAT` (`text`/`json`), `RUST_LOG` | API | Logging |

## GitHub OAuth setup

1. GitHub → **Settings → Developer settings → OAuth Apps → New OAuth App**.
2. **Homepage URL:** your `NEXTAUTH_URL` (e.g. `http://localhost:3000`).
3. **Authorization callback URL:** `<NEXTAUTH_URL>/api/auth/callback/github`
   (e.g. `http://localhost:3000/api/auth/callback/github`).
4. Copy the client id to `GITHUB_ID`, generate a client secret into `GITHUB_SECRET`.

The app requests `read:user user:email repo` (needed to manage issues in private repositories).
**Account → Disconnect GitHub account** deletes everything the app stored for you and revokes
the app's OAuth grant on GitHub.

## Local development

```bash
# 1. Database (any PostgreSQL 16 works; this uses the compose service, published on 127.0.0.1:5432)
cp .env.example .env              # set POSTGRES_PASSWORD at least
docker compose up -d postgresdb

# 2. API server (applies migrations on startup)
cd app
export DATABASE_URL=postgres://githubissuemanager:<password>@localhost:5432/githubissuemanager
export LOG_FORMAT=text
cargo run -p server               # http://localhost:8080/api/v1/health/ready

# 3. Website (in another terminal)
cd Frontend/issuemanager
cat > .env.local <<'EOF'
NEXTAUTH_URL=http://localhost:3000
NEXTAUTH_SECRET=<openssl rand -base64 32>
GITHUB_ID=<oauth client id>
GITHUB_SECRET=<oauth client secret>
BACKEND_URL=http://localhost:8080
EOF
npm install
npm run dev                       # http://localhost:3000
```

`.env.local` is git-ignored. Database schema changes are additive sqlx migrations in
`app/middleware/migrations/` (applied automatically at startup; never edit an applied migration).

## Docker deployment

```bash
cp .env.example .env              # fill in POSTGRES_PASSWORD, NEXTAUTH_SECRET, GITHUB_ID, GITHUB_SECRET
docker compose up --build -d
docker compose ps                 # all services "healthy"
open http://localhost:3000
```

- Images: `githubissuemanager/server` (from `app/`) and `githubissuemanager/website`
  (from `Frontend/issuemanager/`). Both are multi-stage, run as non-root, and have health checks.
  Build individually with `docker build -t githubissuemanager/server:latest app/` and
  `docker build -t githubissuemanager/website:latest Frontend/issuemanager/`.
- Data is stored in the `postgres-data` volume and survives `docker compose down` / restarts.
  Only `docker compose down -v` deletes it.
- The API (`127.0.0.1:8080`) and Postgres (`127.0.0.1:5432`) are published on localhost only.

## Kubernetes deployment

The manifests are portable (default StorageClass, `IfNotPresent` image pulls) and assume an
ingress-nginx controller (`ingressClassName: nginx`).

1. **Build and publish the images** to a registry your cluster can pull from, and update the
   `image:` fields in `k8s/server/server-deployment.yaml` and `k8s/website/website-deployment.yaml`.
   With **minikube** you can load local images instead:

   ```bash
   docker build -t githubissuemanager/server:latest app/
   docker build -t githubissuemanager/website:latest Frontend/issuemanager/
   minikube image load githubissuemanager/server:latest
   minikube image load githubissuemanager/website:latest
   minikube addons enable ingress
   ```

2. **Configure:** set `NEXTAUTH_URL` in `k8s/setting/githubissuemanager-configmap.yaml` to the
   public URL users will open (and register `<NEXTAUTH_URL>/api/auth/callback/github` in the OAuth app).

3. **Create the namespace and Secret** (fill in a copy of the template *outside* the repository):

   ```bash
   kubectl apply -f k8s/00-namespace.yaml
   cp k8s/githubissuemanager-secret.env.example ~/githubissuemanager-secret.env   # edit it
   kubectl -n githubissuemanager create secret generic githubissuemanager-secret \
     --from-env-file="$HOME/githubissuemanager-secret.env"
   ```

4. **Deploy everything:**

   ```bash
   kubectl apply -R -f k8s/
   kubectl -n githubissuemanager rollout status statefulset/postgresdb
   kubectl -n githubissuemanager rollout status deployment/server
   kubectl -n githubissuemanager rollout status deployment/website
   ```

   or with gi-cli: `cargo run --manifest-path gi-cli/Cargo.toml -- cluster bootstrap -a apply -p k8s`
   (the Secret from step 3 must exist first).

5. **Browse:** the ingress serves the website at `/` and the API at `/api/v1`. On minikube,
   `minikube service -n ingress-nginx ingress-nginx-controller --url` prints the address
   (or run `minikube tunnel`).

**Persistence:** PostgreSQL stores its data on the `githubissuemanager-claim` PersistentVolumeClaim
(default StorageClass), so it survives pod restarts and redeployments. Deleting the claim or the
namespace deletes the data unless your StorageClass retains volumes. For clusters without a
dynamic provisioner see [`k8s-extras/static-hostpath-volume`](k8s-extras/static-hostpath-volume/README.md).

**Network policies** (`*-networkpolicy.yaml`) restrict Postgres to the API pods and the API to the
website pods and the `ingress-nginx` namespace. They are only enforced by CNIs that support
NetworkPolicy (e.g. `minikube start --cni=calico`).

**Operator commands** (Discover): with `COMMANDER_ACCESS_TOKEN` set in the Secret and in your shell,
`cargo run --manifest-path gi-cli/Cargo.toml -- control commander -c insert -l 100`.

## Running tests

```bash
# Rust (fmt, lint, unit + HTTP + database tests). Database tests need a PostgreSQL server;
# each test creates and drops its own schema inside DATABASE_URL's database.
cd app
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cargo test --lib --test api        # the subset that needs no database

# Website
cd Frontend/issuemanager
npm ci && npm run lint && npm run typecheck && npm test && npm run build

# gi-cli
cd gi-cli && cargo test
```

```bash
# Full-stack end-to-end suite (real website + API + PostgreSQL, fake GitHub, headless Firefox).
# Use a disposable database. See e2e/README.md.
pip install -r e2e/requirements.txt
DATABASE_URL=postgres://postgres:postgres@localhost:5432/gim_e2e e2e/run.sh
```

The database tests fail (rather than skip) when `DATABASE_URL` is not set. CI
(`.github/workflows/ci.yml`) runs all of the above (including the end-to-end suite, whose logs
and screenshots are uploaded as artifacts) against a PostgreSQL service, builds both
Docker images, validates `docker-compose.yml`, and validates the Kubernetes manifests with kubeconform.

## API reference

All endpoints are under `/api/v1` and (except health) require `Authorization: Bearer <GitHub OAuth token>`.
The website calls them through `/api/backend/<path>`, which adds the token. Responses are
`{"data": …, "meta": {page, per_page, has_next, total_count?}}`; errors are
`{"error": {"code", "message", "details"?}}` with precise HTTP statuses (401, 403, 404, 409, 422,
429 + `Retry-After` for GitHub rate limits, 502/504 when GitHub is unavailable).

| Method & path | Purpose |
|---------------|---------|
| `GET /health/live`, `GET /health/ready` | Liveness; readiness (database reachable) |
| `GET /me`, `DELETE /me` | Current user (records it); delete all stored data for the user |
| `GET /repos?page&per_page` | Repositories the user can access (+ `tracked` flag) |
| `GET /repos/{owner}/{repo}` | Repository details, permissions and tracking state |
| `GET /repos/{owner}/{repo}/labels`, `/assignees` | Pickers |
| `GET /repos/{owner}/{repo}/issues?state&labels&assignee&sort&direction&q&page&per_page` | List/search issues |
| `POST /repos/{owner}/{repo}/issues` | Create (`title`, `body`, `labels`, `assignees`) |
| `GET`/`PATCH /repos/{owner}/{repo}/issues/{n}` | Details; edit title/body, close/reopen (`state`, `state_reason`), set assignees/labels |
| `GET`/`POST /repos/{owner}/{repo}/issues/{n}/comments` | List / add comments |
| `POST /repos/{owner}/{repo}/issues/{n}/labels`, `DELETE …/labels?name=` | Add / remove labels |
| `GET /tracked-repos`, `PUT`/`DELETE /tracked-repos/{owner}/{repo}` | Tracked repositories |
| `POST /tracked-repos/{owner}/{repo}/sync` | Refresh the repository's issue snapshot (full download) |
| `POST /tracked-repos/sync-stale` | Conditionally refresh the user's stale snapshots; returns `updated`, `unchanged`, `failed`, `more_pending` |
| `GET /dashboard` | Statistics over tracked repositories |
| `GET /discover/issues?language&label&page&per_page`, `GET /discover/languages` | Discover |
| `POST /command` | Operator: `{"action":"insert"\|"update","target":"issues","limit":N}` with an operator token |

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Sign-in shows "Sign-in is not configured correctly" | Set `NEXTAUTH_SECRET`, `GITHUB_ID`, `GITHUB_SECRET`; restart the website |
| GitHub says *redirect_uri is not associated* | The OAuth app callback must be exactly `<NEXTAUTH_URL>/api/auth/callback/github` |
| Pages show "The API server is unavailable" | Check `BACKEND_URL` and the API logs; `curl <api>/api/v1/health/ready` |
| API exits with `configuration error` | A required variable is missing/invalid; the message names it |
| API logs `database not reachable yet; retrying` | Postgres is still starting or the host/credentials are wrong (it retries for ~60 s) |
| "GitHub API rate limit exceeded" | Wait for the time shown; the dashboard reads the local snapshot and keeps working |
| Dashboard numbers look stale | Snapshots refresh automatically while the dashboard is open (every `AUTO_SYNC_STALE_SECS`); press **Sync** to force a full refresh now |
| Postgres pod fails after changing the image | A data directory cannot be opened by a different PostgreSQL major version; keep `postgres:16` or dump/restore |
| Mutations return 403 "Cross-origin request rejected" | `NEXTAUTH_URL` must match the URL in the browser's address bar |

## Known limitations

- Changes made directly on GitHub reach the dashboard at the next automatic sync (when the
  snapshot is older than `AUTO_SYNC_STALE_SECS` and the dashboard is open) or a manual **Sync**;
  there is no webhook-driven real-time update. Syncs fetch at most `ISSUE_SYNC_MAX_PAGES` × 100
  issues (the most recently updated ones) and say so when truncated. Issues deleted on GitHub
  are removed by the next full (manual) sync.
- A cached token is trusted for up to `AUTH_CACHE_TTL_SECS` for endpoints that only read local
  data (dashboard, tracked repositories); any request that reaches GitHub with a revoked token
  invalidates it immediately.
- GitHub search returns at most 1,000 results; label/assignee pickers show the first 100 entries.
- Issue bodies and comments are shown as plain text (Markdown is not rendered), which also rules
  out HTML injection.
- Pull requests are intentionally excluded everywhere.
- The Discover background job runs in every API replica; keep one replica while it is enabled.
- Sign-in is GitHub-only (previous Google/LinkedIn/email buttons never worked and could not grant
  access to GitHub issues).
