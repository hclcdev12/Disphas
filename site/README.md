# disphas website

Self-contained: everything the site needs lives in this folder (fonts, logos,
colors, texts). Architecture and decisions: `../docs/site.md`.

| Command | Effect |
|---|---|
| `npm ci` | Install dependencies (Node 24) |
| `npm run dev` | Development server on http://localhost:4321 |
| `npm run build` | Build into `dist/` |
| `npm start` | Run the build (`HOST`, `PORT`; behind Caddy in production) |
| `npm run check` | Type and template checks |
| `npm run migrate` | Apply `migrations/*.sql` (PostgreSQL with `DATABASE_URL`, else `.data/pglite`; stop the dev server first) |

Customer accounts: Better Auth, sign-in by email link (`src/lib/auth.ts`).
Development needs nothing installed: PGlite in `.data/` (ignored by git),
links printed in the server log, packages to test in `.data/releases/`.
Production environment: `DATABASE_URL`, `BETTER_AUTH_SECRET`, `BETTER_AUTH_URL`,
`SMTP_URL`, `MAIL_FROM`, `CONTACT_TO`, `RELEASES_DIR`, `TRUST_PROXY=1` (behind Caddy).

Interactive demo: a standalone folder, `../demo/`, built by `../tools/demo/build.sh`
(application interface on a simulated helper, fictional certificates, audited
before copy). The site no longer holds it: to serve it at `/demo/app/`, copy
`../demo/` into `public/demo/app/` (ignored by git) before a build.

Astro telemetry is disabled in every script (`ASTRO_TELEMETRY_DISABLED=1`).

- Languages: `/en/`, `/fr/`, `/de/`; `/` redirects to the browser language,
  English otherwise (`src/pages/index.astro`).
- Texts: `src/i18n/index.ts`. French is the source; English and German are to
  be proofread.
- Charter colors: `src/styles/global.css` (same values as the application).
- Fonts: `public/fonts/` (SIL Open Font License, licenses alongside), copied
  from `assets/fonts/`. Logos: `public/`, copied from `assets/logo/`.
