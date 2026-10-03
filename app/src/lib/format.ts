/** Capacity in decimal gigabytes or terabytes, in the UI language. */
export function capacity(bytes: number | null, lang: string, unknown: string): string {
  if (bytes === null) return unknown;
  const format = new Intl.NumberFormat(lang, { maximumFractionDigits: 1 });
  return bytes >= 1e12 ? `${format.format(bytes / 1e12)} TB` : `${format.format(bytes / 1e9)} GB`;
}
