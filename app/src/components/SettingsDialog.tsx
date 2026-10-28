import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ImageUp, Trash2 } from "lucide-react";
import { api, type Settings } from "../lib/api";
import { LANGUAGES, type Language } from "../i18n";
import { Button } from "./ui";

/** Same bounds as the helper, which checks again before signing. */
const MAX_OPERATOR = 64;
const MAX_ORGANIZATION = 128;
const MAX_LOGO_BYTES = 1024 * 1024;

function toBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let index = 0; index < bytes.length; index += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(index, index + 0x8000));
  }
  return btoa(binary);
}

/**
 * Operator settings, for this user account: the organization and operator
 * signed into every certificate, and the customer's logo printed on the
 * PDFs. An identifier is enough for the operator (GDPR minimisation).
 *
 * The logo setting is hidden when the offer does not include it
 * (docs/offres.md §4.5): a stored logo is kept, only ignored, and the app
 * backend checks the license again before each PDF.
 */
export function SettingsDialog({
  settings,
  logoAllowed,
  onClose,
  onChanged,
}: {
  settings: Settings;
  logoAllowed: boolean;
  onClose: () => void;
  onChanged: (settings: Settings) => void;
}) {
  const { t, i18n } = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const file = useRef<HTMLInputElement>(null);
  const [organization, setOrganization] = useState(settings.organization ?? "");
  const [operator, setOperator] = useState(settings.operator ?? "");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

  const tooLong = [...organization].length > MAX_ORGANIZATION || [...operator].length > MAX_OPERATOR;

  const save = async () => {
    setError(null);
    const result = await api.saveSettings(organization, operator);
    if (result.ok) {
      onChanged(result.value);
      setSaved(true);
    } else {
      setError(result.error.detail ?? result.error.code);
    }
  };

  const pickLogo = async (picked: File | undefined) => {
    if (!picked) return;
    setError(null);
    if (picked.size > MAX_LOGO_BYTES) {
      setError(t("settings.logoTooLarge"));
      return;
    }
    const result = await api.setLogo(toBase64(new Uint8Array(await picked.arrayBuffer())));
    if (result.ok) onChanged(result.value);
    else setError(result.error.detail ?? result.error.code);
  };

  const removeLogo = async () => {
    const result = await api.clearLogo();
    if (result.ok) onChanged(result.value);
    else setError(result.error.detail ?? result.error.code);
  };

  return (
    <dialog
      ref={dialog}
      onCancel={onClose}
      className="m-auto w-[32rem] max-w-[92vw] rounded-lg border border-edge bg-card p-6 text-soft backdrop:bg-black/70"
    >
      <h2 className="text-lg font-bold">{t("settings.title")}</h2>
      <p className="mt-1 text-sm text-ash">{t("settings.intro")}</p>

      {/* Language moved here from the header (docs/maquettes/disphas-ecran-disques.md §3.1). */}
      <label className="mt-4 block text-sm font-bold">
        {t("settings.language")}
        <select
          className="mt-1.5 block min-h-11 w-full rounded border border-ctrl bg-night px-2 pr-8 font-normal"
          value={i18n.language}
          onChange={(event) => void i18n.changeLanguage(event.target.value as Language)}
        >
          {LANGUAGES.map((lang) => (
            <option key={lang} value={lang}>
              {{ en: "English", fr: "Français", de: "Deutsch" }[lang]}
            </option>
          ))}
        </select>
      </label>

      <label className="mt-4 block text-sm font-bold">
        {t("settings.organization")}
        <input
          className="mt-1.5 block w-full rounded-md border border-edge bg-night px-2 py-1.5 font-normal"
          value={organization}
          placeholder={t("settings.organizationPlaceholder")}
          onChange={(event) => {
            setOrganization(event.target.value);
            setSaved(false);
          }}
        />
      </label>
      <label className="mt-4 block text-sm font-bold">
        {t("settings.operator")}
        <input
          className="mt-1.5 block w-full rounded-md border border-edge bg-night px-2 py-1.5 font-normal"
          value={operator}
          placeholder={t("settings.operatorPlaceholder")}
          onChange={(event) => {
            setOperator(event.target.value);
            setSaved(false);
          }}
        />
        <span className="text-xs font-normal text-ash">{t("settings.operatorHint")}</span>
      </label>
      {tooLong && <p className="mt-2 text-xs text-sayno">{t("settings.tooLong")}</p>}

      {/* Strict mode (docs/cadre_projet.md §5.4): the serial number read on the drive before each erasure. */}
      <label className="mt-5 flex cursor-pointer items-start gap-3 text-sm">
        <input
          type="checkbox"
          className="ctl-check mt-0.5"
          checked={settings.strict_confirmation}
          onChange={async (event) => {
            const result = await api.setStrictConfirmation(event.target.checked);
            if (result.ok) onChanged(result.value);
            else setError(result.error.detail ?? result.error.code);
          }}
        />
        <span>
          <span className="font-bold">{t("settings.strict")}</span>
          <span className="block text-xs text-ash">{t("settings.strictHint")}</span>
        </span>
      </label>

      <div className="mt-5 text-sm">
        <p className="font-bold">{t("settings.logo")}</p>
        {!logoAllowed && (
          <p className="text-xs text-ash">
            {t("settings.logoNotIncluded")}
            {settings.logo && <> {t("settings.logoKept")}</>}
          </p>
        )}
        {logoAllowed && <p className="text-xs text-ash">{t("settings.logoHint")}</p>}
        {logoAllowed && <div className="mt-2 flex items-center gap-2">
          <span className="text-ash">{settings.logo ? t("settings.logoSet", { format: settings.logo.toUpperCase() }) : t("settings.logoNone")}</span>
          <Button className="ml-auto flex items-center gap-2" onClick={() => file.current?.click()}>
            <ImageUp size={14} />
            {t("settings.logoChoose")}
          </Button>
          {settings.logo && (
            <Button className="flex items-center gap-2" onClick={() => void removeLogo()}>
              <Trash2 size={14} />
              {t("settings.logoRemove")}
            </Button>
          )}
          <input
            ref={file}
            type="file"
            accept=".png,.svg,image/png,image/svg+xml"
            className="hidden"
            onChange={(event) => {
              void pickLogo(event.target.files?.[0]);
              event.target.value = "";
            }}
          />
        </div>}
      </div>

      {error && <p className="mt-4 text-sm text-sayno">{error}</p>}
      {saved && <p className="mt-4 text-sm text-ready">{t("settings.saved")}</p>}

      <div className="mt-6 flex justify-end gap-2">
        <Button onClick={onClose}>{t("settings.close")}</Button>
        <Button variant="primary" disabled={tooLong} onClick={() => void save()}>
          {t("settings.save")}
        </Button>
      </div>
    </dialog>
  );
}
