// Contact form: validation, a simple rate limit, and delivery by email
// (src/lib/mail.ts). CONTACT_TO: the address receiving the messages.

import { limiter } from "./limits";
import { send } from "./mail";

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

/** Bounded rate: at most 5 messages per address per hour. */
export const allowed = limiter(5, 60 * 60 * 1000);

/** Sends the message to CONTACT_TO; throws if it cannot be delivered. */
export async function deliver(message: ContactMessage): Promise<void> {
  const to = process.env.CONTACT_TO;
  if (!to && !import.meta.env.DEV) {
    throw new Error("CONTACT_TO is not configured");
  }
  await send({
    to: to ?? "contact@localhost",
    replyTo: `${message.name} <${message.email}>`,
    subject: `[oblivio.eu] ${message.topic}: ${message.name}`,
    text: [
      `Topic: ${message.topic}`,
      `Name: ${message.name}`,
      `Organization: ${message.organization || "-"}`,
      `Email: ${message.email}`,
      `Workstations: ${message.workstations || "-"}`,
      "",
      message.message,
    ].join("\n"),
  });
}
