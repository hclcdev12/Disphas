import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./en.json";
import fr from "./fr.json";
import de from "./de.json";

export const LANGUAGES = ["en", "fr", "de"] as const;
export type Language = (typeof LANGUAGES)[number];

function systemLanguage(): Language {
  const tag = navigator.language.slice(0, 2).toLowerCase();
  return (LANGUAGES as readonly string[]).includes(tag) ? (tag as Language) : "en";
}

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, fr: { translation: fr }, de: { translation: de } },
  lng: systemLanguage(),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
