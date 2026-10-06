import { useState } from "react";
import { useTranslation } from "react-i18next";
import { CheckCircle2, Circle, FileText, History, LoaderCircle, TriangleAlert, XCircle } from "lucide-react";
import type { JobPhase } from "../bindings/JobPhase";
import { api, type HelperError } from "../lib/api";
import type { JobView } from "../lib/jobs";
import type { Language } from "../i18n";
import { Badge, Button, Mono } from "./ui";

const PHASES: JobPhase[] = ["Preparing", "Canaries", "Executing", "Verifying", "Validating", "Signing", "Done"];

function bytes(value: number, lang: string): string {
  const format = new Intl.NumberFormat(lang, { maximumFractionDigits: 1 });
  return value >= 1e9 ? `${format.format(value / 1e9)} GB` : `${format.format(value / 1e6)} MB`;
}

function duration(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  return minutes > 0 ? `${minutes} min ${seconds % 60} s` : `${seconds} s`;
}

/** Monitoring and closing screens of an erasure (UI spec §4, screens 3 and 4). */
export function JobScreen({
  jobId,
  job,
  model,
  resumed = false,
  hostWrite = false,
  onBack,
}: {
  jobId: string;
  job: JobView;
  model: string;
  /** Resumed by the helper after a crash (docs/protocole_ipc.md §10). */
  resumed?: boolean;
  /** A host overwrite has no canaries step. */
  hostWrite?: boolean;
  onBack: () => void;
}) {
  const { t, i18n } = useTranslation();
  const [exporting, setExporting] = useState(false);
  const [saved, setSaved] = useState<string | null>(null);
  const [error, setError] = useState<HelperError | null>(null);
  // Only the steps this method goes through: a host overwrite writes no canaries.
  const phases = hostWrite ? PHASES.filter((phase) => phase !== "Canaries") : PHASES;
  const current = phases.indexOf(job.phase);
  const finished = job.status !== null;
  const progress = job.progress;

  const exportPdf = async () => {
    setExporting(true);
    setError(null);
    const result = await api.renderCertificate(jobId, i18n.language as Language);
    if (result.ok) setSaved(result.value);
    else setError(result.error);
    setExporting(false);
  };

  const tone = job.status === "CONFORME" ? "ready" : "sayno";
  return (
    <section className="rounded-lg border border-edge bg-card p-6">
      <div className="flex items-center gap-3">
        <h1 className="text-xl font-bold">{finished ? t("job.finished") : t("job.title")}</h1>
        <span className="text-ash">{model}</span>
      </div>
      <p className="mt-1 text-xs text-ash">
        <Mono>{jobId}</Mono>
      </p>

      {resumed && (
        <div className="mt-4 flex gap-2 rounded-md border border-brand/40 bg-brand/10 p-3 text-sm text-soft">
          <History size={16} className="mt-0.5 shrink-0 text-brand" />
          {t("job.resumed")}
        </div>
      )}

      {!finished && (
        <div className="mt-4 flex gap-2 rounded-md border border-blocked/40 bg-blocked/10 p-3 text-sm text-blocked">
          <TriangleAlert size={16} className="mt-0.5 shrink-0" />
          {t("job.warning")}
        </div>
      )}

      <ol
        className="mt-5 grid gap-2 text-center text-xs"
        style={{ gridTemplateColumns: `repeat(${phases.length}, minmax(0, 1fr))` }}
      >
        {phases.map((phase, index) => {
          const done = finished || index < current;
          const active = !finished && index === current;
          return (
            <li key={phase} className={`flex flex-col items-center gap-1.5 ${done || active ? "text-soft" : "text-ash/50"}`}>
              {done ? (
                <CheckCircle2 size={18} className="text-ready" />
              ) : active ? (
                <LoaderCircle size={18} className="animate-spin text-brand" />
              ) : (
                <Circle size={18} />
              )}
              {t(`job.phase.${phase}`)}
            </li>
          );
        })}
      </ol>

      {!finished && progress && (
        <div className="mt-5">
          <div className="h-2 overflow-hidden rounded-full bg-night">
            <div className="h-full bg-brand transition-all duration-300" style={{ width: `${progress.percent}%` }} />
          </div>
          <p className="mt-1.5 flex gap-3 text-xs text-ash">
            <span>{progress.percent} %</span>
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

      {finished && job.status && (
        <div className={`mt-6 rounded-lg border p-4 ${tone === "ready" ? "border-ready/40 bg-ready/10" : "border-sayno/40 bg-sayno/10"}`}>
          <div className="flex items-center gap-3">
            {job.status === "CONFORME" ? (
              <CheckCircle2 className="text-ready" />
            ) : (
              <XCircle className="text-sayno" />
            )}
            <Badge tone={tone}>{t(`job.status.${job.status}`)}</Badge>
            <span className="text-sm">{t(`job.statusHint.${job.status}`)}</span>
          </div>
          <div className="mt-4 flex flex-wrap items-center gap-3">
            <Button variant="primary" className="flex items-center gap-2" disabled={exporting} onClick={() => void exportPdf()}>
              <FileText size={15} />
              {exporting ? t("job.exporting") : t("job.export")}
            </Button>
            {saved && (
              <>
                <span className="text-xs text-ash">
                  {t("job.saved")} <Mono>{saved}</Mono>
                </span>
                <Button onClick={() => void api.openCertificate(saved)}>{t("job.open")}</Button>
              </>
            )}
          </div>
          {error && (
            <p className="mt-2 text-sm text-sayno">
              {t(`errors.${error.code}`, { defaultValue: error.code })}
              {error.detail && <span className="block font-mono text-xs text-ash">{error.detail}</span>}
            </p>
          )}
        </div>
      )}

      <details className="mt-6" open={!finished}>
        <summary className="cursor-pointer text-sm font-bold text-ash">{t("job.log")}</summary>
        <ul className="mt-2 max-h-48 space-y-1 overflow-y-auto rounded-md border border-edge bg-night p-3 font-mono text-xs">
          {job.logs.map((log, index) => (
            <li key={index} className={log.level === "error" ? "text-sayno" : log.level === "warning" ? "text-blocked" : "text-ash"}>
              {t(`job.logCodes.${log.code}`, { ...log.params, defaultValue: log.code })}
            </li>
          ))}
        </ul>
      </details>

      {finished && (
        <div className="mt-6 flex justify-end">
          <Button onClick={onBack}>{t("job.back")}</Button>
        </div>
      )}
    </section>
  );
}
