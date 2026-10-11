// Shared by the customer area's forms (src/pages/api/account/): the page to
// return to, and the headers to call Better Auth with on the visitor's behalf.
import { DEFAULT_LOCALE, isLocale } from "../i18n";
import { clientIp } from "./limits";

export async function readForm(request: Request): Promise<{ form: FormData | null; locale: string }> {
  const form = await request.formData().catch(() => null);
  const lang = form?.get("lang");
  return { form, locale: isLocale(typeof lang === "string" ? lang : undefined) ? (lang as string) : DEFAULT_LOCALE };
}

/** The visitor's cookies, with an address Better Auth can rate-limit by. */
export function authHeaders(request: Request, clientAddress: string): Headers {
  const headers = new Headers(request.headers);
  headers.set("x-forwarded-for", clientIp(request, clientAddress));
  return headers;
}

/** Redirect to `path` (303), copying the cookies Better Auth set. */
export function back(path: string, response?: Response): Response {
  const headers = new Headers({ Location: path });
  for (const cookie of response?.headers.getSetCookie() ?? []) headers.append("Set-Cookie", cookie);
  return new Response(null, { status: 303, headers });
}
