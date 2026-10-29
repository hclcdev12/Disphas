import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Moon } from "lucide-react";
import { api, type HelperError } from "../lib/api";
import { Button, Callout, Dialog } from "./ui";

/**
 * Puts the machine to sleep to thaw frozen drives (B1, UX review D5). Most
 * firmwares freeze the ATA security of SATA drives at boot; a sleep cuts
 * their power and they come back thawed. The operator wakes the machine
 * (a key, the power button); the app then lists and inspects the drives
 * again. Refused while an erasure runs: the sleep would cut it.
 */
export function SleepDialog({ frozen, onCancel, onAsleep }: { frozen: number; onCancel: () => void; onAsleep: () => void }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<HelperError | null>(null);

  const sleep = async () => {
    setBusy(true);
    setError(null);
    const result = await api.suspend();
    setBusy(false);
    if (result.ok) onAsleep();
    else setError(result.error);
  };

  return (
    <Dialog
      title={t("sleep.title")}
      width="sm"
      onCancel={onCancel}
      footer={
        <>
          <Button onClick={onCancel}>{t("actions.cancel")}</Button>
          <Button variant="primary" className="flex items-center gap-2" disabled={busy} onClick={() => void sleep()}>
            <Moon size={16} aria-hidden="true" />
            {t("sleep.confirm")}
          </Button>
        </>
      }
    >
      <p>{t("sleep.explain", { count: frozen })}</p>
      <ol className="flex list-decimal flex-col gap-1.5 pl-5 text-sm">
        <li>{t("sleep.step1")}</li>
        <li>{t("sleep.step2")}</li>
        <li>{t("sleep.step3")}</li>
      </ol>
      <Callout tone="blocked">{t("sleep.save")}</Callout>
      {error && (
        <p role="alert" className="text-sm text-sayno">
          {error.code === "JOBS_RUNNING" ? t("sleep.jobsRunning") : t("sleep.failed")}
          {error.detail && <span className="block font-mono text-xs text-ash">{error.detail}</span>}
        </p>
      )}
    </Dialog>
  );
}
