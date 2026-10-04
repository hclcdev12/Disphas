import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw, TriangleAlert, X } from "lucide-react";
import type { Acknowledgement } from "./bindings/Acknowledgement";
import type { HelloResult } from "./bindings/HelloResult";
import type { InspectResult } from "./bindings/InspectResult";
import type { JobSummary } from "./bindings/JobSummary";
import type { ListDevicesResult } from "./bindings/ListDevicesResult";
import type { LicenseStatusResult } from "./bindings/LicenseStatusResult";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { JobScreen } from "./components/JobScreen";
import { LicenseDialog } from "./components/LicenseDialog";
import { UnlockDialog } from "./components/UnlockDialog";
import { applyEvent, emptyJob, seedJobs, type JobView } from "./lib/jobs";
import { DriveCard, type EraseRequest } from "./components/DriveCard";
import { Header } from "./components/Header";
import { HistoryScreen } from "./components/HistoryScreen";
import { JobsScreen, type JobInfo } from "./components/JobsScreen";
import { BatchBar, type BatchNotice } from "./components/BatchBar";
import type { BatchArgs } from "./bindings/BatchArgs";
import { loadBatch, saveBatch } from "./lib/batch";
import { VerifyScreen } from "./components/VerifyScreen";
import { Button, Mono } from "./components/ui";
import { api, type HelperError, type Outcome } from "./lib/api";
import type { Language } from "./i18n";

export default function App() {
  const { t, i18n } = useTranslation();
  const [hello, setHello] = useState<Outcome<HelloResult> | null>(null);
  const [devices, setDevices] = useState<Outcome<ListDevicesResult> | null>(null);
  const [inspections, setInspections] = useState<Record<string, Outcome<InspectResult>>>({});
  const [labels, setLabels] = useState<Record<string, string>>({});
  const [request, setRequest] = useState<EraseRequest | null>(null);
  const [license, setLicense] = useState<LicenseStatusResult | null>(null);
  const [licenseOpen, setLicenseOpen] = useState(false);
  const [jobs, setJobs] = useState<Record<string, JobView>>({});
  // Several drives erase in parallel: every job of the session, and the one opened in detail.
  const [jobInfo, setJobInfo] = useState<Record<string, JobInfo>>({});
  const [openJob, setOpenJob] = useState<string | null>(null);
  const jobInfoRef = useRef(jobInfo);
  useEffect(() => {
    jobInfoRef.current = jobInfo;
  }, [jobInfo]);
  const [interrupted, setInterrupted] = useState<JobSummary[]>([]);
  const [unlocking, setUnlocking] = useState<InspectResult | null>(null);
  const [view, setView] = useState<"drives" | "jobs" | "history" | "verify">("drives");
  const [batch, setBatch] = useState<BatchArgs | null>(loadBatch);
  const [batchNotice, setBatchNotice] = useState<BatchNotice | null>(null);
  const [closingBatch, setClosingBatch] = useState(false);
  const changeBatch = (next: BatchArgs | null) => {
    setBatch(next);
    saveBatch(next);
  };

  // Closing a batch renders its batch certificate at once. Not while one of
  // its erasures runs: its certificate would be missing from the PDF.
  const closeBatch = async () => {
    if (!batch) return;
    // A job resumed after a crash may belong to the batch: it blocks too.
    const running = Object.keys(jobInfo).filter(
      (id) =>
        (jobInfo[id].batch === batch.id || jobInfo[id].resumed) && jobs[id] && jobs[id].status === null,
    ).length;
    if (running > 0) {
      setBatchNotice({ kind: "running", count: running });
      return;
    }
    setClosingBatch(true);
    const id = batch.id;
    const result = await api.renderBatch(id, i18n.language as Language);
    setClosingBatch(false);
    if (!result.ok) {
      // The batch stays open: the operator can retry.
      setBatchNotice({ kind: "error", batch: id, error: result.error });
      return;
    }
    changeBatch(null);
    setBatchNotice(result.value === null ? { kind: "empty", batch: id } : { kind: "pdf", batch: id, path: result.value });
  };

  // Every drive is inspected by the helper; the UI only displays verdicts.
  const reinspect = useCallback(
    async (serial: string) => {
      if (!devices?.ok) return;
      const summary = devices.value.devices.find((drive) => drive.device.identity.serial === serial);
      if (!summary) return;
      const inspection = await api.inspect(summary.device);
      setInspections((current) => ({ ...current, [serial]: inspection }));
    },
    [devices],
  );
  const reinspectRef = useRef(reinspect);
  useEffect(() => {
    reinspectRef.current = reinspect;
  }, [reinspect]);

  // Subscribed before any Sanitize, so no job event is ever missed. A finished
  // erasure changes its drive: it is inspected again.
  useEffect(() => {
    const unsubscribe = api.onEvent((event) => {
      setJobs((current) => applyEvent(current, event));
      if (event.event === "JobFinished") {
        const info = jobInfoRef.current[event.data.job_id];
        if (info) void reinspectRef.current(info.serial);
      }
    });
    return () => void unsubscribe.then((stop) => stop());
  }, []);

  const refresh = useCallback(async () => {
    setInspections({});
    const listed = await api.listDevices();
    setDevices(listed);
    if (!listed.ok) return;
    for (const summary of listed.value.devices) {
      const inspection = await api.inspect(summary.device);
      setInspections((current) => ({ ...current, [summary.device.identity.serial]: inspection }));
    }
  }, []);

  useEffect(() => {
    void (async () => {
      const greeted = await api.hello();
      setHello(greeted);
      if (greeted.ok) {
        setLicense(greeted.value.license);
        await refresh();
        // Jobs known at start-up come from a helper that crashed (protocole_ipc.md §10):
        // a resumed Sanitize is followed on screen, interrupted ones are reported.
        const listed = await api.listJobs();
        if (listed.ok) {
          setJobs((current) => seedJobs(current, listed.value.jobs));
          setInterrupted(listed.value.jobs.filter((job) => job.phase === "Interrupted"));
          const resumed = listed.value.jobs.filter((job) => job.phase !== "Interrupted");
          if (resumed.length > 0) {
            setJobInfo((current) => {
              const next = { ...current };
              for (const job of resumed) {
                next[job.job_id] = {
                  model: job.device.identity.model,
                  serial: job.device.identity.serial,
                  resumed: true,
                  batch: null,
                };
              }
              return next;
            });
            setView("jobs");
          }
        }
      }
    })();
  }, [refresh]);

  useEffect(() => {
    void api.codeLabels(i18n.language as Language).then(setLabels);
  }, [i18n.language]);

  const confirm = async (input: {
    confirmation: string;
    acknowledged: Acknowledgement[];
    justification: string | null;
  }): Promise<HelperError | null> => {
    if (!request) return null;
    const result = await api.sanitize({
      verdict_id: request.inspection.verdict_id,
      device: request.inspection.device,
      method: request.choice.method,
      confirmation: input.confirmation,
      acknowledged: input.acknowledged,
      force: input.justification === null ? null : { justification: input.justification },
      operator: null,
      organization: null,
      batch,
    });
    if (!result.ok) return result.error;
    const { model, serial } = request.inspection.device.identity;
    setJobInfo((current) => ({
      ...current,
      [result.value.job_id]: { model, serial, resumed: false, batch: batch?.id ?? null },
    }));
    setRequest(null);
    setOpenJob(null);
    // In a batch the operator chains drives: stay on the list, each card shows
    // its own progress. A single erasure opens the Erasures tab.
    if (!batch) setView("jobs");
    return null;
  };

  /** The erasure in progress on a drive, if any. */
  const runningOn = (serial: string) => {
    const id = Object.keys(jobInfo).find(
      (key) => jobInfo[key].serial === serial && jobs[key] && jobs[key].status === null,
    );
    return id ? { id, job: jobs[id] } : null;
  };

  const connected = hello?.ok ?? false;
  return (
    <div className="flex min-h-full flex-col">
      <Header
        hello={hello?.ok ? hello.value : null}
        mode={api.mode}
        connected={connected}
        license={license}
        onLicense={() => setLicenseOpen(true)}
      />
      <main className="mx-auto w-full max-w-4xl flex-1 px-6 py-6">
        <>
        <nav className="mb-5 flex gap-1 border-b border-edge">
          {(["drives", "jobs", "history", "verify"] as const).map((tab) => (
            <button
              key={tab}
              type="button"
              onClick={() => {
                setView(tab);
                setOpenJob(null);
              }}
              className={`-mb-px border-b-2 px-3 py-2 text-sm font-bold ${
                view === tab ? "border-brand text-soft" : "border-transparent text-ash hover:text-soft"
              }`}
            >
              {t(`nav.${tab}`)}
            </button>
          ))}
        </nav>
        {view === "jobs" && openJob && jobInfo[openJob] ? (
          <JobScreen
            jobId={openJob}
            model={jobInfo[openJob].model}
            resumed={jobInfo[openJob].resumed}
            job={jobs[openJob] ?? emptyJob()}
            onBack={() => setOpenJob(null)}
          />
        ) : view === "jobs" ? (
          <JobsScreen jobs={jobs} info={jobInfo} onOpen={setOpenJob} />
        ) : view === "history" ? (
          <HistoryScreen labels={labels} />
        ) : view === "verify" ? (
          <VerifyScreen labels={labels} />
        ) : (
        <>
        <div className="mb-4 flex items-center">
          <h1 className="text-xl font-bold">{t("drives.title")}</h1>
          <Button className="ml-auto flex items-center gap-2" onClick={() => void refresh()} disabled={!connected}>
            <RefreshCw size={14} />
            {t("drives.refresh")}
          </Button>
        </div>
        {hello && !hello.ok && <p className="text-sayno">{t(`errors.${hello.error.code}`)}</p>}
        <BatchBar
          batch={batch}
          onOpen={(next) => {
            changeBatch(next);
            setBatchNotice(null);
          }}
          onClose={() => void closeBatch()}
          closing={closingBatch}
          notice={batchNotice}
          onOpenPdf={(path) => void api.openCertificate(path)}
        />
        {interrupted.map((job) => (
          <div
            key={job.job_id}
            className="mb-4 flex gap-3 rounded-lg border border-sayno/40 bg-sayno/10 p-4 text-sm"
          >
            <TriangleAlert size={16} className="mt-0.5 shrink-0 text-sayno" />
            <div>
              <p className="font-bold text-sayno">{t("jobs.interruptedTitle")}</p>
              <p className="mt-1">
                {job.device.identity.model} · S/N <Mono>{job.device.identity.serial}</Mono>
              </p>
              <p className="mt-1 text-ash">{t("jobs.interruptedHint")}</p>
            </div>
            <button
              type="button"
              aria-label={t("actions.dismiss")}
              className="ml-auto self-start text-ash hover:text-soft"
              onClick={() => setInterrupted((current) => current.filter((other) => other.job_id !== job.job_id))}
            >
              <X size={16} />
            </button>
          </div>
        ))}
        {devices?.ok && devices.value.devices.length === 0 && <p className="text-ash">{t("drives.empty")}</p>}
        <div className="space-y-4">
          {devices?.ok &&
            devices.value.devices.map((summary) => (
              <DriveCard
                key={summary.device.identity.serial}
                summary={summary}
                inspection={inspections[summary.device.identity.serial]}
                labels={labels}
                onErase={setRequest}
                onUnlock={setUnlocking}
                running={runningOn(summary.device.identity.serial)}
                onOpenJob={(id) => {
                  setView("jobs");
                  setOpenJob(id);
                }}
              />
            ))}
        </div>
        </>
        )}
        </>
      </main>
      {licenseOpen && license && (
        <LicenseDialog status={license} onClose={() => setLicenseOpen(false)} onChanged={setLicense} />
      )}
      {unlocking && (
        <UnlockDialog
          device={unlocking.device}
          onCancel={() => setUnlocking(null)}
          onUnlocked={() => {
            setUnlocking(null);
            void refresh();
          }}
        />
      )}
      {request && (
        <ConfirmDialog
          key={request.inspection.verdict_id + request.choice.method}
          request={request}
          labels={labels}
          onCancel={() => setRequest(null)}
          onConfirm={confirm}
        />
      )}
    </div>
  );
}
