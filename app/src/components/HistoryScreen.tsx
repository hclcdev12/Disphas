import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileText, RefreshCw, Search, ShieldAlert } from "lucide-react";
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
  const [query, setQuery] = useState("");
  const [saved, setSaved] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<{ id: string; error: HelperError } | null>(null);

  const load = useCallback(async () => {
    setEntries(null);
    setEntries(await api.listReports());
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const shown = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!entries || needle === "") return entries ?? [];
    return entries.filter((entry) =>
      [entry.id, entry.summary?.model, entry.summary?.serial, entry.summary?.operator, entry.summary?.organization]
        .filter((field): field is string => typeof field === "string")
        .some((field) => field.toLowerCase().includes(needle)),
    );
  }, [entries, query]);

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
        <Button className="flex items-center gap-2" onClick={() => void load()}>
          <RefreshCw size={14} />
          {t("drives.refresh")}
        </Button>
      </div>

      {entries === null && <p className="text-ash">{t("history.loading")}</p>}
      {entries?.length === 0 && <p className="text-ash">{t("history.empty")}</p>}
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
