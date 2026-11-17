# GitCore website

Next.js (pages router) front end of GitHub Issue Manager. See the
[repository README](../../README.md) for architecture, configuration and deployment.

```bash
npm install
cp ../../.env.example .env.local   # keep only the website variables, fill them in
npm run dev                        # http://localhost:3000
npm run lint && npm run typecheck && npm test && npm run build
```

- `pages/api/auth/[...nextauth].ts` – GitHub sign-in (token kept server-side in the session JWT)
- `pages/api/backend/[...path].ts` – authenticated, CSRF-checked proxy to the Rust API
- `pages/api/account/disconnect.ts` – deletes stored data and revokes the OAuth grant
- `lib/` – API client, data hook, validation, formatting; `components/` – UI
