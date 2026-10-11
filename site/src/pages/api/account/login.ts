// Sign-in form, without JavaScript: asks Better Auth to email a link, then
// shows the same answer whether or not the address has an account (accounts
// are created on first sign-in). Rate limited per address and per email, so
// nobody can flood someone else's mailbox.
import type { APIRoute } from "astro";
import { DEFAULT_LOCALE, isLocale } from "../../../i18n";
import { auth } from "../../../lib/auth";
import { clientIp, limiter } from "../../../lib/limits";

export const prerender = false;

const perAddress = limiter(10, 60 * 60 * 1000);
const perEmail = limiter(3, 60 * 60 * 1000);
const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export const POST: APIRoute = async ({ request, clientAddress, redirect }) => {
  const form = await request.formData().catch(() => null);
  const lang = form?.get("lang");
  const locale = isLocale(typeof lang === "string" ? lang : undefined) ? (lang as string) : DEFAULT_LOCALE;
  const back = `/${locale}/account/login/`;
  const email = String(form?.get("email") ?? "").trim().toLowerCase();
  if (!EMAIL.test(email) || email.length > 254) {
    return redirect(`${back}?status=invalid`, 303);
  }
  const ip = clientIp(request, clientAddress);
  if (!perAddress(ip) || !perEmail(email)) {
    return redirect(`${back}?status=limited`, 303);
  }
  const headers = new Headers(request.headers);
  headers.set("x-forwarded-for", ip);
  try {
    await auth.api.signInMagicLink({
      body: {
        email,
        callbackURL: `/${locale}/account/`,
        errorCallbackURL: `${back}?status=expired`,
      },
      headers,
    });
  } catch (error) {
    console.error(`[account] sign-in link failed: ${error instanceof Error ? error.message : String(error)}`);
    return redirect(`${back}?status=failed`, 303);
  }
  return redirect(`${back}?status=sent`, 303);
};
