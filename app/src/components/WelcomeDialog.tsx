import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { BadgeCheck, KeyRound, TriangleAlert } from "lucide-react";
import type { LicenseStatusResult } from "../bindings/LicenseStatusResult";
import { LANGUAGES, type Language } from "../i18n";
import { api, type Settings } from "../lib/api";
import { Button } from "./ui";

const STEPS = ["language", "signer", "license", "ready"] as const;
type Step = (typeof STEPS)[number];

/** Same bounds as the helper and app/src-tauri/src/settings.rs. */
const MAX_OPERATOR = 64;
const MAX_ORGANIZATION = 128;

/**
 * First-launch assistant (UX review of 2026-10-26, P1): one question per
 * screen, everything can be skipped and changed later in the settings.
 * Language, who signs the certificates, the license, then how an erasure
 * goes. Shown once; completing or skipping it is remembered.
 */
export function WelcomeDialog({
  settings,
  license,
  onLicense,
  onChanged,
  onDone,
}: {
  settings: Settings;
  license: LicenseStatusResult | null;
  /** Opens the license dialog above the assistant. */
  onLicense: () => void;
  onChanged: (settings: Settings) => void;
  onDone: () => void;
}) {
  const { t, i18n } = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [step, setStep] = useState<Step>("language");
  const [organization, setOrganization] = useState(settings.organization ?? "");
  const [operator, setOperator] = useState(settings.operator ?? "");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const index = STEPS.indexOf(step);

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    // showModal focuses the first control, "Skip": start on the language in use instead.
    element?.querySelector<HTMLInputElement>("input:checked")?.focus();
    return () => element?.close();
  }, []);

  const finish = async () => {
    setBusy(true);
    const result = await api.setOnboarded();
    if (result.ok) onChanged(result.value);
    onDone();
  };

  const chooseLanguage = async (language: Language) => {
    void i18n.changeLanguage(language);
    const result = await api.setLanguage(language);
    if (result.ok) onChanged(result.value);
  };

  const saveSigner = async () => {
    setError(null);
    setBusy(true);
    const result = await api.saveSettings(organization, operator);
    setBusy(false);
    if (!result.ok) {
      setError(result.error.detail ?? result.error.code);
      return;
    }
    onChanged(result.value);
    setStep("license");
  };

  const tooLong = [...organization.trim()].length > MAX_ORGANIZATION || [...operator.trim()].length > MAX_OPERATOR;
  const until =
    license?.valid_until != null
      ? new Intl.DateTimeFormat(i18n.language, { dateStyle: "long" }).format(new Date(license.valid_until * 1000))
      : null;

  return (
    <dialog
      ref={dialog}
      // Escape skips the assistant, as the "Skip" button does.
      onCancel={(event) => {
        event.preventDefault();
        void finish();
      }}
      aria-labelledby="welcome-title"
      className="m-auto w-[36rem] max-w-[92vw] rounded-lg border border-ctrl bg-card p-8 text-soft backdrop:bg-black/80"
    >
      <div className="flex items-center gap-3 text-sm text-ash">
        <span aria-hidden="true" className="flex gap-1.5">
          {STEPS.map((name, position) => (
            <span
              key={name}
              className={`h-1.5 w-6 rounded-full ${position <= index ? "bg-soft" : "bg-edge"}`}
            />
          ))}
        </span>
        <span>{t("welcome.step", { current: index + 1, total: STEPS.length })}</span>
        <button type="button" className="ml-auto min-h-11 px-2 hover:text-soft" disabled={busy} onClick={() => void finish()}>
          {t("welcome.skip")}
        </button>
      </div>

      {step === "language" && (
        <>
          <h2 id="welcome-title" className="mt-4 text-[28px] leading-tight font-bold">
            {t("welcome.title")}
          </h2>
          <p className="mt-2 text-ash">{t("welcome.intro")}</p>
          <fieldset className="mt-6 flex flex-col gap-0.5">
            <legend className="mb-1.5 text-[15px] font-bold text-ash">{t("settings.language")}</legend>
            {LANGUAGES.map((language) => {
              const chosen = i18n.language === language;
              return (
                <label
                  key={language}
                  className={`flex min-h-12 cursor-pointer items-center gap-3.5 rounded-md border px-3 ${
                    chosen ? "border-ctrl bg-night" : "border-transparent"
                  }`}
                >
                  <input
                    type="radio"
                    name="welcome-language"
                    className="ctl-check"
                    checked={chosen}
                    onChange={() => void chooseLanguage(language)}
                  />
                  <span className={chosen ? "font-bold" : undefined}>
                    {{ en: "English", fr: "Français", de: "Deutsch" }[language]}
                  </span>
                </label>
              );
            })}
          </fieldset>
          <Footer>
            <Button variant="primary" onClick={() => setStep("signer")}>
              {t("welcome.next")}
            </Button>
          </Footer>
        </>
      )}

      {step === "signer" && (
        <>
          <h2 id="welcome-title" className="mt-4 text-2xl font-bold">
            {t("welcome.signerTitle")}
          </h2>
          <p className="mt-2 text-ash">{t("welcome.signerIntro")}</p>
          <label className="mt-6 block text-sm font-bold">
            {t("settings.organization")}
            <input
              className="mt-1.5 block min-h-11 w-full rounded border border-ctrl bg-night px-3 font-normal"
              value={organization}
              autoFocus
              onChange={(event) => setOrganization(event.target.value)}
            />
          </label>
          <label className="mt-4 block text-sm font-bold">
            {t("settings.operator")}
            <input
              className="mt-1.5 block min-h-11 w-full rounded border border-ctrl bg-night px-3 font-normal"
              value={operator}
              onChange={(event) => setOperator(event.target.value)}
            />
            <span className="text-xs font-normal text-ash">{t("settings.operatorHint")}</span>
          </label>
          {tooLong && <p className="mt-2 text-sm text-sayno">{t("settings.tooLong")}</p>}
          {error && <p className="mt-2 text-sm text-sayno">{error}</p>}
          <Footer>
            <Button onClick={() => setStep("language")}>{t("welcome.back")}</Button>
            <Button variant="primary" disabled={busy || tooLong} onClick={() => void saveSigner()}>
              {t("welcome.next")}
            </Button>
          </Footer>
        </>
      )}

      {step === "license" && (
        <>
          <h2 id="welcome-title" className="mt-4 text-2xl font-bold">
            {t("welcome.licenseTitle")}
          </h2>
          {license?.state === "Valid" ? (
            <p className="mt-6 flex items-start gap-3 rounded-md border border-ctrl p-4">
              <BadgeCheck className="shrink-0 text-ready" aria-hidden="true" />
              <span>
                <span className="font-bold">{t("welcome.licenseValid")}</span>
                {(license.offer || until) && (
                  <span className="block text-sm text-ash">
                    {[license.offer, until && t("welcome.licenseUntil", { date: until })].filter(Boolean).join(" · ")}
                  </span>
                )}
              </span>
            </p>
          ) : (
            <>
              <p className="mt-6 flex items-start gap-3 rounded-md border border-blocked p-4">
                <TriangleAlert className="shrink-0 text-blocked" aria-hidden="true" />
                <span>
                  <span className="font-bold">
                    {t(`header.license.${license?.state ?? "Missing"}`)}
                  </span>
                  <span className="block text-sm text-ash">{t("welcome.licenseMissing")}</span>
                </span>
              </p>
              <Button className="mt-4 flex items-center gap-2" disabled={license === null} onClick={onLicense}>
                <KeyRound size={14} aria-hidden="true" />
                {t("welcome.licenseActivate")}
              </Button>
            </>
          )}
          <Footer>
            <Button onClick={() => setStep("signer")}>{t("welcome.back")}</Button>
            <Button variant="primary" onClick={() => setStep("ready")}>
              {t("welcome.next")}
            </Button>
          </Footer>
        </>
      )}

      {step === "ready" && (
        <>
          <h2 id="welcome-title" className="mt-4 text-2xl font-bold">
            {t("welcome.readyTitle")}
          </h2>
          <ol className="mt-6 flex flex-col gap-4">
            {(["plug", "erase", "certificate"] as const).map((tip, position) => (
              <li key={tip} className="flex gap-3.5">
                <span className="flex size-7 shrink-0 items-center justify-center rounded-full border border-ctrl font-mono text-sm">
                  {position + 1}
                </span>
                <span>
                  <span className="font-bold">{t(`welcome.tip.${tip}`)}</span>
                  <span className="block text-sm text-ash">{t(`welcome.tip.${tip}Hint`)}</span>
                </span>
              </li>
            ))}
          </ol>
          <p className="mt-6 text-sm text-ash">{t("welcome.later")}</p>
          <Footer>
            <Button onClick={() => setStep("license")}>{t("welcome.back")}</Button>
            <Button variant="primary" disabled={busy} onClick={() => void finish()}>
              {t("welcome.start")}
            </Button>
          </Footer>
        </>
      )}
    </dialog>
  );
}

function Footer({ children }: { children: React.ReactNode }) {
  return <div className="mt-8 flex justify-end gap-2">{children}</div>;
}
