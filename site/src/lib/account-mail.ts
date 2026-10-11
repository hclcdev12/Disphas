// Emails of the customer area, in the language of the page they were asked
// from (read from the link's callbackURL, /<lang>/account/...).
import { DEFAULT_LOCALE, isLocale, type Locale } from "../i18n";

type Kind = "signIn" | "confirmEmailChange" | "verifyNewEmail" | "deleteAccount";
type Texts = Record<Kind, { subject: string; body: (url: string, detail: string) => string }>;

const TEXTS: Record<Locale, Texts> = {
  en: {
    signIn: {
      subject: "Your Oblivio sign-in link",
      body: (url) =>
        `Hello,\n\nTo sign in to your Oblivio customer area, open this link within 5 minutes:\n\n${url}\n\nIt works once. If you did not ask for it, ignore this email: nothing happens without the link.\n\nOblivio`,
    },
    confirmEmailChange: {
      subject: "Confirm the change of your Oblivio email address",
      body: (url, email) =>
        `Hello,\n\nA change of the email address of your Oblivio account to ${email} was requested. To confirm it, open this link within an hour:\n\n${url}\n\nA second link will then be sent to the new address. If you did not ask for this change, ignore this email and sign out your other sessions from your customer area.\n\nOblivio`,
    },
    verifyNewEmail: {
      subject: "Verify your new Oblivio email address",
      body: (url) =>
        `Hello,\n\nTo use this address for your Oblivio account, open this link within an hour:\n\n${url}\n\nIf you did not ask for it, ignore this email.\n\nOblivio`,
    },
    deleteAccount: {
      subject: "Confirm the deletion of your Oblivio account",
      body: (url) =>
        `Hello,\n\nThe deletion of your Oblivio account was requested. To confirm it, open this link within an hour, in the browser where you are signed in:\n\n${url}\n\nThe deletion is final. If you did not ask for it, ignore this email.\n\nOblivio`,
    },
  },
  fr: {
    signIn: {
      subject: "Votre lien de connexion Oblivio",
      body: (url) =>
        `Bonjour,\n\nPour vous connecter à votre espace client Oblivio, ouvrez ce lien dans les 5 minutes :\n\n${url}\n\nIl ne fonctionne qu'une fois. Si vous ne l'avez pas demandé, ignorez ce courriel : rien ne se passe sans le lien.\n\nOblivio`,
    },
    confirmEmailChange: {
      subject: "Confirmez le changement de votre adresse Oblivio",
      body: (url, email) =>
        `Bonjour,\n\nLe remplacement de l'adresse de votre compte Oblivio par ${email} a été demandé. Pour le confirmer, ouvrez ce lien dans l'heure :\n\n${url}\n\nUn second lien sera ensuite envoyé à la nouvelle adresse. Si vous n'êtes pas à l'origine de cette demande, ignorez ce courriel et fermez vos autres sessions depuis votre espace client.\n\nOblivio`,
    },
    verifyNewEmail: {
      subject: "Vérifiez votre nouvelle adresse Oblivio",
      body: (url) =>
        `Bonjour,\n\nPour utiliser cette adresse avec votre compte Oblivio, ouvrez ce lien dans l'heure :\n\n${url}\n\nSi vous ne l'avez pas demandé, ignorez ce courriel.\n\nOblivio`,
    },
    deleteAccount: {
      subject: "Confirmez la suppression de votre compte Oblivio",
      body: (url) =>
        `Bonjour,\n\nLa suppression de votre compte Oblivio a été demandée. Pour la confirmer, ouvrez ce lien dans l'heure, dans le navigateur où vous êtes connecté :\n\n${url}\n\nLa suppression est définitive. Si vous ne l'avez pas demandée, ignorez ce courriel.\n\nOblivio`,
    },
  },
  de: {
    signIn: {
      subject: "Ihr Oblivio-Anmeldelink",
      body: (url) =>
        `Guten Tag,\n\num sich in Ihrem Oblivio-Kundenbereich anzumelden, öffnen Sie diesen Link innerhalb von 5 Minuten:\n\n${url}\n\nEr funktioniert nur einmal. Wenn Sie ihn nicht angefordert haben, ignorieren Sie diese E-Mail: Ohne den Link geschieht nichts.\n\nOblivio`,
    },
    confirmEmailChange: {
      subject: "Bestätigen Sie die Änderung Ihrer Oblivio-E-Mail-Adresse",
      body: (url, email) =>
        `Guten Tag,\n\nes wurde beantragt, die E-Mail-Adresse Ihres Oblivio-Kontos durch ${email} zu ersetzen. Öffnen Sie zur Bestätigung diesen Link innerhalb einer Stunde:\n\n${url}\n\nAnschließend wird ein zweiter Link an die neue Adresse gesendet. Wenn Sie dies nicht beantragt haben, ignorieren Sie diese E-Mail und beenden Sie Ihre anderen Sitzungen im Kundenbereich.\n\nOblivio`,
    },
    verifyNewEmail: {
      subject: "Bestätigen Sie Ihre neue Oblivio-E-Mail-Adresse",
      body: (url) =>
        `Guten Tag,\n\num diese Adresse für Ihr Oblivio-Konto zu verwenden, öffnen Sie diesen Link innerhalb einer Stunde:\n\n${url}\n\nWenn Sie dies nicht beantragt haben, ignorieren Sie diese E-Mail.\n\nOblivio`,
    },
    deleteAccount: {
      subject: "Bestätigen Sie die Löschung Ihres Oblivio-Kontos",
      body: (url) =>
        `Guten Tag,\n\ndie Löschung Ihres Oblivio-Kontos wurde beantragt. Öffnen Sie zur Bestätigung diesen Link innerhalb einer Stunde in dem Browser, in dem Sie angemeldet sind:\n\n${url}\n\nDie Löschung ist endgültig. Wenn Sie sie nicht beantragt haben, ignorieren Sie diese E-Mail.\n\nOblivio`,
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

/** The email of `kind` carrying `url`; `detail` is the new address for a change. */
export function accountMail(kind: Kind, url: string, detail = ""): { subject: string; text: string } {
  const text = TEXTS[localeOf(url)][kind];
  return { subject: text.subject, text: text.body(url, detail) };
}
