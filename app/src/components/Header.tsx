import { useTranslation } from "react-i18next";
import type { HelloResult } from "../bindings/HelloResult";
import type { LicenseStatusResult } from "../bindings/LicenseStatusResult";
import { LANGUAGES, type Language } from "../i18n";
import { Badge, Dot } from "./ui";

export function Header({
  hello,
  mode,
  connected,
  license,
  onLicense,
}: {
  hello: HelloResult | null;
  mode: "tauri" | "preview";
  connected: boolean;
  license: LicenseStatusResult | null;
  onLicense: () => void;
}) {
  const { t, i18n } = useTranslation();
  const helper = mode === "preview" ? "preview" : connected ? "connected" : "disconnected";
  return (
    <header className="flex items-center gap-4 border-b border-edge px-6 py-4">
      <span className="text-lg font-bold tracking-[0.3em] text-brand">OBLIVIO</span>
      {hello && <span className="font-mono text-xs text-ash">{t("header.rules", { version: hello.rules_version })}</span>}
      <div className="ml-auto flex items-center gap-2">
        <Badge tone="ready">
          <Dot tone="ready" />
          {t("header.offline")}
        </Badge>
        <Badge tone={helper === "connected" ? "brand" : helper === "preview" ? "blocked" : "sayno"}>
          <Dot tone={helper === "connected" ? "brand" : helper === "preview" ? "blocked" : "sayno"} />
          {t(`header.helper.${helper}`)}
        </Badge>
        {license && (
          <button type="button" onClick={onLicense} className="cursor-pointer">
            <Badge tone={license.state === "Valid" ? "ready" : "neutral"}>
              {t(`header.license.${license.state}`)}
            </Badge>
          </button>
        )}
        <select
          aria-label="Language"
          className="rounded-md border border-edge bg-card px-2 py-1 text-xs text-ash"
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
