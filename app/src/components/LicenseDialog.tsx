import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { LicenseStatusResult } from "../bindings/LicenseStatusResult";
import { api, type HelperError } from "../lib/api";
import { Badge, Button, Mono } from "./ui";

/**
 * Offline activation (certificat.md §4): the workstation creates a request
 * holding its public key, the operator sends it to disphas and pastes the
 * license received. The helper verifies and installs it.
 */
export function LicenseDialog({
  status,
  onClose,
  onChanged,
}: {
  status: LicenseStatusResult;
  onClose: () => void;
  onChanged: (status: LicenseStatusResult) => void;
}) {
  const { t, i18n } = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [request, setRequest] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [license, setLicense] = useState("");
  const [answer, setAnswer] = useState<HelperError | "installed" | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

  const createRequest = async () => {
    setBusy(true);
    const result = await api.createActivationRequest();
    if (result.ok) setRequest(result.value.request);
    else setAnswer(result.error);
    setBusy(false);
  };

  const install = async () => {
    setBusy(true);
    const result = await api.installLicense(license.trim());
    if (result.ok) {
      setAnswer("installed");
      onChanged(result.value);
    } else {
      setAnswer(result.error);
    }
    setBusy(false);
  };

  const copy = async () => {
    if (!request) return;
    await navigator.clipboard.writeText(request);
    setCopied(true);
  };

  const until =
    status.valid_until === null
      ? null
      : new Intl.DateTimeFormat(i18n.language, { dateStyle: "long" }).format(new Date(status.valid_until * 1000));

  return (
    <dialog
      ref={dialog}
      onCancel={onClose}
      className="m-auto w-[36rem] max-w-[92vw] rounded-lg border border-edge bg-card p-6 text-soft backdrop:bg-black/70"
    >
      <h2 className="text-lg font-bold">{t("license.title")}</h2>
      <dl className="mt-3 grid grid-cols-[9rem_1fr] gap-y-1.5 text-sm">
        <dt className="text-ash">{t("license.state")}</dt>
        <dd>
          <Badge tone={status.state === "Valid" ? "ready" : "neutral"}>{t(`header.license.${status.state}`)}</Badge>
        </dd>
        {status.license_id && (
          <>
            <dt className="text-ash">{t("license.id")}</dt>
            <dd>
              <Mono>{status.license_id}</Mono>
            </dd>
          </>
        )}
        {status.offer && (
          <>
            <dt className="text-ash">{t("license.offer")}</dt>
            <dd>{status.offer}</dd>
          </>
        )}
        {until && (
          <>
            <dt className="text-ash">{t("license.until")}</dt>
            <dd>{until}</dd>
          </>
        )}
      </dl>

      <section className="mt-5 border-t border-edge pt-4">
        <h3 className="text-sm font-bold">{t("license.activate")}</h3>
        {request ? (
          <>
            <p className="mt-1 text-xs text-ash">{t("license.requestHint")}</p>
            <textarea
              readOnly
              className="mt-2 block h-20 w-full resize-none rounded-md border border-edge bg-night p-2 font-mono text-xs break-all"
              value={request}
            />
            <Button className="mt-2" onClick={() => void copy()}>
              {copied ? t("license.copied") : t("license.copy")}
            </Button>
          </>
        ) : (
          <Button className="mt-2" disabled={busy} onClick={() => void createRequest()}>
            {t("license.activate")}
          </Button>
        )}
      </section>

      <section className="mt-5 border-t border-edge pt-4">
        <label className="block text-sm font-bold">
          {t("license.install")}
          <textarea
            className="mt-2 block h-20 w-full resize-none rounded-md border border-edge bg-night p-2 font-mono text-xs font-normal break-all"
            spellCheck={false}
            placeholder="DSL1:…"
            value={license}
            onChange={(event) => setLicense(event.target.value)}
          />
        </label>
        <Button
          variant="primary"
          className="mt-2"
          disabled={busy || !license.trim().startsWith("DSL1:")}
          onClick={() => void install()}
        >
          {t("license.installButton")}
        </Button>
      </section>

      {answer === "installed" && <p className="mt-4 text-sm text-ready">{t("license.installed")}</p>}
      {answer && answer !== "installed" && (
        <p className="mt-4 text-sm text-sayno">
          {t(`errors.${answer.code}`, { defaultValue: answer.code })}
          {answer.detail && <span className="block font-mono text-xs text-ash">{answer.detail}</span>}
        </p>
      )}

      <div className="mt-6 flex justify-end">
        <Button onClick={onClose}>{t("license.close")}</Button>
      </div>
    </dialog>
  );
}
