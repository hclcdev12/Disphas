// The batch the operator is filling: every Sanitize carries it, and the
// helper signs it into each attestation (docs/certificat.md §6.1).
import type { BatchArgs } from "../bindings/BatchArgs";

const KEY = "disphas.currentBatch";

/** A new batch identifier: LOT-YYYYMMDD-XXXX (matches the attestation bounds). */
export function newBatchId(now = new Date()): string {
  const date = now.toISOString().slice(0, 10).replaceAll("-", "");
  const random = crypto.getRandomValues(new Uint8Array(2));
  const suffix = Array.from(random, (byte) => byte.toString(16).padStart(2, "0")).join("").toUpperCase();
  return `LOT-${date}-${suffix}`;
}

/** The batch kept from the previous session, if any (convenience only). */
export function loadBatch(): BatchArgs | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const value = JSON.parse(raw) as BatchArgs;
    return typeof value.id === "string" ? { id: value.id, label: value.label ?? null } : null;
  } catch {
    return null;
  }
}

export function saveBatch(batch: BatchArgs | null): void {
  try {
    if (batch) localStorage.setItem(KEY, JSON.stringify(batch));
    else localStorage.removeItem(KEY);
  } catch {
    // Storage unavailable: the batch simply lasts for this session.
  }
}
