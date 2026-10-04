import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Layers } from "lucide-react";
import type { BatchArgs } from "../bindings/BatchArgs";
import { newBatchId } from "../lib/batch";
import { Button, Mono } from "./ui";

/** Largest label, as in the attestation. */
const MAX_LABEL = 128;

/**
 * The batch erasures are filed in. Opening a batch gives it an identifier
 * and an optional label (a customer order number, say); every erasure
 * started while it is open carries it, signed.
 */
export function BatchBar({ batch, onChange }: { batch: BatchArgs | null; onChange: (batch: BatchArgs | null) => void }) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [label, setLabel] = useState("");
  const tooLong = [...label].length > MAX_LABEL;

  const open = () => {
    const trimmed = label.trim();
    onChange({ id: newBatchId(), label: trimmed === "" ? null : trimmed });
    setLabel("");
    setEditing(false);
  };

  return (
    <div className="mb-4 flex flex-wrap items-center gap-3 rounded-lg border border-edge bg-card px-4 py-3 text-sm">
      <Layers size={16} className="text-ash" />
      {batch ? (
        <>
          <span>
            {t("batch.current")} <Mono>{batch.id}</Mono>
            {batch.label && <span className="text-ash"> — {batch.label}</span>}
          </span>
          <Button className="ml-auto" onClick={() => onChange(null)}>
            {t("batch.close")}
          </Button>
        </>
      ) : editing ? (
        <>
          <input
            className="min-w-56 flex-1 rounded-md border border-edge bg-night px-2 py-1.5"
            placeholder={t("batch.labelPlaceholder")}
            value={label}
            onChange={(event) => setLabel(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !tooLong) open();
            }}
            autoFocus
          />
          <Button onClick={() => setEditing(false)}>{t("actions.cancel")}</Button>
          <Button variant="primary" disabled={tooLong} onClick={open}>
            {t("batch.open")}
          </Button>
          {tooLong && <span className="w-full text-xs text-sayno">{t("batch.tooLong")}</span>}
        </>
      ) : (
        <>
          <span className="text-ash">{t("batch.none")}</span>
          <Button className="ml-auto" onClick={() => setEditing(true)}>
            {t("batch.new")}
          </Button>
        </>
      )}
    </div>
  );
}
