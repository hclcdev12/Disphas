import { useTranslation } from "react-i18next";
import wordmark from "../../../assets/logo/oblivio-wordmark-dark.svg";
import type { HelloResult } from "../bindings/HelloResult";
import type { LicenseStatusResult } from "../bindings/LicenseStatusResult";
import { LANGUAGES, type Language } from "../i18n";
import { Settings } from "lucide-react";
import { Badge, Dot } from "./ui";

export function Header({
  hello,
  mode,
  connected,
  license,
  onLicense,
  onSettings,
}: {
  hello: HelloResult | null;
  mode: "tauri" | "preview" | "demo";
  connected: boolean;
  license: LicenseStatusResult | null;
  onLicense: () => void;
  onSettings: () => void;
}) {
  const { t, i18n } = useTranslation();
  const helper = mode === "tauri" ? (connected ? "connected" : "disconnected") : mode;
  return (
    <header className="flex items-center gap-4 border-b border-edge px-6 py-4">
      <img src={wordmark} alt="disphas" className="h-6" />
      {hello && <span className="font-mono text-xs text-ash">{t("header.rules", { version: hello.rules_version })}</span>}
      <div className="ml-auto flex items-center gap-2">
        <Badge tone="ready">
          <Dot tone="ready" />
          {t("header.offline")}
        </Badge>
        <Badge tone={helper === "connected" || helper === "demo" ? "brand" : helper === "preview" ? "blocked" : "sayno"}>
          <Dot tone={helper === "connected" || helper === "demo" ? "brand" : helper === "preview" ? "blocked" : "sayno"} />
          {t(`header.helper.${helper}`)}
        </Badge>
        {license && (
          <button type="button" onClick={onLicense} className="cursor-pointer">
            <Badge tone={license.state === "Valid" ? "ready" : "neutral"}>
              {t(`header.license.${license.state}`)}
            </Badge>
          </button>
        )}
        <button
          type="button"
          aria-label={t("settings.title")}
          title={t("settings.title")}
          onClick={onSettings}
          className="rounded-md border border-edge p-1.5 text-ash hover:text-soft"
        >
          <Settings size={14} />
        </button>
        <select
          aria-label="Language"
          className="rounded-md border border-edge bg-card py-1 pr-6 pl-2 text-xs text-ash"
          value={i18n.language}
          onChange={(event) => void i18n.changeLanguage(event.target.value as Language)}
        >
          {LANGUAGES.map((lang) => (
            <option key={lang} value={lang}>
              {lang.toUpperCase()}
            </option>
          ))}
        </select>
      </div>
    </header>
  );
}
