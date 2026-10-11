// Receives the contact form (no JavaScript needed) and redirects back to the
// contact page with the outcome. Cross-site posts are refused by Astro's
// `security.checkOrigin` (on by default).
import type { APIRoute } from "astro";
import { DEFAULT_LOCALE, isLocale } from "../../i18n";
import { allowed, deliver, parse } from "../../lib/contact";
import { clientIp } from "../../lib/limits";

export const prerender = false;

export const POST: APIRoute = async ({ request, clientAddress, redirect }) => {
  const form = await request.formData().catch(() => null);
  const lang = form?.get("lang");
  const back = `/${isLocale(typeof lang === "string" ? lang : undefined) ? lang : DEFAULT_LOCALE}/contact/`;
  if (!form) {
    return redirect(`${back}?status=invalid`, 303);
  }
  // Honeypot: a field hidden from people, filled by bots.
  if (form.get("website")) {
    return redirect(`${back}?status=sent`, 303);
  }
  const message = parse(form);
  if (!message) {
    return redirect(`${back}?status=invalid`, 303);
  }
  if (!allowed(clientIp(request, clientAddress))) {
    return redirect(`${back}?status=failed`, 303);
  }
  try {
    await deliver(message);
  } catch (error) {
    console.error(`[contact] delivery failed: ${error instanceof Error ? error.message : String(error)}`);
    return redirect(`${back}?status=failed`, 303);
  }
  return redirect(`${back}?status=sent`, 303);
};
