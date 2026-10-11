// Deletion of the account (right to erasure): after an explicit
// confirmation, Better Auth emails a link to the account's address; the
// account is deleted when that link is opened in the signed-in browser.
import type { APIRoute } from "astro";
import { authHeaders, back, readForm } from "../../../lib/account-form";
import { auth } from "../../../lib/auth";
import { limiter } from "../../../lib/limits";

export const prerender = false;

const perUser = limiter(3, 60 * 60 * 1000);

export const POST: APIRoute = async ({ request, clientAddress, locals }) => {
  const { form, locale } = await readForm(request);
  const account = `/${locale}/account/`;
  if (!locals.user) return back(`/${locale}/account/login/`);
  if (form?.get("confirm") !== "yes") return back(`${account}?status=delete-unconfirmed`);
  if (!perUser(locals.user.id)) return back(`${account}?status=limited`);
  try {
    await auth.api.deleteUser({
      body: { callbackURL: `/${locale}/account/login/?status=deleted` },
      headers: authHeaders(request, clientAddress),
    });
  } catch (error) {
    console.error(`[account] deletion request failed: ${error instanceof Error ? error.message : String(error)}`);
    return back(`${account}?status=failed`);
  }
  return back(`${account}?status=delete-sent`);
};
