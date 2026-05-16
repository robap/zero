// lib/format.ts — pure formatting helpers. No UI, no state mutation.
// The canonical home for cross-cutting helpers; if a helper grows a
// dependency on a store, move it into that store instead.

/**
 * Render an ISO-8601 timestamp as a compact, locale-independent string.
 * Example: `"2026-04-12T11:42:00Z"` → `"2026-04-12 11:42"`.
 *
 * @param iso ISO-8601 timestamp string.
 * @returns Formatted "YYYY-MM-DD HH:MM" string, or the input if unparseable.
 */
export function formatDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const pad = (n: number): string => (n < 10 ? `0${n}` : `${n}`);
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())} ${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}`;
}

/**
 * Human-friendly issue status label.
 *
 * @param status One of `"open"` / `"closed"`.
 * @returns Capitalized label.
 */
export function statusLabel(status: "open" | "closed"): string {
  return status === "open" ? "Open" : "Closed";
}
