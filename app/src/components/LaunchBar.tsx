import { useTranslation } from "react-i18next";
import { TriangleAlert, X } from "lucide-react";
import { requiredAcknowledgements } from "../lib/acknowledgements";
import type { LaunchError, Pending } from "../lib/launch";
import { Button, Mono } from "./ui";

/**
 * Erasures about to start, each with its countdown and a Cancel button
 * (Escape cancels the last one), and the launches the helper refused.
 * Replaces the confirmation dialog of an ordinary erasure
 * (docs/cadre_projet.md §5.4).
 */
export function LaunchBar({
  pending,
  now,
  errors,
  labels,
  onCancel,
  onDismiss,
}: {
  pending: Pending[];
  now: number;
  errors: LaunchError[];
  labels: Record<string, string>;
  onCancel: (serial: string) => void;
  onDismiss: (serial: string) => void;
}) {
  const { t } = useTranslation();
  if (pending.length === 0 && errors.length === 0) return null;
  return (
    <div className="fixed inset-x-4 bottom-4 z-10 mx-auto flex max-w-xl flex-col gap-2" aria-live="assertive">
      {pending.map(({ serial, erase, deadline }) => {
        const seconds = Math.max(0, Math.ceil((deadline - now) / 1000));
        const verdict = erase.inspection.verdict;
        const warnings =
          verdict.state === "READY" ? requiredAcknowledgements(erase.choice, verdict.disposition) : [];
        return (
          <div key={serial} className="flex items-center gap-4 rounded-lg border border-ctrl bg-card p-4 shadow-lg">
            <div className="min-w-0 flex-1">
              <p className="font-bold">
                {t("launch.starting", { model: erase.inspection.device.identity.model, count: seconds })}
              </p>
              <p className="text-sm text-ash">
                {erase.choice.category} · {labels[erase.choice.method] ?? erase.choice.method} · s/n{" "}
                <Mono>{serial}</Mono>
              </p>
              {warnings.length > 0 && (
                <p className="mt-1 flex items-start gap-1.5 text-sm text-ash">
                  <TriangleAlert size={14} className="mt-1 shrink-0" aria-hidden="true" />
                  {warnings
                    .map((code) => (code === "NS_ALL" ? t("scope.NS_ALL") : `${labels[code] ?? code} (${code})`))
                    .join(" · ")}
                </p>
              )}
            </div>
            <Button variant="primary" onClick={() => onCancel(serial)}>
              {t("launch.cancel")}
            </Button>
          </div>
        );
      })}
      {errors.map(({ serial, model, error }) => (
        <div key={serial} className="flex items-start gap-3 rounded-lg border border-sayno bg-card p-4 shadow-lg">
          <TriangleAlert size={16} className="mt-1 shrink-0 text-sayno" aria-hidden="true" />
          <div className="min-w-0 flex-1 text-sm">
            <p className="font-bold">{t("launch.refused", { model })}</p>
            <p className="text-ash">{t(`errors.${error.code}`, { defaultValue: error.code })}</p>
            {error.detail && <p className="font-mono text-xs text-ash">{error.detail}</p>}
          </div>
          <button
            type="button"
            aria-label={t("actions.dismiss")}
            title={t("actions.dismiss")}
            onClick={() => onDismiss(serial)}
            className="inline-flex size-11 shrink-0 items-center justify-center rounded text-soft hover:bg-night"
          >
            <X size={16} aria-hidden="true" />
          </button>
        </div>
      ))}
    </div>
  );
}
