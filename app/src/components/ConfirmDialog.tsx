import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { TriangleAlert } from "lucide-react";
import type { Disposition } from "../bindings/Disposition";
import { requiredAcknowledgements } from "../lib/acknowledgements";
import type { HelperError } from "../lib/api";
import type { EraseRequest } from "./DrivesScreen";
import { Badge, Button } from "./ui";

const REUSE: Disposition = { kind: "Reuse" };

/** Shortest justification of a forced erasure, as the helper requires. */
const MIN_JUSTIFICATION = 20;

/**
 * Warning before every erasure (docs/cadre_projet.md §5.4): the drive, the
 * method and its warnings, confirmed with one click. Strict mode adds the
 * serial number read on the drive (scanned in full, or its last four
 * characters typed); forcing a refused drive adds a written justification,
 * signed into a NON CONFORME certificate. The helper checks everything again.
 */
export function ConfirmDialog({
  request,
  labels,
  signer,
  strict,
  onCancel,
  onConfirm,
}: {
  request: EraseRequest;
  labels: Record<string, string>;
  /** Operator and organization signed into the certificate (Settings). */
  signer: { operator: string | null; organization: string | null };
  /** Strict mode: the serial number read on the drive is required. */
  strict: boolean;
  onCancel: () => void;
  onConfirm: (input: { confirmation: string | null; justification: string | null }) => Promise<HelperError | null>;
}) {
  const { t } = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [confirmation, setConfirmation] = useState("");
  const [justification, setJustification] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<HelperError | null>(null);

  const { inspection, choice, forced } = request;
  const serial = inspection.device.identity.serial;
  const verdict = inspection.verdict;
  const disposition = verdict.state === "READY" ? verdict.disposition : REUSE;
  const warnings = requiredAcknowledgements(choice, disposition);
  const label = (code: string) => (labels[code] ? `${labels[code]} (${code})` : code);
  const warningText = (code: string) => (code === "NS_ALL" ? t("scope.NS_ALL") : label(code));

  // A scanner types the whole serial number then Enter; by hand, the last four characters do.
  const typed = confirmation.trim();
  const serialOk = !strict || typed === serial || (typed.length === 4 && typed === serial.slice(-4));
  const ready = serialOk && (!forced || justification.trim().length >= MIN_JUSTIFICATION) && !busy;

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

  const submit = async () => {
    if (!ready) return;
    setBusy(true);
    setError(null);
    const answer = await onConfirm({
      confirmation: strict ? typed : null,
      justification: forced ? justification : null,
    });
    setError(answer);
    setBusy(false);
  };

  return (
    <dialog
      ref={dialog}
      onCancel={onCancel}
      className="m-auto w-[36rem] max-w-[92vw] rounded-lg border border-ctrl bg-card p-6 text-soft backdrop:bg-black/70"
    >
      <h2 className="text-xl font-bold">
        {forced ? t("confirm.forceTitle") : t("confirm.titleDrive", { model: inspection.device.identity.model })}
      </h2>
      <p className="mt-1 text-ash">
        {inspection.device.identity.model} · s/n <span className="font-mono">{serial}</span>
      </p>
      <p className="mt-4 flex items-start gap-2.5 rounded-md border border-sayno px-3 py-2.5">
        <TriangleAlert size={18} className="mt-0.5 shrink-0 text-sayno" aria-hidden="true" />
        {t("confirm.irreversible")}
      </p>

      <p className="mt-4">
        <Badge tone={choice.category === "Purge" ? "ready" : "neutral"}>{choice.category}</Badge>{" "}
        {labels[choice.method] ?? choice.method}
      </p>
      <p className="mt-1 text-sm text-ash">
        {t("confirm.signedAs", {
          operator: signer.operator ?? t("confirm.notSet"),
          organization: signer.organization ?? t("confirm.notSet"),
        })}
      </p>

      {warnings.length > 0 && (
        <ul className="mt-4 flex flex-col gap-2">
          {warnings.map((warning) => (
            <li key={warning} className="flex items-start gap-2.5 rounded-md border border-ctrl px-3 py-2 text-sm">
              <TriangleAlert size={16} className="mt-0.5 shrink-0" aria-hidden="true" />
              {warningText(warning)}
            </li>
          ))}
        </ul>
      )}

      {forced && (
        <label className="mt-4 block font-bold">
          {t("confirm.justification")}
          <textarea
            className="mt-1.5 block h-24 w-full rounded border border-ctrl bg-night p-2 font-normal"
            maxLength={500}
            value={justification}
            onChange={(event) => setJustification(event.target.value)}
            autoFocus={!strict}
          />
          <span className="text-sm font-normal text-ash">
            {t("confirm.justificationCount", { count: justification.trim().length })}
          </span>
          <span className="block text-sm font-normal text-blocked">{t("confirm.justificationPrivacy")}</span>
        </label>
      )}

      {strict && (
        <label className="mt-4 block font-bold">
          {t("confirm.scan")}
          <input
            className="mt-1.5 block min-h-11 w-full rounded border border-ctrl bg-night px-3 font-mono font-normal"
            autoComplete="off"
            spellCheck={false}
            autoFocus
            placeholder={t("confirm.scanPlaceholder")}
            value={confirmation}
            onChange={(event) => setConfirmation(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void submit();
            }}
          />
          {typed.length > 0 && !serialOk && (
            <span className="text-sm font-normal text-sayno">{t("confirm.scanMismatch")}</span>
          )}
        </label>
      )}

      {error && (
        <p className="mt-4 text-sm text-sayno">
          {t("confirm.refused")} {t(`errors.${error.code}`, { defaultValue: error.code })}
          {error.detail && <span className="block font-mono text-xs text-ash">{error.detail}</span>}
        </p>
      )}

      <div className="mt-6 flex justify-end gap-2">
        <Button onClick={onCancel}>{t("actions.cancel")}</Button>
        <Button variant="danger" disabled={!ready} onClick={() => void submit()}>
          {forced ? t("confirm.submitForce") : t("confirm.submit")}
        </Button>
      </div>
    </dialog>
  );
}
