import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { DeviceRef } from "../bindings/DeviceRef";
import type { PasswordKind } from "../bindings/PasswordKind";
import { api, type HelperError } from "../lib/api";
import { Button, Callout, Dialog, fieldClass } from "./ui";

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
  const [kind, setKind] = useState<PasswordKind>("User");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<HelperError | null>(null);

  const bytes = new TextEncoder().encode(password).length;
  const ready = password.length > 0 && bytes <= MAX_BYTES && !busy;

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
    <Dialog
      title={t("unlock.title")}
      context={
        <>
          {device.identity.model} · s/n {device.identity.serial}
        </>
      }
      onCancel={onCancel}
      footer={
        <>
          <Button onClick={onCancel}>{t("actions.cancel")}</Button>
          <Button variant="primary" disabled={!ready} onClick={() => void submit()}>
            {busy ? t("unlock.busy") : t("unlock.submit")}
          </Button>
        </>
      }
    >
      <p className="text-sm text-ash">{t("unlock.explain")}</p>
      <Callout tone="blocked">{t("unlock.attempts")}</Callout>

      <fieldset className="flex flex-col gap-0.5">
        <legend className="mb-1.5 text-[15px] font-bold text-ash">{t("unlock.kind")}</legend>
        {(["User", "Master"] as const).map((option) => (
          <label
            key={option}
            className={`flex min-h-11 cursor-pointer items-center gap-3.5 rounded-md border px-3 ${
              kind === option ? "border-ctrl bg-night" : "border-transparent"
            }`}
          >
            <input
              type="radio"
              name="password-kind"
              className="ctl-check"
              checked={kind === option}
              onChange={() => setKind(option)}
            />
            <span className={kind === option ? "font-bold" : undefined}>{t(`unlock.${option}`)}</span>
          </label>
        ))}
        {kind === "Master" && <p className="mt-1 px-3 text-sm text-ash">{t("unlock.masterHint")}</p>}
      </fieldset>

      <label className="block text-sm font-bold">
        {t("unlock.password")}
        <input
          type="password"
          className={`${fieldClass} font-mono`}
          autoComplete="off"
          spellCheck={false}
          autoFocus
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && ready) void submit();
          }}
        />
        {bytes > MAX_BYTES && <span className="text-sm font-normal text-sayno">{t("unlock.tooLong")}</span>}
      </label>

      {error && (
        <p role="alert" className="text-sm text-sayno">
          {t(`errors.${error.code}`, { defaultValue: error.code })}
          {error.detail && <span className="block font-mono text-xs text-ash">{error.detail}</span>}
        </p>
      )}
    </Dialog>
  );
}
