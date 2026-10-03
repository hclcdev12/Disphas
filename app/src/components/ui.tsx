import type { ButtonHTMLAttributes, ReactNode } from "react";

type Tone = "ready" | "sayno" | "blocked" | "protected" | "brand" | "neutral";

const toneClasses: Record<Tone, string> = {
  ready: "border-ready/40 bg-ready/10 text-ready",
  sayno: "border-sayno/40 bg-sayno/10 text-sayno",
  blocked: "border-blocked/40 bg-blocked/10 text-blocked",
  protected: "border-protected/50 bg-protected/15 text-ash",
  brand: "border-brand/40 bg-brand/10 text-brand",
  neutral: "border-edge bg-night text-ash",
};

export function Badge({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 text-xs font-bold tracking-wide ${toneClasses[tone]}`}
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

type Variant = "primary" | "danger" | "ghost";

const variantClasses: Record<Variant, string> = {
  // Indigo is reserved for main actions and safety confirmations (UI spec §5).
  primary: "bg-brand text-white hover:bg-brand/85 disabled:bg-brand/30",
  danger: "border border-sayno/60 text-sayno hover:bg-sayno/10 disabled:opacity-40",
  ghost: "border border-edge text-ash hover:text-soft hover:border-ash/40",
};

export function Button({
  variant = "ghost",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      type="button"
      className={`rounded-lg px-3.5 py-2 text-sm font-bold transition-colors disabled:cursor-not-allowed ${variantClasses[variant]} ${className}`}
      {...props}
    />
  );
}

export function Mono({ children }: { children: ReactNode }) {
  return <span className="font-mono text-[0.85em] text-soft">{children}</span>;
}
