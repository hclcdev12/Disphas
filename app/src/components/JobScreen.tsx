import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { CheckCircle2, Circle, FileText, History, LoaderCircle, TriangleAlert, XCircle } from "lucide-react";
import type { JobPhase } from "../bindings/JobPhase";
import { api } from "../lib/api";
import type { PdfState } from "../lib/pdf";
import type { JobView } from "../lib/jobs";
import { Badge, Bar, Button, Mono } from "./ui";

const PHASES: JobPhase[] = ["Preparing", "Canaries", "Executing", "Verifying", "Validating", "Signing", "Done"];

function bytes(value: number, lang: string): string {
  const format = new Intl.NumberFormat(lang, { maximumFractionDigits: 1 });
  return value >= 1e9 ? `${format.format(value / 1e9)} GB` : `${format.format(value / 1e6)} MB`;
}

function duration(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  return minutes > 0 ? `${minutes} min ${seconds % 60} s` : `${seconds} s`;
}

/**
 * One erasure, followed then closed (UI spec §4, screens 3 and 4), laid out
 * like the detail of a drive: the steps level with the first erasure of the
 * list, the progress, then the result and its certificate. Warnings are
 * outlined, never on a tinted background.
 */
export function JobDetail({
  job,
  resumed = false,
  hostWrite = false,
  pdf,
  onRetryPdf,
}: {
  job: JobView;
  /** Resumed by the helper after a crash (docs/protocole_ipc.md §10). */
  resumed?: boolean;
  /** A host overwrite has no canaries step. */
  hostWrite?: boolean;
  /** Certificate rendered automatically when the job ended. */
  pdf: PdfState | undefined;
  onRetryPdf: () => void;
}) {
  const { t, i18n } = useTranslation();
  // Only the steps this method goes through: a host overwrite writes no canaries.
  const phases = hostWrite ? PHASES.filter((phase) => phase !== "Canaries") : PHASES;
  const current = phases.indexOf(job.phase);
  const finished = job.status !== null;
  const progress = job.progress;

  // ATA Security Erase reports nothing until it ends: count down from the
  // drive's own estimate (IDENTIFY words 89/90), sent with its start.
  const estimate =
    !finished && progress?.source === "firmware" && progress.percent === 0 && progress.remaining_seconds !== null
      ? progress.remaining_seconds
      : null;
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (estimate === null) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [estimate]);
  const elapsed =
    estimate !== null && job.progressAt !== null ? Math.max(0, Math.floor((now - job.progressAt) / 1000)) : 0;

  return (
    <>
      {/* Same rhythm as the list: a title like a group's, level with the first erasure. */}
      <div className="flex min-w-0 flex-col gap-0.5">
        <h3 className="mb-1.5 flex items-center gap-2 px-2 text-[15px] font-bold text-ash">{t("job.steps")}</h3>
        <ol
          className="job-steps grid min-h-16 max-w-[64rem] items-center gap-2 text-center text-sm"
          style={{ gridTemplateColumns: `repeat(${phases.length}, minmax(0, 1fr))` }}
        >
          {phases.map((phase, index) => {
            const done = finished || index < current;
            const active = !finished && index === current;
            return (
              <li
                key={phase}
                aria-current={active ? "step" : undefined}
                className={`flex flex-col items-center gap-1.5 ${active ? "font-bold text-soft" : done ? "text-soft" : "text-ash"}`}
              >
                {done ? (
                  <CheckCircle2 size={18} className="text-ready" aria-hidden="true" />
                ) : active ? (
                  <LoaderCircle size={18} className="animate-spin" aria-hidden="true" />
                ) : (
                  <Circle size={18} aria-hidden="true" />
                )}
                {t(`job.phase.${phase}`)}
              </li>
            );
          })}
        </ol>
      </div>

      {estimate !== null && (
        <div>
          <Bar percent={Math.min(99, Math.round((elapsed * 100) / Math.max(1, estimate)))} className="h-2" />
          <p className="mt-2 flex flex-wrap gap-3 text-sm text-ash">
            <span>{t("job.elapsed", { elapsed: duration(elapsed) })}</span>
            {elapsed < estimate ? (
              <span>{t("job.estimateLeft", { eta: duration(estimate - elapsed) })}</span>
            ) : (
              <span className="text-blocked">{t("job.estimateExceeded", { max: duration(estimate * 2) })}</span>
            )}
          </p>
          <p className="mt-1 text-sm text-ash">{t("job.firmwareSilent")}</p>
        </div>
      )}

      {!finished && progress && estimate === null && (
        <div>
          <Bar percent={progress.percent} className="h-2" />
          <p className="mt-2 flex flex-wrap gap-3 font-mono text-sm text-ash">
            <span className="text-soft">{progress.percent} %</span>
            {progress.bytes_done !== null && <span>{bytes(progress.bytes_done, i18n.language)}</span>}
            {progress.bytes_per_second !== null && (
              <span>{t("job.rate", { rate: bytes(progress.bytes_per_second, i18n.language) })}</span>
            )}
            {progress.remaining_seconds !== null && (
              <span>{t("job.eta", { eta: duration(progress.remaining_seconds) })}</span>
            )}
          </p>
        </div>
      )}

      {resumed && (
        <p className="flex items-start gap-3 rounded-md border border-ctrl px-4 py-3.5">
          <History size={18} className="mt-1 shrink-0" aria-hidden="true" />
          {t("job.resumed")}
        </p>
      )}

      {!finished && (
        <p className="flex items-start gap-3 rounded-md border border-blocked px-4 py-3.5">
          <TriangleAlert size={18} className="mt-1 shrink-0 text-blocked" aria-hidden="true" />
          {t("job.warning")}
        </p>
      )}

      {finished && job.status && (
        <div className="flex flex-col gap-4 rounded-lg border border-ctrl p-6">
          <div className="flex flex-wrap items-center gap-3">
            {job.status === "CONFORME" ? (
              <CheckCircle2 className="text-ready" aria-hidden="true" />
            ) : (
              <XCircle className="text-sayno" aria-hidden="true" />
            )}
            <Badge tone={job.status === "CONFORME" ? "ready" : "sayno"}>{t(`job.status.${job.status}`)}</Badge>
            <span className="text-[17px]">{t(`job.statusHint.${job.status}`)}</span>
          </div>
          {/* The certificate is saved as soon as the erasure ends. */}
          <div className="flex flex-wrap items-center gap-4">
            {(pdf === undefined || pdf.kind === "rendering") && <span className="text-ash">{t("job.exporting")}</span>}
            {pdf?.kind === "saved" && (
              <>
                <Button
                  variant="primary"
                  className="flex min-h-12 items-center gap-2 px-5.5 text-[17px]"
                  onClick={() => void api.openCertificate(pdf.path)}
                >
                  <FileText size={16} aria-hidden="true" />
                  {t("job.openCertificate")}
                </Button>
                <span className="text-sm text-ash">
                  {t("job.saved")} <Mono>{pdf.path}</Mono>
                </span>
              </>
            )}
            {pdf?.kind === "failed" && (
              <>
                <span className="text-sayno">
                  {t("job.pdfFailed")} {t(`errors.${pdf.error.code}`, { defaultValue: pdf.error.code })}
                  {pdf.error.detail && <span className="block font-mono text-xs text-ash">{pdf.error.detail}</span>}
                </span>
                <Button onClick={onRetryPdf}>{t("job.retryPdf")}</Button>
              </>
            )}
          </div>
        </div>
      )}

      {/* The technical journal stays folded: it is there for an audit, not to follow the erasure. */}
      <details>
        <summary className="cursor-pointer text-sm font-bold text-ash hover:text-soft">{t("job.log")}</summary>
        <ul className="mt-2 max-h-56 space-y-1 overflow-y-auto rounded-md border border-edge p-3 font-mono text-xs">
          {job.logs.map((log, index) => (
            <li
              key={index}
              className={log.level === "error" ? "text-sayno" : log.level === "warning" ? "text-blocked" : "text-ash"}
            >
              {t(`job.logCodes.${log.code}`, { ...log.params, defaultValue: log.code })}
            </li>
          ))}
        </ul>
      </details>
    </>
  );
}
