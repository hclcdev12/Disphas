// Packages offered to signed-in customers (docs/site.md §4, §7): read from
// RELEASES_DIR, outside the web root, never from a path the visitor gives.
//   RELEASES_DIR/oblivio_0.1.0_amd64.deb
//   RELEASES_DIR/oblivio-0.1.0-1.x86_64.rpm
//   RELEASES_DIR/SHA256SUMS            "<sha256>  <file name>" per line
import { readFile, readdir, stat } from "node:fs/promises";
import { join } from "node:path";

export interface Release {
  name: string;
  kind: "deb" | "rpm";
  bytes: number;
  sha256: string | null;
}

/** Only these names are ever served: no path, no hidden file. */
const PACKAGE = /^oblivio[-_][0-9A-Za-z.+~_-]+\.(deb|rpm)$/;

function releasesDir(): string | null {
  return process.env.RELEASES_DIR ?? (import.meta.env.DEV ? ".data/releases" : null);
}

/** Packages available now, newest name first. */
export async function listReleases(): Promise<Release[]> {
  const dir = releasesDir();
  if (!dir) return [];
  const names = await readdir(dir).catch(() => [] as string[]);
  const sums = new Map<string, string>();
  const text = await readFile(join(dir, "SHA256SUMS"), "utf8").catch(() => "");
  for (const line of text.split("\n")) {
    const match = /^([0-9a-f]{64}) [ *](\S+)$/.exec(line.trim());
    if (match) sums.set(match[2], match[1]);
  }
  const releases: Release[] = [];
  for (const name of names.filter((name) => PACKAGE.test(name)).sort().reverse()) {
    const info = await stat(join(dir, name)).catch(() => null);
    if (!info?.isFile()) continue;
    releases.push({ name, kind: name.endsWith(".deb") ? "deb" : "rpm", bytes: info.size, sha256: sums.get(name) ?? null });
  }
  return releases;
}

/** Absolute path of `name` if it is a listed package, otherwise null. */
export async function releasePath(name: string): Promise<string | null> {
  const dir = releasesDir();
  if (!dir || !PACKAGE.test(name)) return null;
  const listed = await listReleases();
  return listed.some((release) => release.name === name) ? join(dir, name) : null;
}
