// Reads the session for the customer area and the account endpoints only:
// static pages are never rendered on request, and other pages need no
// database access.
import { defineMiddleware } from "astro:middleware";

const ACCOUNT = /^\/(en|fr|de)\/account(\/|$)|^\/api\/(account|download)\//;

export const onRequest = defineMiddleware(async (context, next) => {
  context.locals.user = null;
  context.locals.session = null;
  if (!context.isPrerendered && ACCOUNT.test(context.url.pathname)) {
    const { auth } = await import("./lib/auth");
    const found = await auth.api.getSession({ headers: context.request.headers });
    context.locals.user = found?.user ?? null;
    context.locals.session = found?.session ?? null;
  }
  return next();
});
