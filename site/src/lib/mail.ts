// Outgoing email (contact form, sign-in links). Configuration from the
// environment, never in the code:
//   SMTP_URL      e.g. smtps://user:password@mail.example:465
//   MAIL_FROM     sender address, the site's own
// Without SMTP_URL in development, messages are written to the server log.
import { createTransport } from "nodemailer";

export interface Mail {
  to: string;
  subject: string;
  text: string;
  replyTo?: string;
}

/** Sends `mail`; throws if it cannot be delivered. */
export async function send(mail: Mail): Promise<void> {
  const smtp = process.env.SMTP_URL;
  if (!smtp) {
    if (import.meta.env.DEV) {
      console.info(`[mail] SMTP_URL not set, not sent. To: ${mail.to}\nSubject: ${mail.subject}\n${mail.text}`);
      return;
    }
    throw new Error("SMTP_URL is not configured");
  }
  const from = process.env.MAIL_FROM;
  if (!from) {
    throw new Error("MAIL_FROM is not configured");
  }
  await createTransport(smtp).sendMail({ from, ...mail });
}
