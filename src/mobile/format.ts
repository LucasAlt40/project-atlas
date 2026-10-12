/** Formatting for a small screen. Times are the computer's clock, corrected by `offset`. */

/** "42s", "3m 12s", "1h 05m": how long something has taken. Never negative. */
export function duration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  if (total < 60) return `${String(total)}s`;
  const minutes = Math.floor(total / 60);
  if (minutes < 60) return `${String(minutes)}m ${String(total % 60).padStart(2, '0')}s`;
  const hours = Math.floor(minutes / 60);
  return `${String(hours)}h ${String(minutes % 60).padStart(2, '0')}m`;
}

/** The time of day (or the date as well, when it was not today) of a moment. */
export function clock(at: number, language: string, now = Date.now()): string {
  const date = new Date(at);
  const sameDay = new Date(now).toDateString() === date.toDateString();
  return new Intl.DateTimeFormat(language, {
    hour: '2-digit',
    minute: '2-digit',
    ...(sameDay ? {} : { day: '2-digit', month: 'short' }),
  }).format(date);
}

/** The code a person scanned or typed, as the page keeps it: the 16 characters, upper case. */
export function pairingCodeFrom(text: string): string {
  return text.replace(/[^A-Za-z2-7]/g, '').toUpperCase();
}

/** A name for this phone, from what the browser says it is. */
export function deviceNameFromAgent(agent: string): string {
  if (agent.includes('iPhone')) return 'iPhone';
  if (agent.includes('iPad')) return 'iPad';
  const android = /Android[^;]*;\s*([^;)]+)/.exec(agent);
  if (android?.[1] && !/^(K|Mobile)$/.test(android[1].trim())) return android[1].trim();
  if (agent.includes('Android')) return 'Android';
  return 'Phone';
}
