// Signs out: Better Auth ends the session and clears its cookie; the
// visitor lands on the sign-in page.
import type { APIRoute } from "astro";
import { DEFAULT_LOCALE, isLocale } from "../../../i18n";
import { auth } from "../../../lib/auth";

export const prerender = false;

export const POST: APIRoute = async ({ request }) => {
  const form = await request.formData().catch(() => null);
  const lang = form?.get("lang");
  const locale = isLocale(typeof lang === "string" ? lang : undefined) ? (lang as string) : DEFAULT_LOCALE;
  const ended = await auth.api.signOut({ headers: request.headers, asResponse: true });
  const headers = new Headers({ Location: `/${locale}/account/login/?status=out` });
  for (const cookie of ended.headers.getSetCookie()) headers.append("Set-Cookie", cookie);
  return new Response(null, { status: 303, headers });
};
