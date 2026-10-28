import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileText, LoaderCircle, Search, ShieldAlert, Trash2, TriangleAlert } from "lucide-react";
import { api, type HelperError, type HistoryEntry } from "../lib/api";
import { capacity } from "../lib/format";
import type { Language } from "../i18n";
import { Badge, Button } from "./ui";

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

/** A batch with its certificates, or a certificate erased alone. */
type Item =
  | { kind: "batch"; id: string; label: string | null; entries: HistoryEntry[] }
  | { kind: "single"; entry: HistoryEntry };

/** What happened to a certificate or a batch the operator asked to open. */
type Opening = { kind: "busy" } | { kind: "failed"; error: HelperError } | { kind: "empty" };

/**
 * History of signed certificates, read from the helper's `reports/` folder.
 * One search over everything, certificates grouped by batch, one click on a
 * line opens its PDF (rendered on the fly). Reports that do not verify are
 * listed by identifier only: their content is never shown
 * (docs/protocole_ipc.md §10).
 */
export function HistoryScreen({ labels }: { labels: Record<string, string> }) {
  const { t, i18n } = useTranslation();
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [paths, setPaths] = useState<Record<string, string>>({});
  const [opening, setOpening] = useState<Record<string, Opening>>({});
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleted, setDeleted] = useState<{ count: number } | { error: HelperError } | null>(null);

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

  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
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

  // A batch sits at the place of its newest certificate; the others stay one by one.
  const items = useMemo(() => {
    const result: Item[] = [];
    const batches = new Map<string, Extract<Item, { kind: "batch" }>>();
    for (const entry of shown) {
      const id = entry.summary?.batch_id;
      if (!id) {
        result.push({ kind: "single", entry });
        continue;
      }
      let batch = batches.get(id);
      if (!batch) {
        batch = { kind: "batch", id, label: null, entries: [] };
        batches.set(id, batch);
        result.push(batch);
      }
      batch.label ??= entry.summary?.batch_label ?? null;
      batch.entries.push(entry);
    }
    return result;
  }, [shown]);

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

  return (
    <section>
      <div className="mb-5 flex flex-wrap items-center gap-3">
        <h1 className="text-[25px] leading-tight font-bold">{t("history.count", { count: total })}</h1>
        {total > 0 && (
          <Button className="ml-auto flex items-center gap-2" onClick={() => setDeleteOpen(true)}>
            <Trash2 size={14} aria-hidden="true" />
            {t("history.delete")}
          </Button>
        )}
      </div>

      {total > 0 && (
        <label className="mb-5 flex min-h-11 items-center gap-2.5 rounded border border-ctrl bg-night px-3">
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
        <p role="status" className="mb-4 text-sm text-ash">
          {t("history.deletedCount", { count: deleted.count })}
        </p>
      )}
      {deleted && "error" in deleted && (
        <p role="alert" className="mb-4 text-sm text-sayno">
          {t(`errors.${deleted.error.code}`, { defaultValue: deleted.error.code })}
        </p>
      )}
      {entries === null && <p className="text-ash">{t("history.loading")}</p>}
      {listError !== null && <p className="text-sayno">{listError}</p>}
      {listError === null && entries?.length === 0 && <p className="text-ash">{t("history.empty")}</p>}
      {total > 0 && shown.length === 0 && <p className="text-ash">{t("history.noMatch")}</p>}

      <div className="flex flex-col gap-5">
        {items.map((item) =>
          item.kind === "single" ? (
            <Line
              key={item.entry.id}
              entry={item.entry}
              opening={opening[item.entry.id]}
              date={date}
              label={label}
              onOpen={() => void openReport(item.entry.id)}
            />
          ) : (
            <section
              key={`batch:${item.id}`}
              aria-label={t("history.batch", { name: item.label ?? item.id, count: item.entries.length })}
              className="-mx-px flex flex-col gap-0.5 rounded-lg border border-edge py-2"
            >
              <div className="mb-1 flex flex-wrap items-center gap-3 px-3">
                <h2 className="text-[15px] font-bold text-ash">
                  {t("history.batch", { name: item.label ?? item.id, count: item.entries.length })}
                  {item.label && <span className="ml-2 font-mono text-xs font-normal">{item.id}</span>}
                </h2>
                <Button
                  className="ml-auto flex items-center gap-2"
                  disabled={opening[`batch:${item.id}`]?.kind === "busy"}
                  onClick={() => void openBatch(item.id)}
                >
                  {opening[`batch:${item.id}`]?.kind === "busy" ? (
                    <LoaderCircle size={14} className="animate-spin" aria-hidden="true" />
                  ) : (
                    <FileText size={14} aria-hidden="true" />
                  )}
                  {t("history.batchPdf")}
                </Button>
                <Problem opening={opening[`batch:${item.id}`]} />
              </div>
              {item.entries.map((entry) => (
                <Line
                  key={entry.id}
                  entry={entry}
                  opening={opening[entry.id]}
                  date={date}
                  label={label}
                  onOpen={() => void openReport(entry.id)}
                />
              ))}
            </section>
          ),
        )}
      </div>

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
    </section>
  );
}

/** One certificate: the whole line opens its PDF. */
function Line({
  entry,
  opening,
  date,
  label,
  onOpen,
}: {
  entry: HistoryEntry;
  opening: Opening | undefined;
  date: Intl.DateTimeFormat;
  label: (code: string) => string;
  onOpen: () => void;
}) {
  const { t, i18n } = useTranslation();
  const summary = entry.summary;
  if (entry.check !== "Valid" || !summary) {
    return (
      <div className="flex min-h-16 items-center gap-3.5 rounded-md border border-transparent px-3 py-2">
        <ShieldAlert size={18} className="shrink-0 text-sayno" aria-hidden="true" />
        <div className="min-w-0 text-sm">
          <p className="font-bold text-sayno">{t(`history.check.${entry.check}`)}</p>
          <p className="font-mono text-xs break-all text-ash">{entry.id}</p>
        </div>
      </div>
    );
  }
  const busy = opening?.kind === "busy";
  const who = [summary.operator, summary.organization].filter(Boolean).join(", ");
  return (
    <div>
      <button
        type="button"
        onClick={onOpen}
        disabled={busy}
        title={t("history.open")}
        className="flex min-h-16 w-full items-center gap-3.5 rounded-md border border-transparent px-3 py-2 text-left hover:border-ctrl hover:bg-card disabled:cursor-wait"
      >
        <span className="w-32 shrink-0">
          <Badge tone={summary.status === "CONFORME" ? "ready" : "sayno"}>{t(`job.status.${summary.status}`)}</Badge>
        </span>
        <span className="flex min-w-0 flex-1 flex-col">
          <span>
            <span className="font-bold">{summary.model}</span>
            <span className="ml-2 text-sm text-ash">{capacity(summary.capacity_bytes, i18n.language, "")}</span>
          </span>
          <span className="text-sm break-words text-ash">
            <span className="font-mono">s/n {summary.serial}</span>
            {summary.method && <> · {label(summary.method)}</>}
            {who && <> · {who}</>}
          </span>
        </span>
        <span className="w-40 shrink-0 text-right text-sm text-ash">{date.format(new Date(summary.issued_at * 1000))}</span>
        <span className="flex size-5 shrink-0 items-center justify-center text-ash" aria-hidden="true">
          {busy ? <LoaderCircle size={16} className="animate-spin" /> : <FileText size={16} />}
        </span>
      </button>
      <Problem opening={opening} />
    </div>
  );
}

function Problem({ opening }: { opening: Opening | undefined }) {
  const { t } = useTranslation();
  if (opening?.kind === "empty") return <p className="w-full px-3 text-sm text-ash">{t("batch.empty")}</p>;
  if (opening?.kind !== "failed") return null;
  return (
    <p role="alert" className="w-full px-3 pb-2 text-sm text-sayno">
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
  const dialog = useRef<HTMLDialogElement>(null);
  const [before, setBefore] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, []);

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
    <dialog
      ref={dialog}
      onCancel={onCancel}
      className="m-auto w-[34rem] max-w-[92vw] rounded-lg border border-ctrl bg-card p-6 text-soft backdrop:bg-black/70"
    >
      <h2 className="text-xl font-bold">{t("history.deleteTitle")}</h2>
      <p className="mt-2 text-sm text-ash">{t(searching ? "history.deleteScopeSearch" : "history.deleteScopeAll")}</p>
      <label className="mt-4 block text-sm font-bold">
        {t("history.before")}
        <input
          type="date"
          className="mt-1.5 block min-h-11 w-full rounded border border-ctrl bg-night px-3 font-normal"
          value={before}
          onChange={(event) => setBefore(event.target.value)}
        />
        <span className="text-xs font-normal text-ash">{t("history.beforeHint")}</span>
      </label>
      <p className="mt-4 flex items-start gap-2.5 rounded-md border border-sayno px-3 py-2.5 text-sm">
        <TriangleAlert size={18} className="mt-0.5 shrink-0 text-sayno" aria-hidden="true" />
        {t("history.deleteWarning")}
      </p>
      <div className="mt-6 flex justify-end gap-2">
        <Button onClick={onCancel}>{t("actions.cancel")}</Button>
        <Button variant="danger" disabled={busy || targets.length === 0} onClick={() => void remove()}>
          {targets.length === 0 ? t("history.deleteNone") : t("history.deleteNow", { count: targets.length })}
        </Button>
      </div>
    </dialog>
  );
}
