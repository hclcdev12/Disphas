// New password from a reset link: same rules as everywhere (lib/password.ts);
// Better Auth then signs out every session of the account.
import type { APIRoute } from "astro";
import { authHeaders, back, readForm } from "../../../lib/account-form";
import { auth } from "../../../lib/auth";
import { weakness } from "../../../lib/password";

export const prerender = false;

export const POST: APIRoute = async ({ request, clientAddress }) => {
  const { form, locale } = await readForm(request);
  const token = String(form?.get("token") ?? "");
  const password = String(form?.get("password") ?? "");
  const again = `/${locale}/account/reset/?token=${encodeURIComponent(token)}`;
  if (!/^[A-Za-z0-9_-]{8,128}$/.test(token)) return back(`/${locale}/account/forgot/?status=expired`);
  if (password !== String(form?.get("repeat") ?? "")) return back(`${again}&status=password-mismatch`);
  const weak = weakness(password, "");
  if (weak) return back(`${again}&status=password-${weak}`);
  const response = await auth.api.resetPassword({
    body: { newPassword: password, token },
    headers: authHeaders(request, clientAddress),
    asResponse: true,
  });
  return response.ok ? back(`/${locale}/account/login/?status=reset`) : back(`/${locale}/account/forgot/?status=expired`);
};
