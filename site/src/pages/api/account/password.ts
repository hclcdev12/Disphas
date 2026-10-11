// The optional password of a signed-in customer: set one, change it (other
// sessions signed out), or remove it (sign-in by email link only).
import { and, eq } from "drizzle-orm";
import type { APIRoute } from "astro";
import { account } from "../../../db/schema";
import { authHeaders, back, readForm } from "../../../lib/account-form";
import { auth } from "../../../lib/auth";
import { db } from "../../../lib/db";
import { limiter } from "../../../lib/limits";
import { verifyPassword, weakness } from "../../../lib/password";

export const prerender = false;

const perUser = limiter(10, 60 * 60 * 1000);

export const POST: APIRoute = async ({ request, clientAddress, locals }) => {
  const { form, locale } = await readForm(request);
  const page = `/${locale}/account/`;
  const user = locals.user;
  if (!user) return back(`/${locale}/account/login/`);
  if (!perUser(user.id)) return back(`${page}?status=limited`);
  const action = form?.get("action");
  const headers = authHeaders(request, clientAddress);
  const [credential] = await db
    .select()
    .from(account)
    .where(and(eq(account.userId, user.id), eq(account.providerId, "credential")));

  if (action === "remove") {
    // The current password is asked again: an unattended session alone
    // cannot change how the customer signs in.
    const current = String(form?.get("current") ?? "");
    if (!credential?.password || !(await verifyPassword({ hash: credential.password, password: current }))) {
      return back(`${page}?status=password-wrong`);
    }
    await db.delete(account).where(eq(account.id, credential.id));
    return back(`${page}?status=password-removed`);
  }

  const password = String(form?.get("password") ?? "");
  if (password !== String(form?.get("repeat") ?? "")) return back(`${page}?status=password-mismatch`);
  const weak = weakness(password, user.email);
  if (weak) return back(`${page}?status=password-${weak}`);

  try {
    if (action === "set" && !credential?.password) {
      await auth.api.setPassword({ body: { newPassword: password }, headers });
      return back(`${page}?status=password-set`);
    }
    if (action === "change" && credential?.password) {
      const response = await auth.api.changePassword({
        body: { currentPassword: String(form?.get("current") ?? ""), newPassword: password, revokeOtherSessions: true },
        headers,
        asResponse: true,
      });
      return response.ok
        ? back(`${page}?status=password-changed`, response)
        : back(`${page}?status=password-wrong`);
    }
  } catch (error) {
    console.error(`[account] password update failed: ${error instanceof Error ? error.message : String(error)}`);
  }
  return back(`${page}?status=failed`);
};
