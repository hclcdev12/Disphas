import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { KeyRound, TriangleAlert } from "lucide-react";
import type { DeviceRef } from "../bindings/DeviceRef";
import type { PasswordKind } from "../bindings/PasswordKind";
import { api, type HelperError } from "../lib/api";
import { Button, Mono } from "./ui";

/** ATA passwords are at most 32 bytes (docs/protocole_ipc.md, `Unlock`). */
const MAX_BYTES = 32;

/**
 * Unlocks a drive locked by an ATA password (B2): the operator gives the
 * password set by the previous owner; the helper unlocks the drive and removes
 * the password, so it can be inspected and erased. The password only crosses
 * to the helper and is cleared from this dialog right after.
 */
export function UnlockDialog({
  device,
  onCancel,
  onUnlocked,
}: {
  device: DeviceRef;
  onCancel: () => void;
  onUnlocked: () => void;
}) {
  const { t } = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [kind, setKind] = useState<PasswordKind>("User");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<HelperError | null>(null);

  const bytes = new TextEncoder().encode(password).length;
  const ready = password.length > 0 && bytes <= MAX_BYTES && !busy;

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

  const submit = async () => {
    setBusy(true);
    setError(null);
    const result = await api.unlock(device, kind, password);
    setPassword("");
    setBusy(false);
    if (result.ok) onUnlocked();
    else setError(result.error);
  };

  return (
    <dialog
      ref={dialog}
      onCancel={onCancel}
      className="m-auto w-[32rem] max-w-[92vw] rounded-lg border border-edge bg-card p-6 text-soft backdrop:bg-black/70"
    >
      <h2 className="flex items-center gap-2 text-lg font-bold">
        <KeyRound size={18} className="text-blocked" />
        {t("unlock.title")}
      </h2>
      <p className="mt-1 text-sm text-ash">
        {device.identity.model} · S/N <Mono>{device.identity.serial}</Mono>
      </p>
      <p className="mt-4 text-sm">{t("unlock.explain")}</p>
      <div className="mt-3 flex gap-2 rounded-md border border-blocked/40 bg-blocked/10 p-3 text-sm text-blocked">
        <TriangleAlert size={16} className="mt-0.5 shrink-0" />
        {t("unlock.attempts")}
      </div>

      <fieldset className="mt-4 flex gap-5 text-sm">
        <legend className="mb-1.5 font-bold">{t("unlock.kind")}</legend>
        {(["User", "Master"] as const).map((option) => (
          <label key={option} className="flex cursor-pointer items-center gap-2">
            <input
              type="radio"
              name="password-kind"
              className="ctl-check"
              checked={kind === option}
              onChange={() => setKind(option)}
            />
            {t(`unlock.${option}`)}
          </label>
        ))}
      </fieldset>
      {kind === "Master" && <p className="mt-1.5 text-xs text-ash">{t("unlock.masterHint")}</p>}

      <label className="mt-4 block text-sm font-bold">
        {t("unlock.password")}
        <input
          type="password"
          className="mt-1.5 block w-full rounded-md border border-edge bg-night px-2 py-1.5 font-mono"
          autoComplete="off"
          spellCheck={false}
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && ready) void submit();
          }}
        />
        {bytes > MAX_BYTES && <span className="text-xs font-normal text-sayno">{t("unlock.tooLong")}</span>}
      </label>

      {error && (
        <p className="mt-4 text-sm text-sayno">
          {t(`errors.${error.code}`, { defaultValue: error.code })}
          {error.detail && <span className="block font-mono text-xs text-ash">{error.detail}</span>}
        </p>
      )}

      <div className="mt-6 flex justify-end gap-2">
        <Button onClick={onCancel}>{t("actions.cancel")}</Button>
        <Button variant="primary" disabled={!ready} onClick={() => void submit()}>
          {busy ? t("unlock.busy") : t("unlock.submit")}
        </Button>
      </div>
    </dialog>
  );
}
