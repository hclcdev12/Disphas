import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Ban, HardDrive, Lock, ShieldCheck, TriangleAlert } from "lucide-react";
import type { DeviceSummary } from "../bindings/DeviceSummary";
import type { InspectResult } from "../bindings/InspectResult";
import type { MethodChoice } from "../bindings/MethodChoice";
import type { Outcome } from "../lib/api";
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
}: {
  summary: DeviceSummary;
  inspection: Outcome<InspectResult> | undefined;
  labels: Record<string, string>;
  onErase: (request: EraseRequest) => void;
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
      {inspection?.ok && (
        <div className={`mt-4 rounded-lg border p-4 ${frameTone[inspection.value.verdict.state]}`}>
          <Verdict inspection={inspection.value} label={label} onErase={onErase} />
        </div>
      )}
    </article>
  );
}

function Verdict({
  inspection,
  label,
  onErase,
}: {
  inspection: InspectResult;
  label: (code: string) => string;
  onErase: (request: EraseRequest) => void;
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
        <Reasons
          icon={<Lock size={16} className="text-blocked" />}
          hint={t("verdict.blockedHint")}
          items={verdict.reasons.map(
            (code) => `${t(`block.${code}`)} (${code}) — ${t(`guidance.${code}`)}`,
          )}
        />
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
