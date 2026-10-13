import { useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { BadgeCheck, FileUp, ShieldX, TriangleAlert } from "lucide-react";
import { api, type VerificationReport } from "../lib/api";
import { capacity } from "../lib/format";
import { Badge, Button, Mono } from "./ui";

/** Largest file read (the helper side refuses more anyway). */
const MAX_FILE_BYTES = 32 * 1024 * 1024;

function toBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let index = 0; index < bytes.length; index += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(index, index + 0x8000));
  }
  return btoa(binary);
}

/**
 * Verifies a certificate received from anywhere: a PDF (its attached signed
 * attestation and journal), a `.cose` file or the QR text. Same checks as the
 * open-source `disphas-verify`; the content of a certificate that does not
 * verify is never shown.
 */
export function VerifyScreen({ labels }: { labels: Record<string, string> }) {
  const { t, i18n } = useTranslation();
  const input = useRef<HTMLInputElement>(null);
  const [qr, setQr] = useState("");
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState<string | null>(null);
  const [report, setReport] = useState<VerificationReport | { error: string } | null>(null);

  const run = async (bytes: Uint8Array, label: string) => {
    setBusy(true);
    setName(label);
    setReport(null);
    setReport(await api.verifyCertificate(toBase64(bytes)));
    setBusy(false);
  };

  const fromFile = async (file: File | undefined) => {
    if (!file) return;
    if (file.size > MAX_FILE_BYTES) {
      setName(file.name);
      setReport({ error: t("verify.reason.TooLarge") });
      return;
    }
    await run(new Uint8Array(await file.arrayBuffer()), file.name);
  };

  return (
    <section>
      <h1 className="mb-4 text-xl font-bold">{t("verify.title")}</h1>
      <div
        onDragOver={(event) => {
          event.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(event) => {
          event.preventDefault();
          setDragging(false);
          void fromFile(event.dataTransfer.files[0]);
        }}
        className={`flex flex-col items-center gap-3 rounded-lg border-2 border-dashed p-8 text-center ${
          dragging ? "border-brand bg-brand/10" : "border-edge bg-card"
        }`}
      >
        <FileUp size={28} className="text-ash" />
        <p className="text-sm">{t("verify.drop")}</p>
        <Button variant="primary" disabled={busy} onClick={() => input.current?.click()}>
          {t("verify.choose")}
        </Button>
        <input
          ref={input}
          type="file"
          accept=".pdf,.cose,.txt,application/pdf"
          className="hidden"
          onChange={(event) => {
            void fromFile(event.target.files?.[0]);
            event.target.value = "";
          }}
        />
      </div>

      <label className="mt-4 block text-sm font-bold">
        {t("verify.qr")}
        <textarea
          className="mt-1.5 block h-16 w-full rounded-md border border-edge bg-night p-2 font-mono text-xs font-normal"
          placeholder="DSV1:…"
          spellCheck={false}
          value={qr}
          onChange={(event) => setQr(event.target.value)}
        />
      </label>
      <div className="mt-2 flex justify-end">
        <Button
          disabled={busy || !qr.trim().startsWith("DSV1:")}
          onClick={() => void run(new TextEncoder().encode(qr.trim()), t("verify.qrSource"))}
        >
          {t("verify.verifyQr")}
        </Button>
      </div>

      {busy && <p className="mt-4 text-ash">{t("verify.busy")}</p>}
      {report && "error" in report && <p className="mt-4 text-sayno">{report.error}</p>}
      {report && !("error" in report) && (
        <Result report={report} name={name} labels={labels} lang={i18n.language} />
      )}
    </section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[11rem_1fr] gap-3 border-b border-edge/60 py-1.5 text-sm last:border-0">
      <dt className="text-ash">{label}</dt>
      <dd className="min-w-0 break-words">{children}</dd>
    </div>
  );
}

function Result({
  report,
  name,
  labels,
  lang,
}: {
  report: VerificationReport;
  name: string | null;
  labels: Record<string, string>;
  lang: string;
}) {
  const { t } = useTranslation();
  const label = (code: string) => (labels[code] ? `${labels[code]} (${code})` : code);
  const list = (codes: string[]) => (codes.length === 0 ? t("verdict.none") : codes.map(label).join(" · "));
  const yesNo = (value: boolean | null) => (value === null ? "—" : value ? t("verify.yes") : t("verify.no"));

  if (report.batch) {
    return <BatchResult report={report} batch={report.batch} name={name} />;
  }

  if (!report.authentic || !report.details) {
    return (
      <div className="mt-6 rounded-lg border border-sayno/40 bg-sayno/10 p-5">
        <div className="flex items-center gap-3">
          <ShieldX className="text-sayno" />
          <h2 className="text-lg font-bold text-sayno">{t("verify.rejected")}</h2>
        </div>
        {name && <p className="mt-1 text-sm text-ash">{name}</p>}
        <p className="mt-3 text-sm">{t(`verify.reason.${report.reason ?? "Unknown"}`)}</p>
      </div>
    );
  }

  const d = report.details;
  const tone = d.status === "CONFORME" ? "ready" : "sayno";
  const date = new Intl.DateTimeFormat(lang, { dateStyle: "long", timeStyle: "medium", timeZone: "UTC" });
  // A journal that is not the signed one fails the check, as in disphas-verify:
  // never a green header.
  const journalOk = report.journal !== "Mismatch";
  return (
    <div className={`mt-6 rounded-lg border p-5 ${journalOk ? "border-ready/40 bg-card" : "border-sayno/40 bg-sayno/10"}`}>
      <div className="flex flex-wrap items-center gap-3">
        {journalOk ? <BadgeCheck className="text-ready" /> : <ShieldX className="text-sayno" />}
        <h2 className={`text-lg font-bold ${journalOk ? "text-ready" : "text-sayno"}`}>
          {t(journalOk ? "verify.authentic" : "verify.authenticJournalMismatch")}
        </h2>
        <span className="text-sm text-ash">
          {name} · {t(`verify.source.${report.source}`)}
        </span>
      </div>

      {report.root === "Development" && (
        <Notice tone="blocked">{t("verify.developmentRoot")}</Notice>
      )}
      {report.journal === "Mismatch" && <Notice tone="sayno">{t("verify.journalMismatch")}</Notice>}
      {report.journal === "Absent" && <p className="mt-3 text-xs text-ash">{t("verify.journalAbsent")}</p>}
      {report.journal === "Matches" && <p className="mt-3 text-xs text-ash">{t("verify.journalMatches")}</p>}

      <div className="mt-4 flex flex-wrap items-center gap-2">
        <Badge tone={tone}>{t(`job.status.${d.status}`)}</Badge>
        {d.category !== "None" && <Badge tone={d.category === "Purge" ? "ready" : "neutral"}>{d.category}</Badge>}
        <span className="text-sm">{t(`job.statusHint.${d.status}`)}</span>
      </div>

      <dl className="mt-4">
        <Row label={t("verify.field.date")}>{date.format(new Date(d.issued_at * 1000))} UTC</Row>
        <Row label={t("verify.field.drive")}>
          {d.model} · {capacity(d.capacity_bytes, lang, "")} · {d.drive_type}
        </Row>
        <Row label="S/N">
          <Mono>{d.serial}</Mono>
          {d.hardware_id && (
            <>
              {" · "}
              <Mono>{d.hardware_id}</Mono>
            </>
          )}
          {" · "}
          {t("verify.field.firmware")} <Mono>{d.firmware}</Mono>
        </Row>
        <Row label={t("verify.field.method")}>
          {d.method ? label(d.method) : "—"}
          {d.zero_finish && <span className="block text-xs text-ash">{t("verify.zeroFinish")}</span>}
          {d.all_namespaces && <span className="block text-xs text-ash">{t("verify.allNamespaces")}</span>}
        </Row>
        <Row label={t("verify.field.verified")}>{yesNo(d.verified)}</Row>
        <Row label={t("verify.field.validation")}>
          {d.validation
            ? t("verify.validation", {
                method: t(`verify.validationMethod.${d.validation.method}`, { defaultValue: d.validation.method }),
                samples: d.validation.samples,
                result: yesNo(d.validation.passed),
              })
            : "—"}
        </Row>
        {d.disposition && (
          <Row label={t("verify.field.disposition")}>
            {t(`verify.disposition.${d.disposition}`)}
            {d.disposition_codes.length > 0 && <> — {list(d.disposition_codes)}</>}
          </Row>
        )}
        <Row label={t("verify.field.reserves")}>{list(d.reserves)}</Row>
        {d.refusals.length > 0 && <Row label={t("verify.field.refusals")}>{list(d.refusals)}</Row>}
        {d.justification && <Row label={t("verify.field.justification")}>« {d.justification} »</Row>}
        <Row label={t("verify.field.operator")}>
          {[d.operator, d.organization].filter(Boolean).join(", ") || "—"}
        </Row>
        {d.batch_id && (
          <Row label={t("verify.field.batch")}>
            <Mono>{d.batch_id}</Mono>
            {d.batch_label && <> — {d.batch_label}</>}
          </Row>
        )}
        <Row label={t("verify.field.license")}>
          {d.customer} · <Mono>{d.license_id}</Mono>
        </Row>
        <Row label={t("verify.field.tool")}>
          disphas {d.tool_version} · {t("verify.rules", { version: d.rules_version })}
        </Row>
        <Row label={t("verify.field.binary")}>
          <Mono>{d.binary_sha256}</Mono>
        </Row>
        <Row label={t("verify.field.id")}>
          <Mono>{d.id}</Mono>
        </Row>
      </dl>
    </div>
  );
}

function BatchResult({
  report,
  batch,
  name,
}: {
  report: VerificationReport;
  batch: NonNullable<VerificationReport["batch"]>;
  name: string | null;
}) {
  const { t } = useTranslation();
  const ok = report.authentic;
  return (
    <div className={`mt-6 rounded-lg border p-5 ${ok ? "border-ready/40 bg-card" : "border-sayno/40 bg-sayno/10"}`}>
      <div className="flex flex-wrap items-center gap-3">
        {ok ? <BadgeCheck className="text-ready" /> : <ShieldX className="text-sayno" />}
        <h2 className={`text-lg font-bold ${ok ? "text-ready" : "text-sayno"}`}>
          {ok ? t("verify.batchAuthentic") : t("verify.batchRejected")}
        </h2>
        {name && <span className="text-sm text-ash">{name}</span>}
      </div>
      {ok && report.root === "Development" && <Notice tone="blocked">{t("verify.developmentRoot")}</Notice>}
      <p className="mt-3 text-sm">
        {t("verify.field.batch")} <Mono>{batch.id ?? "—"}</Mono>
        {batch.label && <> — {batch.label}</>} · {t("verify.batchDrives", { count: batch.drives.length })}
      </p>
      <ul className="mt-3 divide-y divide-edge/60 rounded-md border border-edge text-sm">
        {batch.drives.map((drive) => (
          <li key={drive.report} className="flex flex-wrap items-center gap-3 px-3 py-2">
            {drive.accepted ? (
              <BadgeCheck size={16} className="text-ready" />
            ) : (
              <ShieldX size={16} className="text-sayno" />
            )}
            {drive.serial ? (
              <>
                <Mono>{drive.serial}</Mono>
                <span className="text-ash">{drive.model}</span>
              </>
            ) : (
              <Mono>{drive.report}</Mono>
            )}
            <span className="ml-auto flex items-center gap-2">
              {drive.status && (
                <Badge tone={drive.status === "CONFORME" ? "ready" : "sayno"}>{t(`job.status.${drive.status}`)}</Badge>
              )}
              {drive.category && drive.category !== "None" && <Badge tone="neutral">{drive.category}</Badge>}
            </span>
            {drive.reason && (
              <span className="w-full text-xs text-sayno">{t(`verify.reason.${drive.reason}`, { defaultValue: drive.reason })}</span>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

function Notice({ tone, children }: { tone: "blocked" | "sayno"; children: ReactNode }) {
  const classes = tone === "sayno" ? "border-sayno/40 bg-sayno/10 text-sayno" : "border-blocked/40 bg-blocked/10 text-blocked";
  return (
    <div className={`mt-3 flex gap-2 rounded-md border p-3 text-sm ${classes}`}>
      <TriangleAlert size={16} className="mt-0.5 shrink-0" />
      {children}
    </div>
  );
}
