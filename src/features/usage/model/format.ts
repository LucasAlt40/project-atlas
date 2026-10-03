/** Number formatting for usage, in the interface language. */

export function formatTokens(tokens: number, language: string): string {
  return new Intl.NumberFormat(language, { notation: 'compact', maximumFractionDigits: 1 }).format(
    tokens,
  );
}

/**
 * A cost with its currency when the runtime said which one it is; otherwise the bare number (no
 * currency is invented). Amounts under one unit keep up to four decimals, so a run costing a cent and a half is not rounded away.
 */
export function formatCost(cost: number, currency: string | null, language: string): string {
  const options = { minimumFractionDigits: 2, maximumFractionDigits: Math.abs(cost) < 1 ? 4 : 2 };
  return currency
    ? new Intl.NumberFormat(language, { style: 'currency', currency, ...options }).format(cost)
    : new Intl.NumberFormat(language, options).format(cost);
}

/** "12s", "1m 05s", "1h 02m". */
export function formatDuration(milliseconds: number): string {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  if (seconds < 60) return `${String(seconds)}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${String(minutes)}m ${String(seconds % 60).padStart(2, '0')}s`;
  return `${String(Math.floor(minutes / 60))}h ${String(minutes % 60).padStart(2, '0')}m`;
}

/** When a quota window resets, relative to now ("in 2 hours"). */
export function formatReset(resetsAtSeconds: number, nowMs: number, language: string): string {
  const minutes = Math.round((resetsAtSeconds * 1000 - nowMs) / 60000);
  const formatter = new Intl.RelativeTimeFormat(language, { numeric: 'auto' });
  if (Math.abs(minutes) < 60) return formatter.format(minutes, 'minute');
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 48) return formatter.format(hours, 'hour');
  return formatter.format(Math.round(hours / 24), 'day');
}
