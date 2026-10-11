// Visitor address and in-memory rate limits, shared by the forms.

/**
 * The visitor's address. Behind Caddy (TRUST_PROXY=1) the socket peer is the
 * proxy: the address is the last one Caddy added to X-Forwarded-For.
 * Without TRUST_PROXY the header is ignored, so a visitor cannot forge it.
 */
export function clientIp(request: Request, clientAddress: string): string {
  if (process.env.TRUST_PROXY === "1") {
    const forwarded = request.headers.get("x-forwarded-for")?.split(",").at(-1)?.trim();
    if (forwarded) return forwarded;
  }
  return clientAddress;
}

/** At most `max` events per key in `windowMs`, counted in memory. */
export function limiter(max: number, windowMs: number) {
  const recent = new Map<string, number[]>();
  return (key: string, now = Date.now()): boolean => {
    const times = (recent.get(key) ?? []).filter((time) => now - time < windowMs);
    if (times.length >= max) {
      recent.set(key, times);
      return false;
    }
    times.push(now);
    recent.set(key, times);
    if (recent.size > 10_000) recent.clear();
    return true;
  };
}
