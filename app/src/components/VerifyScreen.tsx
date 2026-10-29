import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { BadgeCheck, FileUp, ShieldX, TriangleAlert } from "lucide-react";
import { api, type VerificationReport } from "../lib/api";
import { capacity } from "../lib/format";
import { BackButton, Badge, Fact, Mono } from "./ui";

/** Largest file read (the helper side refuses more anyway). */
const MAX_FILE_BYTES = 32 * 1024 * 1024;

function toBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let index = 0; index < bytes.length; index += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(index, index + 0x8000));
  }
  return btoa(binary);
}

/** Text of the QR code printed on a certificate. */
const QR_PREFIX = "DSV1:";

/**
 * Verifies a certificate received from anywhere, in one zone: drop a PDF or a
 * `.cose` file, click to choose one, or paste or scan the QR text (Enter).
 * A file or QR text pasted anywhere on the tab is verified too. Same checks
 * as the open-source `disphas-verify`; the content of a certificate that
 * does not verify is never shown.
 */
export function VerifyScreen({ labels }: { labels: Record<string, string> }) {
  const { t, i18n } = useTranslation();
  const input = useRef<HTMLInputElement>(null);
  const [qr, setQr] = useState("");
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState<string | null>(null);
  const [report, setReport] = useState<VerificationReport | { error: string } | null>(null);
  const [view, setView] = useState<"list" | "detail">("list");

  // On a phone the result replaces the drop zone, with a way back.
  const show = (next: VerificationReport | { error: string }) => {
    setReport(next);
    if (window.matchMedia("(max-width: 760px)").matches) {
      setView("detail");
      window.scrollTo(0, 0);
    }
  };

  const run = async (bytes: Uint8Array, label: string) => {
    setBusy(true);
    setName(label);
    setReport(null);
    show(await api.verifyCertificate(toBase64(bytes)));
    setBusy(false);
  };

  const fromFile = async (file: File | undefined) => {
    if (!file) return;
    if (file.size > MAX_FILE_BYTES) {
      setName(file.name);
      show({ error: t("verify.reason.TooLarge") });
      return;
    }
    await run(new Uint8Array(await file.arrayBuffer()), file.name);
  };

  const fromText = async (text: string) => {
    const trimmed = text.trim();
    if (!trimmed.startsWith(QR_PREFIX)) {
      setName(t("verify.qrSource"));
      show({ error: t("verify.reason.NotDisphas") });
      return;
    }
    setQr("");
    await run(new TextEncoder().encode(trimmed), t("verify.qrSource"));
  };

  // A file or QR text pasted anywhere on the tab, outside the text field.
  const fromFileRef = useRef(fromFile);
  const fromTextRef = useRef(fromText);
  useEffect(() => {
    fromFileRef.current = fromFile;
    fromTextRef.current = fromText;
  });
  useEffect(() => {
    const onPaste = (event: ClipboardEvent) => {
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
      const file = event.clipboardData?.files[0];
      const text = event.clipboardData?.getData("text") ?? "";
      if (file) void fromFileRef.current(file);
      else if (text.trim().startsWith(QR_PREFIX)) void fromTextRef.current(text);
      else return;
      event.preventDefault();
    };
    document.addEventListener("paste", onPaste);
    return () => document.removeEventListener("paste", onPaste);
  }, []);

  return (
    <div className="split-screen grid min-h-0 flex-1" data-view={view}>
      <div className="split-lhead flex flex-col gap-4 border-r border-edge px-4 pt-6 pb-5">
        <div className="px-2">
          <h1 className="text-[25px] leading-tight font-bold">{t("verify.title")}</h1>
          <p className="mt-0.5 font-mono text-sm text-ash">{t("verify.meta")}</p>
        </div>
      </div>

      <div className="split-dhead flex min-w-0 max-w-[960px] flex-col gap-4 px-12 pt-10 pb-5">
        <BackButton label={t("verify.backLabel")} onClick={() => setView("list")}>
          {t("nav.verify")}
        </BackButton>
        {report && <ResultHead report={report} name={name} />}
      </div>

      {/* Same row as the detail below: the drop zone faces the result. */}
      <aside className="split-lbody flex flex-col gap-0.5 border-r border-edge px-4 pb-6" aria-label={t("verify.input")}>
        <h2 className="mb-1.5 px-2 text-[15px] font-bold text-ash">{t("verify.input")}</h2>
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
          className={`flex flex-col gap-4 rounded-lg border-2 border-dashed p-4 ${
            dragging ? "border-soft bg-card" : "border-ctrl"
          }`}
        >
          <button
            type="button"
            disabled={busy}
            onClick={() => input.current?.click()}
            className="flex flex-col items-center gap-2 rounded-md px-3 py-6 text-center hover:bg-card disabled:cursor-wait"
          >
            <FileUp size={28} className="text-ash" aria-hidden="true" />
            <span className="text-lg font-bold">{t("verify.drop")}</span>
            <span className="text-sm text-ash">{t("verify.dropHint")}</span>
          </button>
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
          <label className="flex flex-col gap-1.5 text-sm font-bold">
            {t("verify.qr")}
            <input
              className="block min-h-11 w-full rounded border border-ctrl bg-night px-3 font-mono text-sm font-normal"
              placeholder={t("verify.qrPlaceholder")}
              spellCheck={false}
              autoComplete="off"
              value={qr}
              disabled={busy}
              onChange={(event) => setQr(event.target.value)}
              onPaste={(event) => {
                const text = event.clipboardData.getData("text");
                if (!text.trim().startsWith(QR_PREFIX)) return;
                event.preventDefault();
                void fromText(text);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter" && qr.trim() !== "") void fromText(qr);
              }}
            />
          </label>
        </div>
      </aside>

      <main className="split-dbody flex min-w-0 max-w-[960px] flex-col gap-0.5 px-12 pb-10" aria-live="polite">
        <h3 className="mb-1.5 px-2 text-[15px] font-bold text-ash">{t("verify.result")}</h3>
        {!busy && !report && <p className="px-2 text-ash">{t("verify.emptyHint")}</p>}
        {busy && <p className="px-2 text-ash">{t("verify.busy")}</p>}
        {report && "error" in report && (
          <div className="flex items-start gap-3 rounded-lg border border-sayno p-5">
            <ShieldX className="shrink-0 text-sayno" aria-hidden="true" />
            <p className="font-bold text-sayno">{report.error}</p>
          </div>
        )}
        {report && !("error" in report) && <Result report={report} name={name} labels={labels} lang={i18n.language} />}
      </main>
    </div>
  );
}

/** Top of the result, as for a drive: where it came from, the model, the serial number. */
function ResultHead({ report, name }: { report: VerificationReport | { error: string }; name: string | null }) {
  const { t } = useTranslation();
  const source = "error" in report ? name : [name, t(`verify.source.${report.source}`)].filter(Boolean).join(" · ");
  let title: string;
  let line: string | null = null;
  let tone = "";
  if ("error" in report || (!report.batch && (!report.authentic || !report.details))) {
    title = t("verify.rejected");
    tone = "text-sayno";
  } else if (report.batch) {
    title = report.batch.label ?? report.batch.id ?? t("verify.batchAuthentic");
    line = t("verify.batchDrives", { count: report.batch.drives.length });
  } else {
    const details = report.details;
    title = details?.model ?? "";
    line = details ? `s/n ${details.serial}` : null;
  }
  return (
    <div className="min-w-0">
      {source && <div className="font-mono text-sm break-words text-ash">{source}</div>}
      <h2 className={`split-title mt-0.5 mb-1 text-[40px] leading-tight font-bold break-words ${tone}`}>{title}</h2>
      {line && <div className="font-mono text-sm break-all text-ash">{line}</div>}
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
    return <BatchResult report={report} batch={report.batch} />;
  }

  if (!report.authentic || !report.details) {
    return (
      <div className="flex items-start gap-3 rounded-lg border border-sayno p-5">
        <ShieldX className="shrink-0 text-sayno" aria-hidden="true" />
        <div>
          <h2 className="text-lg font-bold text-sayno">
            {t("verify.rejected")} : {t(`verify.reason.${report.reason ?? "Unknown"}`)}
          </h2>
        </div>
      </div>
    );
  }

  const d = report.details;
  const day = new Intl.DateTimeFormat(lang, { dateStyle: "long" });
  const date = new Intl.DateTimeFormat(lang, { dateStyle: "long", timeStyle: "medium", timeZone: "UTC" });
  const who = [d.operator, d.organization].filter(Boolean).join(", ");
  // A journal that is not the signed one fails the check, as in disphas-verify:
  // never a green header.
  const journalOk = report.journal !== "Mismatch";
  return (
    <div className={`rounded-lg border p-5 ${journalOk ? "border-ctrl" : "border-sayno"}`}>
      {/* The answer in one sentence: authentic, status, when and by whom. */}
      <div className="flex items-start gap-3">
        {journalOk ? (
          <BadgeCheck className="mt-0.5 shrink-0 text-ready" aria-hidden="true" />
        ) : (
          <ShieldX className="mt-0.5 shrink-0 text-sayno" aria-hidden="true" />
        )}
        <div className="min-w-0">
          <h2 className="text-lg font-bold">
            <span className={journalOk ? "text-ready" : "text-sayno"}>
              {t(journalOk ? "verify.authentic" : "verify.authenticJournalMismatch")}
            </span>
            {" · "}
            {t(`job.status.${d.status}`)}
            {" · "}
            {who
              ? t("verify.issuedBy", { date: day.format(new Date(d.issued_at * 1000)), who })
              : t("verify.issued", { date: day.format(new Date(d.issued_at * 1000)) })}
          </h2>
          {/* The model and serial number are in the title above. */}
          {d.method && (
            <p className="mt-1 text-ash">
              {d.category !== "None" && <>{d.category} · </>}
              {labels[d.method] ?? d.method}
            </p>
          )}
          <p className="mt-1 text-sm">{t(`job.statusHint.${d.status}`)}</p>
        </div>
      </div>

      {report.root === "Development" && <Notice tone="blocked">{t("verify.developmentRoot")}</Notice>}
      {report.journal === "Mismatch" && <Notice tone="sayno">{t("verify.journalMismatch")}</Notice>}

      <details className="mt-4">
        <summary className="cursor-pointer text-sm font-bold text-ash hover:text-soft">{t("verify.details")}</summary>
        <div className="mt-3 flex flex-wrap items-center gap-2 text-sm text-ash">
          {name} · {t(`verify.source.${report.source}`)}
          {d.category !== "None" && <Badge tone={d.category === "Purge" ? "ready" : "neutral"}>{d.category}</Badge>}
        </div>
        {report.journal === "Absent" && <p className="mt-2 text-xs text-ash">{t("verify.journalAbsent")}</p>}
        {report.journal === "Matches" && <p className="mt-2 text-xs text-ash">{t("verify.journalMatches")}</p>}
        <dl className="mt-3">
          <Fact label={t("verify.field.date")}>{date.format(new Date(d.issued_at * 1000))} UTC</Fact>
          <Fact label={t("verify.field.drive")}>
            {d.model} · {capacity(d.capacity_bytes, lang, "")} · {d.drive_type}
          </Fact>
          <Fact label="S/N">
            <Mono>{d.serial}</Mono>
            {d.hardware_id && (
              <>
                {" · "}
                <Mono>{d.hardware_id}</Mono>
              </>
            )}
            {" · "}
            {t("verify.field.firmware")} <Mono>{d.firmware}</Mono>
          </Fact>
          <Fact label={t("verify.field.method")}>
            {d.method ? label(d.method) : "—"}
            {d.zero_finish && <span className="block text-xs text-ash">{t("verify.zeroFinish")}</span>}
            {d.all_namespaces && <span className="block text-xs text-ash">{t("verify.allNamespaces")}</span>}
          </Fact>
          <Fact label={t("verify.field.verified")}>{yesNo(d.verified)}</Fact>
          <Fact label={t("verify.field.validation")}>
            {d.validation
              ? t("verify.validation", {
                  method: t(`verify.validationMethod.${d.validation.method}`, { defaultValue: d.validation.method }),
                  samples: d.validation.samples,
                  result: yesNo(d.validation.passed),
                })
              : "—"}
          </Fact>
          {d.disposition && (
            <Fact label={t("verify.field.disposition")}>
              {t(`verify.disposition.${d.disposition}`)}
              {d.disposition_codes.length > 0 && <> — {list(d.disposition_codes)}</>}
            </Fact>
          )}
          <Fact label={t("verify.field.reserves")}>{list(d.reserves)}</Fact>
          {d.refusals.length > 0 && <Fact label={t("verify.field.refusals")}>{list(d.refusals)}</Fact>}
          {d.justification && <Fact label={t("verify.field.justification")}>« {d.justification} »</Fact>}
          <Fact label={t("verify.field.operator")}>{who || "—"}</Fact>
          {d.batch_id && (
            <Fact label={t("verify.field.batch")}>
              <Mono>{d.batch_id}</Mono>
              {d.batch_label && <> — {d.batch_label}</>}
            </Fact>
          )}
          <Fact label={t("verify.field.license")}>
            {d.customer} · <Mono>{d.license_id}</Mono>
          </Fact>
          <Fact label={t("verify.field.tool")}>
            disphas {d.tool_version} · {t("verify.rules", { version: d.rules_version })}
          </Fact>
          <Fact label={t("verify.field.binary")}>
            <Mono>{d.binary_sha256}</Mono>
          </Fact>
          <Fact label={t("verify.field.id")}>
            <Mono>{d.id}</Mono>
          </Fact>
        </dl>
      </details>
    </div>
  );
}

function BatchResult({
  report,
  batch,
}: {
  report: VerificationReport;
  batch: NonNullable<VerificationReport["batch"]>;
}) {
  const { t } = useTranslation();
  const ok = report.authentic;
  const conforme = batch.drives.filter((drive) => drive.status === "CONFORME").length;
  return (
    <div className={`rounded-lg border p-5 ${ok ? "border-ctrl" : "border-sayno"}`}>
      <div className="flex items-start gap-3">
        {ok ? (
          <BadgeCheck className="mt-0.5 shrink-0 text-ready" aria-hidden="true" />
        ) : (
          <ShieldX className="mt-0.5 shrink-0 text-sayno" aria-hidden="true" />
        )}
        <div className="min-w-0">
          <h2 className="text-lg font-bold">
            <span className={ok ? "text-ready" : "text-sayno"}>
              {ok ? t("verify.batchAuthentic") : t("verify.batchRejected")}
            </span>
            {" · "}
            {batch.label ?? batch.id ?? "—"}
            {" · "}
            {t("verify.batchDrives", { count: batch.drives.length })}
            {ok && (
              <>
                {" · "}
                {t("finished.conforme", { count: conforme })}
              </>
            )}
          </h2>
        </div>
      </div>
      {ok && report.root === "Development" && <Notice tone="blocked">{t("verify.developmentRoot")}</Notice>}
      <ul className="mt-4 divide-y divide-edge rounded-md border border-edge text-sm">
        {batch.drives.map((drive) => (
          <li key={drive.report} className="flex flex-wrap items-center gap-3 px-3 py-2">
            {drive.accepted ? (
              <BadgeCheck size={16} className="text-ready" aria-hidden="true" />
            ) : (
              <ShieldX size={16} className="text-sayno" aria-hidden="true" />
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
  const classes = tone === "sayno" ? "border-sayno text-sayno" : "border-blocked text-blocked";
  return (
    <div className={`mt-4 flex gap-2 rounded-md border p-3 text-sm ${classes}`}>
      <TriangleAlert size={16} className="mt-0.5 shrink-0" aria-hidden="true" />
      {children}
    </div>
  );
}
