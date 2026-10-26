import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  ChevronLeft,
  KeyRound,
  LoaderCircle,
  Lock,
  Play,
  Plus,
  RefreshCw,
  Shield,
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
import { Station } from "./Header";
import { JobProgress } from "./JobsScreen";
import { Badge, Button, Mono } from "./ui";

/** An erasure the operator asked for, before the confirmation dialog. */
export type EraseRequest = {
  inspection: InspectResult;
  choice: MethodChoice;
  forced: boolean;
};

/** What closing the batch produced. */
export type BatchNotice =
  | { kind: "running"; count: number }
  | { kind: "pdf"; batch: string; path: string }
  | { kind: "empty"; batch: string }
  | { kind: "error"; batch: string; error: HelperError };

/** What the operator has to do with a drive, in display order (docs/maquettes/disphas-ecran-disques.md §3.2). */
type Group = "ready" | "blocked" | "refus" | "protected" | "pending";

const GROUPS: Group[] = ["ready", "blocked", "refus", "protected", "pending"];

const groupText: Record<Group, string> = {
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
  onUnlock: (inspection: InspectResult) => void;
  onOpenJob: (id: string) => void;
  onLicense: () => void;
}) {
  const { t, i18n } = useTranslation();
  const { devices, inspections, labels } = props;
  const drives = devices?.ok ? devices.value.devices : [];
  const [picked, setPicked] = useState<string | null>(null);
  // Method chosen per verdict: a new inspection starts again from the recommended one.
  const [methods, setMethods] = useState<Record<string, number>>({});
  const [view, setView] = useState<"list" | "detail">("list");
  const [editingBatch, setEditingBatch] = useState(false);
  const titleRef = useRef<HTMLHeadingElement>(null);
  // Bumped when the drive title must take the focus: on a phone, or after a row's action button.
  const [focusTick, setFocusTick] = useState(0);

  const serialOf = (drive: DeviceSummary) => drive.device.identity.serial;
  const grouped = GROUPS.map((group) => ({
    group,
    drives: drives.filter((drive) => groupOf(inspections[serialOf(drive)]) === group),
  })).filter((entry) => entry.drives.length > 0);
  const ordered = grouped.flatMap((entry) => entry.drives);
  const selected = ordered.find((drive) => serialOf(drive) === picked) ?? ordered[0] ?? null;

  useEffect(() => {
    if (focusTick === 0) return;
    if (narrow()) window.scrollTo(0, 0);
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

  const open = (serial: string, focusDetail: boolean) => {
    setPicked(serial);
    setView("detail");
    if (focusDetail || narrow()) setFocusTick((tick) => tick + 1);
  };
  const back = () => {
    setView("list");
    const serial = selected ? serialOf(selected) : null;
    requestAnimationFrame(() => document.querySelector<HTMLButtonElement>(`[data-pick="${serial}"]`)?.focus());
  };

  /** What the action button of a row does. */
  const act = (drive: DeviceSummary) => {
    const serial = serialOf(drive);
    const running = props.runningOn(serial);
    if (running) return props.onOpenJob(running.id);
    const inspection = inspections[serial];
    if (!inspection?.ok) return inspection ? props.onReinspect(serial) : open(serial, true);
    const value = inspection.value;
    const verdict = value.verdict;
    switch (verdict.state) {
      case "READY": {
        const choice = chosen(value);
        if (choice) props.onErase({ inspection: value, choice, forced: false });
        return;
      }
      case "BLOCKED":
        // B6: attempts exhausted, only a power cycle helps, then a new inspection.
        if (verdict.reasons.includes("B2") && !verdict.reasons.includes("B6")) return props.onUnlock(value);
        return props.onReinspect(serial);
      case "SAY_NO":
        if (verdict.forcible) return props.onErase({ inspection: value, choice: verdict.forcible, forced: true });
        return open(serial, true);
      case "PROTECTED":
        return open(serial, true);
    }
  };

  const actionLabel = (drive: DeviceSummary): string => {
    const serial = serialOf(drive);
    const model = drive.device.identity.model;
    if (props.runningOn(serial)) return t("drives.row.viewJob", { model });
    const inspection = inspections[serial];
    if (!inspection) return t("drives.row.inspecting", { model });
    if (!inspection.ok) return t("drives.row.reinspect", { model });
    const verdict = inspection.value.verdict;
    switch (verdict.state) {
      case "READY":
        return t("drives.row.erase", { model, category: chosen(inspection.value)?.category ?? "" });
      case "BLOCKED":
        return verdict.reasons.includes("B2") && !verdict.reasons.includes("B6")
          ? t("drives.row.unlock", { model })
          : t("drives.row.reinspect", { model });
      case "SAY_NO":
        return verdict.forcible ? t("drives.row.force", { model }) : t("drives.row.whyRefused", { model });
      case "PROTECTED":
        return t("drives.row.whyProtected", { model });
    }
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
      return choice ? `${choice.category} · ${name(choice.method)}` : "";
    }
    const codes = reasonsOf(value);
    return codes.length > 0 ? `${reasonText(value, codes[0])} (${codes.join(", ")})` : "";
  };

  const rowIcon = (drive: DeviceSummary, group: Group): ReactNode => {
    if (props.runningOn(serialOf(drive))) return <LoaderCircle size={18} className="animate-spin" aria-hidden="true" />;
    switch (group) {
      case "ready":
        return <Play size={16} fill="currentColor" aria-hidden="true" />;
      case "blocked":
        return <Lock size={20} aria-hidden="true" />;
      case "refus":
        return <X size={18} strokeWidth={2.4} aria-hidden="true" />;
      case "protected":
        return <Shield size={20} aria-hidden="true" />;
      case "pending":
        return <RefreshCw size={18} aria-hidden="true" />;
    }
  };

  const actionTone: Record<Group, string> = {
    ready: "border-2 border-current text-ready",
    blocked: "border-2 border-current text-blocked",
    refus: "border-2 border-current text-sayno",
    protected: "border border-ctrl text-ash",
    pending: "border border-ctrl text-ash",
  };

  const parallel =
    props.maxParallel === undefined
      ? null
      : props.maxParallel === null
        ? t("drives.meta.unlimited", { count: props.runningCount })
        : t("drives.meta.limited", { count: props.runningCount, max: props.maxParallel });

  return (
    <div className="drives-screen grid min-h-0 flex-1" data-view={view}>
      <aside className="drives-list flex flex-col gap-5 border-r border-edge px-4 py-6" aria-label={t("drives.listLabel")}>
        <div className="flex items-start justify-between gap-2 px-2">
          <div>
            <h1 className="text-[25px] leading-tight font-bold">{t("drives.count", { count: drives.length })}</h1>
            {parallel && <p className="mt-0.5 font-mono text-sm text-ash">{parallel}</p>}
            <BatchHint
              batch={props.batch}
              closing={props.closingBatch}
              onClose={props.onCloseBatch}
            />
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
          <BatchForm
            onCancel={() => setEditingBatch(false)}
            onOpen={(batch) => {
              props.onOpenBatch(batch);
              setEditingBatch(false);
            }}
          />
        )}
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
            </h2>
            {members.map((drive) => {
              const serial = serialOf(drive);
              const isSelected = selected !== null && serialOf(selected) === serial;
              const actionText = actionLabel(drive);
              return (
                <div
                  key={serial}
                  className={`flex min-h-16 items-center gap-3 rounded-md border px-3 py-1 ${
                    isSelected ? "border-ctrl bg-card" : "border-transparent"
                  }`}
                >
                  <button
                    type="button"
                    aria-label={actionText}
                    title={actionText}
                    disabled={group === "ready" && !props.connected}
                    onClick={() => act(drive)}
                    className={`inline-flex size-11 shrink-0 items-center justify-center rounded-md disabled:opacity-40 ${actionTone[group]}`}
                  >
                    {rowIcon(drive, group)}
                  </button>
                  <button
                    type="button"
                    data-pick={serial}
                    aria-pressed={isSelected}
                    onClick={() => open(serial, false)}
                    className="flex min-h-14 min-w-0 flex-1 flex-col justify-center px-1 py-2 text-left"
                  >
                    <span className="flex justify-between gap-2">
                      <span className="truncate text-base">{drive.device.identity.model}</span>
                      <span className="font-mono text-sm whitespace-nowrap text-ash">
                        {t(`kind.${drive.kind ?? "unknown"}`)}
                      </span>
                    </span>
                    <span className="text-sm text-ash">{rowLine(drive)}</span>
                  </button>
                </div>
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

      <main className="drives-detail flex min-w-0 max-w-[960px] flex-col gap-7 px-12 py-10" aria-live="polite">
        <button
          type="button"
          onClick={back}
          aria-label={t("drives.backLabel")}
          className="drives-back min-h-11 items-center gap-2 self-start rounded pr-3 pl-1"
        >
          <ChevronLeft size={18} aria-hidden="true" />
          {t("drives.back")}
        </button>
        {selected && (
          <Detail
            drive={selected}
            inspection={inspections[serialOf(selected)]}
            running={props.runningOn(serialOf(selected))}
            finished={props.finishedOn(serialOf(selected))}
            connected={props.connected}
            titleRef={titleRef}
            choiceIndex={(id) => methods[id] ?? 0}
            onChoose={(id, index) => setMethods((current) => ({ ...current, [id]: index }))}
            name={name}
            reasonText={reasonText}
            onErase={props.onErase}
            onUnlock={props.onUnlock}
            onReinspect={props.onReinspect}
            onOpenJob={props.onOpenJob}
            language={i18n.language}
          />
        )}
      </main>
    </div>
  );
}

function Detail({
  drive,
  inspection,
  running,
  finished,
  connected,
  titleRef,
  choiceIndex,
  onChoose,
  name,
  reasonText,
  onErase,
  onUnlock,
  onReinspect,
  onOpenJob,
  language,
}: {
  drive: DeviceSummary;
  inspection: Outcome<InspectResult> | undefined;
  running: Running;
  finished: Finished;
  connected: boolean;
  titleRef: React.RefObject<HTMLHeadingElement | null>;
  choiceIndex: (verdictId: string) => number;
  onChoose: (verdictId: string, index: number) => void;
  name: (code: string) => string;
  reasonText: (inspection: InspectResult, code: string) => string;
  onErase: (request: EraseRequest) => void;
  onUnlock: (inspection: InspectResult) => void;
  onReinspect: (serial: string) => void;
  onOpenJob: (id: string) => void;
  language: string;
}) {
  const { t } = useTranslation();
  const { identity } = drive.device;
  const group = groupOf(inspection);
  const id = identity.eui64 ? `EUI-64 ${identity.eui64}` : identity.wwn ? `WWN ${identity.wwn}` : null;
  const chip: Record<Group, ReactNode> = {
    ready: <Badge tone="ready">▶ {t("drives.chip.ready")}</Badge>,
    blocked: <Badge tone="blocked">{t("drives.chip.blocked")}</Badge>,
    refus: <Badge tone="sayno">✕ {t("drives.chip.refus")}</Badge>,
    protected: <Badge tone="protected">{t("drives.chip.protected")}</Badge>,
    pending: <Badge tone="neutral">{t("drives.chip.pending")}</Badge>,
  };

  return (
    <>
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="font-mono text-sm break-words text-ash">
            {t(`kind.${drive.kind ?? "unknown"}`)} · {capacity(drive.capacity_bytes, language, t("drives.unknownSize"))}
          </div>
          <h2
            ref={titleRef}
            tabIndex={-1}
            className="drives-title mt-0.5 mb-1 text-[40px] leading-tight font-bold break-words"
          >
            {identity.model}
          </h2>
          <div className="font-mono text-sm break-all text-ash">
            s/n {identity.serial}
            {id && ` · ${id}`}
          </div>
        </div>
        {chip[group]}
      </div>

      {running && (
        <div className="flex flex-col gap-4 rounded-lg border border-ctrl bg-card p-6">
          <p className="font-semibold">{t("jobs.inProgress")}</p>
          <JobProgress job={running.job} />
          <div className="drives-actions flex flex-wrap items-center gap-4">
            <Button onClick={() => onOpenJob(running.id)}>{t("jobs.view")}</Button>
          </div>
        </div>
      )}

      {!running && finished && (
        <div className="flex flex-wrap items-center gap-3 rounded-md border border-ctrl p-4 text-sm">
          <span>{t("jobs.lastErasure")}</span>
          <Badge tone={finished.status === "CONFORME" ? "ready" : "sayno"}>{t(`job.status.${finished.status}`)}</Badge>
          <span className="w-full text-ash">{t("jobs.lastErasureHint")}</span>
          <Button onClick={() => onOpenJob(finished.id)}>{t("jobs.view")}</Button>
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
          name={name}
          reasonText={reasonText}
          onErase={onErase}
          onUnlock={onUnlock}
          onReinspect={() => onReinspect(identity.serial)}
        />
      )}
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
  onReinspect,
}: {
  inspection: InspectResult;
  connected: boolean;
  index: number;
  onChoose: (index: number) => void;
  name: (code: string) => string;
  reasonText: (inspection: InspectResult, code: string) => string;
  onErase: (request: EraseRequest) => void;
  onUnlock: (inspection: InspectResult) => void;
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
          <fieldset className="flex min-w-0 flex-col gap-1 rounded-lg border border-ctrl bg-card p-6">
            <legend className="px-2 text-[15px] font-semibold text-ash">{t("verdict.recommendation")}</legend>
            {choices.map((option, position) => (
              <label key={option.method} className="flex min-h-14 cursor-pointer items-center gap-3.5 rounded-md px-3 py-1.5">
                <input
                  type="radio"
                  name={`method-${inspection.verdict_id}`}
                  checked={position === index}
                  onChange={() => onChoose(position)}
                  className="method-radio"
                />
                <span
                  className={`w-[52px] shrink-0 rounded-[3px] py-0.5 text-center font-mono text-[13px] ${
                    option.category === "Purge" ? "bg-soft text-night" : "border border-ctrl"
                  }`}
                >
                  {option.category}
                </span>
                <span className="flex min-w-0 flex-col">
                  <span className={position === index ? "font-bold" : undefined}>
                    {name(option.method)}
                    {position === 0 && (
                      <>
                        {" · "}
                        <span className="font-bold text-ready">{t("verdict.recommendedShort")}</span>
                      </>
                    )}
                  </span>
                  <span className="font-mono text-sm break-all text-ash">{option.method}</span>
                </span>
              </label>
            ))}
          </fieldset>
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
              onClick={() => onErase({ inspection, choice, forced: false })}
              className="inline-flex min-h-13 items-center gap-2.5 rounded border-2 border-current px-7 text-lg font-bold text-ready disabled:opacity-40"
            >
              <Play size={16} fill="currentColor" aria-hidden="true" />
              {t("drives.erase")}
            </button>
            <span className="text-[15px] text-ash">
              {name(choice.method)} ·{" "}
              {choice.reserves.length > 0 ? t("drives.withReserves") : t("drives.withoutReserves")}
            </span>
          </Actions>
        </>
      );
    }
    case "BLOCKED": {
      const unlock = verdict.reasons.includes("B2") && !verdict.reasons.includes("B6");
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

function IconButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className="inline-flex size-11 shrink-0 items-center justify-center rounded text-soft hover:bg-card disabled:opacity-40"
    >
      {children}
    </button>
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
function BatchForm({ onCancel, onOpen }: { onCancel: () => void; onOpen: (batch: BatchArgs) => void }) {
  const { t } = useTranslation();
  const [label, setLabel] = useState("");
  const tooLong = [...label].length > MAX_LABEL;
  const open = () => {
    const trimmed = label.trim();
    onOpen({ id: newBatchId(), label: trimmed === "" ? null : trimmed });
  };
  return (
    <div className="flex flex-col gap-2 rounded-md border border-ctrl p-3 text-sm">
      <label className="font-semibold" htmlFor="batch-label">
        {t("batch.new")}
      </label>
      <input
        id="batch-label"
        className="min-h-11 rounded border border-ctrl bg-night px-2"
        placeholder={t("batch.labelPlaceholder")}
        value={label}
        onChange={(event) => setLabel(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !tooLong) open();
          if (event.key === "Escape") onCancel();
        }}
        autoFocus
      />
      {tooLong && <span className="text-sayno">{t("batch.tooLong")}</span>}
      <div className="flex justify-end gap-2">
        <Button onClick={onCancel}>{t("actions.cancel")}</Button>
        <Button variant="primary" disabled={tooLong} onClick={open}>
          {t("batch.open")}
        </Button>
      </div>
    </div>
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
