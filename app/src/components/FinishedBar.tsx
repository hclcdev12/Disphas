import { useTranslation } from "react-i18next";
import { CheckCircle2, X } from "lucide-react";
import { Button } from "./ui";

/**
 * Summary of the erasures that ended since the operator last looked:
 * "3 erasures finished · 3 CONFORME", while the others go on.
 */
export function FinishedBar({
  conforme,
  other,
  running,
  onShow,
  onDismiss,
}: {
  conforme: number;
  other: number;
  running: number;
  onShow: () => void;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  const total = conforme + other;
  return (
    <div
      role="status"
      className="fixed right-4 bottom-4 z-10 flex max-w-md items-center gap-3 rounded-lg border border-ctrl bg-card p-4 shadow-lg"
    >
      <CheckCircle2 size={20} className={`shrink-0 ${other === 0 ? "text-ready" : "text-sayno"}`} aria-hidden="true" />
      <div className="min-w-0 flex-1 text-sm">
        <p className="font-bold">{t("finished.title", { count: total })}</p>
        <p className="text-ash">
          {t("finished.conforme", { count: conforme })}
          {other > 0 && <> · {t("finished.other", { count: other })}</>}
          {running > 0 && <> · {t("finished.running", { count: running })}</>}
        </p>
      </div>
      <Button onClick={onShow}>{t("finished.show")}</Button>
      <button
        type="button"
        aria-label={t("actions.dismiss")}
        title={t("actions.dismiss")}
        onClick={onDismiss}
        className="inline-flex size-11 shrink-0 items-center justify-center rounded text-soft hover:bg-night"
      >
        <X size={16} aria-hidden="true" />
      </button>
    </div>
  );
}
