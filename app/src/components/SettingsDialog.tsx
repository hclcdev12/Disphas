import { useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ImageUp, Trash2 } from "lucide-react";
import { api, type Settings } from "../lib/api";
import { LANGUAGES, type Language } from "../i18n";
import { Button, Dialog, fieldClass, Section } from "./ui";

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
  const file = useRef<HTMLInputElement>(null);
  const [organization, setOrganization] = useState(settings.organization ?? "");
  const [operator, setOperator] = useState(settings.operator ?? "");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

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

  const languageSelect = (
    <label className="block text-sm font-bold">
      {t("settings.language")}
      <select
        className={`ctl-select ${fieldClass} pr-9`}
        value={i18n.language}
        onChange={async (event) => {
          const language = event.target.value as Language;
          void i18n.changeLanguage(language);
          // Remembered for the next launch.
          const result = await api.setLanguage(language);
          if (result.ok) onChanged(result.value);
        }}
      >
        {LANGUAGES.map((lang) => (
          <option key={lang} value={lang}>
            {{ en: "English", fr: "Français", de: "Deutsch" }[lang]}
          </option>
        ))}
      </select>
    </label>
  );

  return (
    <Dialog
      title={t("settings.title")}
      width="lg"
      onCancel={onClose}
      footer={
        <>
          {error && (
            <p role="alert" className="mr-auto text-sm text-sayno">
              {error}
            </p>
          )}
          {saved && !error && (
            <p role="status" className="mr-auto text-sm text-ready">
              {t("settings.saved")}
            </p>
          )}
          <Button onClick={onClose}>{t("settings.close")}</Button>
          <Button variant="primary" disabled={tooLong} onClick={() => void save()}>
            {t("settings.save")}
          </Button>
        </>
      }
    >
      <Section title={t("settings.sectionCertificates")}>
        <p className="text-sm text-ash">{t("settings.intro")}</p>
        <label className="block text-sm font-bold">
          {t("settings.organization")}
          <input
            className={fieldClass}
            value={organization}
            placeholder={t("settings.organizationPlaceholder")}
            onChange={(event) => {
              setOrganization(event.target.value);
              setSaved(false);
            }}
          />
        </label>
        <label className="mt-2 block text-sm font-bold">
          {t("settings.operator")}
          <input
            className={fieldClass}
            value={operator}
            placeholder={t("settings.operatorPlaceholder")}
            onChange={(event) => {
              setOperator(event.target.value);
              setSaved(false);
            }}
          />
          <span className="text-xs font-normal text-ash">{t("settings.operatorHint")}</span>
        </label>
        {tooLong && <p className="text-xs text-sayno">{t("settings.tooLong")}</p>}

        <div className="mt-2 text-sm">
          <p className="font-bold">{t("settings.logo")}</p>
          {!logoAllowed && (
            <p className="text-xs text-ash">
              {t("settings.logoNotIncluded")}
              {settings.logo && <> {t("settings.logoKept")}</>}
            </p>
          )}
          {logoAllowed && (
            <>
              <p className="text-xs text-ash">{t("settings.logoHint")}</p>
              <div className="mt-2 flex flex-wrap items-center gap-2">
                <span className="text-ash">
                  {settings.logo
                    ? t("settings.logoSet", { format: settings.logo.toUpperCase() })
                    : t("settings.logoNone")}
                </span>
                <Button className="ml-auto flex items-center gap-2" onClick={() => file.current?.click()}>
                  <ImageUp size={14} aria-hidden="true" />
                  {t("settings.logoChoose")}
                </Button>
                {settings.logo && (
                  <Button className="flex items-center gap-2" onClick={() => void removeLogo()}>
                    <Trash2 size={14} aria-hidden="true" />
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
              </div>
            </>
          )}
        </div>
      </Section>

      <Section title={t("settings.sectionErasure")}>
        {/* Strict mode (docs/cadre_projet.md §5.4): the serial number read on the drive before each erasure. */}
        <Toggle
          checked={settings.strict_confirmation}
          title={t("settings.strict")}
          hint={t("settings.strictHint")}
          onChange={async (checked) => {
            const result = await api.setStrictConfirmation(checked);
            if (result.ok) onChanged(result.value);
            else setError(result.error.detail ?? result.error.code);
          }}
        />
      </Section>

      <Section title={t("settings.sectionApp")}>
        {languageSelect}
        {/* Off by default: a desktop notification when erasures finish while the window is in the background. */}
        <Toggle
          checked={settings.notifications}
          title={t("settings.notifications")}
          hint={t("settings.notificationsHint")}
          onChange={async (checked) => {
            const result = await api.setNotifications(checked);
            if (result.ok) onChanged(result.value);
            else setError(result.error.detail ?? result.error.code);
          }}
        />
      </Section>
    </Dialog>
  );
}

/** An option on or off, saved at once: the ticked box, its name and what it does. */
function Toggle({
  checked,
  title,
  hint,
  onChange,
}: {
  checked: boolean;
  title: ReactNode;
  hint: ReactNode;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="mt-2 flex cursor-pointer items-start gap-3 text-sm">
      <input
        type="checkbox"
        className="ctl-check mt-0.5"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span>
        <span className="font-bold">{title}</span>
        <span className="block text-xs text-ash">{hint}</span>
      </span>
    </label>
  );
}
