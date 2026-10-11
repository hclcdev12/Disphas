// Contact form: validation, a simple rate limit, and delivery by email.
// Configuration (environment, never in the code):
//   SMTP_URL    e.g. smtps://user:password@mail.example:465
//   CONTACT_TO  address receiving the messages
//   CONTACT_FROM sender address of the messages (the site's own address)
// Without SMTP_URL in development, messages are written to the server log.

import { createTransport } from "nodemailer";

export const TOPICS = ["quote", "question", "security", "other"] as const;
export type Topic = (typeof TOPICS)[number];

export interface ContactMessage {
  name: string;
  organization: string;
  email: string;
  topic: Topic;
  workstations: string;
  message: string;
}

const LIMITS = { name: 120, organization: 160, email: 254, workstations: 20, message: 5000 };

function field(form: FormData, key: keyof typeof LIMITS): string | null {
  const value = form.get(key);
  if (typeof value !== "string") {
    return "";
  }
  const trimmed = value.trim();
  return trimmed.length <= LIMITS[key] ? trimmed : null;
}

/** The message carried by `form`, or `null` if it is invalid. */
export function parse(form: FormData): ContactMessage | null {
  const name = field(form, "name");
  const organization = field(form, "organization");
  const email = field(form, "email");
  const workstations = field(form, "workstations");
  const message = field(form, "message");
  const topic = form.get("topic");
  if (name === null || organization === null || email === null || workstations === null || message === null) {
    return null;
  }
  if (!name || !message || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
    return null;
  }
  // Header injection: no line break in what goes into the mail headers.
  if (/[\r\n]/.test(name + email)) {
    return null;
  }
  return {
    name,
    organization,
    email,
    topic: TOPICS.includes(topic as Topic) ? (topic as Topic) : "other",
    workstations,
    message,
  };
}

/** Bounded rate: at most 5 messages per address per hour, in memory. */
const recent = new Map<string, number[]>();
export function allowed(address: string, now = Date.now()): boolean {
  const hour = 60 * 60 * 1000;
  const times = (recent.get(address) ?? []).filter((time) => now - time < hour);
  if (times.length >= 5) {
    recent.set(address, times);
    return false;
  }
  times.push(now);
  recent.set(address, times);
  if (recent.size > 10_000) {
    recent.clear();
  }
  return true;
}

/** Sends the message; throws if it cannot be delivered. */
export async function deliver(message: ContactMessage): Promise<void> {
  const smtp = process.env.SMTP_URL;
  const text = [
    `Topic: ${message.topic}`,
    `Name: ${message.name}`,
    `Organization: ${message.organization || "-"}`,
    `Email: ${message.email}`,
    `Workstations: ${message.workstations || "-"}`,
    "",
    message.message,
  ].join("\n");
  if (!smtp) {
    if (import.meta.env.DEV) {
      console.info(`[contact] SMTP_URL not set, message not sent:\n${text}`);
      return;
    }
    throw new Error("SMTP_URL is not configured");
  }
  const to = process.env.CONTACT_TO;
  const from = process.env.CONTACT_FROM;
  if (!to || !from) {
    throw new Error("CONTACT_TO and CONTACT_FROM are required");
  }
  await createTransport(smtp).sendMail({
    from,
    to,
    replyTo: `${message.name} <${message.email}>`,
    subject: `[oblivio.eu] ${message.topic}: ${message.name}`,
    text,
  });
}
