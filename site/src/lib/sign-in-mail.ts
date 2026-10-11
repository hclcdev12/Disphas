// The sign-in email, in the language of the page it was asked from (read
// from the link's callbackURL, /<lang>/account/).
import { DEFAULT_LOCALE, isLocale, type Locale } from "../i18n";

const TEXTS: Record<Locale, { subject: string; body: (url: string) => string }> = {
  en: {
    subject: "Your Oblivio sign-in link",
    body: (url) =>
      `Hello,\n\nTo sign in to your Oblivio customer area, open this link within 5 minutes:\n\n${url}\n\nIt works once. If you did not ask for it, ignore this email: nothing happens without the link.\n\nOblivio`,
  },
  fr: {
    subject: "Votre lien de connexion Oblivio",
    body: (url) =>
      `Bonjour,\n\nPour vous connecter à votre espace client Oblivio, ouvrez ce lien dans les 5 minutes :\n\n${url}\n\nIl ne fonctionne qu'une fois. Si vous ne l'avez pas demandé, ignorez ce courriel : rien ne se passe sans le lien.\n\nOblivio`,
  },
  de: {
    subject: "Ihr Oblivio-Anmeldelink",
    body: (url) =>
      `Guten Tag,\n\num sich in Ihrem Oblivio-Kundenbereich anzumelden, öffnen Sie diesen Link innerhalb von 5 Minuten:\n\n${url}\n\nEr funktioniert nur einmal. Wenn Sie ihn nicht angefordert haben, ignorieren Sie diese E-Mail: Ohne den Link geschieht nichts.\n\nOblivio`,
  },
};

export function signInMail(url: string): { subject: string; text: string } {
  let locale: Locale = DEFAULT_LOCALE;
  try {
    const callback = new URL(url).searchParams.get("callbackURL") ?? "";
    const segment = callback.split("/")[1];
    if (isLocale(segment)) locale = segment;
  } catch {
    // Keep the default language.
  }
  const text = TEXTS[locale];
  return { subject: text.subject, text: text.body(url) };
}
