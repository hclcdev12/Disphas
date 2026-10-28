import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronLeft, LoaderCircle } from "lucide-react";
import type { JobView } from "../lib/jobs";
import type { PdfState } from "../lib/pdf";
import { JobDetail } from "./JobScreen";
import { Bar } from "./ui";

/** What the app knows about a job besides its events. */
export type JobInfo = {
  model: string;
  serial: string;
  resumed: boolean;
  batch: string | null;
  /** Label of the batch, as typed when it was opened. */
  batchLabel?: string | null;
  /** A host overwrite: no canaries are written (docs/moteur_decision.md §5). */
  hostWrite: boolean;
  /** Method and category, unknown for a job resumed after a crash. */
  method?: string | null;
  category?: string | null;
  zeroPass?: boolean;
};

/** Thin progress line of a running job. */
export function JobProgress({ job }: { job: JobView }) {
  const { t } = useTranslation();
  const percent = job.progress?.percent ?? null;
  return (
    <div>
      <div className="flex items-center gap-2 text-sm text-ash">
        <LoaderCircle size={14} className="animate-spin" aria-hidden="true" />
        {t(`job.phase.${job.phase}`)}
        {percent !== null && <span className="ml-auto font-mono">{percent} %</span>}
      </div>
      <Bar percent={percent ?? 0} className="mt-1.5 h-1.5" />
    </div>
  );
}

/**
 * The Effacements tab, laid out like the Disques tab: the erasures of the
 * session on the left (running, then finished, newest first), the chosen
 * one on the right. Drives erase in parallel (protocole_ipc.md §7, check 9);
 * this is where the operator follows them, batch by batch.
 */
export function JobsScreen({
  jobs,
  info,
  labels,
  open,
  onOpen,
  pdfOf,
  onRetryPdf,
}: {
  jobs: Record<string, JobView>;
  info: Record<string, JobInfo>;
  labels: Record<string, string>;
  /** The erasure shown in detail; `null`: the first one. */
  open: string | null;
  onOpen: (id: string) => void;
  pdfOf: (id: string) => PdfState | undefined;
  onRetryPdf: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [view, setView] = useState<"list" | "detail">("list");
  const ids = Object.keys(info)
    .reverse()
    .filter((id) => jobs[id]);
  const running = ids.filter((id) => jobs[id].status === null);
  const done = ids.filter((id) => jobs[id].status !== null);
  const selected = (open !== null && ids.includes(open) ? open : null) ?? running[0] ?? done[0] ?? null;
  const name = (code: string) => labels[code] ?? code;

  // A batch erased in this session: how far it has gone, all drives together.
  const batches = [...new Set(ids.map((id) => info[id].batch).filter((batch): batch is string => batch !== null))].map(
    (batch) => {
      const members = ids.filter((id) => info[id].batch === batch);
      const finished = members.filter((id) => jobs[id].status !== null).length;
      const percent = Math.round(
        members.reduce(
          (sum, id) => sum + (jobs[id].status !== null ? 100 : (jobs[id].progress?.percent ?? 0)),
          0,
        ) / members.length,
      );
      const label = members.map((id) => info[id].batchLabel).find(Boolean) ?? batch;
      return { batch, label, finished, total: members.length, percent };
    },
  );

  const line = (id: string): string => {
    const job = jobs[id];
    const meta = info[id];
    const method = meta.method
      ? `${meta.category ?? ""} · ${name(meta.method)}${meta.zeroPass ? ` ${t("drives.thenZeros")}` : ""}`
      : t("job.resumedShort");
    if (job.status !== null) return method;
    return t(`job.phase.${job.phase}`);
  };

  const row = (id: string) => {
    const job = jobs[id];
    const meta = info[id];
    const isSelected = id === selected;
    const percent = job.progress?.percent ?? 0;
    return (
      <button
        key={id}
        type="button"
        aria-pressed={isSelected}
        onClick={() => {
          onOpen(id);
          setView("detail");
        }}
        className={`flex min-h-16 min-w-0 flex-col justify-center rounded-md border px-3 py-2 text-left ${
          isSelected ? "border-ctrl bg-card" : "border-transparent hover:bg-card/50"
        }`}
      >
        <span className="flex justify-between gap-2">
          <span className="flex min-w-0 items-center gap-2">
            {job.status === null && <LoaderCircle size={14} className="shrink-0 animate-spin" aria-hidden="true" />}
            <span className={`truncate text-base ${isSelected ? "font-bold" : ""}`}>{meta.model}</span>
          </span>
          {job.status === null ? (
            <span className="font-mono text-sm whitespace-nowrap text-ash">{percent} %</span>
          ) : (
            <span
              className={`font-mono text-sm whitespace-nowrap ${job.status === "CONFORME" ? "text-ready" : "text-sayno"}`}
            >
              {t(`job.status.${job.status}`)}
            </span>
          )}
        </span>
        <span className="truncate text-sm text-ash">
          {line(id)}
          {meta.batch && <> · {t("jobs.inBatch", { name: meta.batchLabel ?? meta.batch })}</>}
        </span>
        {job.status === null && <Bar percent={percent} className="mt-2 h-1" />}
      </button>
    );
  };

  const groups = [
    { key: "running", ids: running, tone: "text-soft" },
    { key: "done", ids: done, tone: "text-ash" },
  ].filter((group) => group.ids.length > 0);

  return (
    <div className="split-screen grid min-h-0 flex-1" data-view={view}>
      <div className="split-lhead flex flex-col gap-4 border-r border-edge px-4 pt-6 pb-5">
        <div className="px-2">
          <h1 className="text-[25px] leading-tight font-bold">{t("jobs.count", { count: ids.length })}</h1>
          <p className="mt-0.5 font-mono text-sm text-ash">
            {t("jobs.meta", { running: running.length, done: done.length })}
          </p>
        </div>
        {/* Each batch of the session, all its drives together. */}
        {batches.map((batch) => (
          <div key={batch.batch} className="flex flex-col gap-1.5 px-2 text-sm">
            <span className="flex justify-between gap-2">
              <span className="truncate font-bold">{t("jobs.batch", { name: batch.label })}</span>
              <span className="font-mono whitespace-nowrap text-ash">
                {t("jobs.batchDone", { done: batch.finished, total: batch.total })}
              </span>
            </span>
            <Bar percent={batch.percent} className="h-1.5" />
          </div>
        ))}
      </div>

      <div className="split-dhead flex min-w-0 max-w-[960px] flex-col gap-4 px-12 pt-10 pb-5">
        <button
          type="button"
          onClick={() => setView("list")}
          aria-label={t("jobs.backLabel")}
          className="split-back min-h-11 items-center gap-2 self-start rounded pr-3 pl-1"
        >
          <ChevronLeft size={18} aria-hidden="true" />
          {t("jobs.title")}
        </button>
        {selected && (
          <div className="min-w-0">
            <div className="font-mono text-sm break-words text-ash">
              {jobs[selected].status === null ? t("job.title") : t("job.finished")}
              {info[selected].method && <> · {name(info[selected].method ?? "")}</>}
              {info[selected].zeroPass && <> {t("drives.thenZeros")}</>}
            </div>
            <h2 className="split-title mt-0.5 mb-1 text-[40px] leading-tight font-bold break-words">
              {info[selected].model}
            </h2>
            <div className="font-mono text-sm break-all text-ash">
              s/n {info[selected].serial} · {selected}
            </div>
          </div>
        )}
      </div>

      {/* Same row as the detail below: the first erasure faces the steps. */}
      <aside className="split-lbody flex flex-col gap-5 border-r border-edge px-4 pb-6" aria-label={t("jobs.title")}>
        {ids.length === 0 && <p className="px-2 text-ash">{t("jobs.empty")}</p>}
        {groups.map((group) => (
          <section key={group.key} aria-labelledby={`jobs-${group.key}`} className="flex flex-col gap-0.5">
            <h2
              id={`jobs-${group.key}`}
              className={`mb-1.5 flex items-center gap-2 px-2 text-[15px] font-bold ${group.tone}`}
            >
              <span className="size-2 rounded-full bg-current" aria-hidden="true" />
              {t(`jobs.group.${group.key}`, { count: group.ids.length })}
            </h2>
            {group.ids.map(row)}
          </section>
        ))}
      </aside>

      <main className="split-dbody flex min-w-0 max-w-[960px] flex-col gap-7 px-12 pb-10" aria-live="polite">
        {!selected && <p className="pt-1 text-ash">{t("jobs.emptyHint")}</p>}
        {selected && (
          <JobDetail
            job={jobs[selected]}
            resumed={info[selected].resumed}
            hostWrite={info[selected].hostWrite}
            pdf={pdfOf(selected)}
            onRetryPdf={() => onRetryPdf(selected)}
          />
        )}
      </main>
    </div>
  );
}
