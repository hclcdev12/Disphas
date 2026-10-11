// Forgotten password: Better Auth emails a reset link if the account exists,
// and answers the same either way. Limited per address and per email.
import type { APIRoute } from "astro";
import { authHeaders, back, readForm } from "../../../lib/account-form";
import { auth } from "../../../lib/auth";
import { clientIp, limiter } from "../../../lib/limits";

export const prerender = false;

const perAddress = limiter(10, 60 * 60 * 1000);
const perEmail = limiter(3, 60 * 60 * 1000);
const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export const POST: APIRoute = async ({ request, clientAddress }) => {
  const { form, locale } = await readForm(request);
  const forgot = `/${locale}/account/forgot/`;
  const email = String(form?.get("email") ?? "").trim().toLowerCase();
  if (!EMAIL.test(email) || email.length > 254) return back(`${forgot}?status=invalid`);
  if (!perAddress(clientIp(request, clientAddress)) || !perEmail(email)) return back(`${forgot}?status=limited`);
  try {
    await auth.api.requestPasswordReset({
      body: { email, redirectTo: `/${locale}/account/reset/` },
      headers: authHeaders(request, clientAddress),
    });
  } catch (error) {
    console.error(`[account] reset request failed: ${error instanceof Error ? error.message : String(error)}`);
    return back(`${forgot}?status=failed`);
  }
  return back(`${forgot}?status=reset-sent`);
};
