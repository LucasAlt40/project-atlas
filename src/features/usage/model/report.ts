import type { UsageRecordDto, UsageReportDto } from '@/lib/tauri/commands';

/**
 * The usage screen's numbers, from the executions Atlas observed. Pure: the screen asks, this
 * answers. A value no execution reported stays `null` (never zero), and every sum says how many
 * executions it rests on, so a partial number is never shown as a complete one.
 */

export type Period = 'today' | 'week' | 'month' | 'all';
export type Dimension = 'agent' | 'model' | 'workspace' | 'runtime';
export type Metric = 'tokens' | 'cost';

const DAY_MS = 24 * 60 * 60 * 1000;
/** The ledger keeps 100 days (`application::usage::RETENTION_MS`). */
export const RETENTION_DAYS = 100;

function startOfDay(ms: number): number {
  const date = new Date(ms);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

/** Where a period starts, in local days: today, the last 7 days, the last 30, all kept. */
export function periodStart(period: Period, now: number): number {
  const today = startOfDay(now);
  const days = { today: 0, week: 6, month: 29, all: RETENTION_DAYS - 1 }[period];
  return startOfDay(today - days * DAY_MS + DAY_MS / 2);
}

export interface Totals {
  runs: number;
  failed: number;
  runsWithTokens: number;
  runsWithCost: number;
  input: number | null;
  output: number | null;
  /** Of `input`, what the provider served from its cache (when the runtime says). */
  cached: number | null;
  /** Input of the executions that reported their cache, so `cached / cachedOf` is a fair share. */
  cachedOf: number;
  total: number | null;
  cost: number | null;
  currency: string | null;
  /** Several currencies were reported: the sum is not shown. */
  mixedCurrency: boolean;
  lastAt: number | null;
}

function add(sum: number | null, value: number | null | undefined): number | null {
  return value === null || value === undefined ? sum : (sum ?? 0) + value;
}

export function totals(records: readonly UsageRecordDto[]): Totals {
  const t: Totals = {
    runs: 0,
    failed: 0,
    runsWithTokens: 0,
    runsWithCost: 0,
    input: null,
    output: null,
    cached: null,
    cachedOf: 0,
    total: null,
    cost: null,
    currency: null,
    mixedCurrency: false,
    lastAt: null,
  };
  for (const r of records) {
    t.runs += 1;
    if (!r.succeeded) t.failed += 1;
    t.lastAt = Math.max(t.lastAt ?? 0, r.completedAt);
    const m = r.metrics;
    if (!m) continue;
    const total = m.totalTokens ?? (m.inputTokens ?? 0) + (m.outputTokens ?? 0);
    if (m.totalTokens !== null || m.inputTokens !== null || m.outputTokens !== null) {
      t.runsWithTokens += 1;
      t.total = add(t.total, total);
    }
    t.input = add(t.input, m.inputTokens);
    t.output = add(t.output, m.outputTokens);
    if (m.cachedInputTokens !== null) {
      t.cached = add(t.cached, m.cachedInputTokens);
      t.cachedOf += m.inputTokens ?? 0;
    }
    if (m.cost !== null) {
      t.runsWithCost += 1;
      t.cost = add(t.cost, m.cost);
      if (t.currency !== null && m.currency !== null && t.currency !== m.currency) {
        t.mixedCurrency = true;
      }
      t.currency = t.currency ?? m.currency;
    }
  }
  return t;
}

/** The share of input tokens served from the provider's cache, or `null` when none was reported. */
export function cacheShare(t: Totals): number | null {
  return t.cached === null || t.cachedOf === 0 ? null : t.cached / t.cachedOf;
}

export interface Names {
  agents: Map<string, string>;
  workspaces: Map<string, string>;
  runtimes: Map<string, string>;
}

export function namesOf(report: UsageReportDto): Names {
  const map = (labels: { id: string; name: string }[]) =>
    new Map(labels.map((l) => [l.id, l.name]));
  return {
    agents: map(report.agents),
    workspaces: map(report.workspaces),
    runtimes: map(report.runtimes),
  };
}

export function runtimeName(names: Names, runtimeId: string): string {
  return names.runtimes.get(runtimeId) ?? runtimeId;
}

/** What a record is grouped under, for each dimension. */
export function keyOf(record: UsageRecordDto, by: Dimension): string {
  switch (by) {
    case 'agent':
      return record.agentId;
    case 'model':
      return `${record.runtimeId}\u0000${record.modelId}`;
    case 'workspace':
      return record.workspaceId;
    case 'runtime':
      return record.runtimeId;
  }
}

export interface GroupLabel {
  label: string;
  /** An agent or workspace that no longer exists: its executions are still counted. */
  removed: boolean;
  /** A second line: the runtime and model of an agent, the runtime of a model. */
  detail: string | null;
}

export function labelOf(
  key: string,
  by: Dimension,
  names: Names,
  sample?: UsageRecordDto,
  models = 1,
): GroupLabel {
  switch (by) {
    case 'agent': {
      const name = names.agents.get(key);
      // The latest model; an agent whose runtime or model changed says how many more it used.
      const more = models > 1 ? ` +${String(models - 1)}` : '';
      const detail = sample
        ? `${runtimeName(names, sample.runtimeId)} · ${sample.modelId}${more}`
        : null;
      return { label: name ?? key, removed: name === undefined, detail };
    }
    case 'model': {
      const [runtimeId = '', modelId = ''] = key.split('\u0000');
      return { label: modelId, removed: false, detail: runtimeName(names, runtimeId) };
    }
    case 'workspace': {
      const name = names.workspaces.get(key);
      return { label: name ?? key, removed: name === undefined, detail: null };
    }
    case 'runtime':
      return { label: runtimeName(names, key), removed: false, detail: null };
  }
}

export interface Group extends GroupLabel {
  key: string;
  totals: Totals;
  /** Of the period's total tokens (or cost), what this group used: 0 to 1. */
  share: number;
  records: UsageRecordDto[];
}

export function value(t: Totals, metric: Metric): number {
  return (metric === 'tokens' ? t.total : t.cost) ?? 0;
}

/** One group per agent, model, workspace or runtime, the largest first. */
export function groupBy(
  records: readonly UsageRecordDto[],
  by: Dimension,
  names: Names,
  metric: Metric = 'tokens',
): Group[] {
  const buckets = new Map<string, UsageRecordDto[]>();
  for (const r of records) {
    const key = keyOf(r, by);
    const list = buckets.get(key);
    if (list) list.push(r);
    else buckets.set(key, [r]);
  }
  const whole = value(totals(records), metric);
  return [...buckets.entries()]
    .map(([key, list]) => {
      const t = totals(list);
      const latest = list.reduce((a, b) => (b.completedAt > a.completedAt ? b : a));
      const models = new Set(list.map((r) => keyOf(r, 'model'))).size;
      return {
        key,
        ...labelOf(key, by, names, latest, models),
        totals: t,
        share: whole > 0 ? value(t, metric) / whole : 0,
        records: list,
      };
    })
    .sort(
      (a, b) =>
        value(b.totals, metric) - value(a.totals, metric) ||
        b.totals.runs - a.totals.runs ||
        a.label.localeCompare(b.label),
    );
}

export interface Series {
  key: string;
  label: string;
  /** 0-based slot in the chart palette; `null` for the folded "others". */
  slot: number | null;
}

export interface Day {
  start: number;
  /** Per series key, the day's tokens (or cost). */
  values: Map<string, number>;
  total: number;
  runs: number;
}

/** At most this many series get their own color; the rest fold into "others". */
export const MAX_SERIES = 5;
export const OTHERS = '\u0000others';

/**
 * One bar per local day from `since` to `now`, stacked by `by` (the largest series first). Series
 * keep their color whatever the filter: the slot follows the series' rank in the whole period.
 */
export function daily(
  records: readonly UsageRecordDto[],
  since: number,
  now: number,
  by: Dimension,
  names: Names,
  metric: Metric,
): { days: Day[]; series: Series[] } {
  const ranked = groupBy(records, by, names, metric).filter((g) => value(g.totals, metric) > 0);
  const own = ranked.slice(0, MAX_SERIES);
  const series: Series[] = own.map((g, i) => ({
    key: g.key,
    label: g.detail && by === 'model' ? `${g.label} (${g.detail})` : g.label,
    slot: i,
  }));
  if (ranked.length > MAX_SERIES) series.push({ key: OTHERS, label: '', slot: null });
  const ownKeys = new Set(own.map((g) => g.key));

  const days: Day[] = [];
  for (
    let start = startOfDay(since);
    start <= now;
    start = startOfDay(start + DAY_MS + DAY_MS / 2)
  ) {
    days.push({ start, values: new Map(), total: 0, runs: 0 });
  }
  for (const r of records) {
    const day = days.findLast((d) => d.start <= r.completedAt);
    if (!day) continue;
    day.runs += 1;
    const t = totals([r]);
    const v = value(t, metric);
    if (v === 0) continue;
    const key = ownKeys.has(keyOf(r, by)) ? keyOf(r, by) : OTHERS;
    day.values.set(key, (day.values.get(key) ?? 0) + v);
    day.total += v;
  }
  return { days, series };
}
