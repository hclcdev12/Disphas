import type { ButtonHTMLAttributes, ReactNode } from "react";
import { ChevronLeft } from "lucide-react";

type Tone = "ready" | "sayno" | "blocked" | "protected" | "brand" | "neutral";

/** Status chips are filled; every other control is outlined (docs/maquettes/disphas-ecran-disques.md §2). */
const toneClasses: Record<Tone, string> = {
  ready: "border-ready-fill bg-ready-fill text-white",
  sayno: "border-sayno-fill bg-sayno-fill text-white",
  blocked: "border-blocked bg-blocked text-night",
  protected: "border-ctrl text-ash",
  brand: "border-ctrl text-soft",
  neutral: "border-ctrl text-ash",
};

export function Badge({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-[3px] border px-2 py-0.5 font-mono text-xs whitespace-nowrap ${toneClasses[tone]}`}
    >
      {children}
    </span>
  );
}

export function Dot({ tone }: { tone: Tone }) {
  const color = {
    ready: "bg-ready",
    sayno: "bg-sayno",
    blocked: "bg-blocked",
    protected: "bg-protected",
    brand: "bg-brand",
    neutral: "bg-ash",
  }[tone];
  return <span className={`h-2 w-2 rounded-full ${color}`} />;
}

type Variant = "primary" | "danger" | "blocked" | "ghost";

/** Buttons are never filled with colour: a 2 px outline, text in the colour of the action. */
const variantClasses: Record<Variant, string> = {
  primary: "border-2 border-soft text-soft hover:bg-card disabled:opacity-40",
  danger: "border-2 border-sayno text-sayno hover:bg-card disabled:opacity-40",
  blocked: "border-2 border-blocked text-blocked hover:bg-card disabled:opacity-40",
  ghost: "border border-ctrl text-soft hover:bg-card disabled:opacity-40",
};

export function Button({
  variant = "ghost",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      type="button"
      className={`min-h-11 rounded px-3.5 py-2 text-sm font-bold transition-colors disabled:cursor-not-allowed ${variantClasses[variant]} ${className}`}
      {...props}
    />
  );
}

export function Mono({ children }: { children: ReactNode }) {
  return <span className="font-mono text-[0.85em] text-soft">{children}</span>;
}

/** A progress bar: bone on a grey track, no colour (docs/maquettes/disphas-ecran-disques.md §2). */
export function Bar({ percent, className = "h-1" }: { percent: number; className?: string }) {
  return (
    <span className={`block overflow-hidden rounded-full bg-edge ${className}`} aria-hidden="true">
      <span className="block h-full bg-soft transition-all duration-300" style={{ width: `${percent}%` }} />
    </span>
  );
}

/** A square button showing only an icon; `label` is read aloud and shown on hover. */
export function IconButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className="inline-flex size-11 shrink-0 items-center justify-center rounded text-soft hover:bg-card disabled:opacity-40"
    >
      {children}
    </button>
  );
}

/** Phone width: goes back from the detail to the list (hidden above 760 px). */
export function BackButton({ label, children, onClick }: { label: string; children: ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      className="split-back min-h-11 items-center gap-2 self-start rounded pr-3 pl-1"
    >
      <ChevronLeft size={18} aria-hidden="true" />
      {children}
    </button>
  );
}

/** One line of facts: a label, then its value. */
export function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[minmax(0,11rem)_minmax(0,1fr)] gap-3 border-b border-edge py-1.5 text-sm last:border-0">
      <dt className="text-ash">{label}</dt>
      <dd className="min-w-0 break-words">{children}</dd>
    </div>
  );
}
