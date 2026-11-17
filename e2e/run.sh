#!/usr/bin/env bash
# Full-stack end-to-end test: real website (Next.js standalone) + real API server + real
# PostgreSQL, with a fake GitHub (OAuth + REST). See e2e/README.md.
#
# Required:  DATABASE_URL   a DISPOSABLE PostgreSQL database (tables are created and written)
# Optional:  SKIP_BUILD=1   reuse existing builds (app/target/release/server, .next/standalone)
#            SKIP_BROWSER=1 skip the headless Firefox UI test
#            E2E_FAKE_PORT / E2E_API_PORT / E2E_WEB_PORT (defaults 14010 / 18080 / 13000)
#            E2E_ARTIFACTS  directory for logs and screenshots (default e2e/artifacts)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
: "${DATABASE_URL:?set DATABASE_URL to a disposable PostgreSQL database}"
FAKE_PORT="${E2E_FAKE_PORT:-14010}"
API_PORT="${E2E_API_PORT:-18080}"
WEB_PORT="${E2E_WEB_PORT:-13000}"
ARTIFACTS="${E2E_ARTIFACTS:-$ROOT/e2e/artifacts}"
STATE_DIR="$(mktemp -d)"
mkdir -p "$ARTIFACTS"

export E2E_WEB_URL="http://localhost:$WEB_PORT"
export E2E_API_URL="http://127.0.0.1:$API_PORT"
export E2E_STATE_DIR="$STATE_DIR"
export E2E_ARTIFACTS="$ARTIFACTS"
FAKE_URL="http://127.0.0.1:$FAKE_PORT"
STANDALONE="$ROOT/Frontend/issuemanager/.next/standalone"
SERVER_BIN="$ROOT/app/target/release/server"

PIDS=()
cleanup() {
  local status=$?
  for pid in "${PIDS[@]:-}"; do
    [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
  rm -rf "$STATE_DIR"
  if [ "$status" -ne 0 ]; then
    echo
    echo "E2E FAILED (exit $status). Last log lines:"
    for log in fake-github api website; do
      [ -f "$ARTIFACTS/$log.log" ] && { echo "--- $log"; tail -n 25 "$ARTIFACTS/$log.log"; }
    done
  fi
  exit "$status"
}
trap cleanup EXIT INT TERM

log() { printf '\n==> %s\n' "$*"; }

wait_for() {
  local url=$1 name=$2
  for _ in $(seq 1 120); do
    if curl -fsS -o /dev/null "$url" 2>/dev/null; then return 0; fi
    sleep 0.5
  done
  echo "$name did not become ready at $url" >&2
  return 1
}

for port in "$FAKE_PORT" "$API_PORT" "$WEB_PORT"; do
  if curl -s -o /dev/null --max-time 1 "http://127.0.0.1:$port/"; then
    echo "port $port is already in use; set E2E_*_PORT" >&2
    exit 1
  fi
done

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  log "Building API server (release)"
  (cd "$ROOT/app" && cargo build --release --locked -p server)
  log "Building website (standalone)"
  (cd "$ROOT/Frontend/issuemanager" && NEXT_TELEMETRY_DISABLED=1 npm run build)
fi
# The standalone bundle needs the static assets next to it (as in the Dockerfile).
rm -rf "$STANDALONE/public" "$STANDALONE/.next/static"
cp -r "$ROOT/Frontend/issuemanager/public" "$STANDALONE/public"
cp -r "$ROOT/Frontend/issuemanager/.next/static" "$STANDALONE/.next/static"

FAKE_PID=""
API_PID=""
start_fake() {
  FAKE_GH_PORT="$FAKE_PORT" node "$ROOT/e2e/fake-github.mjs" >>"$ARTIFACTS/fake-github.log" 2>&1 &
  FAKE_PID=$!
  PIDS+=("$FAKE_PID")
  wait_for "$FAKE_URL/__calls" "fake GitHub"
}
start_api() {
  DATABASE_URL="$DATABASE_URL" GITHUB_API_URL="$FAKE_URL" BIND_ADDRESS="127.0.0.1:$API_PORT" \
    LOG_FORMAT=json RUST_LOG="info,sqlx=warn" "$SERVER_BIN" >>"$ARTIFACTS/api.log" 2>&1 &
  API_PID=$!
  PIDS+=("$API_PID")
  wait_for "$E2E_API_URL/api/v1/health/ready" "API server"
}
stop() { kill "$1" 2>/dev/null || true; wait "$1" 2>/dev/null || true; }

: >"$ARTIFACTS/fake-github.log"; : >"$ARTIFACTS/api.log"; : >"$ARTIFACTS/website.log"

log "Starting fake GitHub, API server and website"
start_fake
start_api
(
  cd "$STANDALONE"
  NODE_ENV=production PORT="$WEB_PORT" HOSTNAME=127.0.0.1 \
    NEXTAUTH_URL="$E2E_WEB_URL" NEXTAUTH_SECRET="e2e-only-secret-$(date +%s)-not-for-production" \
    GITHUB_ID=e2e-client GITHUB_SECRET=e2e-client-secret \
    GITHUB_WEB_URL="$FAKE_URL" GITHUB_API_URL="$FAKE_URL" BACKEND_URL="$E2E_API_URL" \
    exec node server.js
) >>"$ARTIFACTS/website.log" 2>&1 &
PIDS+=("$!")
wait_for "$E2E_WEB_URL/api/health" "website"

log "Workflows"
node "$ROOT/e2e/workflows.mjs" main

log "Restarting the API server (persistence check)"
stop "$API_PID"
start_api
node "$ROOT/e2e/workflows.mjs" after-restart
node "$ROOT/e2e/workflows.mjs" disconnect

if [ "${SKIP_BROWSER:-0}" != "1" ]; then
  log "Browser (headless Firefox) against a fresh fake GitHub"
  stop "$FAKE_PID"
  start_fake
  python3 "$ROOT/e2e/browser_test.py"
else
  log "Browser test skipped (SKIP_BROWSER=1)"
fi

log "All end-to-end checks passed. Logs and screenshots: $ARTIFACTS"
