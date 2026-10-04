import { useCallback, useEffect, useState } from "react";
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
  const [activeJob, setActiveJob] = useState<{ id: string; model: string; resumed: boolean } | null>(null);
  const [interrupted, setInterrupted] = useState<JobSummary[]>([]);
  const [unlocking, setUnlocking] = useState<InspectResult | null>(null);
  const [view, setView] = useState<"drives" | "history">("drives");

  // Subscribed before any Sanitize, so no job event is ever missed.
  useEffect(() => {
    const unsubscribe = api.onEvent((event) => setJobs((current) => applyEvent(current, event)));
    return () => void unsubscribe.then((stop) => stop());
  }, []);

  // Every drive is inspected by the helper; the UI only displays verdicts.
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
          const resumed = listed.value.jobs.find((job) => job.phase !== "Interrupted");
          if (resumed) {
            setActiveJob({ id: resumed.job_id, model: resumed.device.identity.model, resumed: true });
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
    });
    if (!result.ok) return result.error;
    setActiveJob({ id: result.value.job_id, model: request.inspection.device.identity.model, resumed: false });
    setRequest(null);
    return null;
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
        {activeJob ? (
          <JobScreen
            jobId={activeJob.id}
            model={activeJob.model}
            resumed={activeJob.resumed}
            job={jobs[activeJob.id] ?? emptyJob()}
            onBack={() => {
              setActiveJob(null);
              void refresh();
            }}
          />
        ) : (
        <>
        <nav className="mb-5 flex gap-1 border-b border-edge">
          {(["drives", "history"] as const).map((tab) => (
            <button
              key={tab}
              type="button"
              onClick={() => setView(tab)}
              className={`-mb-px border-b-2 px-3 py-2 text-sm font-bold ${
                view === tab ? "border-brand text-soft" : "border-transparent text-ash hover:text-soft"
              }`}
            >
              {t(`nav.${tab}`)}
            </button>
          ))}
        </nav>
        {view === "history" ? (
          <HistoryScreen labels={labels} />
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
              />
            ))}
        </div>
        </>
        )}
        </>
        )}
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
