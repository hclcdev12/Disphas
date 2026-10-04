import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileText, RefreshCw, Search, ShieldAlert, Trash2, TriangleAlert } from "lucide-react";
import { api, type HelperError, type HistoryEntry } from "../lib/api";
import { capacity } from "../lib/format";
import type { Language } from "../i18n";
import { Badge, Button, Mono } from "./ui";

/**
 * History of signed certificates, read from the helper's `reports/` folder.
 * Reports that do not verify are listed by identifier only: their content
 * is never shown (docs/protocole_ipc.md §10).
 */
export function HistoryScreen({ labels }: { labels: Record<string, string> }) {
  const { t, i18n } = useTranslation();
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const receive = (result: HistoryEntry[] | { error: string }) => {
    if (Array.isArray(result)) {
      setListError(null);
      setEntries(result);
    } else {
      setListError(result.error);
      setEntries([]);
    }
  };
  const [query, setQuery] = useState("");
  const [saved, setSaved] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<{ id: string; error: HelperError } | null>(null);
  const [batch, setBatch] = useState("");
  const [batchPdf, setBatchPdf] = useState<{ path: string } | { empty: true } | { error: HelperError } | null>(null);
  const [batchBusy, setBatchBusy] = useState(false);
  const [before, setBefore] = useState("");
  const [deleting, setDeleting] = useState<"confirm" | "busy" | null>(null);
  const [deleted, setDeleted] = useState<{ count: number } | { error: HelperError } | null>(null);

  // Batches present in the history, with their label and number of drives.
  const batches = useMemo(() => {
    const found = new Map<string, { label: string | null; count: number }>();
    for (const entry of entries ?? []) {
      const id = entry.summary?.batch_id;
      if (!id) continue;
      const known = found.get(id);
      found.set(id, { label: known?.label ?? entry.summary?.batch_label ?? null, count: (known?.count ?? 0) + 1 });
    }
    return [...found.entries()].sort((a, b) => b[0].localeCompare(a[0]));
  }, [entries]);

  const exportBatch = async () => {
    setBatchBusy(true);
    setBatchPdf(null);
    const result = await api.renderBatch(batch, i18n.language as Language);
    setBatchPdf(
      !result.ok ? { error: result.error } : result.value === null ? { empty: true } : { path: result.value },
    );
    setBatchBusy(false);
  };

  useEffect(() => {
    void api.listReports().then(receive);
  }, []);

  const reload = async () => {
    setEntries(null);
    receive(await api.listReports());
  };

  const shown = useMemo(() => {
    const needle = query.trim().toLowerCase();
    // "Before" is a calendar day: certificates issued before that day, UTC.
    const limit = before === "" ? null : Date.parse(`${before}T00:00:00Z`) / 1000;
    const inBatch = (entries ?? []).filter(
      (entry) =>
        (batch === "" || entry.summary?.batch_id === batch) &&
        (limit === null || (entry.summary !== null && entry.summary.issued_at < limit)),
    );
    if (needle === "") return inBatch;
    return inBatch.filter((entry) =>
      [entry.id, entry.summary?.model, entry.summary?.serial, entry.summary?.operator, entry.summary?.organization]
        .filter((field): field is string => typeof field === "string")
        .some((field) => field.toLowerCase().includes(needle)),
    );
  }, [entries, query, batch, before]);

  const deleteShown = async () => {
    setDeleting("busy");
    const result = await api.deleteReports(shown.map((entry) => entry.id));
    setDeleting(null);
    setDeleted(result.ok ? { count: result.value.deleted } : { error: result.error });
    receive(await api.listReports());
  };

  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });
  const label = (code: string) => labels[code] ?? code;

  const exportPdf = async (id: string) => {
    setBusy(id);
    setError(null);
    const result = await api.renderReport(id, i18n.language as Language);
    if (result.ok) setSaved((current) => ({ ...current, [id]: result.value }));
    else setError({ id, error: result.error });
    setBusy(null);
  };

  return (
    <section>
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <h1 className="text-xl font-bold">{t("history.title")}</h1>
        <label className="ml-auto flex items-center gap-2 rounded-lg border border-edge bg-card px-2.5 py-1.5 text-sm">
          <Search size={14} className="text-ash" />
          <input
            className="w-56 bg-transparent outline-none placeholder:text-ash/60"
            placeholder={t("history.search")}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <Button className="flex items-center gap-2" onClick={() => void reload()}>
          <RefreshCw size={14} />
          {t("drives.refresh")}
        </Button>
      </div>

      {batches.length > 0 && (
        <div className="mb-4 flex flex-wrap items-center gap-3 rounded-lg border border-edge bg-card px-4 py-3 text-sm">
          <label className="flex items-center gap-2">
            {t("verify.field.batch")}
            <select
              className="rounded-md border border-edge bg-night px-2 py-1.5"
              value={batch}
              onChange={(event) => {
                setBatch(event.target.value);
                setBatchPdf(null);
              }}
            >
              <option value="">{t("history.allBatches")}</option>
              {batches.map(([id, info]) => (
                <option key={id} value={id}>
                  {id}
                  {info.label ? ` — ${info.label}` : ""} ({info.count})
                </option>
              ))}
            </select>
          </label>
          {batch !== "" && (
            <Button variant="primary" className="ml-auto flex items-center gap-2" disabled={batchBusy} onClick={() => void exportBatch()}>
              <FileText size={14} />
              {batchBusy ? t("job.exporting") : t("history.batchPdf")}
            </Button>
          )}
          {batchPdf && "path" in batchPdf && (
            <p className="w-full text-xs text-ash">
              {t("job.saved")} <Mono>{batchPdf.path}</Mono>{" "}
              <Button onClick={() => void api.openCertificate(batchPdf.path)}>{t("job.open")}</Button>
            </p>
          )}
          {batchPdf && "empty" in batchPdf && <p className="w-full text-xs text-ash">{t("batch.empty")}</p>}
          {batchPdf && "error" in batchPdf && (
            <p className="w-full text-sm text-sayno">
              {t(`errors.${batchPdf.error.code}`, { defaultValue: batchPdf.error.code })}
              {batchPdf.error.detail && <span className="block font-mono text-xs text-ash">{batchPdf.error.detail}</span>}
            </p>
          )}
        </div>
      )}
      {entries !== null && entries.length > 0 && (
        <div className="mb-4 flex flex-wrap items-center gap-3 rounded-lg border border-edge bg-card px-4 py-3 text-sm">
          <label className="flex items-center gap-2">
            {t("history.before")}
            <input
              type="date"
              className="rounded-md border border-edge bg-night px-2 py-1"
              value={before}
              onChange={(event) => {
                setBefore(event.target.value);
                setDeleting(null);
              }}
            />
          </label>
          {deleting === null && shown.length > 0 && (
            <Button variant="danger" className="ml-auto flex items-center gap-2" onClick={() => setDeleting("confirm")}>
              <Trash2 size={14} />
              {t("history.deleteShown", { count: shown.length })}
            </Button>
          )}
          {deleting !== null && (
            <div className="flex w-full flex-wrap items-center gap-3 rounded-md border border-sayno/40 bg-sayno/10 p-3">
              <TriangleAlert size={16} className="shrink-0 text-sayno" />
              <span className="text-sayno">{t("history.deleteConfirm", { count: shown.length })}</span>
              <Button className="ml-auto" disabled={deleting === "busy"} onClick={() => setDeleting(null)}>
                {t("actions.cancel")}
              </Button>
              <Button variant="danger" disabled={deleting === "busy"} onClick={() => void deleteShown()}>
                {t("history.deleteNow")}
              </Button>
            </div>
          )}
          {deleted && "count" in deleted && (
            <p className="w-full text-xs text-ash">{t("history.deletedCount", { count: deleted.count })}</p>
          )}
          {deleted && "error" in deleted && (
            <p className="w-full text-xs text-sayno">
              {t(`errors.${deleted.error.code}`, { defaultValue: deleted.error.code })}
            </p>
          )}
        </div>
      )}
      {entries === null && <p className="text-ash">{t("history.loading")}</p>}
      {listError === "PermissionDenied" && (
        <div className="rounded-lg border border-blocked/40 bg-blocked/10 p-4 text-sm">
          <p className="font-bold text-blocked">{t("history.noAccessTitle")}</p>
          <p className="mt-1">{t("history.noAccessHint")}</p>
          <p className="mt-2">
            <Mono>sudo usermod -aG oblivio $USER</Mono>
          </p>
        </div>
      )}
      {listError !== null && listError !== "PermissionDenied" && <p className="text-sayno">{listError}</p>}
      {listError === null && entries?.length === 0 && <p className="text-ash">{t("history.empty")}</p>}
      {entries && entries.length > 0 && shown.length === 0 && <p className="text-ash">{t("history.noMatch")}</p>}

      <ul className="space-y-3">
        {shown.map((entry) => {
          const summary = entry.summary;
          if (entry.check !== "Valid" || !summary) {
            return (
              <li key={entry.id} className="flex items-start gap-3 rounded-lg border border-sayno/40 bg-sayno/5 p-4">
                <ShieldAlert size={18} className="mt-0.5 shrink-0 text-sayno" />
                <div className="text-sm">
                  <p className="font-bold text-sayno">{t(`history.check.${entry.check}`)}</p>
                  <p className="mt-0.5 text-ash">
                    <Mono>{entry.id}</Mono>
                  </p>
                </div>
              </li>
            );
          }
          const tone = summary.status === "CONFORME" ? "ready" : "sayno";
          return (
            <li key={entry.id} className="rounded-lg border border-edge bg-card p-4">
              <div className="flex flex-wrap items-center gap-3">
                <Badge tone={tone}>{t(`job.status.${summary.status}`)}</Badge>
                {summary.category !== "None" && (
                  <Badge tone={summary.category === "Purge" ? "ready" : "neutral"}>{summary.category}</Badge>
                )}
                <span className="font-bold">{summary.model}</span>
                <span className="text-sm text-ash">{capacity(summary.capacity_bytes, i18n.language, "")}</span>
                <span className="ml-auto text-sm text-ash">{date.format(new Date(summary.issued_at * 1000))}</span>
              </div>
              <p className="mt-1.5 text-sm text-ash">
                S/N <Mono>{summary.serial}</Mono>
                {summary.method && <> · {label(summary.method)}</>}
                {summary.batch_id && (
                  <>
                    {" · "}
                    {t("verify.field.batch")} <Mono>{summary.batch_id}</Mono>
                    {summary.batch_label && <> ({summary.batch_label})</>}
                  </>
                )}
                {(summary.operator || summary.organization) && (
                  <> · {[summary.operator, summary.organization].filter(Boolean).join(", ")}</>
                )}
              </p>
              <div className="mt-3 flex flex-wrap items-center gap-3">
                <span className="text-xs text-ash">
                  <Mono>{entry.id}</Mono>
                </span>
                <div className="ml-auto flex items-center gap-2">
                  {saved[entry.id] && (
                    <Button onClick={() => void api.openCertificate(saved[entry.id])}>{t("job.open")}</Button>
                  )}
                  <Button
                    variant="primary"
                    className="flex items-center gap-2"
                    disabled={busy !== null}
                    onClick={() => void exportPdf(entry.id)}
                  >
                    <FileText size={14} />
                    {busy === entry.id ? t("job.exporting") : t("history.pdf")}
                  </Button>
                </div>
              </div>
              {saved[entry.id] && (
                <p className="mt-2 text-xs text-ash">
                  {t("job.saved")} <Mono>{saved[entry.id]}</Mono>
                </p>
              )}
              {error?.id === entry.id && (
                <p className="mt-2 text-sm text-sayno">
                  {t(`errors.${error.error.code}`, { defaultValue: error.error.code })}
                  {error.error.detail && <span className="block font-mono text-xs text-ash">{error.error.detail}</span>}
                </p>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
