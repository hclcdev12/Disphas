import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  Check,
  ChevronDown,
  ChevronLeft,
  KeyRound,
  Moon,
  LoaderCircle,
  Play,
  Plus,
  RefreshCw,
  TriangleAlert,
  X,
} from "lucide-react";
import type { BatchArgs } from "../bindings/BatchArgs";
import type { DeviceSummary } from "../bindings/DeviceSummary";
import type { FinalStatus } from "../bindings/FinalStatus";
import type { HelloResult } from "../bindings/HelloResult";
import type { InspectResult } from "../bindings/InspectResult";
import type { JobSummary } from "../bindings/JobSummary";
import type { LicenseStatusResult } from "../bindings/LicenseStatusResult";
import type { ListDevicesResult } from "../bindings/ListDevicesResult";
import type { MethodChoice } from "../bindings/MethodChoice";
import type { HelperError, Outcome } from "../lib/api";
import { newBatchId } from "../lib/batch";
import { capacity } from "../lib/format";
import type { JobView } from "../lib/jobs";
import type { PdfState } from "../lib/pdf";
import { Station } from "./Header";
import { JobProgress } from "./JobsScreen";
import { Badge, Bar, Button, Category, Dialog, fieldClass, IconButton, Mono } from "./ui";

/** An erasure the operator asked for, before the confirmation dialog. */
/** Sleeping to thaw frozen drives (B1, UX review D5). */
export type Sleep = {
  /** The system accepts a sleep requested by the app. */
  possible: boolean;
  /** Erasures running: no sleep until they end. */
  running: number;
  onSleep: () => void;
};

export type EraseRequest = {
  inspection: InspectResult;
  choice: MethodChoice;
  forced: boolean;
  /** Host overwrite only: a final pass of zeros, for an inconspicuous drive. */
  zeroPass?: boolean;
};

/** What closing the batch produced. */
export type BatchNotice =
  | { kind: "running"; count: number }
  | { kind: "pdf"; batch: string; path: string }
  | { kind: "empty"; batch: string }
  | { kind: "error"; batch: string; error: HelperError };

/** What the operator has to do with a drive, in display order (docs/maquettes/disphas-ecran-disques.md §3.2). */
type Group = "running" | "ready" | "blocked" | "refus" | "protected" | "pending";

/** Drives being erased come first: they are what the operator follows. */
const GROUPS: Group[] = ["running", "ready", "blocked", "refus", "protected", "pending"];

const groupText: Record<Group, string> = {
  running: "text-soft",
  ready: "text-ready",
  blocked: "text-blocked",
  refus: "text-sayno",
  protected: "text-protected",
  pending: "text-ash",
};

/** Below this width the list and the detail are two views (docs/maquettes/disphas-ecran-disques.md §4). */
const NARROW = "(max-width: 760px)";
const narrow = () => window.matchMedia(NARROW).matches;

/** Largest batch label, as in the attestation. */
const MAX_LABEL = 128;

type Running = { id: string; job: JobView } | null;
type Finished = { id: string; status: FinalStatus } | null;

function groupOf(inspection: Outcome<InspectResult> | undefined): Group {
  if (!inspection?.ok) return "pending";
  switch (inspection.value.verdict.state) {
    case "READY":
      return "ready";
    case "BLOCKED":
      return "blocked";
    case "SAY_NO":
      return "refus";
    case "PROTECTED":
      return "protected";
  }
}

/** Methods offered for a ready drive; index 0 is the recommended one. */
function choicesOf(inspection: InspectResult): MethodChoice[] {
  const verdict = inspection.verdict;
  return verdict.state === "READY" ? [verdict.recommended, ...verdict.alternatives] : [];
}

/**
 * What the operator can pick for a ready drive: each method, and host
 * overwrite a second time with its final pass of zeros, so that the zero
 * pass is one choice among the others rather than an extra setting.
 */
type MethodOption = { position: number; zero: boolean; choice: MethodChoice };

function optionsOf(inspection: InspectResult): MethodOption[] {
  return choicesOf(inspection).flatMap((choice, position) =>
    choice.method === "HostOverwrite"
      ? [
          { position, zero: false, choice },
          { position, zero: true, choice },
        ]
      : [{ position, zero: false, choice }],
  );
}

/** The zero pass only goes with host overwrite (the helper refuses it otherwise). */
function zeroFor(choice: MethodChoice, zeroPass: boolean): boolean {
  return choice.method === "HostOverwrite" && zeroPass;
}

/** Reason codes of a drive that is not ready. */
function reasonsOf(inspection: InspectResult): string[] {
  const verdict = inspection.verdict;
  return verdict.state === "READY" ? [] : verdict.reasons;
}

/**
 * The Disques tab: the detected drives on the left, grouped by what the
 * operator has to do, and the selected drive on the right
 * (docs/maquettes/disphas-ecran-disques.md). A drive is named by its model,
 * serial number and WWN / EUI-64, never by a /dev path.
 */
export function DrivesScreen(props: {
  hello: Outcome<HelloResult> | null;
  mode: "tauri" | "preview" | "demo";
  connected: boolean;
  license: LicenseStatusResult | null;
  devices: Outcome<ListDevicesResult> | null;
  inspections: Record<string, Outcome<InspectResult>>;
  labels: Record<string, string>;
  runningCount: number;
  maxParallel: number | null | undefined;
  runningOn: (serial: string) => Running;
  finishedOn: (serial: string) => Finished;
  interrupted: JobSummary[];
  onDismissInterrupted: (jobId: string) => void;
  batch: BatchArgs | null;
  batchNotice: BatchNotice | null;
  closingBatch: boolean;
  onOpenBatch: (batch: BatchArgs) => void;
  onCloseBatch: () => void;
  onOpenPdf: (path: string) => void;
  onRefresh: () => void;
  onReinspect: (serial: string) => void;
  onErase: (request: EraseRequest) => void;
  /** Several ready drives at once, each with its chosen method. */
  onEraseMany: (requests: EraseRequest[]) => void;
  /** Certificate of a finished job, rendered automatically. */
  pdfOf: (jobId: string) => PdfState | undefined;
  onUnlock: (inspection: InspectResult) => void;
  /** Sleep to thaw frozen drives (B1). */
  sleep: Sleep;
  onOpenJob: (id: string) => void;
  onLicense: () => void;
}) {
  const { t, i18n } = useTranslation();
  const { devices, inspections, labels } = props;
  const drives = devices?.ok ? devices.value.devices : [];
  // Selected drives, by serial number: one click selects a drive, Ctrl + click
  // adds or removes one, Shift + click a range. Empty: the first drive.
  const [selection, setSelection] = useState<string[]>([]);
  const [anchor, setAnchor] = useState<string | null>(null);
  // The row the arrows move from; the only row reached by Tab (one stop for the whole list).
  const [cursor, setCursor] = useState<string | null>(null);
  // Method chosen per verdict: a new inspection starts again from the recommended one.
  const [methods, setMethods] = useState<Record<string, number>>({});
  const [zeroPass, setZeroPass] = useState<Record<string, boolean>>({});
  const [view, setView] = useState<"list" | "detail">("list");
  const [editingBatch, setEditingBatch] = useState(false);
  const titleRef = useRef<HTMLHeadingElement>(null);
  // Bumped when the drive title must take the focus: in a narrow window.
  const [focusTick, setFocusTick] = useState(0);

  const serialOf = (drive: DeviceSummary) => drive.device.identity.serial;
  /** Being erased first, whatever the verdict that allowed it. */
  const groupFor = (drive: DeviceSummary): Group =>
    props.runningOn(serialOf(drive)) !== null ? "running" : groupOf(inspections[serialOf(drive)]);
  const grouped = GROUPS.map((group) => ({
    group,
    drives: drives.filter((drive) => groupFor(drive) === group),
  })).filter((entry) => entry.drives.length > 0);
  const ordered = grouped.flatMap((entry) => entry.drives);
  const selectedDrives = ordered.filter((drive) => selection.includes(serialOf(drive)));
  const many = selectedDrives.length > 1;
  // One drive in detail; several: the list of the selection instead.
  const selected = many ? null : (selectedDrives[0] ?? ordered[0] ?? null);
  const tabStop =
    [cursor, anchor].find((serial) => serial !== null && ordered.some((drive) => serialOf(drive) === serial)) ??
    (selectedDrives[0] ? serialOf(selectedDrives[0]) : selected ? serialOf(selected) : null);
  /** Ready and not being erased: what "Erase N drives" takes. */
  const erasable = (drive: DeviceSummary) =>
    groupOf(inspections[serialOf(drive)]) === "ready" && props.runningOn(serialOf(drive)) === null;
  const erasableSerials = ordered.filter(erasable).map(serialOf);
  const allSelected =
    erasableSerials.length > 0 && erasableSerials.every((serial) => selectedDrives.some((d) => serialOf(d) === serial));

  /** Click: this drive alone. Ctrl (or Cmd) + click: add or remove it. Shift + click: the range from the last one. */
  /** Shows the detail; in a narrow window it replaces the list. */
  const open = (focusDetail: boolean) => {
    setView("detail");
    if (focusDetail || narrow()) setFocusTick((tick) => tick + 1);
  };
  const pick = (serial: string, event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }) => {
    const current = selectedDrives.length > 0 ? selectedDrives.map(serialOf) : selected ? [serialOf(selected)] : [];
    if (event.ctrlKey || event.metaKey) {
      // The last selected drive stays: the detail always shows something.
      if (current.includes(serial)) {
        if (current.length > 1) setSelection(current.filter((other) => other !== serial));
      } else {
        setSelection([...current, serial]);
      }
      setAnchor(serial);
      setCursor(serial);
      return;
    }
    if (event.shiftKey && anchor !== null) {
      const from = ordered.findIndex((drive) => serialOf(drive) === anchor);
      const to = ordered.findIndex((drive) => serialOf(drive) === serial);
      if (from >= 0 && to >= 0) {
        const [low, high] = from < to ? [from, to] : [to, from];
        setSelection(ordered.slice(low, high + 1).map(serialOf));
        setCursor(serial);
        return;
      }
    }
    setSelection([serial]);
    setAnchor(serial);
    setCursor(serial);
    open(false);
  };
  const focusRow = (serial: string) =>
    requestAnimationFrame(() => document.querySelector<HTMLButtonElement>(`[data-pick="${serial}"]`)?.focus());
  /** Up and down move the selection; with Shift, they stretch it from the anchor. */
  const arrow = (serial: string, event: React.KeyboardEvent) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const at = ordered.findIndex((drive) => serialOf(drive) === serial);
    const next = ordered[at + (event.key === "ArrowDown" ? 1 : -1)];
    if (!next) return;
    const target = serialOf(next);
    if (event.shiftKey) {
      const from = ordered.findIndex((drive) => serialOf(drive) === (anchor ?? serial));
      const to = at + (event.key === "ArrowDown" ? 1 : -1);
      const [low, high] = from < to ? [from, to] : [to, from];
      setSelection(ordered.slice(low, high + 1).map(serialOf));
      if (anchor === null) setAnchor(serial);
    } else {
      setSelection([target]);
      setAnchor(target);
    }
    setCursor(target);
    focusRow(target);
  };
  const selectAll = () => {
    setSelection(allSelected ? erasableSerials.slice(0, 1) : erasableSerials);
    setAnchor(erasableSerials[0] ?? null);
    // In a narrow window the list and the detail are two views: show the selection.
    if (narrow()) open(true);
  };
  const unselect = (serial: string) => setSelection(selectedDrives.map(serialOf).filter((other) => other !== serial));
  const eraseSelected = () => {
    const requests: EraseRequest[] = [];
    for (const drive of selectedDrives.filter(erasable)) {
      const inspection = inspections[serialOf(drive)];
      if (!inspection?.ok) continue;
      const choice = chosen(inspection.value);
      if (choice)
        requests.push({
          inspection: inspection.value,
          choice,
          forced: false,
          zeroPass: zeroFor(choice, zeroPass[inspection.value.verdict_id] ?? false),
        });
    }
    props.onEraseMany(requests);
  };

  const frozenCount = drives.filter((drive) => {
    const inspection = inspections[serialOf(drive)];
    return inspection?.ok && inspection.value.verdict.state === "BLOCKED" && inspection.value.verdict.reasons.includes("B1");
  }).length;

  // Ctrl + A: every ready drive. Escape: back to one drive. Not while typing or in a dialog.
  const shortcuts = useRef<(event: KeyboardEvent) => void>(() => {});
  useEffect(() => {
    shortcuts.current = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        event.defaultPrevented ||
        (target instanceof Element && target.closest("input, textarea, select, [contenteditable=true]")) ||
        document.querySelector("dialog[open]")
      )
        return;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
        if (erasableSerials.length === 0) return;
        event.preventDefault();
        setSelection(erasableSerials);
        setAnchor(erasableSerials[0]);
        if (narrow()) open(true);
      } else if (event.key === "Escape" && many) {
        const keep = selectedDrives.map(serialOf).find((serial) => serial === cursor) ?? serialOf(selectedDrives[0]);
        setSelection([keep]);
        setAnchor(keep);
        setCursor(keep);
      }
    };
  });
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => shortcuts.current(event);
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (focusTick === 0) return;
    if (narrow()) document.querySelector(".split-dbody")?.scrollTo(0, 0);
    titleRef.current?.focus();
  }, [focusTick]);

  const chosen = (inspection: InspectResult): MethodChoice | null => {
    const choices = choicesOf(inspection);
    return choices[methods[inspection.verdict_id] ?? 0] ?? choices[0] ?? null;
  };
  const name = (code: string) => labels[code] ?? code;
  const reasonText = (inspection: InspectResult, code: string) => {
    switch (inspection.verdict.state) {
      case "BLOCKED":
        return t(`block.${code}`);
      case "PROTECTED":
        return t(`protect.${code}`);
      default:
        return name(code);
    }
  };

  const back = () => {
    setView("list");
    const serial = selected ? serialOf(selected) : (selectedDrives[0] ? serialOf(selectedDrives[0]) : null);
    requestAnimationFrame(() => document.querySelector<HTMLButtonElement>(`[data-pick="${serial}"]`)?.focus());
  };

  /** Second line of a row: the method, or the reason with its codes. */
  const rowLine = (drive: DeviceSummary): string => {
    const serial = serialOf(drive);
    const running = props.runningOn(serial);
    if (running) {
      const percent = running.job.progress?.percent;
      return percent == null ? t("drives.row.running") : `${t("drives.row.running")} · ${percent} %`;
    }
    const inspection = inspections[serial];
    if (!inspection) return t("drives.inspecting");
    if (!inspection.ok) return t(`errors.${inspection.error.code}`);
    const value = inspection.value;
    if (value.verdict.state === "READY") {
      const choice = chosen(value);
      if (!choice) return "";
      const zero = zeroFor(choice, zeroPass[value.verdict_id] ?? false) ? ` ${t("drives.thenZeros")}` : "";
      return `${choice.category} · ${name(choice.method)}${zero}`;
    }
    const codes = reasonsOf(value);
    return codes.length > 0 ? `${reasonText(value, codes[0])} (${codes.join(", ")})` : "";
  };

  const parallel =
    props.maxParallel === undefined
      ? null
      : props.maxParallel === null
        ? t("drives.meta.unlimited", { count: props.runningCount })
        : t("drives.meta.limited", {
            count: props.runningCount,
            max: props.maxParallel,
          });

  return (
    <div className="split-screen grid min-h-0 flex-1" data-view={view}>
      <div className="split-lhead flex flex-col gap-4 border-r border-edge px-4 pt-6 pb-5">
        <div className="flex items-start justify-between gap-2 px-2">
          <div>
            <h1 className="text-[25px] leading-tight font-bold">{t("drives.count", { count: drives.length })}</h1>
            {parallel && <p className="mt-0.5 font-mono text-sm text-ash">{parallel}</p>}
            <BatchHint
              batch={props.batch}
              closing={props.closingBatch}
              onClose={props.onCloseBatch}
            />
            {erasableSerials.length > 1 && <p className="drives-multi-hint mt-1 text-[13px] text-ash">{t("drives.multiHint")}</p>}
          </div>
          <div className="flex gap-1">
            <IconButton
              label={props.batch ? t("batch.alreadyOpen") : t("batch.new")}
              disabled={props.batch !== null}
              onClick={() => setEditingBatch(true)}
            >
              <Plus size={20} aria-hidden="true" />
            </IconButton>
            <IconButton label={t("drives.refresh")} disabled={!props.connected} onClick={props.onRefresh}>
              <RefreshCw size={18} aria-hidden="true" />
            </IconButton>
          </div>
        </div>
        <p className="drives-station px-2 font-mono text-[13px] text-ash">
          <Station
            hello={props.hello?.ok ? props.hello.value : null}
            mode={props.mode}
            connected={props.connected}
            license={props.license}
          />
        </p>

        {editingBatch && props.batch === null && (
          <BatchDialog
            onCancel={() => setEditingBatch(false)}
            onOpen={(batch) => {
              props.onOpenBatch(batch);
              setEditingBatch(false);
            }}
          />
        )}
      </div>

      <div className="split-dhead flex min-w-0 flex-col gap-4 px-12 pt-10 pb-5">
        <button
          type="button"
          onClick={back}
          aria-label={t("drives.backLabel")}
          className="split-back min-h-11 items-center gap-2 self-start rounded pr-3 pl-1"
        >
          <ChevronLeft size={18} aria-hidden="true" />
          {t("drives.back")}
        </button>
        {selected && (
          <DetailHead
            drive={selected}
            titleRef={titleRef}
            language={i18n.language}
          />
        )}
        {many && (
          <div className="min-w-0">
            <div className="font-mono text-sm text-ash">
              {selectedDrives.some((drive) => props.runningOn(serialOf(drive)) !== null)
                ? t("drives.multiRunning", {
                    ready: selectedDrives.filter(erasable).length,
                    running: selectedDrives.filter((drive) => props.runningOn(serialOf(drive)) !== null).length,
                  })
                : t("drives.multiReady", { count: selectedDrives.filter(erasable).length })}
            </div>
            <h2 ref={titleRef} tabIndex={-1} className="split-title mt-0.5 mb-1 text-[40px] leading-tight font-bold">
              {t("drives.selectedCount", { count: selectedDrives.length })}
            </h2>
          </div>
        )}
      </div>

      {/* Same row as the detail below: the first drive faces the first method. */}
      <aside className="split-lbody flex flex-col gap-5 border-r border-edge px-4 pb-6" aria-label={t("drives.listLabel")}>
        {/* Passing notices sit above the groups, not in the header: the header row
            sets where the detail starts, and must not grow with them. */}
        {props.batchNotice && <BatchNoticeLine notice={props.batchNotice} onOpenPdf={props.onOpenPdf} />}

        {props.interrupted.map((job) => (
          <div key={job.job_id} className="flex gap-3 rounded-md border border-ctrl p-3 text-sm">
            <TriangleAlert size={16} className="mt-1 shrink-0 text-sayno" aria-hidden="true" />
            <div>
              <p className="font-bold">{t("jobs.interruptedTitle")}</p>
              <p className="mt-1">
                {job.device.identity.model} · s/n <Mono>{job.device.identity.serial}</Mono>
              </p>
              <p className="mt-1 text-ash">{t("jobs.interruptedHint")}</p>
            </div>
            <IconButton label={t("actions.dismiss")} onClick={() => props.onDismissInterrupted(job.job_id)}>
              <X size={16} aria-hidden="true" />
            </IconButton>
          </div>
        ))}

        {props.hello && !props.hello.ok && (
          <p className="px-2 text-sayno">{t(`errors.${props.hello.error.code}`)}</p>
        )}
        {devices && !devices.ok && <p className="px-2 text-sayno">{t(`errors.${devices.error.code}`)}</p>}
        {devices?.ok && drives.length === 0 && <p className="px-2 text-ash">{t("drives.empty")}</p>}
        {grouped.map(({ group, drives: members }) => (
          <section key={group} aria-labelledby={`group-${group}`} className="flex flex-col gap-0.5">
            <h2
              id={`group-${group}`}
              className={`mb-1.5 flex items-center gap-2 px-2 text-[15px] font-bold ${groupText[group]}`}
            >
              <span className="size-2 rounded-full bg-current" aria-hidden="true" />
              {t(`drives.group.${group}`, { count: members.length })}
              {group === "ready" && erasableSerials.length > 1 && (
                <button
                  type="button"
                  onClick={selectAll}
                  className="-my-2 ml-auto min-h-11 rounded px-2 text-sm font-normal text-ash hover:bg-card hover:text-soft"
                >
                  {allSelected ? t("drives.unselectAll") : t("drives.selectAll")}
                </button>
              )}
            </h2>
            {/* Several frozen drives: one sleep thaws them all (UX review D5). */}
            {group === "blocked" && props.sleep.possible && frozenCount > 1 && (
              <p className="-mt-1 mb-1 flex flex-wrap items-center gap-x-2 px-2 text-sm text-ash">
                {t("sleep.frozenGroup", { count: frozenCount })}
                <button
                  type="button"
                  disabled={props.sleep.running > 0}
                  onClick={props.sleep.onSleep}
                  className="min-h-9 rounded px-1 text-blocked hover:underline disabled:opacity-40"
                >
                  {t("sleep.confirm")}
                </button>
              </p>
            )}
            {members.map((drive) => {
              const serial = serialOf(drive);
              const isSelected = many
                ? selectedDrives.some((other) => serialOf(other) === serial)
                : selected !== null && serialOf(selected) === serial;
              const running = props.runningOn(serial) !== null;
              return (
                <button
                  key={serial}
                  type="button"
                  data-pick={serial}
                  aria-pressed={isSelected}
                  tabIndex={serial === tabStop ? 0 : -1}
                  onKeyDown={(event) => arrow(serial, event)}
                  // Shift + click selects a range, not the text of the rows.
                  onMouseDown={(event) => {
                    if (event.shiftKey) event.preventDefault();
                  }}
                  onClick={(event) => pick(serial, event)}
                  className={`flex min-h-16 min-w-0 flex-col justify-center rounded-md border px-3 py-2 text-left ${
                    isSelected ? "border-ctrl bg-card" : "border-transparent hover:bg-card/50"
                  }`}
                >
                  <span className="flex justify-between gap-2">
                    <span className="flex min-w-0 items-center gap-2">
                      {running && <LoaderCircle size={14} className="shrink-0 animate-spin" aria-hidden="true" />}
                      <span className={`truncate text-base ${isSelected ? "font-bold" : ""}`}>
                        {drive.device.identity.model}
                      </span>
                    </span>
                    <span className="font-mono text-sm whitespace-nowrap text-ash">
                      {t(`kind.${drive.kind ?? "unknown"}`)}
                    </span>
                  </span>
                  <span className="text-sm text-ash">{rowLine(drive)}</span>
                </button>
              );
            })}
          </section>
        ))}

        {props.license && (
          <button
            type="button"
            onClick={props.onLicense}
            className="mt-auto flex min-h-11 items-baseline justify-between gap-2 border-t border-edge px-2 pt-4 text-left hover:text-soft"
          >
            <span className="text-[15px] font-semibold text-ash">{t("license.title")}</span>
            <span className="font-mono text-sm">{licenseSummary(props.license, t, i18n.language)}</span>
          </button>
        )}
      </aside>

      <main className="split-dbody flex min-w-0 flex-col gap-7 px-12 pb-10" aria-live="polite">
        {many && (
          <SelectionDetail
            drives={selectedDrives}
            inspections={inspections}
            connected={props.connected}
            erasable={erasable}
            choiceIndex={(id) => methods[id] ?? 0}
            onChoose={(id, index) => setMethods((current) => ({ ...current, [id]: index }))}
            zeroPass={(id) => zeroPass[id] ?? false}
            onZeroPass={(id, value) => setZeroPass((current) => ({ ...current, [id]: value }))}
            name={name}
            rowLine={rowLine}
            groupFor={groupFor}
            runningOn={props.runningOn}
            onOpenJob={props.onOpenJob}
            onUnselect={unselect}
            onErase={eraseSelected}
          />
        )}
        {selected && (
          <Detail
            drive={selected}
            inspection={inspections[serialOf(selected)]}
            running={props.runningOn(serialOf(selected))}
            finished={props.finishedOn(serialOf(selected))}
            connected={props.connected}
            choiceIndex={(id) => methods[id] ?? 0}
            onChoose={(id, index) => setMethods((current) => ({ ...current, [id]: index }))}
            name={name}
            reasonText={reasonText}
            onErase={props.onErase}
            onUnlock={props.onUnlock}
            sleep={props.sleep}
            onReinspect={props.onReinspect}
            onOpenJob={props.onOpenJob}
            pdfOf={props.pdfOf}
            onOpenPdf={props.onOpenPdf}
            zeroPass={(id) => zeroPass[id] ?? false}
            onZeroPass={(id, value) => setZeroPass((current) => ({ ...current, [id]: value }))}
          />
        )}
      </main>
    </div>
  );
}

/**
 * The selected drive: type and capacity, model, serial number and WWN / EUI-64.
 * No state chip: the list already shows the state, and a coloured pill read as a button.
 */
function DetailHead({
  drive,
  titleRef,
  language,
}: {
  drive: DeviceSummary;
  titleRef: React.RefObject<HTMLHeadingElement | null>;
  language: string;
}) {
  const { t } = useTranslation();
  const { identity } = drive.device;
  const id = identity.eui64 ? `EUI-64 ${identity.eui64}` : identity.wwn ? `WWN ${identity.wwn}` : null;
  return (
    <div className="min-w-0">
      <div className="font-mono text-sm break-words text-ash">
        {t(`kind.${drive.kind ?? "unknown"}`)} · {capacity(drive.capacity_bytes, language, t("drives.unknownSize"))}
      </div>
      <h2
        ref={titleRef}
        tabIndex={-1}
        className="split-title mt-0.5 mb-1 text-[40px] leading-tight font-bold break-words"
      >
        {identity.model}
      </h2>
      <div className="font-mono text-sm break-all text-ash">
        s/n {identity.serial}
        {id && ` · ${id}`}
      </div>
    </div>
  );
}

function Detail({
  drive,
  inspection,
  running,
  finished,
  connected,
  choiceIndex,
  onChoose,
  name,
  reasonText,
  onErase,
  onUnlock,
  sleep,
  onReinspect,
  onOpenJob,
  pdfOf,
  onOpenPdf,
  zeroPass,
  onZeroPass,
}: {
  drive: DeviceSummary;
  inspection: Outcome<InspectResult> | undefined;
  running: Running;
  finished: Finished;
  connected: boolean;
  choiceIndex: (verdictId: string) => number;
  onChoose: (verdictId: string, index: number) => void;
  name: (code: string) => string;
  reasonText: (inspection: InspectResult, code: string) => string;
  onErase: (request: EraseRequest) => void;
  onUnlock: (inspection: InspectResult) => void;
  sleep: Sleep;
  onReinspect: (serial: string) => void;
  onOpenJob: (id: string) => void;
  pdfOf: (jobId: string) => PdfState | undefined;
  onOpenPdf: (path: string) => void;
  zeroPass: (verdictId: string) => boolean;
  onZeroPass: (verdictId: string, value: boolean) => void;
}) {
  const { t } = useTranslation();
  const { identity } = drive.device;

  // The decision comes first, level with the drive list; the last erasure follows it.
  const lastErasure = !running && finished && (
    <div className="flex flex-wrap items-center gap-3 rounded-md border border-ctrl p-4 text-sm">
      <span>{t("jobs.lastErasure")}</span>
      <Badge tone={finished.status === "CONFORME" ? "ready" : "sayno"}>{t(`job.status.${finished.status}`)}</Badge>
      <span className="w-full text-ash">{t("jobs.lastErasureHint")}</span>
      <PdfButton pdf={pdfOf(finished.id)} onOpenPdf={onOpenPdf} />
      <Button onClick={() => onOpenJob(finished.id)}>{t("jobs.view")}</Button>
    </div>
  );

  return (
    <>
      {running && (
        <div className="flex flex-col gap-4 rounded-lg border border-ctrl bg-card p-6">
          <p className="font-semibold">{t("jobs.inProgress")}</p>
          <JobProgress job={running.job} />
          <div className="drives-actions flex flex-wrap items-center gap-4">
            <Button onClick={() => onOpenJob(running.id)}>{t("jobs.view")}</Button>
          </div>
        </div>
      )}

      {!running && inspection === undefined && (
        <Panel message={t("drives.inspecting")} items={[]} />
      )}

      {!running && inspection && !inspection.ok && (
        <>
          <Panel
            message={t(`errors.${inspection.error.code}`)}
            items={inspection.error.detail ? [inspection.error.detail] : []}
          />
          <Actions>
            <Button onClick={() => onReinspect(identity.serial)}>{t("drives.reinspect")}</Button>
          </Actions>
        </>
      )}

      {!running && inspection?.ok && (
        <Verdict
          inspection={inspection.value}
          connected={connected}
          index={choiceIndex(inspection.value.verdict_id)}
          onChoose={(index) => onChoose(inspection.value.verdict_id, index)}
          zeroPass={zeroPass(inspection.value.verdict_id)}
          onZeroPass={(value) => onZeroPass(inspection.value.verdict_id, value)}
          name={name}
          reasonText={reasonText}
          onErase={onErase}
          onUnlock={onUnlock}
          sleep={sleep}
          onReinspect={() => onReinspect(identity.serial)}
        />
      )}

      {lastErasure}
    </>
  );
}

function Verdict({
  inspection,
  connected,
  index,
  onChoose,
  name,
  reasonText,
  onErase,
  onUnlock,
  sleep,
  onReinspect,
  zeroPass,
  onZeroPass,
}: {
  inspection: InspectResult;
  connected: boolean;
  index: number;
  onChoose: (index: number) => void;
  zeroPass: boolean;
  onZeroPass: (value: boolean) => void;
  name: (code: string) => string;
  reasonText: (inspection: InspectResult, code: string) => string;
  onErase: (request: EraseRequest) => void;
  onUnlock: (inspection: InspectResult) => void;
  sleep: Sleep;
  onReinspect: () => void;
}) {
  const { t } = useTranslation();
  const verdict = inspection.verdict;
  const reasons = (codes: string[]) => codes.map((code) => ({ code, text: reasonText(inspection, code) }));

  switch (verdict.state) {
    case "READY": {
      const choices = choicesOf(inspection);
      const choice = choices[index] ?? verdict.recommended;
      const destroy = verdict.disposition.kind === "DestroyRecommended" ? verdict.disposition.reasons : [];
      return (
        <>
          {/* Same recipe as several drives: the method button and its menu, level with the first drive. */}
          <div className="flex min-w-0 flex-col gap-0.5">
            <h3 className="mb-1.5 flex items-center gap-2 px-2 text-[15px] font-bold text-ash">
              {t("verdict.recommendation")}
            </h3>
            <div className="flex min-h-16 items-center">
              <MethodMenu
                options={optionsOf(inspection)}
                position={index}
                zero={zeroPass}
                name={name}
                label={t("drives.methodFor", { model: inspection.device.identity.model })}
                wide
                onPick={(position, zero) => {
                  onChoose(position);
                  onZeroPass(zero);
                }}
              />
            </div>
          </div>
          {destroy.map((code) => (
            <Note key={code} code={code} text={t("verdict.destroyNote", { reason: name(code) })} />
          ))}
          {choice.reserves.map((code) => (
            <Note key={code} code={code} text={t("verdict.reserveNote", { reason: name(code) })} />
          ))}
          <Actions>
            <button
              type="button"
              disabled={!connected}
              onClick={() =>
                onErase({
                  inspection,
                  choice,
                  forced: false,
                  zeroPass: zeroFor(choice, zeroPass),
                })
              }
              className="inline-flex min-h-13 items-center gap-2.5 rounded border-2 border-current px-7 text-lg font-bold text-ready disabled:opacity-40"
            >
              <Play size={16} fill="currentColor" aria-hidden="true" />
              {t("drives.erase")}
            </button>
            <span className="text-[15px] text-ash">
              {name(choice.method)}
              {zeroFor(choice, zeroPass) && <> {t("drives.thenZeros")}</>} ·{" "}
              {choice.reserves.length > 0 ? t("drives.withReserves") : t("drives.withoutReserves")}
            </span>
          </Actions>
        </>
      );
    }
    case "BLOCKED": {
      const unlock = verdict.reasons.includes("B2") && !verdict.reasons.includes("B6");
      // Frozen (B1): a sleep thaws it, when this machine can sleep (UX review D5).
      const frozen = verdict.reasons.includes("B1") && sleep.possible;
      return (
        <>
          <Panel
            message={t("verdict.blockedHint")}
            items={verdict.reasons.map((code) => `${t(`block.${code}`)} — ${t(`guidance.${code}`)}`)}
            codes={verdict.reasons}
          />
          <Actions>
            {unlock ? (
              <Button variant="blocked" className="flex items-center gap-2" onClick={() => onUnlock(inspection)}>
                <KeyRound size={16} aria-hidden="true" />
                {t("actions.unlock")}
              </Button>
            ) : frozen ? (
              <>
                <Button
                  variant="blocked"
                  className="flex items-center gap-2"
                  disabled={sleep.running > 0}
                  onClick={sleep.onSleep}
                >
                  <Moon size={16} aria-hidden="true" />
                  {t("sleep.action")}
                </Button>
                <span className="text-sm text-ash">
                  {sleep.running > 0 ? t("sleep.waitJobs", { count: sleep.running }) : t("sleep.hint")}
                </span>
              </>
            ) : (
              <Button variant="blocked" onClick={onReinspect}>
                {t("drives.reinspect")}
              </Button>
            )}
          </Actions>
        </>
      );
    }
    case "SAY_NO": {
      const forcible = verdict.forcible;
      const items = reasons(verdict.reasons);
      return (
        <>
          <Panel
            message={t("verdict.sayNoHint")}
            items={items.map((item) => item.text)}
            codes={items.map((item) => item.code)}
            footer={forcible ? t("verdict.forcible") : undefined}
          />
          {forcible && (
            <Actions>
              <Button variant="danger" onClick={() => onErase({ inspection, choice: forcible, forced: true })}>
                {t("actions.force")}
              </Button>
            </Actions>
          )}
        </>
      );
    }
    case "PROTECTED":
      return (
        <Panel
          message={t("verdict.protectedHint")}
          items={verdict.reasons.map((code) => t(`protect.${code}`))}
          codes={verdict.reasons}
        />
      );
  }
}

/**
 * Several drives selected: one line per drive, with its method for the ready
 * ones, then one button for all of them. A drive that is not ready stays in
 * the list with its state, and is left out of the erasure.
 */
function SelectionDetail({
  drives,
  inspections,
  connected,
  erasable,
  choiceIndex,
  onChoose,
  zeroPass,
  onZeroPass,
  name,
  rowLine,
  groupFor,
  runningOn,
  onOpenJob,
  onUnselect,
  onErase,
}: {
  drives: DeviceSummary[];
  inspections: Record<string, Outcome<InspectResult>>;
  connected: boolean;
  erasable: (drive: DeviceSummary) => boolean;
  choiceIndex: (verdictId: string) => number;
  onChoose: (verdictId: string, index: number) => void;
  zeroPass: (verdictId: string) => boolean;
  onZeroPass: (verdictId: string, value: boolean) => void;
  name: (code: string) => string;
  rowLine: (drive: DeviceSummary) => string;
  groupFor: (drive: DeviceSummary) => Group;
  runningOn: (serial: string) => Running;
  onOpenJob: (id: string) => void;
  onUnselect: (serial: string) => void;
  onErase: () => void;
}) {
  const { t } = useTranslation();
  const ready = drives.filter(erasable);
  const running = drives.map((drive) => runningOn(drive.device.identity.serial)).filter((job) => job !== null);
  const skipped = drives.length - ready.length - running.length;
  return (
    <>
      {/* Same rhythm as the drive list: the first selected drive faces the first drive. */}
      <div role="group" aria-labelledby="selection-methods" className="flex min-w-0 flex-col gap-0.5">
        <h3 id="selection-methods" className="mb-1.5 flex items-center gap-2 px-2 text-[15px] font-bold text-ash">
          {ready.length === 0 && running.length > 0 ? t("drives.selectionRunning") : t("drives.methodEach")}
        </h3>
        {drives.map((drive) => {
          const { model, serial } = drive.device.identity;
          const inspection = inspections[serial];
          const value = inspection?.ok ? inspection.value : null;
          const choices = value ? choicesOf(value) : [];
          const group = groupFor(drive);
          const job = runningOn(serial);
          return (
            <div key={serial} className="flex min-h-16 items-center gap-3 rounded-md px-3 py-1">
              {/* At most 24 rem: on a wide screen the method stays next to its drive. */}
              <div className="flex min-w-0 max-w-[24rem] flex-1 flex-col">
                <span className="truncate font-bold">{model}</span>
                <span className="truncate font-mono text-sm text-ash">s/n {serial}</span>
              </div>
              {erasable(drive) && value && choices.length > 0 ? (
                <MethodMenu
                  options={optionsOf(value)}
                  position={choiceIndex(value.verdict_id)}
                  zero={zeroPass(value.verdict_id)}
                  name={name}
                  label={t("drives.methodFor", { model })}
                  onPick={(position, zero) => {
                    onChoose(value.verdict_id, position);
                    onZeroPass(value.verdict_id, zero);
                  }}
                />
              ) : job ? (
                // Being erased: its progress, as in the Effacements tab.
                <div className="flex w-[26rem] max-w-[55%] flex-col gap-1.5">
                  <span className="text-sm">{rowLine(drive)}</span>
                  <Bar percent={job.job.progress?.percent ?? 0} className="h-1" />
                </div>
              ) : (
                <span className={`max-w-[55%] text-sm ${groupText[group]}`}>{rowLine(drive)}</span>
              )}
              <IconButton label={t("drives.unselect", { model })} onClick={() => onUnselect(serial)}>
                <X size={16} aria-hidden="true" />
              </IconButton>
            </div>
          );
        })}
      </div>
      <Actions>
        {ready.length > 0 || running.length === 0 ? (
          <button
            type="button"
            disabled={!connected || ready.length === 0}
            onClick={onErase}
            className="inline-flex min-h-13 items-center gap-2.5 rounded border-2 border-current px-7 text-lg font-bold text-ready disabled:opacity-40"
          >
            <Play size={16} fill="currentColor" aria-hidden="true" />
            {t("drives.eraseTicked", { count: ready.length })}
          </button>
        ) : (
          // Everything selected is being erased: follow it, nothing to launch.
          <Button className="min-h-12 px-5" onClick={() => onOpenJob(running[0].id)}>
            {t("drives.followJobs")}
          </Button>
        )}
        {running.length > 0 && (
          <span className="text-[15px] text-ash">{t("drives.multiBeingErased", { count: running.length })}</span>
        )}
        {skipped > 0 && <span className="text-[15px] text-ash">{t("drives.multiSkipped", { count: skipped })}</span>}
      </Actions>
    </>
  );
}

/** Purge filled, Clear outlined: the category of a method. */
/**
 * The method of one drive among several: a quiet button showing the method
 * chosen (the recommended one at first), which opens a menu of the methods.
 * Host overwrite appears twice, alone and with its final pass of zeros.
 * Arrows move in the menu, Enter picks, Escape closes.
 */
function MethodMenu({
  options,
  position,
  zero,
  name,
  label,
  wide = false,
  onPick,
}: {
  options: MethodOption[];
  position: number;
  zero: boolean;
  name: (code: string) => string;
  label: string;
  /** One drive: a larger button under the title, its menu opening to the right. */
  wide?: boolean;
  onPick: (position: number, zero: boolean) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const items = useRef<(HTMLButtonElement | null)[]>([]);
  const current =
    options.find((option) => option.position === position && option.zero === zeroFor(option.choice, zero)) ??
    options[0];
  const currentIndex = options.indexOf(current);

  useEffect(() => {
    if (!open) return;
    items.current[currentIndex]?.focus();
    const outside = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", outside);
    return () => document.removeEventListener("mousedown", outside);
  }, [open, currentIndex]);

  const close = () => {
    setOpen(false);
    trigger.current?.focus();
  };
  const onKey = (event: React.KeyboardEvent) => {
    if (!open) return;
    const at = items.current.findIndex((item) => item === document.activeElement);
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      items.current[(at + step + options.length) % options.length]?.focus();
    } else if (event.key === "Escape") {
      // Closes the menu only, not the selection.
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (event.key === "Tab") {
      setOpen(false);
    }
  };

  const title = (option: MethodOption, bold: boolean) => (
    <span className={bold ? "font-bold" : undefined}>
      {name(option.choice.method)}
      {option.zero && <> {t("drives.thenZeros")}</>}
    </span>
  );

  return (
    <div ref={root} className={`relative shrink-0 ${wide ? "w-[34rem] max-w-full" : "w-[26rem] max-w-[55%]"}`} onKeyDown={onKey}>
      <button
        ref={trigger}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`${label} : ${name(current.choice.method)}`}
        onClick={() => setOpen((value) => !value)}
        className={`flex min-h-12 w-full items-center gap-3 rounded-md border px-3 text-left hover:border-ctrl hover:bg-card ${
          open ? "border-ctrl bg-card" : "border-transparent"
        }`}
      >
        <Category category={current.choice.category} />
        <span className="flex min-w-0 flex-1 flex-col leading-snug">
          <span className="truncate">{name(current.choice.method)}</span>
          {current.zero ? (
            <span className="text-xs text-ash">{t("drives.thenZeros")}</span>
          ) : (
            current.position === 0 && <span className="text-xs font-bold text-ready">{t("verdict.recommendedShort")}</span>
          )}
        </span>
        <ChevronDown
          size={16}
          className={`shrink-0 text-ash transition-transform ${open ? "rotate-180" : ""}`}
          aria-hidden="true"
        />
      </button>
      {open && (
        <div
          role="menu"
          aria-label={label}
          className={`absolute top-full z-20 mt-1 flex ${wide ? "left-0 w-full" : "right-0 w-[30rem]"} max-w-[85vw] flex-col gap-0.5 rounded-lg border border-ctrl bg-card p-1.5 shadow-xl shadow-black/50`}
        >
          {options.map((option, at) => {
            const checked = option === current;
            return (
              <button
                key={`${option.choice.method}${option.zero ? "+zero" : ""}`}
                ref={(item) => {
                  items.current[at] = item;
                }}
                type="button"
                role="menuitemradio"
                aria-checked={checked}
                tabIndex={-1}
                onClick={() => {
                  onPick(option.position, option.zero);
                  close();
                }}
                className="flex min-h-12 w-full items-center gap-3 rounded-md px-2.5 py-1.5 text-left hover:bg-night focus:bg-night"
              >
                <span className="flex w-4 shrink-0 justify-center" aria-hidden="true">
                  {checked && <Check size={16} />}
                </span>
                <Category category={option.choice.category} />
                <span className="flex min-w-0 flex-col">
                  <span>
                    {title(option, checked)}
                    {option.position === 0 && !option.zero && (
                      <>
                        {" · "}
                        <span className="font-bold text-ready">{t("verdict.recommendedShort")}</span>
                      </>
                    )}
                  </span>
                  <span className="text-xs text-ash">
                    {option.zero ? (
                      t("confirm.zeroPassHint")
                    ) : (
                      <span className="font-mono">{option.choice.method}</span>
                    )}
                  </span>
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** Opens the certificate saved when the erasure ended, or says why there is none. */
function PdfButton({ pdf, onOpenPdf }: { pdf: PdfState | undefined; onOpenPdf: (path: string) => void }) {
  const { t } = useTranslation();
  if (!pdf) return null;
  if (pdf.kind === "rendering") return <span className="text-ash">{t("job.exporting")}</span>;
  if (pdf.kind === "failed") return <span className="text-sayno">{t("job.pdfFailed")}</span>;
  return (
    <Button variant="primary" onClick={() => onOpenPdf(pdf.path)}>
      {t("job.openCertificate")}
    </Button>
  );
}

function Panel({
  message,
  items,
  codes,
  footer,
}: {
  message: string;
  items: string[];
  codes?: string[];
  footer?: string;
}) {
  return (
    <div className="drives-panel flex flex-col gap-2.5 rounded-lg border border-ctrl bg-card p-6">
      <p className="drives-message text-[21px]">{message}</p>
      {items.length > 0 && (
        <ul className="list-disc pl-5 text-ash">
          {items.map((item, position) => (
            <li key={item}>
              {item}
              {codes?.[position] && <span className="font-mono text-[13px]"> ({codes[position]})</span>}
            </li>
          ))}
        </ul>
      )}
      {footer && <p className="text-ash">{footer}</p>}
    </div>
  );
}

function Note({ code, text }: { code: string; text: string }) {
  return (
    <div className="flex items-start gap-3 rounded-md border border-ctrl px-4 py-3.5">
      <TriangleAlert size={18} className="mt-1 shrink-0" aria-hidden="true" />
      <div>
        <div>{text}</div>
        <div className="font-mono text-sm text-ash">{code}</div>
      </div>
    </div>
  );
}

/** Action row of the detail; its buttons are 48 px high, 17 px bold (docs/maquettes/disphas-ecran-disques.md §3.3). */
function Actions({ children }: { children: ReactNode }) {
  return (
    <div className="drives-actions flex flex-wrap items-center gap-4 [&>button]:min-h-12 [&>button]:px-5.5 [&>button]:text-[17px]">
      {children}
    </div>
  );
}

/** Under the title: outside a batch, or the open batch and how to close it. */
function BatchHint({ batch, closing, onClose }: { batch: BatchArgs | null; closing: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  if (!batch) return <p className="mt-1 text-[13px] text-ash italic">{t("drives.outsideBatch")}</p>;
  return (
    <p className="mt-1 text-[13px] text-ash">
      {t("batch.current")} <Mono>{batch.id}</Mono>
      {batch.label && <> · {batch.label}</>}{" "}
      <button
        type="button"
        disabled={closing}
        onClick={onClose}
        className="font-bold text-soft underline underline-offset-2 disabled:opacity-40"
      >
        {closing ? t("batch.closing") : t("batch.close")}
      </button>
    </p>
  );
}

/** Opening a batch: an optional label (a customer order number, say). */
/** Opens a batch before the erasures: one window, the label only (optional). */
function BatchDialog({ onCancel, onOpen }: { onCancel: () => void; onOpen: (batch: BatchArgs) => void }) {
  const { t } = useTranslation();
  const [label, setLabel] = useState("");
  const tooLong = [...label].length > MAX_LABEL;
  const open = () => {
    const trimmed = label.trim();
    onOpen({ id: newBatchId(), label: trimmed === "" ? null : trimmed });
  };
  return (
    <Dialog
      title={t("batch.new")}
      width="sm"
      onCancel={onCancel}
      footer={
        <>
          <Button onClick={onCancel}>{t("actions.cancel")}</Button>
          <Button variant="primary" disabled={tooLong} onClick={open}>
            {t("batch.open")}
          </Button>
        </>
      }
    >
      <p className="text-sm text-ash">{t("confirm.groupInBatchHint")}</p>
      <label className="block text-sm font-bold">
        {t("batch.label")}
        <input
          className={fieldClass}
          placeholder={t("batch.labelPlaceholder")}
          value={label}
          onChange={(event) => setLabel(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !tooLong) open();
          }}
          autoFocus
        />
        {tooLong && <span className="text-sm font-normal text-sayno">{t("batch.tooLong")}</span>}
      </label>
    </Dialog>
  );
}

function BatchNoticeLine({ notice, onOpenPdf }: { notice: BatchNotice; onOpenPdf: (path: string) => void }) {
  const { t } = useTranslation();
  switch (notice.kind) {
    case "running":
      return <p className="px-2 text-sm text-blocked">{t("batch.stillRunning", { count: notice.count })}</p>;
    case "pdf":
      return (
        <div className="flex flex-col gap-2 px-2 text-sm text-ash">
          <span>
            {t("batch.pdfSaved", { batch: notice.batch })} <Mono>{notice.path}</Mono>
          </span>
          <Button className="self-start" onClick={() => onOpenPdf(notice.path)}>
            {t("job.open")}
          </Button>
        </div>
      );
    case "empty":
      return <p className="px-2 text-sm text-ash">{t("batch.closedEmpty", { batch: notice.batch })}</p>;
    case "error":
      return (
        <p className="px-2 text-sm text-sayno">
          {t("batch.pdfFailed", { batch: notice.batch })}
          {notice.error.detail && <span className="block font-mono text-ash">{notice.error.detail}</span>}
        </p>
      );
  }
}

/** "Atelier8 · jusqu'au 25/11/2026", or the state of a license that is not valid. */
function licenseSummary(license: LicenseStatusResult, t: TFunction, lang: string) {
  if (license.state !== "Valid") return t(`station.license.${license.state}`);
  const until =
    license.valid_until === null
      ? null
      : new Intl.DateTimeFormat(lang, { dateStyle: "short" }).format(new Date(license.valid_until * 1000));
  const offer = license.offer ?? t("license.title");
  return until ? t("drives.licenseUntil", { offer, until }) : offer;
}
