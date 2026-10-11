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
import { MAX_LENGTH, MIN_LENGTH, hashPassword, verifyPassword } from "./password";
import { accountMail, confirmationUrl } from "./account-mail";

// Never run outside development with a default secret or a guessed URL.
if (!import.meta.env.DEV && (!process.env.BETTER_AUTH_SECRET || !process.env.BETTER_AUTH_URL)) {
  throw new Error("BETTER_AUTH_SECRET and BETTER_AUTH_URL are required outside development");
}

export const auth = betterAuth({
  baseURL: process.env.BETTER_AUTH_URL ?? "http://127.0.0.1:4321",
  database: drizzleAdapter(db, { provider: "pg", schema }),
  // A password is optional (docs/site.md §8): accounts are created by an
  // email link only, then a customer may add a password from the account.
  emailAndPassword: {
    enabled: true,
    disableSignUp: true,
    minPasswordLength: MIN_LENGTH,
    maxPasswordLength: MAX_LENGTH,
    password: { hash: hashPassword, verify: verifyPassword },
    resetPasswordTokenExpiresIn: 3600,
    revokeSessionsOnPasswordReset: true,
    sendResetPassword: async ({ user, url }) => {
      await send({ to: user.email, ...accountMail("resetPassword", url) });
    },
  },
  plugins: [
    magicLink({
      // Better Auth's default, stated: a link is valid 5 minutes, once.
      expiresIn: 300,
      // The email points to a confirmation page with a button: mail
      // scanners open links but do not submit forms, so they cannot use up
      // the single-use link before the customer (src/lib/account-mail.ts).
      sendMagicLink: async ({ email, url }) => {
        await send({ to: email, ...accountMail("signIn", confirmationUrl(url)) });
      },
    }),
  ],
  // Used only by the change of address: the link sent to the new address.
  emailVerification: {
    expiresIn: 3600,
    sendVerificationEmail: async ({ user, url }) => {
      await send({ to: user.email, ...accountMail("verifyNewEmail", url) });
    },
  },
  user: {
    // Two steps: confirmed from the current address, then verified from the
    // new one, so a stolen session alone cannot take the account over.
    changeEmail: {
      enabled: true,
      sendChangeEmailConfirmation: async ({ user, newEmail, url }) => {
        await send({ to: user.email, ...accountMail("confirmEmailChange", url, newEmail) });
      },
    },
    // Right to erasure: confirmed by a link sent to the address, opened in
    // the signed-in browser (Better Auth checks both).
    deleteUser: {
      enabled: true,
      deleteTokenExpiresIn: 3600,
      sendDeleteAccountVerification: async ({ user, url }) => {
        await send({ to: user.email, ...accountMail("deleteAccount", url) });
      },
    },
  },
  rateLimit: { enabled: true },
  // Off by default in Better Auth; stated so it never depends on the environment.
  telemetry: { enabled: false },
  advanced: {
    // Behind Caddy, the visitor's address comes from X-Forwarded-For.
    ipAddress: { ipAddressHeaders: ["x-forwarded-for"] },
  },
});
