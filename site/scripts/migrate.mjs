// Applies the SQL migrations (migrations/NNNN_name.sql) not applied yet, in
// order, each in a transaction, recorded in "schema_migrations".
// PostgreSQL with DATABASE_URL; otherwise the development PGlite database
// (.data/pglite, or PGLITE_DIR). Stop the development server first: PGlite
// allows one process at a time.
//
//   npm run migrate
import { readFile, readdir } from "node:fs/promises";

const dir = new URL("../migrations/", import.meta.url);
const files = (await readdir(dir)).filter((name) => /^\d{4}_[a-z0-9_]+\.sql$/.test(name)).sort();

async function client() {
  const url = process.env.DATABASE_URL;
  if (url) {
    const { default: postgres } = await import("postgres");
    const sql = postgres(url, { max: 1 });
    return {
      query: async (text, params = []) => sql.unsafe(text, params),
      transaction: (run) => sql.begin((tx) => run((text, params = []) => tx.unsafe(text, params))),
      close: () => sql.end(),
    };
  }
  const { PGlite } = await import("@electric-sql/pglite");
  const { mkdir } = await import("node:fs/promises");
  const path = process.env.PGLITE_DIR ?? ".data/pglite";
  await mkdir(path, { recursive: true, mode: 0o700 });
  const db = new PGlite(path);
  return {
    query: async (text, params = []) => (await db.query(text, params)).rows,
    transaction: (run) =>
      db.transaction((tx) => run(async (text, params = []) => (params.length ? tx.query(text, params) : tx.exec(text)))),
    close: () => db.close(),
  };
}

const db = await client();
try {
  await db.query(
    'CREATE TABLE IF NOT EXISTS "schema_migrations" ("name" text PRIMARY KEY, "applied_at" timestamptz NOT NULL DEFAULT now())',
  );
  const applied = new Set((await db.query('SELECT "name" FROM "schema_migrations"')).map((row) => row.name));
  let count = 0;
  for (const name of files) {
    if (applied.has(name)) continue;
    const text = await readFile(new URL(name, dir), "utf8");
    await db.transaction(async (run) => {
      await run(text);
      await run('INSERT INTO "schema_migrations" ("name") VALUES ($1)', [name]);
    });
    console.log(`applied ${name}`);
    count += 1;
  }
  console.log(count ? `${count} migration(s) applied` : "database up to date");
} finally {
  await db.close();
}
