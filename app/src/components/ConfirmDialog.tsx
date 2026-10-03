import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { TriangleAlert } from "lucide-react";
import type { Acknowledgement } from "../bindings/Acknowledgement";
import type { Disposition } from "../bindings/Disposition";
import { requiredAcknowledgements } from "../lib/acknowledgements";
import type { HelperError } from "../lib/api";
import type { EraseRequest } from "./DriveCard";
import { Badge, Button, Mono } from "./ui";

const REUSE: Disposition = { kind: "Reuse" };

/**
 * Safety guard before an erasure (cadre_projet.md §5.4): the operator types the
 * last four characters of the serial number, acknowledges every warning and,
 * when forcing, writes a justification. The helper checks all of it again.
 */
export function ConfirmDialog({
  request,
  labels,
  onCancel,
  onConfirm,
}: {
  request: EraseRequest;
  labels: Record<string, string>;
  onCancel: () => void;
  onConfirm: (input: {
    confirmation: string;
    acknowledged: Acknowledgement[];
    justification: string | null;
  }) => Promise<HelperError | null>;
}) {
  const { t } = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [confirmation, setConfirmation] = useState("");
  const [checked, setChecked] = useState<Set<Acknowledgement>>(new Set());
  const [justification, setJustification] = useState("");
  const [busy, setBusy] = useState(false);
  const [answer, setAnswer] = useState<HelperError | "sent" | null>(null);

  const { inspection, choice, forced } = request;
  const verdict = inspection.verdict;
  const disposition = verdict.state === "READY" ? verdict.disposition : REUSE;
  const warnings = requiredAcknowledgements(choice, disposition);
  const label = (code: string) => (labels[code] ? `${labels[code]} (${code})` : code);
  const warningText = (code: Acknowledgement) => (code === "NS_ALL" ? t("scope.NS_ALL") : label(code));

  // The serial number is never shown in full here: the operator reads it on the drive.
  const ready =
    confirmation.length === 4 &&
    warnings.every((warning) => checked.has(warning)) &&
    (!forced || justification.trim().length >= 20) &&
    !busy;

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

  const toggle = (warning: Acknowledgement) =>
    setChecked((current) => {
      const next = new Set(current);
      if (next.has(warning)) next.delete(warning);
      else next.add(warning);
      return next;
    });

  const submit = async () => {
    setBusy(true);
    const error = await onConfirm({
      confirmation,
      acknowledged: [...checked],
      justification: forced ? justification : null,
    });
    setAnswer(error ?? "sent");
    setBusy(false);
  };

  return (
    <dialog
      ref={dialog}
      onCancel={onCancel}
      className="m-auto w-[34rem] max-w-[92vw] rounded-lg border border-edge bg-card p-6 text-soft backdrop:bg-black/70"
    >
      <h2 className="text-lg font-bold">{forced ? t("confirm.forceTitle") : t("confirm.title")}</h2>
      <p className="mt-1 text-sm text-ash">
        {inspection.device.identity.model} · {inspection.device.node_hint && <Mono>{inspection.device.node_hint}</Mono>}
      </p>
      <div className="mt-4 flex gap-2 rounded-md border border-sayno/40 bg-sayno/10 p-3 text-sm text-sayno">
        <TriangleAlert size={16} className="mt-0.5 shrink-0" />
        {t("confirm.irreversible")}
      </div>

      <p className="mt-4 text-sm">
        {t("confirm.method")} <Badge tone={choice.category === "Purge" ? "ready" : "neutral"}>{choice.category}</Badge>{" "}
        {label(choice.method)}
      </p>

      {warnings.length > 0 && (
        <fieldset className="mt-4">
          <legend className="text-sm font-bold">{t("confirm.acknowledge")}</legend>
          {warnings.map((warning) => (
            <label key={warning} className="mt-1.5 flex cursor-pointer items-start gap-2 text-sm text-ash">
              <input
                type="checkbox"
                className="mt-1 accent-brand"
                checked={checked.has(warning)}
                onChange={() => toggle(warning)}
              />
              {warningText(warning)}
            </label>
          ))}
        </fieldset>
      )}

      {forced && (
        <label className="mt-4 block text-sm font-bold">
          {t("confirm.justification")}
          <textarea
            className="mt-1.5 block h-20 w-full rounded-md border border-edge bg-night p-2 text-sm font-normal"
            value={justification}
            onChange={(event) => setJustification(event.target.value)}
          />
          <span className="text-xs font-normal text-ash">
            {t("confirm.justificationCount", { count: justification.trim().length })}
          </span>
        </label>
      )}

      <label className="mt-4 block text-sm font-bold">
        {t("confirm.serial")}
        <input
          className="mt-1.5 block w-32 rounded-md border border-edge bg-night px-2 py-1.5 text-center font-mono tracking-[0.4em]"
          maxLength={4}
          autoComplete="off"
          spellCheck={false}
          value={confirmation}
          onChange={(event) => setConfirmation(event.target.value)}
        />
      </label>

      {answer === "sent" && <p className="mt-4 text-sm text-ready">{t("confirm.sent")}</p>}
      {answer && answer !== "sent" && (
        <p className="mt-4 text-sm text-sayno">
          {t("confirm.refused")} {t(`errors.${answer.code}`, { defaultValue: answer.code })}
          {answer.detail && <span className="block font-mono text-xs text-ash">{answer.detail}</span>}
        </p>
      )}

      <div className="mt-6 flex justify-end gap-2">
        <Button onClick={onCancel}>{t("actions.cancel")}</Button>
        <Button variant={forced ? "danger" : "primary"} disabled={!ready} onClick={() => void submit()}>
          {forced ? t("confirm.submitForce") : t("confirm.submit")}
        </Button>
      </div>
    </dialog>
  );
}
