// Interactive demo for the website (docs/site.md): a simulated helper on
// fictional drives, built with `npm run build:demo`. Nothing here touches a
// drive, the network or the real helper. Verdicts come from a recording of
// `disphas-helper --features demo`; erasures are played back in accelerated
// time, and certificates are PDFs rendered beforehand by the real renderer
// with public test keys (crates/disphas-report/examples/demo_certificates.rs).
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Event as HelperEvent } from "../bindings/Event";
import type { DeviceSummary } from "../bindings/DeviceSummary";
import type { FinalStatus } from "../bindings/FinalStatus";
import type { HelloResult } from "../bindings/HelloResult";
import type { InspectResult } from "../bindings/InspectResult";
import type { JobPhase } from "../bindings/JobPhase";
import type { LicenseStatusResult } from "../bindings/LicenseStatusResult";
import type { Method } from "../bindings/Method";
import type { MethodChoice } from "../bindings/MethodChoice";
import type { SanitizeArgs } from "../bindings/SanitizeArgs";
import type { Verdict } from "../bindings/Verdict";
import type { Language } from "../i18n";
import type { HistoryEntry, DisphasApi, Outcome, Settings, VerificationReport } from "../lib/api";
import recording from "../dev/demo-recording.json";
import labels from "../dev/code-labels.json";
import history from "../dev/history-sample.json";
import verification from "../dev/verification-sample.json";

/** The demo keeps no settings file: the chosen language lives in the browser's storage. */
const LANGUAGE_KEY = "disphas-demo-language";

/**
 * The first-launch assistant shows in the desktop demo (npm run demo), once;
 * never on the website, where visitors come to see the drives.
 */
const ONBOARDED_KEY = "disphas-demo-onboarded";

function demoOnboarded(): boolean {
  if (!isTauri()) return true;
  try {
    return localStorage.getItem(ONBOARDED_KEY) === "1";
  } catch {
    return true;
  }
}

function storedLanguage(): string | null {
  try {
    return localStorage.getItem(LANGUAGE_KEY);
  } catch {
    return null;
  }
}

const LICENSE: LicenseStatusResult = {
  state: "Valid",
  offer: "Demo",
  license_id: "DEMO-0001",
  valid_until: null,
  entitlements: { max_parallel: 8, customer_logo: true },
};

/** Capacities matching the model names (the recording used small image files). */
const CAPACITY: Record<string, number | null> = {
  "DEMO-NVME-4567": 2_000_398_934_016,
  "DEMO-HDD-8C21": 4_000_787_030_016,
  "DEMO-SSD-F0Z3": 500_107_862_016,
  "DEMO-MUTE-0N1X": null,
  "DEMO-LOCK-7A11": 256_060_514_304,
};

/** Host write speed shown during an overwrite, bytes per second. */
const HOST_RATE: Record<string, number> = {
  "DEMO-NVME-4567": 1_600_000_000,
  "DEMO-HDD-8C21": 190_000_000,
  "DEMO-SSD-F0Z3": 450_000_000,
  "DEMO-LOCK-7A11": 450_000_000,
};

/** Drives that become READY once their blocking condition is handled. */
const SATA_READY: Verdict = {
  state: "READY",
  recommended: { method: "AtaSanitizeBlockErase", category: "Purge", reserves: [], all_namespaces: false },
  alternatives: [
    { method: "AtaSecurityEraseEnhanced", category: "Purge", reserves: [], all_namespaces: false },
    { method: "HostOverwrite", category: "Clear", reserves: ["R7"], all_namespaces: false },
  ],
  disposition: { kind: "Reuse" },
};

/** The SATA SSD reads back badly: every erasure of it ends in a REFUS (N8). */
const READ_ERRORS = "DEMO-SSD-F0Z3";
/** Password of the locked drive, as in the demo helper. */
const PASSWORD = "disphas";
const MAX_UNLOCK_ATTEMPTS = 5;

const ok = <T,>(value: T): Outcome<T> => ({ ok: true, value });
const fail = (code: string, detail: string | null = null): Outcome<never> => ({ ok: false, error: { code, detail } });
const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

type Job = {
  id: string;
  serial: string;
  model: string;
  method: Method;
  zeroPass: boolean;
  status: FinalStatus | null;
  category: "Clear" | "Purge" | "None";
  capacity: number;
  operator: string | null;
  organization: string | null;
  batch: string | null;
  finishedAt: number | null;
};

const state = {
  listings: 0,
  verdicts: 0,
  jobs: 0,
  unfrozen: false,
  unlocked: false,
  failedUnlocks: 0,
  running: new Set<string>(),
  finished: [] as Job[],
  byId: new Map<string, Job>(),
  settings: { organization: "Atelier XYZ", operator: "OP-042", logo: null, strict_confirmation: false, notifications: false, language: storedLanguage(), onboarded: demoOnboarded(), recent_operators: ["OP-042", "OP-017", "M. Kervella"] } as Settings,
  handlers: new Set<(event: HelperEvent) => void>(),
};

function emit(event: HelperEvent) {
  for (const handler of state.handlers) handler(event);
}

function devices(): DeviceSummary[] {
  return (recording.devices as DeviceSummary[]).map((summary) => {
    const serial = summary.device.identity.serial;
    return serial in CAPACITY ? { ...summary, capacity_bytes: CAPACITY[serial] } : summary;
  });
}

function verdictOf(serial: string): Verdict | null {
  const recorded = (recording.inspections as Record<string, InspectResult>)[serial];
  if (!recorded) return null;
  if (serial === "DEMO-SSD-F0Z3" && state.unfrozen) return SATA_READY;
  if (serial === "DEMO-LOCK-7A11") {
    if (state.unlocked) return SATA_READY;
    if (state.failedUnlocks >= MAX_UNLOCK_ATTEMPTS) return { state: "BLOCKED", reasons: ["B6"] };
  }
  return recorded.verdict;
}

function inspect(serial: string): Outcome<InspectResult> {
  const recorded = (recording.inspections as Record<string, InspectResult>)[serial];
  const verdict = verdictOf(serial);
  if (!recorded || !verdict) return fail("DEVICE_NOT_FOUND");
  state.verdicts += 1;
  return ok({
    ...recorded,
    verdict_id: `demo-v${state.verdicts}`,
    valid_for_seconds: 600,
    capacity_bytes: serial in CAPACITY ? CAPACITY[serial] : recorded.capacity_bytes,
    verdict,
  });
}

function choiceOf(verdict: Verdict, method: Method | null, forced: boolean): MethodChoice | null {
  if (verdict.state === "READY") {
    const choices = [verdict.recommended, ...verdict.alternatives];
    return method ? (choices.find((choice) => choice.method === method) ?? null) : verdict.recommended;
  }
  if (verdict.state === "SAY_NO" && forced) return verdict.forcible;
  return null;
}

function requiredAcks(verdict: Verdict, choice: MethodChoice): string[] {
  const acks: string[] = [...choice.reserves];
  if (verdict.state === "READY" && verdict.disposition.kind === "DestroyRecommended") acks.push(...verdict.disposition.reasons);
  if (choice.all_namespaces) acks.push("NS_ALL");
  return acks;
}

/** Plays an erasure back in accelerated time, as the helper would report it. */
async function run(job: Job, choice: MethodChoice) {
  const id = job.id;
  const host = choice.method === "HostOverwrite";
  const phase = (value: JobPhase) => emit({ event: "JobPhase", data: { job_id: id, phase: value } });
  const log = (level: "info" | "warning" | "error", code: string, params: Record<string, string> = {}) =>
    emit({ event: "JobLog", data: { job_id: id, level, code, params } });
  const progress = (percent: number, remaining: number | null, bytes = false) => {
    const rate = HOST_RATE[job.serial] ?? 150_000_000;
    const done = bytes ? Math.round((job.capacity * percent) / 100) : null;
    emit({
      event: "JobProgress",
      data: {
        job_id: id,
        percent,
        source: host ? "host" : "firmware",
        bytes_done: done,
        bytes_per_second: bytes ? rate : null,
        remaining_seconds: bytes && job.capacity > 0 ? Math.round((job.capacity - (done ?? 0)) / rate) : remaining,
      },
    });
  };

  const execute = async (passes: number) => {
    phase("Executing");
    if (host) {
      for (let pass = 0; pass < passes; pass += 1) {
        for (let percent = 0; percent <= 100; percent += 5) {
          progress(percent, null, true);
          await sleep(350);
        }
        if (pass + 1 < passes) log("info", "validation_random_pass", { result: "Passed" });
      }
    } else if (choice.method.startsWith("AtaSecurityErase")) {
      // ATA Security Erase reports nothing: the screen counts down the drive's estimate.
      progress(0, 9);
      await sleep(9000);
      progress(100, 0);
    } else if (choice.method.startsWith("NvmeFormat")) {
      progress(0, null);
      await sleep(1800);
      progress(100, 0);
    } else {
      for (let percent = 0; percent <= 100; percent += 5) {
        progress(percent, null);
        await sleep(400);
      }
    }
  };

  phase("Preparing");
  await sleep(900);
  if (!host) {
    phase("Canaries");
    await sleep(800);
  }
  log("info", "samples_selected", { count: "1000" });
  await sleep(500);

  const readErrors = job.serial === READ_ERRORS;
  for (let attempt = 1; ; attempt += 1) {
    await execute(job.zeroPass && host ? 2 : 1);
    phase("Verifying");
    await sleep(800);
    log("info", "verification", { passed: "true" });
    phase("Validating");
    await sleep(1200);
    log(readErrors ? "error" : "info", "validation", { result: readErrors ? "ReadErrors" : "Passed" });
    if (!readErrors || attempt === 2) break;
    log("warning", "attempt_retry");
    await sleep(600);
  }

  phase("Signing");
  await sleep(900);
  const status: FinalStatus = readErrors ? "REFUS" : choice.category === "Clear" && job.serial === "DEMO-MUTE-0N1X" ? "NON_CONFORME" : "CONFORME";
  job.status = status;
  job.category = status === "REFUS" ? "None" : choice.category;
  job.finishedAt = Math.floor(Date.now() / 1000);
  state.running.delete(job.serial);
  state.finished.unshift(job);
  phase("Done");
  emit({ event: "JobFinished", data: { job_id: id, status } });
}

function sanitize(args: SanitizeArgs): Outcome<{ job_id: string }> {
  const serial = args.device.identity.serial;
  const verdict = verdictOf(serial);
  if (!verdict) return fail("DEVICE_NOT_FOUND");
  if (state.running.has(serial)) return fail("BUSY");
  if (verdict.state === "PROTECTED") return fail("PROTECTED");
  const forced = args.force !== null;
  const choice = choiceOf(verdict, args.method, forced);
  if (!choice) return fail(forced ? "FORCE_NOT_ALLOWED" : "VERDICT_MISMATCH");
  if (forced && (args.force?.justification.trim().length ?? 0) < 20) return fail("FORCE_NOT_ALLOWED");
  // Same rule as the helper (check 4): optional, else the whole serial number or its last four characters.
  const typed = args.confirmation?.trim().toUpperCase();
  if (typed !== undefined && typed !== serial.toUpperCase() && typed !== serial.slice(-4).toUpperCase())
    return fail("CONFIRMATION_MISMATCH");
  const acknowledged = new Set<string>(args.acknowledged);
  if (requiredAcks(verdict, choice).some((ack) => !acknowledged.has(ack))) return fail("ACK_MISSING");

  state.jobs += 1;
  const job: Job = {
    id: `demo-job-${state.jobs}`,
    serial,
    model: args.device.identity.model,
    method: choice.method,
    zeroPass: args.zero_pass && choice.method === "HostOverwrite",
    status: null,
    category: choice.category,
    capacity: CAPACITY[serial] ?? 0,
    operator: args.operator,
    organization: args.organization,
    batch: args.batch?.id ?? null,
    finishedAt: null,
  };
  state.running.add(serial);
  state.byId.set(job.id, job);
  // Started after the reply, as the helper does: the UI subscribes first.
  window.setTimeout(() => void run(job, choice), 50);
  return ok({ job_id: job.id });
}

/** URL of the fictional certificate of a finished job, in `lang`. */
function certificate(job: Job, lang: Language): string {
  const zero = job.zeroPass ? "-zero" : "";
  return `${import.meta.env.BASE_URL}pdf/${job.serial}-${job.method}${zero}-${lang}.pdf`;
}

function historyEntry(job: Job): HistoryEntry {
  return {
    id: job.id,
    check: "Valid",
    summary: {
      issued_at: job.finishedAt ?? 0,
      status: job.status ?? "REFUS",
      category: job.category,
      method: job.method,
      model: job.model,
      serial: job.serial,
      capacity_bytes: job.capacity,
      operator: job.operator,
      organization: job.organization,
      batch_id: job.batch,
      batch_label: null,
    },
  };
}

export const demoApi: DisphasApi = {
  mode: "demo",
  hello: async () =>
    ok({ ...(recording.hello as HelloResult), binary_sha256: "0".repeat(64), license: LICENSE }),
  listDevices: async () => {
    // Refreshing is how the operator reports having handled a frozen drive (B1).
    state.listings += 1;
    if (state.listings > 1) state.unfrozen = true;
    return ok({ devices: devices() });
  },
  inspect: async (device) => {
    await sleep(150);
    return inspect(device.identity.serial);
  },
  sanitize: async (args) => sanitize(args),
  listJobs: async () => ok({ jobs: [] }),
  unlock: async (device, _kind, password) => {
    await sleep(600);
    if (device.identity.serial !== "DEMO-LOCK-7A11" || state.unlocked) return fail("UNLOCK_FAILED");
    if (state.failedUnlocks >= MAX_UNLOCK_ATTEMPTS) return fail("UNLOCK_FAILED", "attempts expired (B6)");
    if (password !== PASSWORD) {
      state.failedUnlocks += 1;
      return fail("UNLOCK_FAILED");
    }
    state.unlocked = true;
    return ok({});
  },
  licenseStatus: async () => ok(LICENSE),
  createActivationRequest: async () => fail("UNAVAILABLE", "demo"),
  installLicense: async () => fail("UNAVAILABLE", "demo"),
  onEvent: async (handler) => {
    state.handlers.add(handler);
    return () => state.handlers.delete(handler);
  },
  renderCertificate: async (jobId, lang) => {
    const job = state.byId.get(jobId);
    if (!job?.status) return fail("UNAVAILABLE", "job not finished");
    await sleep(700);
    return ok(certificate(job, lang));
  },
  openCertificate: async (path) => {
    window.open(path, "_blank", "noopener");
  },
  listReports: async () => [...state.finished.map(historyEntry), ...(history.entries as HistoryEntry[])],
  renderReport: async (id, lang) => {
    const job = state.byId.get(id);
    if (job?.status) return ok(certificate(job, lang));
    // The sample certificates of the history follow the same naming.
    const sample = (history.entries as HistoryEntry[]).find((entry) => entry.id === id)?.summary;
    return sample?.method
      ? ok(`${import.meta.env.BASE_URL}pdf/${sample.serial}-${sample.method}-${lang}.pdf`)
      : fail("UNAVAILABLE", "demo");
  },
  renderBatch: async () => fail("UNAVAILABLE", "demo"),
  deleteReports: async (ids) => {
    const before = state.finished.length;
    state.finished = state.finished.filter((job) => !ids.includes(job.id));
    return ok({ deleted: before - state.finished.length });
  },
  getSettings: async () => state.settings,
  saveSettings: async (organization, operator) => {
    const name = operator.trim();
    const recent = name === "" ? state.settings.recent_operators : [name, ...state.settings.recent_operators.filter((other) => other !== name)].slice(0, 8);
    state.settings = { ...state.settings, organization: organization || null, operator: name || null, recent_operators: recent };
    return ok(state.settings);
  },
  setOperator: async (operator) => {
    const name = operator.trim();
    if ([...name].length > 64) return fail("INVALID", "operator: 64 characters at most");
    const recent = name === "" ? state.settings.recent_operators : [name, ...state.settings.recent_operators.filter((other) => other !== name)].slice(0, 8);
    state.settings = { ...state.settings, operator: name || null, recent_operators: recent };
    return ok(state.settings);
  },
  forgetOperator: async (operator) => {
    state.settings = { ...state.settings, recent_operators: state.settings.recent_operators.filter((other) => other !== operator) };
    return ok(state.settings);
  },
  setLogo: async () => fail("UNAVAILABLE", "demo"),
  clearLogo: async () => fail("UNAVAILABLE", "demo"),
  setStrictConfirmation: async (enabled) => {
    state.settings = { ...state.settings, strict_confirmation: enabled };
    return { ok: true, value: state.settings };
  },
  setOnboarded: async () => {
    state.settings = { ...state.settings, onboarded: true };
    try {
      localStorage.setItem(ONBOARDED_KEY, "1");
    } catch {
      // Storage refused: the assistant shows again at the next launch.
    }
    return { ok: true, value: state.settings };
  },
  setLanguage: async (language) => {
    state.settings = { ...state.settings, language };
    try {
      localStorage.setItem(LANGUAGE_KEY, language);
    } catch {
      // Storage refused: the language lasts for this session only.
    }
    return { ok: true, value: state.settings };
  },
  setNotifications: async (enabled) => {
    state.settings = { ...state.settings, notifications: enabled };
    return { ok: true, value: state.settings };
  },
  // Under Tauri (npm run demo) the desktop shows it, as in the real app; in a browser, nothing.
  notify: async (title, body) => {
    if (isTauri()) await invoke<void>("notify", { title, body }).catch(() => {});
  },
  onDrivesChanged: async () => () => {},
  // The browser cannot verify: a fictional verified report, for layout only.
  verifyCertificate: async () => verification.report as VerificationReport,
  codeLabels: async (lang) => (labels as Record<Language, Record<string, string>>)[lang],
};
