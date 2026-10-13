// Emails of the customer area, in the language of the page they were asked
// from (read from the link's callbackURL, /<lang>/account/...).
import { DEFAULT_LOCALE, isLocale, type Locale } from "../i18n";

type Kind = "signIn" | "confirmEmailChange" | "verifyNewEmail" | "deleteAccount" | "resetPassword";
type Texts = Record<Kind, { subject: string; body: (url: string, detail: string) => string }>;

const TEXTS: Record<Locale, Texts> = {
  en: {
    signIn: {
      subject: "Your disphas sign-in link",
      body: (url) =>
        `Hello,\n\nTo sign in to your disphas customer area, open this link within 5 minutes:\n\n${url}\n\nIt works once. If you did not ask for it, ignore this email: nothing happens without the link.\n\ndisphas`,
    },
    confirmEmailChange: {
      subject: "Confirm the change of your disphas email address",
      body: (url, email) =>
        `Hello,\n\nA change of the email address of your disphas account to ${email} was requested. To confirm it, open this link within an hour:\n\n${url}\n\nA second link will then be sent to the new address. If you did not ask for this change, ignore this email and sign out your other sessions from your customer area.\n\ndisphas`,
    },
    verifyNewEmail: {
      subject: "Verify your new disphas email address",
      body: (url) =>
        `Hello,\n\nTo use this address for your disphas account, open this link within an hour:\n\n${url}\n\nIf you did not ask for it, ignore this email.\n\ndisphas`,
    },
    resetPassword: {
      subject: "Reset your disphas password",
      body: (url) =>
        `Hello,\n\nTo choose a new password for your disphas account, open this link within an hour:\n\n${url}\n\nEvery session of the account will then be signed out. If you did not ask for it, ignore this email: your password does not change.\n\ndisphas`,
    },
    deleteAccount: {
      subject: "Confirm the deletion of your disphas account",
      body: (url) =>
        `Hello,\n\nThe deletion of your disphas account was requested. To confirm it, open this link within an hour, in the browser where you are signed in:\n\n${url}\n\nThe deletion is final. If you did not ask for it, ignore this email.\n\ndisphas`,
    },
  },
  fr: {
    signIn: {
      subject: "Votre lien de connexion disphas",
      body: (url) =>
        `Bonjour,\n\nPour vous connecter à votre espace client disphas, ouvrez ce lien dans les 5 minutes :\n\n${url}\n\nIl ne fonctionne qu'une fois. Si vous ne l'avez pas demandé, ignorez ce courriel : rien ne se passe sans le lien.\n\ndisphas`,
    },
    confirmEmailChange: {
      subject: "Confirmez le changement de votre adresse disphas",
      body: (url, email) =>
        `Bonjour,\n\nLe remplacement de l'adresse de votre compte disphas par ${email} a été demandé. Pour le confirmer, ouvrez ce lien dans l'heure :\n\n${url}\n\nUn second lien sera ensuite envoyé à la nouvelle adresse. Si vous n'êtes pas à l'origine de cette demande, ignorez ce courriel et fermez vos autres sessions depuis votre espace client.\n\ndisphas`,
    },
    verifyNewEmail: {
      subject: "Vérifiez votre nouvelle adresse disphas",
      body: (url) =>
        `Bonjour,\n\nPour utiliser cette adresse avec votre compte disphas, ouvrez ce lien dans l'heure :\n\n${url}\n\nSi vous ne l'avez pas demandé, ignorez ce courriel.\n\ndisphas`,
    },
    resetPassword: {
      subject: "Réinitialisez votre mot de passe disphas",
      body: (url) =>
        `Bonjour,\n\nPour choisir un nouveau mot de passe pour votre compte disphas, ouvrez ce lien dans l'heure :\n\n${url}\n\nToutes les sessions du compte seront ensuite fermées. Si vous ne l'avez pas demandé, ignorez ce courriel : votre mot de passe ne change pas.\n\ndisphas`,
    },
    deleteAccount: {
      subject: "Confirmez la suppression de votre compte disphas",
      body: (url) =>
        `Bonjour,\n\nLa suppression de votre compte disphas a été demandée. Pour la confirmer, ouvrez ce lien dans l'heure, dans le navigateur où vous êtes connecté :\n\n${url}\n\nLa suppression est définitive. Si vous ne l'avez pas demandée, ignorez ce courriel.\n\ndisphas`,
    },
  },
  de: {
    signIn: {
      subject: "Ihr disphas-Anmeldelink",
      body: (url) =>
        `Guten Tag,\n\num sich in Ihrem disphas-Kundenbereich anzumelden, öffnen Sie diesen Link innerhalb von 5 Minuten:\n\n${url}\n\nEr funktioniert nur einmal. Wenn Sie ihn nicht angefordert haben, ignorieren Sie diese E-Mail: Ohne den Link geschieht nichts.\n\ndisphas`,
    },
    confirmEmailChange: {
      subject: "Bestätigen Sie die Änderung Ihrer disphas-E-Mail-Adresse",
      body: (url, email) =>
        `Guten Tag,\n\nes wurde beantragt, die E-Mail-Adresse Ihres disphas-Kontos durch ${email} zu ersetzen. Öffnen Sie zur Bestätigung diesen Link innerhalb einer Stunde:\n\n${url}\n\nAnschließend wird ein zweiter Link an die neue Adresse gesendet. Wenn Sie dies nicht beantragt haben, ignorieren Sie diese E-Mail und beenden Sie Ihre anderen Sitzungen im Kundenbereich.\n\ndisphas`,
    },
    verifyNewEmail: {
      subject: "Bestätigen Sie Ihre neue disphas-E-Mail-Adresse",
      body: (url) =>
        `Guten Tag,\n\num diese Adresse für Ihr disphas-Konto zu verwenden, öffnen Sie diesen Link innerhalb einer Stunde:\n\n${url}\n\nWenn Sie dies nicht beantragt haben, ignorieren Sie diese E-Mail.\n\ndisphas`,
    },
    resetPassword: {
      subject: "Setzen Sie Ihr disphas-Passwort zurück",
      body: (url) =>
        `Guten Tag,\n\num ein neues Passwort für Ihr disphas-Konto zu wählen, öffnen Sie diesen Link innerhalb einer Stunde:\n\n${url}\n\nDanach werden alle Sitzungen des Kontos abgemeldet. Wenn Sie dies nicht angefordert haben, ignorieren Sie diese E-Mail: Ihr Passwort bleibt unverändert.\n\ndisphas`,
    },
    deleteAccount: {
      subject: "Bestätigen Sie die Löschung Ihres disphas-Kontos",
      body: (url) =>
        `Guten Tag,\n\ndie Löschung Ihres disphas-Kontos wurde beantragt. Öffnen Sie zur Bestätigung diesen Link innerhalb einer Stunde in dem Browser, in dem Sie angemeldet sind:\n\n${url}\n\nDie Löschung ist endgültig. Wenn Sie sie nicht beantragt haben, ignorieren Sie diese E-Mail.\n\ndisphas`,
    },
  },
};

function localeOf(url: string): Locale {
  try {
    const callback = new URL(url).searchParams.get("callbackURL") ?? "";
    const segment = callback.split("/")[1];
    if (isLocale(segment)) return segment;
  } catch {
    // Keep the default language.
  }
  return DEFAULT_LOCALE;
}

/**
 * The sign-in link of the email: our confirmation page, carrying Better
 * Auth's token and callback, instead of Better Auth's own verify URL.
 */
export function confirmationUrl(verifyUrl: string): string {
  const url = new URL(verifyUrl);
  const callback = url.searchParams.get("callbackURL") ?? "/";
  const segment = callback.split("/")[1];
  const locale = isLocale(segment) ? segment : DEFAULT_LOCALE;
  const page = new URL(`/${locale}/account/confirm/`, url.origin);
  page.searchParams.set("token", url.searchParams.get("token") ?? "");
  page.searchParams.set("callbackURL", callback);
  return page.toString();
}

/** The email of `kind` carrying `url`; `detail` is the new address for a change. */
export function accountMail(kind: Kind, url: string, detail = ""): { subject: string; text: string } {
  const text = TEXTS[localeOf(url)][kind];
  return { subject: text.subject, text: text.body(url, detail) };
}
