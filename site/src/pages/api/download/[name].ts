// Serves a package to a signed-in customer only (docs/site.md §4).
import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { Readable } from "node:stream";
import type { APIRoute } from "astro";
import { releasePath } from "../../../lib/releases";

export const prerender = false;

export const GET: APIRoute = async ({ params, locals }) => {
  if (!locals.user) {
    return new Response("Sign in to download.", { status: 401 });
  }
  const path = await releasePath(params.name ?? "");
  if (!path) {
    return new Response("Not found.", { status: 404 });
  }
  const info = await stat(path);
  return new Response(Readable.toWeb(createReadStream(path)) as ReadableStream, {
    headers: {
      "Content-Type": "application/octet-stream",
      "Content-Length": String(info.size),
      "Content-Disposition": `attachment; filename="${params.name}"`,
      "Cache-Control": "private, no-store",
      "X-Content-Type-Options": "nosniff",
    },
  });
};
