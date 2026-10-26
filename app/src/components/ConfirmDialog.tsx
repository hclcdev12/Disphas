import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { CheckCircle2, Circle, TriangleAlert } from "lucide-react";
import type { BatchArgs } from "../bindings/BatchArgs";
import type { Disposition } from "../bindings/Disposition";
import { requiredAcknowledgements } from "../lib/acknowledgements";
import type { HelperError } from "../lib/api";
import { newBatchId } from "../lib/batch";
import type { EraseRequest } from "./DrivesScreen";
import { Badge, Button } from "./ui";

const REUSE: Disposition = { kind: "Reuse" };

/** Shortest justification of a forced erasure, as the helper requires. */
const MIN_JUSTIFICATION = 20;

/** Largest batch label, as in the attestation. */
const MAX_LABEL = 128;

/**
 * Warning before erasing one drive or a selection (docs/cadre_projet.md §5.4):
 * each drive, its method and its warnings, confirmed with one click. Strict
 * mode adds the serial number read on each drive (scanned in full, or its last
 * four characters typed). Forcing a refused drive, always alone, adds a
 * written justification, signed into a NON CONFORME certificate. With several
 * drives and no open batch, the dialog offers to file them in a new batch.
 * The helper checks everything again.
 */
export function ConfirmDialog({
  requests,
  labels,
  signer,
  strict,
  batch,
  freeSlots,
  onCancel,
  onConfirm,
}: {
  requests: EraseRequest[];
  labels: Record<string, string>;
  /** Operator and organization signed into the certificate (Settings). */
  signer: { operator: string | null; organization: string | null };
  /** Strict mode: the serial number read on each drive is required. */
  strict: boolean;
  /** The batch open now, if any. */
  batch: BatchArgs | null;
  /** Erasures the offer still allows at once; `null`: no limit known. */
  freeSlots: number | null;
  onCancel: () => void;
  /** Sends the erasures; returns the refusals by serial number. */
  onConfirm: (input: {
    confirmations: Record<string, string | null>;
    justification: string | null;
    newBatch: BatchArgs | null;
  }) => Promise<Record<string, HelperError>>;
}) {
  const { t } = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [scan, setScan] = useState("");
  const [scanned, setScanned] = useState<Record<string, string>>({});
  const [scanMiss, setScanMiss] = useState(false);
  const [justification, setJustification] = useState("");
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<Record<string, HelperError>>({});
  const [sent, setSent] = useState(false);
  const many = requests.length > 1;
  const [grouped, setGrouped] = useState(many && batch === null);
  const [label, setLabel] = useState("");

  const forced = requests.length === 1 && requests[0].forced;
  const serials = requests.map((request) => request.inspection.device.identity.serial);
  // One drive: what is typed confirms it as it is typed. Several: each scan ticks its drive.
  const typedOne = scan.trim();
  const oneOk = !many && (typedOne === serials[0] || (typedOne.length === 4 && typedOne === serials[0].slice(-4)));
  const allScanned = !strict || (many ? serials.every((serial) => scanned[serial] !== undefined) : oneOk);
  const labelTooLong = [...label].length > MAX_LABEL;
  const ready =
    allScanned &&
    (!forced || justification.trim().length >= MIN_JUSTIFICATION) &&
    !(grouped && labelTooLong) &&
    !busy &&
    !sent;
  const name = (code: string) =>
    code === "NS_ALL" ? t("scope.NS_ALL") : labels[code] ? `${labels[code]} (${code})` : code;

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

  /** A scanner types the whole serial number then Enter; by hand, the last four characters do. */
  const take = () => {
    const typed = scan.trim();
    if (typed === "") return;
    const waiting = serials.filter((serial) => scanned[serial] === undefined);
    const exact = waiting.filter((serial) => serial === typed);
    const tail = waiting.filter((serial) => typed.length === 4 && serial.slice(-4) === typed);
    const match = exact.length === 1 ? exact[0] : tail.length === 1 ? tail[0] : null;
    if (match === null) {
      setScanMiss(true);
      return;
    }
    setScanned((current) => ({ ...current, [match]: typed }));
    setScan("");
    setScanMiss(false);
  };

  const submit = async () => {
    if (!ready) return;
    setBusy(true);
    const trimmed = label.trim();
    const answer = await onConfirm({
      confirmations: Object.fromEntries(
        serials.map((serial) => [serial, !strict ? null : many ? (scanned[serial] ?? null) : typedOne]),
      ),
      justification: forced ? justification : null,
      newBatch: grouped && batch === null ? { id: newBatchId(), label: trimmed === "" ? null : trimmed } : null,
    });
    setRefused(answer);
    setSent(true);
    setBusy(false);
  };

  const over = freeSlots !== null && requests.length > freeSlots;

  return (
    <dialog
      ref={dialog}
      onCancel={onCancel}
      className="m-auto w-[40rem] max-w-[92vw] rounded-lg border border-ctrl bg-card p-6 text-soft backdrop:bg-black/70"
    >
      <h2 className="text-xl font-bold">
        {forced
          ? t("confirm.forceTitle")
          : many
            ? t("confirm.titleMany", { count: requests.length })
            : t("confirm.titleDrive", { model: requests[0].inspection.device.identity.model })}
      </h2>
      <p className="mt-4 flex items-start gap-2.5 rounded-md border border-sayno px-3 py-2.5">
        <TriangleAlert size={18} className="mt-0.5 shrink-0 text-sayno" aria-hidden="true" />
        {t("confirm.irreversible", { count: requests.length })}
      </p>

      <ul className="mt-4 flex max-h-[45vh] flex-col gap-2 overflow-y-auto">
        {requests.map(({ inspection, choice }) => {
          const serial = inspection.device.identity.serial;
          const verdict = inspection.verdict;
          const disposition = verdict.state === "READY" ? verdict.disposition : REUSE;
          const warnings = requiredAcknowledgements(choice, disposition);
          const error = refused[serial];
          return (
            <li key={serial} className="rounded-md border border-ctrl px-3 py-2.5">
              <div className="flex items-start gap-2.5">
                {strict && !sent && (
                  <span className="mt-1 shrink-0" aria-hidden="true">
                    {(many ? scanned[serial] !== undefined : oneOk) ? (
                      <CheckCircle2 size={16} className="text-ready" />
                    ) : (
                      <Circle size={16} className="text-ash" />
                    )}
                  </span>
                )}
                {sent && (
                  <span className="mt-1 shrink-0" aria-hidden="true">
                    {error ? (
                      <TriangleAlert size={16} className="text-sayno" />
                    ) : (
                      <CheckCircle2 size={16} className="text-ready" />
                    )}
                  </span>
                )}
                <div className="min-w-0 flex-1">
                  <p>
                    <span className="font-bold">{inspection.device.identity.model}</span>{" "}
                    <span className="font-mono text-sm text-ash">s/n {serial}</span>
                  </p>
                  <p className="mt-1 text-sm">
                    <Badge tone={choice.category === "Purge" ? "ready" : "neutral"}>{choice.category}</Badge>{" "}
                    {labels[choice.method] ?? choice.method}
                  </p>
                  {warnings.length > 0 && <p className="mt-1 text-sm text-ash">{warnings.map(name).join(" · ")}</p>}
                  {error && (
                    <p className="mt-1 text-sm text-sayno">
                      {t("confirm.refused")} {t(`errors.${error.code}`, { defaultValue: error.code })}
                      {error.detail && <span className="block font-mono text-xs text-ash">{error.detail}</span>}
                    </p>
                  )}
                </div>
              </div>
            </li>
          );
        })}
      </ul>

      <p className="mt-3 text-sm text-ash">
        {t("confirm.signedAs", {
          operator: signer.operator ?? t("confirm.notSet"),
          organization: signer.organization ?? t("confirm.notSet"),
        })}
      </p>
      {over && !sent && (
        <p className="mt-2 text-sm text-blocked">{t("confirm.overLimit", { count: freeSlots ?? 0 })}</p>
      )}

      {many && batch === null && !sent && (
        <div className="mt-4">
          <label className="flex cursor-pointer items-start gap-3">
            <input
              type="checkbox"
              className="mt-1.5 size-4 accent-soft"
              checked={grouped}
              onChange={(event) => setGrouped(event.target.checked)}
            />
            <span>
              <span className="font-bold">{t("confirm.groupInBatch")}</span>
              <span className="block text-sm text-ash">{t("confirm.groupInBatchHint")}</span>
            </span>
          </label>
          {grouped && (
            <input
              className="mt-2 block min-h-11 w-full rounded border border-ctrl bg-night px-3"
              placeholder={t("batch.labelPlaceholder")}
              value={label}
              onChange={(event) => setLabel(event.target.value)}
            />
          )}
          {grouped && labelTooLong && <span className="text-sm text-sayno">{t("batch.tooLong")}</span>}
        </div>
      )}
      {many && batch !== null && !sent && (
        <p className="mt-2 text-sm text-ash">{t("confirm.inBatch", { batch: batch.label ?? batch.id })}</p>
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

      {strict && !sent && (
        <label className="mt-4 block font-bold">
          {many ? t("confirm.scanEach") : t("confirm.scan")}
          <input
            className="mt-1.5 block min-h-11 w-full rounded border border-ctrl bg-night px-3 font-mono font-normal"
            autoComplete="off"
            spellCheck={false}
            autoFocus
            placeholder={t("confirm.scanPlaceholder")}
            value={scan}
            onChange={(event) => {
              setScan(event.target.value);
              setScanMiss(false);
            }}
            onKeyDown={(event) => {
              if (event.key !== "Enter") return;
              if (allScanned) void submit();
              else if (many) take();
              else setScanMiss(true);
            }}
          />
          {(scanMiss || (!many && typedOne.length >= 4 && !oneOk && !serials[0].startsWith(typedOne))) && <span className="text-sm font-normal text-sayno">{t("confirm.scanMismatch")}</span>}
        </label>
      )}

      <div className="mt-6 flex justify-end gap-2">
        {sent ? (
          <Button variant="primary" onClick={onCancel}>
            {t("actions.close")}
          </Button>
        ) : (
          <>
            <Button onClick={onCancel}>{t("actions.cancel")}</Button>
            <Button variant="danger" disabled={!ready} onClick={() => void submit()}>
              {forced
                ? t("confirm.submitForce")
                : many
                  ? t("confirm.submitMany", { count: requests.length })
                  : t("confirm.submit")}
            </Button>
          </>
        )}
      </div>
    </dialog>
  );
}
