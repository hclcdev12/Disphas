import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { LicenseStatusResult } from "../bindings/LicenseStatusResult";
import { api, type HelperError } from "../lib/api";
import { Button, Dialog, Fact, Mono, Section } from "./ui";

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
  const [request, setRequest] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [license, setLicense] = useState("");
  const [answer, setAnswer] = useState<HelperError | "installed" | null>(null);
  const [busy, setBusy] = useState(false);

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

  const entitlements = status.entitlements;
  return (
    <Dialog
      title={t("license.title")}
      onCancel={onClose}
      footer={<Button onClick={onClose}>{t("license.close")}</Button>}
    >
      <dl>
        <Fact label={t("license.state")}>
          {/* The state in its colour, as in the lists: no filled chip that looks like a button. */}
          <span className={`font-mono ${status.state === "Valid" ? "text-ready" : "text-blocked"}`}>
            {t(`header.license.${status.state}`)}
          </span>
        </Fact>
        {status.license_id && (
          <Fact label={t("license.id")}>
            <Mono>{status.license_id}</Mono>
          </Fact>
        )}
        {status.offer && <Fact label={t("license.offer")}>{status.offer}</Fact>}
        {entitlements && (
          <>
            <Fact label={t("license.parallel")}>{entitlements.max_parallel ?? t("license.parallelUnlimited")}</Fact>
            <Fact label={t("license.customerLogo")}>
              {entitlements.customer_logo ? t("license.yes") : t("license.no")}
            </Fact>
          </>
        )}
        {until && <Fact label={t("license.until")}>{until}</Fact>}
      </dl>

      <Section title={t("license.activateTitle")}>
        <ol className="flex flex-col gap-5">
          <Step number={1} title={t("license.stepRequest")}>
            <p className="text-sm text-ash">{t("license.requestHint")}</p>
            {request ? (
              <>
                <textarea
                  readOnly
                  className="block h-20 w-full resize-none rounded border border-ctrl bg-night p-2.5 font-mono text-xs break-all"
                  value={request}
                />
                <Button className="self-start" onClick={() => void copy()}>
                  {copied ? t("license.copied") : t("license.copy")}
                </Button>
              </>
            ) : (
              <Button className="self-start" disabled={busy} onClick={() => void createRequest()}>
                {t("license.activate")}
              </Button>
            )}
          </Step>
          <Step number={2} title={t("license.stepLicense")}>
            <textarea
              aria-label={t("license.stepLicense")}
              className="block h-20 w-full resize-none rounded border border-ctrl bg-night p-2.5 font-mono text-xs break-all"
              spellCheck={false}
              placeholder="DSL1:…"
              value={license}
              onChange={(event) => setLicense(event.target.value)}
            />
            <Button
              variant="primary"
              className="self-start"
              disabled={busy || !license.trim().startsWith("DSL1:")}
              onClick={() => void install()}
            >
              {t("license.installButton")}
            </Button>
          </Step>
        </ol>
      </Section>

      {answer === "installed" && (
        <p role="status" className="text-sm text-ready">
          {t("license.installed")}
        </p>
      )}
      {answer && answer !== "installed" && (
        <p role="alert" className="text-sm text-sayno">
          {t(`errors.${answer.code}`, { defaultValue: answer.code })}
          {answer.detail && <span className="block font-mono text-xs text-ash">{answer.detail}</span>}
        </p>
      )}
    </Dialog>
  );
}

/** One step of the activation, numbered as in the first-launch assistant. */
function Step({ number, title, children }: { number: number; title: string; children: React.ReactNode }) {
  return (
    <li className="flex gap-3.5">
      <span className="flex size-7 shrink-0 items-center justify-center rounded-full border border-ctrl font-mono text-sm">
        {number}
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-2 pt-0.5">
        <span className="font-bold">{title}</span>
        {children}
      </div>
    </li>
  );
}
