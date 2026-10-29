import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, ChevronDown, UserRound, X } from "lucide-react";
import { api, type Settings } from "../lib/api";

/** Same bound as the helper and app/src-tauri/src/settings.rs. */
const MAX_OPERATOR = 64;

/**
 * Quick operator switch in the header (UX review P2), for workshops where
 * several people share a workstation: the operators who signed here, the
 * current one ticked, and a field for another. The next erasures are signed
 * with the chosen name; the running ones keep theirs. Built like the method
 * menu: arrows, Escape, a click outside closes.
 */
export function OperatorMenu({
  settings,
  onChanged,
}: {
  settings: Settings;
  onChanged: (settings: Settings) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [other, setOther] = useState("");
  const [error, setError] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const field = useRef<HTMLInputElement>(null);
  const current = settings.operator;
  // The current operator first, then the others signed here.
  const names = [...new Set([...(current ? [current] : []), ...settings.recent_operators])];

  useEffect(() => {
    if (!open) return;
    const first = root.current?.querySelector<HTMLElement>('[role="menuitemradio"][aria-checked="true"]');
    (first ?? field.current)?.focus();
    const outside = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", outside);
    return () => document.removeEventListener("mousedown", outside);
  }, [open]);

  const close = () => {
    setOpen(false);
    setOther("");
    setError(null);
    // Next frame: the Enter that chose a name must not press the button too.
    requestAnimationFrame(() => trigger.current?.focus());
  };

  const choose = async (name: string) => {
    setError(null);
    const result = await api.setOperator(name);
    if (!result.ok) {
      setError(result.error.detail ?? result.error.code);
      return;
    }
    onChanged(result.value);
    close();
  };

  const forget = async (name: string) => {
    const result = await api.forgetOperator(name);
    if (result.ok) onChanged(result.value);
  };

  // Up and Down go through the names and the field, Escape closes the menu only.
  const onKey = (event: React.KeyboardEvent) => {
    if (!open) return;
    const stops = [
      ...(root.current?.querySelectorAll<HTMLElement>('[role="menuitemradio"]') ?? []),
      ...(field.current ? [field.current] : []),
    ];
    const at = stops.findIndex((stop) => stop === document.activeElement);
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      stops[(at + step + stops.length) % stops.length]?.focus();
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (event.key === "Delete" && document.activeElement instanceof HTMLElement) {
      // Delete on a name forgets it, as the cross does with the mouse.
      const name = document.activeElement.dataset.operator;
      if (name && name !== current) {
        event.preventDefault();
        stops[at + 1]?.focus();
        void forget(name);
      }
    } else if (event.key === "Tab") {
      setOpen(false);
    }
  };

  const tooLong = [...other.trim()].length > MAX_OPERATOR;

  return (
    <div ref={root} className="relative" onKeyDown={onKey}>
      <button
        ref={trigger}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`${t("operator.switchLabel")} : ${current ?? t("operator.none")}`}
        title={t("operator.switchLabel")}
        onClick={() => (open ? close() : setOpen(true))}
        className={`flex min-h-11 max-w-56 items-center gap-2 rounded-md border px-2.5 hover:border-ctrl hover:bg-card ${
          open ? "border-ctrl bg-card" : "border-transparent"
        }`}
      >
        <UserRound size={16} className="shrink-0 text-ash" aria-hidden="true" />
        <span className={`truncate ${current ? "" : "text-blocked"}`}>{current ?? t("operator.none")}</span>
        <ChevronDown
          size={14}
          className={`shrink-0 text-ash transition-transform ${open ? "rotate-180" : ""}`}
          aria-hidden="true"
        />
      </button>
      {open && (
        <div
          role="menu"
          aria-label={t("operator.switchLabel")}
          className="absolute top-full right-0 z-30 mt-1 flex w-72 max-w-[85vw] flex-col gap-0.5 rounded-lg border border-ctrl bg-card p-1.5 shadow-xl shadow-black/50"
        >
          <p className="px-2.5 pt-1 pb-1.5 text-xs text-ash">{t("operator.hint")}</p>
          {names.map((name) => {
            const checked = name === current;
            return (
              <div key={name} className="group flex items-center rounded-md hover:bg-night focus-within:bg-night">
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={checked}
                  tabIndex={-1}
                  data-operator={name}
                  onClick={() => void choose(name)}
                  className="flex min-h-11 min-w-0 flex-1 items-center gap-3 px-2.5 text-left"
                >
                  <span className="flex w-4 shrink-0 justify-center" aria-hidden="true">
                    {checked && <Check size={16} />}
                  </span>
                  <span className={`truncate ${checked ? "font-bold" : ""}`}>{name}</span>
                </button>
                {/* The one signing now cannot be forgotten. */}
                {!checked && (
                  <button
                    type="button"
                    tabIndex={-1}
                    aria-label={t("operator.forget", { name })}
                    title={t("operator.forget", { name })}
                    onClick={() => void forget(name)}
                    className="mr-1 inline-flex size-8 shrink-0 items-center justify-center rounded text-ash opacity-0 group-hover:opacity-100 hover:text-soft"
                  >
                    <X size={14} aria-hidden="true" />
                  </button>
                )}
              </div>
            );
          })}
          <label className="mt-1 flex flex-col gap-1 border-t border-edge px-2.5 pt-2 pb-1 text-xs font-bold text-ash">
            {t("operator.other")}
            <input
              ref={field}
              className="block min-h-10 w-full rounded border border-ctrl bg-night px-2.5 text-sm font-normal text-soft"
              placeholder={t("operator.otherPlaceholder")}
              autoComplete="off"
              spellCheck={false}
              value={other}
              onChange={(event) => {
                setOther(event.target.value);
                setError(null);
              }}
              onKeyDown={(event) => {
                if (event.key !== "Enter") return;
                event.preventDefault();
                if (other.trim() !== "" && !tooLong) void choose(other.trim());
              }}
            />
          </label>
          {(tooLong || error) && (
            <p role="alert" className="px-2.5 pb-1 text-xs text-sayno">
              {tooLong ? t("operator.tooLong") : error}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
