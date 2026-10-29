import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { CheckCircle2, FileText, LoaderCircle, Search, ShieldAlert, Trash2, XCircle } from "lucide-react";
import { api, type HelperError, type HistoryEntry } from "../lib/api";
import { capacity } from "../lib/format";
import type { Language } from "../i18n";
import { BackButton, Badge, Button, Callout, Dialog, Fact, fieldClass, IconButton, Mono } from "./ui";

/**
 * Sort key of the history, newest first by the date shown: the end of the
 * operation. The helper lists by identifier, i.e. by start, so a long erasure
 * finished last would sit below a shorter one started after it. A report that
 * does not verify has no date: the time in its UUID v7 identifier stands in.
 */
function issuedAt(entry: HistoryEntry): number {
  return entry.summary?.issued_at ?? (Number.parseInt(entry.id.replaceAll("-", "").slice(0, 12), 16) / 1000 || 0);
}

/** `record` without `key`. */
function without<T>(record: Record<string, T>, key: string): Record<string, T> {
  const next = { ...record };
  delete next[key];
  return next;
}

/** Certificates issued the same day, under one heading. */
type Day = { key: string; title: string; entries: HistoryEntry[] };

/** What happened to a certificate or a batch the operator asked to open. */
type Opening = { kind: "busy" } | { kind: "failed"; error: HelperError } | { kind: "empty" };

/** Narrow window: one view at a time (same breakpoint as index.css). */
const narrow = () => window.matchMedia("(max-width: 760px)").matches;

/**
 * History of signed certificates, read from the helper's `reports/` folder,
 * laid out like the Disques tab: the certificates on the left, newest first
 * and grouped by day, the chosen one on the right with its PDF and its
 * batch. One search over everything; a double click opens the PDF at once.
 * Reports that do not verify are listed by identifier only: their content is
 * never shown (docs/protocole_ipc.md §10).
 */
export function HistoryScreen({ labels }: { labels: Record<string, string> }) {
  const { t, i18n } = useTranslation();
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<string | null>(null);
  const [view, setView] = useState<"list" | "detail">("list");
  const [paths, setPaths] = useState<Record<string, string>>({});
  const [opening, setOpening] = useState<Record<string, Opening>>({});
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleted, setDeleted] = useState<{ count: number } | { error: HelperError } | null>(null);
  const list = useRef<HTMLElement>(null);
  // "Today" in the day headings: the day the tab was opened.
  const [today] = useState(() => new Date());

  const receive = (result: HistoryEntry[] | { error: string }) => {
    if (Array.isArray(result)) {
      setListError(null);
      setEntries(result);
    } else {
      setListError(result.error);
      setEntries([]);
    }
  };

  useEffect(() => {
    void api.listReports().then(receive);
  }, []);

  const label = (code: string) => labels[code] ?? code;

  // One search: model, serial number, batch, customer, operator, status, method, date.
  const shown = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const day = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
    const sorted = [...(entries ?? [])].sort((a, b) => issuedAt(b) - issuedAt(a));
    if (needle === "") return sorted;
    return sorted.filter((entry) => {
      const s = entry.summary;
      const fields = [entry.id];
      if (s) {
        fields.push(
          s.model,
          s.serial,
          s.status,
          t(`job.status.${s.status}`),
          s.category,
          day.format(new Date(s.issued_at * 1000)),
        );
        for (const field of [s.operator, s.organization, s.batch_id, s.batch_label, s.method])
          if (field) fields.push(field);
        if (s.method) fields.push(labels[s.method] ?? s.method);
      }
      return fields.some((field) => field.toLowerCase().includes(needle));
    });
  }, [entries, query, t, i18n.language, labels]);

  // Newest first, one heading per day: today, yesterday, then the date.
  const days = useMemo(() => {
    const long = new Intl.DateTimeFormat(i18n.language, { dateStyle: "full" });
    const keyOf = (date: Date) => `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
    const yesterday = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1);
    const result: Day[] = [];
    for (const entry of shown) {
      const date = new Date(issuedAt(entry) * 1000);
      const key = keyOf(date);
      let day = result.at(-1);
      if (day?.key !== key) {
        const title =
          key === keyOf(today)
            ? t("history.today")
            : key === keyOf(yesterday)
              ? t("history.yesterday")
              : long.format(date);
        day = { key, title, entries: [] };
        result.push(day);
      }
      day.entries.push(entry);
    }
    return result;
  }, [shown, t, i18n.language, today]);

  const selected = shown.find((entry) => entry.id === picked) ?? shown[0] ?? null;

  const pick = (id: string, detail: boolean) => {
    setPicked(id);
    if (detail && narrow()) {
      setView("detail");
      document.querySelector(".split-dbody")?.scrollTo(0, 0);
    }
  };

  // Up and Down move through the list, as in the Disques tab.
  const arrow = (id: string, event: React.KeyboardEvent) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const index = shown.findIndex((entry) => entry.id === id);
    const next = shown[index + (event.key === "ArrowDown" ? 1 : -1)];
    if (!next) return;
    pick(next.id, false);
    list.current?.querySelector<HTMLButtonElement>(`[data-report="${next.id}"]`)?.focus();
  };

  const openReport = async (id: string) => {
    if (paths[id]) {
      await api.openCertificate(paths[id]);
      return;
    }
    setOpening((current) => ({ ...current, [id]: { kind: "busy" } }));
    const result = await api.renderReport(id, i18n.language as Language);
    if (result.ok) {
      setPaths((current) => ({ ...current, [id]: result.value }));
      setOpening((current) => without(current, id));
      await api.openCertificate(result.value);
    } else {
      setOpening((current) => ({ ...current, [id]: { kind: "failed", error: result.error } }));
    }
  };

  const openBatch = async (id: string) => {
    const key = `batch:${id}`;
    if (paths[key]) {
      await api.openCertificate(paths[key]);
      return;
    }
    setOpening((current) => ({ ...current, [key]: { kind: "busy" } }));
    const result = await api.renderBatch(id, i18n.language as Language);
    if (!result.ok) {
      setOpening((current) => ({ ...current, [key]: { kind: "failed", error: result.error } }));
    } else if (result.value === null) {
      setOpening((current) => ({ ...current, [key]: { kind: "empty" } }));
    } else {
      const path = result.value;
      setPaths((current) => ({ ...current, [key]: path }));
      setOpening((current) => without(current, key));
      await api.openCertificate(path);
    }
  };

  const total = entries?.length ?? 0;
  const hidden = (entries ?? []).filter((entry) => entry.check !== "Valid" || !entry.summary).length;
  const time = new Intl.DateTimeFormat(i18n.language, { timeStyle: "short" });

  const row = (entry: HistoryEntry) => {
    const summary = entry.check === "Valid" ? entry.summary : null;
    const isSelected = entry.id === selected?.id;
    return (
      <button
        key={entry.id}
        type="button"
        data-report={entry.id}
        aria-pressed={isSelected}
        tabIndex={isSelected ? 0 : -1}
        title={summary ? t("history.open") : undefined}
        onClick={() => pick(entry.id, true)}
        onDoubleClick={() => summary && void openReport(entry.id)}
        onKeyDown={(event) => arrow(entry.id, event)}
        className={`flex min-h-16 min-w-0 flex-col justify-center rounded-md border px-3 py-2 text-left ${
          isSelected ? "border-ctrl bg-card" : "border-transparent hover:bg-card/50"
        }`}
      >
        {summary ? (
          <>
            <span className="flex justify-between gap-2">
              <span className={`truncate text-base ${isSelected ? "font-bold" : ""}`}>{summary.model}</span>
              <span
                className={`font-mono text-sm whitespace-nowrap ${summary.status === "CONFORME" ? "text-ready" : "text-sayno"}`}
              >
                {t(`job.status.${summary.status}`)}
              </span>
            </span>
            <span className="truncate text-sm text-ash">
              {time.format(new Date(summary.issued_at * 1000))} · <span className="font-mono">s/n {summary.serial}</span>
              {summary.batch_id && <> · {t("jobs.inBatch", { name: summary.batch_label ?? summary.batch_id })}</>}
            </span>
          </>
        ) : (
          <>
            <span className="flex items-center gap-2 text-sayno">
              <ShieldAlert size={14} className="shrink-0" aria-hidden="true" />
              <span className={`truncate ${isSelected ? "font-bold" : ""}`}>{t("history.hidden")}</span>
            </span>
            <span className="truncate font-mono text-xs text-ash">{entry.id}</span>
          </>
        )}
      </button>
    );
  };

  return (
    <div className="split-screen grid min-h-0 flex-1" data-view={view}>
      <div className="split-lhead flex flex-col gap-4 border-r border-edge px-4 pt-6 pb-5">
        <div className="flex items-start justify-between gap-2 px-2">
          <div>
            <h1 className="text-[25px] leading-tight font-bold">{t("history.count", { count: total })}</h1>
            {total > 0 && (
              <p className="mt-0.5 font-mono text-sm text-ash">
                {t("history.meta", { count: total - hidden })}
                {hidden > 0 && <> · {t("history.hiddenCount", { count: hidden })}</>}
              </p>
            )}
          </div>
          {total > 0 && (
            <IconButton label={t("history.delete")} onClick={() => setDeleteOpen(true)}>
              <Trash2 size={18} aria-hidden="true" />
            </IconButton>
          )}
        </div>

        {total > 0 && (
          <label className="flex min-h-11 items-center gap-2.5 rounded border border-ctrl bg-night px-3">
            <Search size={16} className="shrink-0 text-ash" aria-hidden="true" />
            <input
              type="search"
              className="min-w-0 flex-1 bg-transparent outline-none placeholder:text-ash"
              aria-label={t("history.searchLabel")}
              placeholder={t("history.search")}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
        )}

        {deleted && "count" in deleted && (
          <p role="status" className="px-2 text-sm text-ash">
            {t("history.deletedCount", { count: deleted.count })}
          </p>
        )}
        {deleted && "error" in deleted && (
          <p role="alert" className="px-2 text-sm text-sayno">
            {t(`errors.${deleted.error.code}`, { defaultValue: deleted.error.code })}
          </p>
        )}
      </div>

      <div className="split-dhead flex min-w-0 flex-col gap-4 px-12 pt-10 pb-5">
        <BackButton label={t("history.backLabel")} onClick={() => setView("list")}>
          {t("nav.history")}
        </BackButton>
        {selected && <Head entry={selected} label={label} />}
      </div>

      {/* Same row as the detail below: the first day faces the certificate. */}
      <aside
        ref={list}
        className="split-lbody flex flex-col gap-5 border-r border-edge px-4 pb-6"
        aria-label={t("nav.history")}
      >
        {entries === null && <p className="px-2 text-ash">{t("history.loading")}</p>}
        {listError !== null && <p className="px-2 text-sayno">{listError}</p>}
        {listError === null && entries?.length === 0 && <p className="px-2 text-ash">{t("history.empty")}</p>}
        {total > 0 && shown.length === 0 && <p className="px-2 text-ash">{t("history.noMatch")}</p>}
        {days.map((day) => (
          <section key={day.key} aria-labelledby={`day-${day.key}`} className="flex flex-col gap-0.5">
            <h2 id={`day-${day.key}`} className="mb-1.5 px-2 text-[15px] font-bold text-ash first-letter:uppercase">
              {day.title}
            </h2>
            {day.entries.map(row)}
          </section>
        ))}
      </aside>

      <main className="split-dbody flex min-w-0 flex-col gap-7 px-12 pb-10">
        {selected === null && entries !== null && <p className="pt-1 text-ash">{t("history.emptyHint")}</p>}
        {selected && (
          <Detail
            entry={selected}
            all={entries ?? []}
            label={label}
            opening={opening}
            onOpen={(id) => void openReport(id)}
            onOpenBatch={(id) => void openBatch(id)}
            onPick={(id) => pick(id, false)}
          />
        )}
      </main>

      {deleteOpen && (
        <DeleteDialog
          shown={shown}
          searching={query.trim() !== ""}
          onCancel={() => setDeleteOpen(false)}
          onDeleted={async (result) => {
            setDeleteOpen(false);
            setDeleted(result);
            receive(await api.listReports());
          }}
        />
      )}
    </div>
  );
}

/** Top of the detail, as for a drive: when, the model, the serial number. */
function Head({ entry, label }: { entry: HistoryEntry; label: (code: string) => string }) {
  const { t, i18n } = useTranslation();
  const summary = entry.check === "Valid" ? entry.summary : null;
  if (!summary) {
    return (
      <div className="min-w-0">
        <div className="font-mono text-sm text-sayno">{t(`history.check.${entry.check}`)}</div>
        <h2 className="split-title mt-0.5 mb-1 text-[40px] leading-tight font-bold break-words">
          {t("history.hidden")}
        </h2>
        <div className="font-mono text-sm break-all text-ash">{entry.id}</div>
      </div>
    );
  }
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "long", timeStyle: "short" });
  return (
    <div className="min-w-0">
      <div className="font-mono text-sm break-words text-ash">
        {t("history.issuedOn", { date: date.format(new Date(summary.issued_at * 1000)) })}
        {summary.method && <> · {label(summary.method)}</>}
      </div>
      <h2 className="split-title mt-0.5 mb-1 text-[40px] leading-tight font-bold break-words">{summary.model}</h2>
      <div className="font-mono text-sm break-all text-ash">
        s/n {summary.serial} · {capacity(summary.capacity_bytes, i18n.language, "")}
      </div>
    </div>
  );
}

/**
 * The chosen certificate: its status and its PDF, who signed it, and the
 * batch it belongs to with the batch certificate.
 */
function Detail({
  entry,
  all,
  label,
  opening,
  onOpen,
  onOpenBatch,
  onPick,
}: {
  entry: HistoryEntry;
  all: HistoryEntry[];
  label: (code: string) => string;
  opening: Record<string, Opening>;
  onOpen: (id: string) => void;
  onOpenBatch: (id: string) => void;
  onPick: (id: string) => void;
}) {
  const { t } = useTranslation();
  const summary = entry.check === "Valid" ? entry.summary : null;
  if (!summary) {
    return (
      <div className="flex flex-col gap-0.5">
        <h3 className="mb-1.5 px-2 text-[15px] font-bold text-ash">{t("history.certificate")}</h3>
        <p className="flex items-start gap-3 rounded-md border border-sayno px-4 py-3.5">
          <ShieldAlert size={18} className="mt-1 shrink-0 text-sayno" aria-hidden="true" />
          {t("history.hiddenHint")}
        </p>
      </div>
    );
  }
  const busy = opening[entry.id]?.kind === "busy";
  const who = [summary.operator, summary.organization].filter(Boolean).join(", ");
  const batchId = summary.batch_id;
  const members = batchId
    ? [...all].filter((other) => other.summary?.batch_id === batchId).sort((a, b) => issuedAt(a) - issuedAt(b))
    : [];
  const batchKey = `batch:${batchId}`;
  return (
    <>
      {/* Same rhythm as the list: a title like a day's, level with the first certificate. */}
      <div className="flex flex-col gap-0.5">
        <h3 className="mb-1.5 px-2 text-[15px] font-bold text-ash">{t("history.certificate")}</h3>
        <div className="flex flex-col gap-4 rounded-lg border border-ctrl p-6">
          <div className="flex flex-wrap items-center gap-3">
            {summary.status === "CONFORME" ? (
              <CheckCircle2 className="text-ready" aria-hidden="true" />
            ) : (
              <XCircle className="text-sayno" aria-hidden="true" />
            )}
            <Badge tone={summary.status === "CONFORME" ? "ready" : "sayno"}>{t(`job.status.${summary.status}`)}</Badge>
            <span className="text-[17px]">{t(`job.statusHint.${summary.status}`)}</span>
          </div>
          <div className="flex flex-wrap items-center gap-4">
            <Button
              variant="primary"
              disabled={busy}
              className="flex min-h-12 items-center gap-2 px-5.5 text-[17px] disabled:cursor-wait"
              onClick={() => onOpen(entry.id)}
            >
              {busy ? (
                <LoaderCircle size={16} className="animate-spin" aria-hidden="true" />
              ) : (
                <FileText size={16} aria-hidden="true" />
              )}
              {t("job.openCertificate")}
            </Button>
            <span className="text-sm text-ash max-[760px]:hidden">{t("history.openHint")}</span>
          </div>
          <Problem opening={opening[entry.id]} />
        </div>
      </div>

      <dl>
        <Fact label={t("verify.field.method")}>
          {summary.method ? `${summary.category} · ${label(summary.method)}` : "—"}
        </Fact>
        <Fact label={t("verify.field.operator")}>{who || "—"}</Fact>
        <Fact label={t("verify.field.id")}>
          <Mono>{entry.id}</Mono>
        </Fact>
      </dl>

      {batchId && (
        <section className="flex flex-col gap-0.5">
          <div className="mb-1.5 flex flex-wrap items-center gap-3 px-2">
            <h3 className="text-[15px] font-bold text-ash">
              {t("history.batch", { name: summary.batch_label ?? batchId, count: members.length })}
            </h3>
            <Button
              className="ml-auto flex items-center gap-2"
              disabled={opening[batchKey]?.kind === "busy"}
              onClick={() => onOpenBatch(batchId)}
            >
              {opening[batchKey]?.kind === "busy" ? (
                <LoaderCircle size={14} className="animate-spin" aria-hidden="true" />
              ) : (
                <FileText size={14} aria-hidden="true" />
              )}
              {t("history.batchPdf")}
            </Button>
          </div>
          <Problem opening={opening[batchKey]} />
          {members.map((member) => (
            <button
              key={member.id}
              type="button"
              onClick={() => onPick(member.id)}
              aria-current={member.id === entry.id ? "true" : undefined}
              className={`flex min-h-11 items-center justify-between gap-3 rounded-md px-2 text-left text-sm hover:bg-card/50 ${
                member.id === entry.id ? "font-bold" : ""
              }`}
            >
              <span className="min-w-0 truncate">
                {member.summary?.model}{" "}
                <span className="font-mono font-normal text-ash">s/n {member.summary?.serial}</span>
              </span>
              {member.summary && (
                <span
                  className={`font-mono whitespace-nowrap ${member.summary.status === "CONFORME" ? "text-ready" : "text-sayno"}`}
                >
                  {t(`job.status.${member.summary.status}`)}
                </span>
              )}
            </button>
          ))}
        </section>
      )}
    </>
  );
}

function Problem({ opening }: { opening: Opening | undefined }) {
  const { t } = useTranslation();
  if (opening?.kind === "empty") return <p className="w-full px-2 text-sm text-ash">{t("batch.empty")}</p>;
  if (opening?.kind !== "failed") return null;
  return (
    <p role="alert" className="w-full px-2 text-sm text-sayno">
      {t(`errors.${opening.error.code}`, { defaultValue: opening.error.code })}
      {opening.error.detail && <span className="block font-mono text-xs text-ash">{opening.error.detail}</span>}
    </p>
  );
}

/**
 * Deletes certificates from the history (customer's retention, GDPR): those
 * shown by the search, optionally only those issued before a day. One
 * warning, confirmed with one click; the helper logs every deletion.
 */
function DeleteDialog({
  shown,
  searching,
  onCancel,
  onDeleted,
}: {
  shown: HistoryEntry[];
  searching: boolean;
  onCancel: () => void;
  onDeleted: (result: { count: number } | { error: HelperError }) => void;
}) {
  const { t } = useTranslation();
  const [before, setBefore] = useState("");
  const [busy, setBusy] = useState(false);

  // "Before" is a calendar day: certificates issued before that day, UTC.
  const limit = before === "" ? null : Date.parse(`${before}T00:00:00Z`) / 1000;
  const targets = shown.filter(
    (entry) => limit === null || (entry.summary !== null && entry.summary.issued_at < limit),
  );

  const remove = async () => {
    setBusy(true);
    const result = await api.deleteReports(targets.map((entry) => entry.id));
    onDeleted(result.ok ? { count: result.value.deleted } : { error: result.error });
  };

  return (
    <Dialog
      title={t("history.deleteTitle")}
      onCancel={onCancel}
      footer={
        <>
          <Button onClick={onCancel}>{t("actions.cancel")}</Button>
          <Button variant="danger" disabled={busy || targets.length === 0} onClick={() => void remove()}>
            {targets.length === 0 ? t("history.deleteNone") : t("history.deleteNow", { count: targets.length })}
          </Button>
        </>
      }
    >
      <p className="text-sm text-ash">{t(searching ? "history.deleteScopeSearch" : "history.deleteScopeAll")}</p>
      <label className="block text-sm font-bold">
        {t("history.before")}
        <input type="date" className={fieldClass} value={before} onChange={(event) => setBefore(event.target.value)} />
        <span className="text-xs font-normal text-ash">{t("history.beforeHint")}</span>
      </label>
      <Callout tone="sayno">{t("history.deleteWarning")}</Callout>
    </Dialog>
  );
}
