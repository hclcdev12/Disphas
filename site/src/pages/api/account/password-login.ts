// Sign-in by password (optional, docs/site.md §8). One answer for a wrong
// address or a wrong password; attempts limited per address and per email.
import type { APIRoute } from "astro";
import { authHeaders, back, readForm } from "../../../lib/account-form";
import { auth } from "../../../lib/auth";
import { clientIp, limiter } from "../../../lib/limits";

export const prerender = false;

const perAddress = limiter(30, 60 * 60 * 1000);
const perEmail = limiter(5, 15 * 60 * 1000);

export const POST: APIRoute = async ({ request, clientAddress }) => {
  const { form, locale } = await readForm(request);
  const login = `/${locale}/account/login/`;
  const email = String(form?.get("email") ?? "").trim().toLowerCase();
  const password = String(form?.get("password") ?? "");
  if (!email || !password || email.length > 254 || password.length > 1024) return back(`${login}?status=wrong`);
  if (!perAddress(clientIp(request, clientAddress)) || !perEmail(email)) return back(`${login}?status=limited`);
  const response = await auth.api.signInEmail({
    body: { email, password, rememberMe: true },
    headers: authHeaders(request, clientAddress),
    asResponse: true,
  });
  return response.ok ? back(`/${locale}/account/`, response) : back(`${login}?status=wrong`);
};
