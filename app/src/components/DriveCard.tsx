import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Ban, CheckCircle2, HardDrive, KeyRound, Lock, ShieldCheck, TriangleAlert, XCircle } from "lucide-react";
import type { DeviceSummary } from "../bindings/DeviceSummary";
import type { FinalStatus } from "../bindings/FinalStatus";
import type { InspectResult } from "../bindings/InspectResult";
import type { MethodChoice } from "../bindings/MethodChoice";
import type { Outcome } from "../lib/api";
import type { JobView } from "../lib/jobs";
import { JobProgress } from "./JobsScreen";
import { capacity } from "../lib/format";
import { Badge, Button, Mono } from "./ui";

export type EraseRequest = {
  inspection: InspectResult;
  choice: MethodChoice;
  forced: boolean;
};

const stateTone = {
  READY: "ready",
  PROTECTED: "protected",
  BLOCKED: "blocked",
  SAY_NO: "sayno",
} as const;

const frameTone = {
  READY: "border-ready/40 bg-ready/5",
  PROTECTED: "border-protected/40 bg-protected/5",
  BLOCKED: "border-blocked/40 bg-blocked/5",
  SAY_NO: "border-sayno/40 bg-sayno/5",
} as const;

export function DriveCard({
  summary,
  inspection,
  labels,
  onErase,
  onUnlock,
  running,
  finished,
  onOpenJob,
}: {
  summary: DeviceSummary;
  inspection: Outcome<InspectResult> | undefined;
  labels: Record<string, string>;
  onErase: (request: EraseRequest) => void;
  onUnlock: (inspection: InspectResult) => void;
  /** Erasure in progress on this drive, if any. */
  running: { id: string; job: JobView } | null;
  /**
   * Last erasure of this drive finished in this session. The verdict shown
   * comes from the inspection made afterwards, which may well offer the same
   * methods again: without this line the card looks untouched.
   */
  finished: { id: string; status: FinalStatus } | null;
  onOpenJob: (id: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const label = (code: string) => (labels[code] ? `${labels[code]} (${code})` : code);
  const { identity, node_hint } = summary.device;

  return (
    <article className="rounded-lg border border-edge bg-card p-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]">
      <div className="flex items-center gap-3">
        <Badge tone="neutral">
          <HardDrive size={12} />
          {t(`kind.${summary.kind ?? "unknown"}`)}
        </Badge>
        <h2 className="font-bold">{identity.model}</h2>
        <span className="text-sm text-ash">
          {capacity(summary.capacity_bytes, i18n.language, t("drives.unknownSize"))}
        </span>
        <div className="ml-auto">
          {inspection?.ok ? (
            <Badge tone={stateTone[inspection.value.verdict.state]}>
              {t(`state.${inspection.value.verdict.state}`)}
            </Badge>
          ) : (
            <span className="text-xs text-ash">{t("drives.inspecting")}</span>
          )}
        </div>
      </div>
      <p className="mt-1.5 text-sm text-ash">
        S/N <Mono>{identity.serial}</Mono>
        {node_hint && (
          <>
            {" · "}
            <Mono>{node_hint}</Mono>
          </>
        )}
        {(identity.eui64 || identity.wwn) && (
          <>
            {" · "}
            {identity.eui64 ? "EUI-64" : "WWN"} <Mono>{identity.eui64 ?? identity.wwn}</Mono>
          </>
        )}
      </p>

      {inspection && !inspection.ok && (
        <p className="mt-4 text-sm text-sayno">{t(`errors.${inspection.error.code}`)}</p>
      )}
      {running && (
        <div className="mt-4 rounded-lg border border-brand/40 bg-brand/5 p-4">
          <p className="mb-2 text-sm font-bold">{t("jobs.inProgress")}</p>
          <JobProgress job={running.job} />
          <div className="mt-3 flex justify-end">
            <Button onClick={() => onOpenJob(running.id)}>{t("jobs.view")}</Button>
          </div>
        </div>
      )}
      {!running && finished && (
        <div className="mt-4 flex items-center gap-3 rounded-lg border border-edge bg-night/40 p-3 text-sm">
          {finished.status === "CONFORME" ? (
            <CheckCircle2 size={16} className="shrink-0 text-ready" />
          ) : (
            <XCircle size={16} className="shrink-0 text-sayno" />
          )}
          <div>
            <p>
              {t("jobs.lastErasure")}{" "}
              <Badge tone={finished.status === "CONFORME" ? "ready" : "sayno"}>
                {t(`job.status.${finished.status}`)}
              </Badge>
            </p>
            <p className="mt-0.5 text-xs text-ash">{t("jobs.lastErasureHint")}</p>
          </div>
          <Button className="ml-auto shrink-0" onClick={() => onOpenJob(finished.id)}>
            {t("jobs.view")}
          </Button>
        </div>
      )}
      {!running && inspection?.ok && (
        <div className={`mt-4 rounded-lg border p-4 ${frameTone[inspection.value.verdict.state]}`}>
          <Verdict inspection={inspection.value} label={label} onErase={onErase} onUnlock={onUnlock} />
        </div>
      )}
    </article>
  );
}

function Verdict({
  inspection,
  label,
  onErase,
  onUnlock,
}: {
  inspection: InspectResult;
  label: (code: string) => string;
  onErase: (request: EraseRequest) => void;
  onUnlock: (inspection: InspectResult) => void;
}) {
  const { t } = useTranslation();
  const verdict = inspection.verdict;
  const [selected, setSelected] = useState(0);

  switch (verdict.state) {
    case "PROTECTED":
      return (
        <Reasons
          icon={<ShieldCheck size={16} className="text-ash" />}
          hint={t("verdict.protectedHint")}
          items={verdict.reasons.map((code) => `${t(`protect.${code}`)} (${code})`)}
        />
      );
    case "BLOCKED":
      return (
        <>
          <Reasons
            icon={<Lock size={16} className="text-blocked" />}
            hint={t("verdict.blockedHint")}
            items={verdict.reasons.map(
              (code) => `${t(`block.${code}`)} (${code}) — ${t(`guidance.${code}`)}`,
            )}
          />
          {/* B6: attempts exhausted, only a power cycle helps. */}
          {verdict.reasons.includes("B2") && !verdict.reasons.includes("B6") && (
            <div className="mt-4 flex justify-end">
              <Button className="flex items-center gap-2" onClick={() => onUnlock(inspection)}>
                <KeyRound size={14} />
                {t("actions.unlock")}
              </Button>
            </div>
          )}
        </>
      );
    case "SAY_NO": {
      const forcible = verdict.forcible;
      return (
        <>
          <Reasons
            icon={<Ban size={16} className="text-sayno" />}
            hint={t("verdict.sayNoHint")}
            items={verdict.reasons.map(label)}
          />
          {forcible && (
            <div className="mt-4 flex items-center gap-4">
              <p className="text-xs text-ash">{t("verdict.forcible")}</p>
              <Button
                variant="danger"
                className="ml-auto shrink-0"
                onClick={() => onErase({ inspection, choice: forcible, forced: true })}
              >
                {t("actions.force")}
              </Button>
            </div>
          )}
        </>
      );
    }
    case "READY": {
      const choices = [verdict.recommended, ...verdict.alternatives];
      const choice = choices[selected] ?? verdict.recommended;
      const destroy = verdict.disposition.kind === "DestroyRecommended" ? verdict.disposition.reasons : [];
      return (
        <>
          <p className="text-xs font-bold tracking-wide text-ash uppercase">{t("verdict.recommendation")}</p>
          <fieldset className="mt-2 space-y-1.5">
            {choices.map((option, index) => (
              <label key={option.method} className="flex cursor-pointer items-center gap-2.5 text-sm">
                <input
                  type="radio"
                  name={`method-${inspection.verdict_id}`}
                  checked={index === selected}
                  onChange={() => setSelected(index)}
                  className="accent-brand"
                />
                <Badge tone={option.category === "Purge" ? "ready" : "neutral"}>{option.category}</Badge>
                <span className={index === 0 ? "font-bold" : "text-ash"}>{label(option.method)}</span>
              </label>
            ))}
          </fieldset>
          {destroy.length > 0 && (
            <div className="mt-3 flex gap-2 rounded-md border border-sayno/40 bg-sayno/10 p-2.5 text-sm text-sayno">
              <TriangleAlert size={16} className="mt-0.5 shrink-0" />
              <div>
                {t("verdict.destroyRecommended")}
                <ul className="mt-1 text-xs">
                  {destroy.map((code) => (
                    <li key={code}>{label(code)}</li>
                  ))}
                </ul>
              </div>
            </div>
          )}
          <p className="mt-3 text-sm text-ash">
            {t("verdict.reservesLine", {
              list: choice.reserves.length === 0 ? t("verdict.none") : choice.reserves.map(label).join(" · "),
            })}
          </p>
          <div className="mt-4 flex justify-end">
            <Button variant="primary" onClick={() => onErase({ inspection, choice, forced: false })}>
              {t("actions.erase", { category: choice.category })}
            </Button>
          </div>
        </>
      );
    }
  }
}

function Reasons({ icon, hint, items }: { icon: React.ReactNode; hint: string; items: string[] }) {
  return (
    <div className="flex gap-3">
      <div className="mt-0.5">{icon}</div>
      <div>
        <p className="text-sm">{hint}</p>
        <ul className="mt-1.5 list-disc pl-4 text-sm text-ash">
          {items.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </div>
    </div>
  );
}
