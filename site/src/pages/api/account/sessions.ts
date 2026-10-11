// Open sessions: sign out one other session, or every other session. The
// page never holds session tokens: it names a session by its id, and the
// token is looked up here, among the visitor's own sessions only.
import type { APIRoute } from "astro";
import { authHeaders, back, readForm } from "../../../lib/account-form";
import { auth } from "../../../lib/auth";

export const prerender = false;

export const POST: APIRoute = async ({ request, clientAddress, locals }) => {
  const { form, locale } = await readForm(request);
  const account = `/${locale}/account/`;
  if (!locals.user || !locals.session) return back(`/${locale}/account/login/`);
  const headers = authHeaders(request, clientAddress);
  try {
    if (form?.get("action") === "others") {
      await auth.api.revokeOtherSessions({ headers });
      return back(`${account}?status=sessions-closed`);
    }
    const id = String(form?.get("id") ?? "");
    if (id === locals.session.id) return back(`${account}?status=failed`);
    const sessions = await auth.api.listSessions({ headers });
    const target = sessions.find((session) => session.id === id);
    if (!target) return back(`${account}?status=failed`);
    await auth.api.revokeSession({ body: { token: target.token }, headers });
  } catch (error) {
    console.error(`[account] session revocation failed: ${error instanceof Error ? error.message : String(error)}`);
    return back(`${account}?status=failed`);
  }
  return back(`${account}?status=session-closed`);
};
