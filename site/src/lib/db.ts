// Database: PostgreSQL in production (DATABASE_URL), PGlite in development
// (PostgreSQL compiled for Node, in .data/pglite: nothing to install).
// Same SQL, same migrations (migrations/, applied by `npm run migrate`).
import { drizzle as drizzlePostgres } from "drizzle-orm/postgres-js";
import type { PgDatabase, PgQueryResultHKT } from "drizzle-orm/pg-core";
import postgres from "postgres";
import * as schema from "../db/schema";

export type Database = PgDatabase<PgQueryResultHKT, typeof schema>;

async function connect(): Promise<Database> {
  const url = process.env.DATABASE_URL;
  if (url) {
    return drizzlePostgres(postgres(url, { max: 10 }), { schema }) as unknown as Database;
  }
  if (!import.meta.env.DEV) {
    throw new Error("DATABASE_URL is required outside development");
  }
  // Development only: never loaded when DATABASE_URL is set.
  const { PGlite } = await import("@electric-sql/pglite");
  const { drizzle } = await import("drizzle-orm/pglite");
  const { mkdir } = await import("node:fs/promises");
  const path = process.env.PGLITE_DIR ?? ".data/pglite";
  await mkdir(path, { recursive: true, mode: 0o700 });
  return drizzle(new PGlite(path), { schema }) as unknown as Database;
}

export const db = await connect();
