import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import type { Acknowledgement } from "./bindings/Acknowledgement";
import type { HelloResult } from "./bindings/HelloResult";
import type { InspectResult } from "./bindings/InspectResult";
import type { ListDevicesResult } from "./bindings/ListDevicesResult";
import type { LicenseStatusResult } from "./bindings/LicenseStatusResult";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { LicenseDialog } from "./components/LicenseDialog";
import { DriveCard, type EraseRequest } from "./components/DriveCard";
import { Header } from "./components/Header";
import { Button } from "./components/ui";
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
    return result.ok ? null : result.error;
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
        <div className="mb-4 flex items-center">
          <h1 className="text-xl font-bold">{t("drives.title")}</h1>
          <Button className="ml-auto flex items-center gap-2" onClick={() => void refresh()} disabled={!connected}>
            <RefreshCw size={14} />
            {t("drives.refresh")}
          </Button>
        </div>
        {hello && !hello.ok && <p className="text-sayno">{t(`errors.${hello.error.code}`)}</p>}
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
              />
            ))}
        </div>
      </main>
      {licenseOpen && license && (
        <LicenseDialog status={license} onClose={() => setLicenseOpen(false)} onChanged={setLicense} />
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
