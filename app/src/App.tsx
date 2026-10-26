import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { HelloResult } from "./bindings/HelloResult";
import type { InspectResult } from "./bindings/InspectResult";
import type { JobSummary } from "./bindings/JobSummary";
import type { ListDevicesResult } from "./bindings/ListDevicesResult";
import type { LicenseStatusResult } from "./bindings/LicenseStatusResult";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { JobScreen } from "./components/JobScreen";
import { LicenseDialog } from "./components/LicenseDialog";
import { SettingsDialog } from "./components/SettingsDialog";
import { UnlockDialog } from "./components/UnlockDialog";
import { applyEvent, emptyJob, seedJobs, type JobView } from "./lib/jobs";
import { DrivesScreen, type BatchNotice, type EraseRequest } from "./components/DrivesScreen";
import { FinishedBar } from "./components/FinishedBar";
import { Header, type View } from "./components/Header";
import { HistoryScreen } from "./components/HistoryScreen";
import { JobsScreen, type JobInfo } from "./components/JobsScreen";
import type { BatchArgs } from "./bindings/BatchArgs";
import { loadBatch, saveBatch } from "./lib/batch";
import { VerifyScreen } from "./components/VerifyScreen";
import { api, type HelperError, type Outcome, type Settings } from "./lib/api";
import type { Language } from "./i18n";
import { requiredAcknowledgements } from "./lib/acknowledgements";
import type { PdfState } from "./lib/pdf";

export default function App() {
  const { i18n } = useTranslation();
  const [hello, setHello] = useState<Outcome<HelloResult> | null>(null);
  const [devices, setDevices] = useState<Outcome<ListDevicesResult> | null>(null);
  const [inspections, setInspections] = useState<Record<string, Outcome<InspectResult>>>({});
  const inspectionsRef = useRef(inspections);
  useEffect(() => {
    inspectionsRef.current = inspections;
  }, [inspections]);
  const [labels, setLabels] = useState<Record<string, string>>({});
  // Erasures waiting for the confirmation dialog: one drive, or a selection.
  const [requests, setRequests] = useState<EraseRequest[] | null>(null);
  // Certificate of each finished job, rendered as soon as it ends (or why not).
  const [pdfs, setPdfs] = useState<Record<string, PdfState>>({});
  // Erasures finished since the operator last dismissed the summary.
  const [finishedCount, setFinishedCount] = useState<{ conforme: number; other: number }>({ conforme: 0, other: 0 });
  const [license, setLicense] = useState<LicenseStatusResult | null>(null);
  const [licenseOpen, setLicenseOpen] = useState(false);
  const [settings, setSettings] = useState<Settings>({
    organization: null,
    operator: null,
    logo: null,
    strict_confirmation: false,
  });
  const [settingsOpen, setSettingsOpen] = useState(false);
  useEffect(() => {
    void api.getSettings().then(setSettings);
  }, []);
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
  const [view, setView] = useState<View>("drives");
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

  const renderPdf = useCallback(
    async (id: string) => {
      setPdfs((current) => ({ ...current, [id]: { kind: "rendering" } }));
      const result = await api.renderCertificate(id, i18n.language as Language);
      setPdfs((current) => ({
        ...current,
        [id]: result.ok ? { kind: "saved", path: result.value } : { kind: "failed", error: result.error },
      }));
    },
    [i18n.language],
  );
  const renderPdfRef = useRef(renderPdf);
  useEffect(() => {
    renderPdfRef.current = renderPdf;
  }, [renderPdf]);

  // Subscribed before any Sanitize, so no job event is ever missed. A finished
  // erasure changes its drive: it is inspected again.
  useEffect(() => {
    const unsubscribe = api.onEvent((event) => {
      setJobs((current) => applyEvent(current, event));
      if (event.event === "JobFinished") {
        const id = event.data.job_id;
        const info = jobInfoRef.current[id];
        if (info) void reinspectRef.current(info.serial);
        // The certificate is saved at once, in the language shown now.
        void renderPdfRef.current(id);
        const conforme = event.data.status === "CONFORME";
        setFinishedCount((current) => ({
          conforme: current.conforme + (conforme ? 1 : 0),
          other: current.other + (conforme ? 0 : 1),
        }));
      }
    });
    return () => void unsubscribe.then((stop) => stop());
  }, []);

  // Drives plugged in or out: new ones are inspected, the others keep their verdict.
  const follow = useCallback(async () => {
    const listed = await api.listDevices();
    setDevices(listed);
    if (!listed.ok) return;
    const serials = new Set(listed.value.devices.map((summary) => summary.device.identity.serial));
    const known = inspectionsRef.current;
    setInspections((current) =>
      Object.fromEntries(Object.entries(current).filter(([serial]) => serials.has(serial))),
    );
    for (const summary of listed.value.devices) {
      const serial = summary.device.identity.serial;
      if (known[serial]?.ok) continue;
      const inspection = await api.inspect(summary.device);
      setInspections((current) => ({ ...current, [serial]: inspection }));
    }
  }, []);
  useEffect(() => {
    if (!(hello?.ok ?? false)) return;
    const unsubscribe = api.onDrivesChanged(() => void follow());
    return () => void unsubscribe.then((stop) => stop());
  }, [hello, follow]);

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
                  // Only firmware erasures are resumed.
                  hostWrite: false,
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

  /** Sends one erasure to the helper; the helper checks everything again. */
  const start = async (
    erase: EraseRequest,
    input: { confirmation: string | null; justification: string | null },
    into: BatchArgs | null,
  ): Promise<HelperError | null> => {
    const verdict = erase.inspection.verdict;
    const disposition = verdict.state === "READY" ? verdict.disposition : { kind: "Reuse" as const };
    const result = await api.sanitize({
      verdict_id: erase.inspection.verdict_id,
      device: erase.inspection.device,
      method: erase.choice.method,
      confirmation: input.confirmation,
      // The warnings were on screen when the operator launched the erasure.
      acknowledged: requiredAcknowledgements(erase.choice, disposition),
      force: input.justification === null ? null : { justification: input.justification },
      operator: settings.operator,
      organization: settings.organization,
      batch: into,
      zero_pass: erase.choice.method === "HostOverwrite" && (erase.zeroPass ?? false),
    });
    if (!result.ok) return result.error;
    const { model, serial } = erase.inspection.device.identity;
    setJobInfo((current) => ({
      ...current,
      [result.value.job_id]: {
        model,
        serial,
        resumed: false,
        batch: into?.id ?? null,
        hostWrite: erase.choice.method === "HostOverwrite",
      },
    }));
    // The operator stays on the list: each drive shows its own progress.
    return null;
  };

  /**
   * An erasure asked for from the Disques tab always goes through the
   * confirmation dialog: one click for an ordinary erasure, the serial
   * number in strict mode, a justification to force (cadre_projet.md §5.4).
   */
  const launch = (erase: EraseRequest) => setRequests([erase]);
  /** Several ready drives at once: one dialog for all of them. */
  const launchMany = (erases: EraseRequest[]) => {
    if (erases.length > 0) setRequests(erases);
  };

  /**
   * Sends the confirmed erasures one after the other, opening the batch the
   * operator asked for first. Returns the refusals, by serial number.
   */
  const confirmAll = async (
    erases: EraseRequest[],
    input: {
      confirmations: Record<string, string | null>;
      justification: string | null;
      newBatch: BatchArgs | null;
    },
  ): Promise<Record<string, HelperError>> => {
    let into = batch;
    if (input.newBatch) {
      changeBatch(input.newBatch);
      setBatchNotice(null);
      into = input.newBatch;
    }
    const refused: Record<string, HelperError> = {};
    for (const erase of erases) {
      const serial = erase.inspection.device.identity.serial;
      const error = await start(
        erase,
        { confirmation: input.confirmations[serial] ?? null, justification: input.justification },
        into,
      );
      if (error) refused[serial] = error;
    }
    return refused;
  };

  /** The erasure in progress on a drive, if any. */
  const runningOn = (serial: string) => {
    const id = Object.keys(jobInfo).find(
      (key) => jobInfo[key].serial === serial && jobs[key] && jobs[key].status === null,
    );
    return id ? { id, job: jobs[id] } : null;
  };

  /** The last erasure of a drive finished in this session, if any. */
  const finishedOn = (serial: string) => {
    const id = Object.keys(jobInfo)
      .reverse()
      .find((key) => jobInfo[key].serial === serial && jobs[key] && jobs[key].status !== null);
    const status = id ? jobs[id].status : null;
    return id && status ? { id, status } : null;
  };

  /** Erasures running now, resumed ones included (the helper counts the same, offres.md §4.4). */
  const runningCount = Object.values(jobs).filter((job) => job.status === null && job.phase !== "Interrupted").length;
  const maxParallel = license?.entitlements ? license.entitlements.max_parallel : undefined;

  const connected = hello?.ok ?? false;
  return (
    <div className="flex min-h-full flex-col">
      <Header
        hello={hello?.ok ? hello.value : null}
        mode={api.mode}
        connected={connected}
        license={license}
        view={view}
        onView={(tab) => {
          setView(tab);
          setOpenJob(null);
        }}
        onSettings={() => setSettingsOpen(true)}
      />
      {view === "drives" ? (
        <DrivesScreen
          hello={hello}
          mode={api.mode}
          connected={connected}
          license={license}
          devices={devices}
          inspections={inspections}
          labels={labels}
          runningCount={runningCount}
          maxParallel={maxParallel}
          runningOn={runningOn}
          finishedOn={finishedOn}
          interrupted={interrupted}
          onDismissInterrupted={(id) => setInterrupted((current) => current.filter((job) => job.job_id !== id))}
          batch={batch}
          batchNotice={batchNotice}
          closingBatch={closingBatch}
          onOpenBatch={(next) => {
            changeBatch(next);
            setBatchNotice(null);
          }}
          onCloseBatch={() => void closeBatch()}
          onOpenPdf={(path) => void api.openCertificate(path)}
          onRefresh={() => void refresh()}
          onReinspect={(serial) => void reinspect(serial)}
          onErase={launch}
          onEraseMany={launchMany}
          pdfOf={(id) => pdfs[id]}
          onUnlock={setUnlocking}
          onOpenJob={(id) => {
            setView("jobs");
            setOpenJob(id);
          }}
          onLicense={() => setLicenseOpen(true)}
        />
      ) : (
        <main className="mx-auto w-full max-w-4xl flex-1 px-6 py-6">
          {view === "jobs" && openJob && jobInfo[openJob] ? (
            <JobScreen
              jobId={openJob}
              model={jobInfo[openJob].model}
              resumed={jobInfo[openJob].resumed}
              hostWrite={jobInfo[openJob].hostWrite}
              job={jobs[openJob] ?? emptyJob()}
              pdf={pdfs[openJob]}
              onRetryPdf={() => void renderPdf(openJob)}
              onBack={() => setOpenJob(null)}
            />
          ) : view === "jobs" ? (
            <JobsScreen jobs={jobs} info={jobInfo} onOpen={setOpenJob} />
          ) : view === "history" ? (
            <HistoryScreen labels={labels} />
          ) : (
            <VerifyScreen labels={labels} />
          )}
        </main>
      )}
      {finishedCount.conforme + finishedCount.other > 0 && (
        <FinishedBar
          conforme={finishedCount.conforme}
          other={finishedCount.other}
          running={runningCount}
          onShow={() => {
            setView("jobs");
            setOpenJob(null);
          }}
          onDismiss={() => setFinishedCount({ conforme: 0, other: 0 })}
        />
      )}
      {licenseOpen && license && (
        <LicenseDialog status={license} onClose={() => setLicenseOpen(false)} onChanged={setLicense} />
      )}
      {settingsOpen && (
        <SettingsDialog settings={settings} logoAllowed={license?.entitlements?.customer_logo ?? false} onClose={() => setSettingsOpen(false)} onChanged={setSettings} />
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
      {requests && (
        <ConfirmDialog
          key={requests.map((erase) => erase.inspection.verdict_id + erase.choice.method).join()}
          requests={requests}
          labels={labels}
          signer={{ operator: settings.operator, organization: settings.organization }}
          strict={settings.strict_confirmation}
          batch={batch}
          freeSlots={maxParallel == null ? null : Math.max(0, maxParallel - runningCount)}
          onCancel={() => setRequests(null)}
          onConfirm={async (input) => {
            const refused = await confirmAll(requests, input);
            if (Object.keys(refused).length === 0) setRequests(null);
            return refused;
          }}
        />
      )}
    </div>
  );
}
