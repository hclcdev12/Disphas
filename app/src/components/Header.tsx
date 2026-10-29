import { useTranslation } from "react-i18next";
import { Settings } from "lucide-react";
import type { HelloResult } from "../bindings/HelloResult";
import type { LicenseStatusResult } from "../bindings/LicenseStatusResult";
import type { Settings as OperatorSettings } from "../lib/api";
import { OperatorMenu } from "./OperatorMenu";

export type View = "drives" | "jobs" | "history" | "verify";

const VIEWS: View[] = ["drives", "jobs", "history", "verify"];

/**
 * Header shared by every tab (docs/maquettes/disphas-ecran-disques.md §3.1): the
 * brand, the tabs, the state of the workstation and the settings. Language
 * and theme live in the settings.
 */
export function Header({
  hello,
  mode,
  connected,
  license,
  view,
  onView,
  onSettings,
  settings,
  onSettingsChanged,
}: {
  hello: HelloResult | null;
  mode: "tauri" | "preview" | "demo";
  connected: boolean;
  license: LicenseStatusResult | null;
  view: View;
  onView: (view: View) => void;
  onSettings: () => void;
  /** The operator signing the next certificates, switched from the header. */
  settings: OperatorSettings;
  onSettingsChanged: (settings: OperatorSettings) => void;
}) {
  const { t } = useTranslation();
  return (
    <header className="topbar flex flex-wrap items-center justify-between gap-x-6 gap-y-2 border-b border-edge px-8">
      <div className="topbar-left flex min-w-0 items-center gap-8">
        <span className="topbar-brand py-4 font-brand text-[26px] leading-none font-bold">Disphas</span>
        <nav className="topbar-tabs flex gap-1 self-stretch overflow-x-auto" aria-label={t("nav.label")}>
          {VIEWS.map((tab) => (
            <button
              key={tab}
              type="button"
              aria-current={view === tab ? "page" : undefined}
              onClick={() => onView(tab)}
              className={`flex min-h-14 items-center border-b-2 px-3.5 whitespace-nowrap ${
                view === tab ? "border-soft font-bold text-soft" : "border-transparent text-ash hover:text-soft"
              }`}
            >
              {t(`nav.${tab}`)}
            </button>
          ))}
        </nav>
      </div>
      <div className="topbar-right flex items-center gap-4">
        <span className="topbar-status font-mono text-sm text-ash">
          <Station hello={hello} mode={mode} connected={connected} license={license} />
        </span>
        <OperatorMenu settings={settings} onChanged={onSettingsChanged} />
        <button
          type="button"
          aria-label={t("settings.title")}
          title={t("settings.title")}
          onClick={onSettings}
          className="inline-flex size-11 items-center justify-center rounded text-soft hover:bg-card"
        >
          <Settings size={18} aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}

/** "hors ligne · assistant connecté · licence active · règles 1.0.0". */
export function Station({
  hello,
  mode,
  connected,
  license,
}: {
  hello: HelloResult | null;
  mode: "tauri" | "preview" | "demo";
  connected: boolean;
  license: LicenseStatusResult | null;
}) {
  const { t } = useTranslation();
  const helper = mode === "tauri" ? (connected ? "connected" : "disconnected") : mode;
  const parts = [t("station.offline"), t(`station.helper.${helper}`)];
  if (license) parts.push(t(`station.license.${license.state}`));
  const rules = hello ? t("station.rules", { version: hello.rules_version }) : null;
  // The rules version is the first to go when the header is short (index.css).
  return (
    <span title={[...parts, ...(rules ? [rules] : [])].join(" · ")}>
      {parts.join(" · ")}
      {rules && <span className="station-rules"> · {rules}</span>}
    </span>
  );
}
