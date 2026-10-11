// Customer accounts (docs/site.md §2, §8): Better Auth, sign-in by a link
// sent by email, no password stored. Configuration from the environment:
//   BETTER_AUTH_SECRET  random secret, 32 bytes or more (session signing)
//   BETTER_AUTH_URL     public URL of the site, e.g. https://oblivio.eu
import { drizzleAdapter } from "@better-auth/drizzle-adapter";
import { betterAuth } from "better-auth";
import { magicLink } from "better-auth/plugins";
import * as schema from "../db/schema";
import { db } from "./db";
import { send } from "./mail";
import { signInMail } from "./sign-in-mail";

// Never run outside development with a default secret or a guessed URL.
if (!import.meta.env.DEV && (!process.env.BETTER_AUTH_SECRET || !process.env.BETTER_AUTH_URL)) {
  throw new Error("BETTER_AUTH_SECRET and BETTER_AUTH_URL are required outside development");
}

export const auth = betterAuth({
  baseURL: process.env.BETTER_AUTH_URL ?? "http://127.0.0.1:4321",
  database: drizzleAdapter(db, { provider: "pg", schema }),
  // Only the email link: no password, no third-party provider.
  emailAndPassword: { enabled: false },
  plugins: [
    magicLink({
      // Better Auth's default, stated: a link is valid 5 minutes, once.
      expiresIn: 300,
      sendMagicLink: async ({ email, url }) => {
        const mail = signInMail(url);
        await send({ to: email, subject: mail.subject, text: mail.text });
      },
    }),
  ],
  rateLimit: { enabled: true },
  // Off by default in Better Auth; stated so it never depends on the environment.
  telemetry: { enabled: false },
  advanced: {
    // Behind Caddy, the visitor's address comes from X-Forwarded-For.
    ipAddress: { ipAddressHeaders: ["x-forwarded-for"] },
  },
});
