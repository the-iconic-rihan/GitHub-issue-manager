# End-to-end tests

`run.sh` starts the **real** website (Next.js standalone build), the **real** API server
(release build) and uses a **real** PostgreSQL database. Only GitHub is replaced, by
`fake-github.mjs`: a stateful fake of the OAuth web flow and the REST endpoints the app uses,
including Link-header pagination and ETag / `If-None-Match` (304) responses.

| File | Purpose |
|------|---------|
| `run.sh` | Builds, starts everything, runs all phases, cleans up; prints logs on failure |
| `workflows.mjs` | HTTP-level checks through the website: OAuth sign-in, repositories, track/sync, filters/search/sort/pagination, details, create, edit, assign, labels, comments, close/reopen, dashboard, CSRF, persistence after an API restart, automatic sync, disconnect |
| `browser_test.py` | Headless Firefox drives the UI; fails on any console error/warning or uncaught JS error; checks a 390 px viewport for horizontal overflow; saves screenshots |
| `fake-github.mjs` | The fake GitHub |

## Running

Requirements: Rust, Node.js ≥ 20.9, Python 3 with `pip install -r e2e/requirements.txt`, and
Firefox + geckodriver for the browser test. Use a **disposable** database: the test creates
tables and writes rows.

```bash
createdb gim_e2e                      # or any empty database
DATABASE_URL=postgres://postgres:postgres@localhost:5432/gim_e2e e2e/run.sh

SKIP_BUILD=1 ...    # reuse existing builds
SKIP_BROWSER=1 ...  # HTTP checks only
```

Logs (`fake-github.log`, `api.log`, `website.log`) and screenshots go to `e2e/artifacts/`
(git-ignored). CI runs this suite in the `e2e` job and uploads the artifacts.
