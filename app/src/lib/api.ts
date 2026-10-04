// Access to the root helper. The UI never decides anything: it forwards
// requests and displays the helper's answers (docs/protocole_ipc.md).
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Command } from "../bindings/Command";
import type { DeviceRef } from "../bindings/DeviceRef";
import type { HelloResult } from "../bindings/HelloResult";
import type { ActivationRequestResult } from "../bindings/ActivationRequestResult";
import type { InspectResult } from "../bindings/InspectResult";
import type { LicenseStatusResult } from "../bindings/LicenseStatusResult";
import type { ListDevicesResult } from "../bindings/ListDevicesResult";
import type { SanitizeArgs } from "../bindings/SanitizeArgs";
import type { SanitizeResult } from "../bindings/SanitizeResult";
import type { Language } from "../i18n";
import recording from "../dev/demo-recording.json";
import previewLabels from "../dev/code-labels.json";

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
  licenseStatus(): Promise<Outcome<LicenseStatusResult>>;
  createActivationRequest(): Promise<Outcome<ActivationRequestResult>>;
  installLicense(license: string): Promise<Outcome<LicenseStatusResult>>;
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
  licenseStatus: () => call<LicenseStatusResult>({ cmd: "LicenseStatus" }),
  createActivationRequest: () => call<ActivationRequestResult>({ cmd: "CreateActivationRequest" }),
  installLicense: (license) => call<LicenseStatusResult>({ cmd: "InstallLicense", args: { license } }),
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
  licenseStatus: async () => ({ ok: true, value: recording.hello.license as LicenseStatusResult }),
  createActivationRequest: async () => unavailable,
  installLicense: async () => unavailable,
  codeLabels: async (lang) => (previewLabels as Record<Language, Record<string, string>>)[lang],
};

export const api: OblivioApi = isTauri() ? tauriApi : previewApi;
