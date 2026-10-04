// Access to the root helper. The UI never decides anything: it forwards
// requests and displays the helper's answers (docs/protocole_ipc.md).
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Event as HelperEvent } from "../bindings/Event";
import type { Command } from "../bindings/Command";
import type { DeviceRef } from "../bindings/DeviceRef";
import type { HelloResult } from "../bindings/HelloResult";
import type { ActivationRequestResult } from "../bindings/ActivationRequestResult";
import type { InspectResult } from "../bindings/InspectResult";
import type { LicenseStatusResult } from "../bindings/LicenseStatusResult";
import type { ListDevicesResult } from "../bindings/ListDevicesResult";
import type { ListJobsResult } from "../bindings/ListJobsResult";
import type { PasswordKind } from "../bindings/PasswordKind";
import type { SanitizeArgs } from "../bindings/SanitizeArgs";
import type { SanitizeResult } from "../bindings/SanitizeResult";
import type { Language } from "../i18n";
import recording from "../dev/demo-recording.json";
import previewLabels from "../dev/code-labels.json";
import previewHistory from "../dev/history-sample.json";
import previewVerification from "../dev/verification-sample.json";

/** One report of the certificate history (app/src-tauri/src/history.rs). */
export type HistoryEntry = {
  id: string;
  check: "Valid" | "Invalid" | "Revoked" | "JournalMismatch" | "Unreadable";
  summary: {
    issued_at: number;
    status: "CONFORME" | "NON_CONFORME" | "REFUS";
    category: "Clear" | "Purge" | "None";
    method: string | null;
    model: string;
    serial: string;
    capacity_bytes: number;
    operator: string | null;
    organization: string | null;
    batch_id: string | null;
    batch_label: string | null;
  } | null;
};

/** Outcome of a certificate verification (app/src-tauri/src/verification.rs). */
export type VerificationReport = {
  source: "Pdf" | "Cose" | "Qr" | null;
  authentic: boolean;
  reason: string | null;
  root: "Official" | "Development" | null;
  journal: "Matches" | "Mismatch" | "Absent";
  details: {
    id: string;
    issued_at: number;
    status: "CONFORME" | "NON_CONFORME" | "REFUS";
    category: "Clear" | "Purge" | "None";
    method: string | null;
    verified: boolean | null;
    validation: { method: string; samples: number; passed: boolean } | null;
    disposition: "Reuse" | "DestroyRecommended" | null;
    disposition_codes: string[];
    reserves: string[];
    refusals: string[];
    justification: string | null;
    model: string;
    serial: string;
    firmware: string;
    capacity_bytes: number;
    drive_type: string;
    hardware_id: string | null;
    operator: string | null;
    organization: string | null;
    batch_id: string | null;
    batch_label: string | null;
    tool_version: string;
    rules_version: string;
    binary_sha256: string;
    license_id: string;
    customer: string;
  } | null;
};

/** An error from the helper, or `UNAVAILABLE` when it cannot be reached. */
export type HelperError = { code: string; detail: string | null };

export type Outcome<T> = { ok: true; value: T } | { ok: false; error: HelperError };

export interface OblivioApi {
  /** `tauri`: real helper. `preview`: browser, replays a recording of the demo helper. */
  readonly mode: "tauri" | "preview";
  hello(): Promise<Outcome<HelloResult>>;
  listDevices(): Promise<Outcome<ListDevicesResult>>;
  inspect(device: DeviceRef): Promise<Outcome<InspectResult>>;
  sanitize(args: SanitizeArgs): Promise<Outcome<SanitizeResult>>;
  /** Jobs known to the helper: at start-up, those resumed or interrupted after a crash. */
  listJobs(): Promise<Outcome<ListJobsResult>>;
  /** Unlocks a drive locked by an ATA password (B2) and removes that password. */
  unlock(device: DeviceRef, kind: PasswordKind, password: string): Promise<Outcome<unknown>>;
  licenseStatus(): Promise<Outcome<LicenseStatusResult>>;
  createActivationRequest(): Promise<Outcome<ActivationRequestResult>>;
  installLicense(license: string): Promise<Outcome<LicenseStatusResult>>;
  /** Subscribes to job events; returns the unsubscribe function. */
  onEvent(handler: (event: HelperEvent) => void): Promise<() => void>;
  /** Renders and saves the PDF certificate of a finished job; returns its path. */
  renderCertificate(jobId: string, lang: Language): Promise<Outcome<string>>;
  /** Opens a saved certificate with the desktop's PDF viewer. */
  openCertificate(path: string): Promise<void>;
  /** Signed reports stored by the helper, newest first, each one verified. */
  listReports(): Promise<HistoryEntry[]>;
  /** Renders and saves the PDF certificate of a report from the history. */
  renderReport(id: string, lang: Language): Promise<Outcome<string>>;
  /** Renders and saves the batch certificate of a batch from the history. */
  renderBatch(batchId: string, lang: Language): Promise<Outcome<string>>;
  /** Verifies a certificate (PDF, COSE or QR text, base64), fully offline. */
  verifyCertificate(data: string): Promise<VerificationReport | { error: string }>;
  /** Labels of the attestation codes (N, D, R, methods), from Rust. */
  codeLabels(lang: Language): Promise<Record<string, string>>;
}

function asHelperError(error: unknown): HelperError {
  if (typeof error === "object" && error !== null && "code" in error) {
    const { code, detail } = error as { code: unknown; detail?: unknown };
    return { code: String(code), detail: typeof detail === "string" ? detail : null };
  }
  return { code: "UNAVAILABLE", detail: String(error) };
}

async function call<T>(command: Command): Promise<Outcome<T>> {
  try {
    return { ok: true, value: await invoke<T>("helper", { command }) };
  } catch (error) {
    return { ok: false, error: asHelperError(error) };
  }
}

const tauriApi: OblivioApi = {
  mode: "tauri",
  hello: () => call<HelloResult>({ cmd: "Hello", args: { protocol_version: 1, app_version: "0.1.0" } }),
  listDevices: () => call<ListDevicesResult>({ cmd: "ListDevices" }),
  inspect: (device) => call<InspectResult>({ cmd: "Inspect", args: { device } }),
  sanitize: (args) => call<SanitizeResult>({ cmd: "Sanitize", args }),
  listJobs: () => call<ListJobsResult>({ cmd: "ListJobs" }),
  unlock: (device, kind, password) => call<unknown>({ cmd: "Unlock", args: { device, kind, password } }),
  licenseStatus: () => call<LicenseStatusResult>({ cmd: "LicenseStatus" }),
  createActivationRequest: () => call<ActivationRequestResult>({ cmd: "CreateActivationRequest" }),
  installLicense: (license) => call<LicenseStatusResult>({ cmd: "InstallLicense", args: { license } }),
  onEvent: (handler) => listen<HelperEvent>("oblivio://event", (event) => handler(event.payload)),
  renderCertificate: async (jobId, lang) => {
    try {
      return { ok: true, value: await invoke<string>("render_certificate", { jobId, lang }) };
    } catch (error) {
      return { ok: false, error: asHelperError(error) };
    }
  },
  openCertificate: (path) => invoke<void>("open_certificate", { path }),
  listReports: () => invoke<HistoryEntry[]>("list_reports").catch(() => []),
  renderReport: async (id, lang) => {
    try {
      return { ok: true, value: await invoke<string>("render_report", { id, lang }) };
    } catch (error) {
      return { ok: false, error: asHelperError(error) };
    }
  },
  renderBatch: async (batchId, lang) => {
    try {
      return { ok: true, value: await invoke<string>("render_batch", { batchId, lang }) };
    } catch (error) {
      return { ok: false, error: asHelperError(error) };
    }
  },
  verifyCertificate: (data) =>
    invoke<VerificationReport>("verify_certificate", { data }).catch((error) => ({ error: String(error) })),
  codeLabels: (lang) => invoke<Record<string, string>>("code_labels", { lang }),
};

const unavailable = {
  ok: false,
  error: { code: "UNAVAILABLE", detail: "browser preview: no helper" },
} as const;

/** Browser preview: replays answers recorded from `oblivio-helper --features demo`. */
const previewApi: OblivioApi = {
  mode: "preview",
  hello: async () => ({ ok: true, value: recording.hello as HelloResult }),
  listDevices: async () => ({ ok: true, value: { devices: recording.devices } as ListDevicesResult }),
  inspect: async (device) => {
    const inspections = recording.inspections as Record<string, unknown>;
    const result = inspections[device.identity.serial];
    return result
      ? { ok: true, value: result as InspectResult }
      : { ok: false, error: { code: "DEVICE_NOT_FOUND", detail: null } };
  },
  sanitize: async () => unavailable,
  listJobs: async () => ({ ok: true, value: { jobs: [] } }),
  unlock: async () => unavailable,
  licenseStatus: async () => ({ ok: true, value: recording.hello.license as LicenseStatusResult }),
  createActivationRequest: async () => unavailable,
  installLicense: async () => unavailable,
  onEvent: async () => () => {},
  renderCertificate: async () => unavailable,
  openCertificate: async () => {},
  listReports: async () => previewHistory.entries as HistoryEntry[],
  renderReport: async () => unavailable,
  renderBatch: async () => unavailable,
  // The browser cannot verify: shows a fictional verified report, for layout only.
  verifyCertificate: async () => previewVerification.report as VerificationReport,
  codeLabels: async (lang) => (previewLabels as Record<Language, Record<string, string>>)[lang],
};

export const api: OblivioApi = isTauri() ? tauriApi : previewApi;
