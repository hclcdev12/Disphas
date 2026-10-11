// Change of email address (two steps, src/lib/auth.ts): the request sends a
// confirmation link to the current address. Signed-in customers only.
import type { APIRoute } from "astro";
import { authHeaders, back, readForm } from "../../../lib/account-form";
import { auth } from "../../../lib/auth";
import { limiter } from "../../../lib/limits";

export const prerender = false;

const perUser = limiter(3, 60 * 60 * 1000);
const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export const POST: APIRoute = async ({ request, clientAddress, locals }) => {
  const { form, locale } = await readForm(request);
  const account = `/${locale}/account/`;
  if (!locals.user) return back(`/${locale}/account/login/`);
  const email = String(form?.get("email") ?? "").trim().toLowerCase();
  if (!EMAIL.test(email) || email.length > 254) return back(`${account}?status=email-invalid`);
  if (email === locals.user.email) return back(`${account}?status=email-same`);
  if (!perUser(locals.user.id)) return back(`${account}?status=limited`);
  try {
    await auth.api.changeEmail({
      body: { newEmail: email, callbackURL: `${account}?status=email-step` },
      headers: authHeaders(request, clientAddress),
    });
  } catch (error) {
    console.error(`[account] change of email failed: ${error instanceof Error ? error.message : String(error)}`);
    return back(`${account}?status=failed`);
  }
  return back(`${account}?status=email-sent`);
};
