import { useTranslation } from "react-i18next";
import { CheckCircle2, ChevronRight, LoaderCircle, XCircle } from "lucide-react";
import type { JobView } from "../lib/jobs";
import { Badge, Mono } from "./ui";

/** What the app knows about a job besides its events. */
export type JobInfo = {
  model: string;
  serial: string;
  resumed: boolean;
  batch: string | null;
  /** A host overwrite: no canaries are written (docs/moteur_decision.md §5). */
  hostWrite: boolean;
};

/** Thin progress line of a running job. */
export function JobProgress({ job }: { job: JobView }) {
  const { t } = useTranslation();
  const percent = job.progress?.percent ?? null;
  return (
    <div>
      <div className="flex items-center gap-2 text-xs text-ash">
        <LoaderCircle size={12} className="animate-spin text-brand" />
        {t(`job.phase.${job.phase}`)}
        {percent !== null && <span className="ml-auto font-mono">{percent} %</span>}
      </div>
      <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-night">
        <div
          className="h-full bg-brand transition-all duration-300"
          style={{ width: `${percent ?? 0}%` }}
        />
      </div>
    </div>
  );
}

/**
 * Every erasure of the session, newest first: running ones with their
 * progress, finished ones with their status. Drives erase in parallel
 * (protocole_ipc.md §7, check 9); this is where the operator follows them.
 */
export function JobsScreen({
  jobs,
  info,
  onOpen,
}: {
  jobs: Record<string, JobView>;
  info: Record<string, JobInfo>;
  onOpen: (id: string) => void;
}) {
  const { t } = useTranslation();
  const ids = Object.keys(info).reverse();
  const running = ids.filter((id) => jobs[id] && jobs[id].status === null).length;

  return (
    <section>
      <div className="mb-4 flex items-baseline gap-3">
        <h1 className="text-xl font-bold">{t("jobs.title")}</h1>
        {ids.length > 0 && <span className="text-sm text-ash">{t("jobs.running", { count: running })}</span>}
      </div>
      {ids.length === 0 && <p className="text-ash">{t("jobs.empty")}</p>}
      <ul className="space-y-2">
        {ids.map((id) => {
          const job = jobs[id];
          const meta = info[id];
          if (!job || !meta) return null;
          return (
            <li key={id}>
              <button
                type="button"
                onClick={() => onOpen(id)}
                className="flex w-full items-center gap-4 rounded-lg border border-edge bg-card p-4 text-left hover:border-ash/40"
              >
                <div className="min-w-0 flex-1">
                  <p className="font-bold">{meta.model}</p>
                  <p className="text-sm text-ash">
                    S/N <Mono>{meta.serial}</Mono>
                  </p>
                </div>
                <div className="w-56 shrink-0">
                  {job.status === null ? (
                    <JobProgress job={job} />
                  ) : (
                    <div className="flex items-center justify-end gap-2">
                      {job.status === "CONFORME" ? (
                        <CheckCircle2 size={16} className="text-ready" />
                      ) : (
                        <XCircle size={16} className="text-sayno" />
                      )}
                      <Badge tone={job.status === "CONFORME" ? "ready" : "sayno"}>
                        {t(`job.status.${job.status}`)}
                      </Badge>
                    </div>
                  )}
                </div>
                <ChevronRight size={16} className="shrink-0 text-ash" />
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
