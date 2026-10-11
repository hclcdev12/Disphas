// Better Auth endpoints (docs/site.md §2): the sign-in link lands here
// (/api/auth/magic-link/verify), which opens the session.
import type { APIRoute } from "astro";
import { auth } from "../../../lib/auth";
import { clientIp } from "../../../lib/limits";

export const prerender = false;

export const ALL: APIRoute = async ({ request, clientAddress }) => {
  // Better Auth rate-limits by X-Forwarded-For: set it ourselves, so a
  // visitor's own header never counts (lib/limits.ts decides what to trust).
  const headers = new Headers(request.headers);
  headers.set("x-forwarded-for", clientIp(request, clientAddress));
  return auth.handler(new Request(request, { headers }));
};
