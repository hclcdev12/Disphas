// Site texts: French is the source (fr.ts); English and German are
// translations to be proofread. Every language has the same keys (type Texts).
import de from "./de";
import en from "./en";
import fr, { type Texts } from "./fr";

export const LOCALES = ["en", "fr", "de"] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "en";

export function isLocale(value: string | undefined): value is Locale {
  return LOCALES.includes(value as Locale);
}

const TEXTS: Record<Locale, Texts> = { en, fr, de };

export function texts(locale: Locale): Texts {
  return TEXTS[locale];
}

/** Pages of the site, the same path in every language. */
export const PAGES = {
  home: "",
  product: "product",
  security: "security",
  pricing: "pricing",
  download: "download",
  demo: "demo",
  contact: "contact",
  account: "account",
  login: "account/login",
  notice: "legal",
  privacy: "privacy",
  terms: "terms",
} as const;

export function href(locale: Locale, page: keyof typeof PAGES): string {
  const path = PAGES[page];
  return path ? `/${locale}/${path}/` : `/${locale}/`;
}
