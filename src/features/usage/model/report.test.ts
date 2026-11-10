import type { UsageMetricsDto, UsageRecordDto, UsageReportDto } from '@/lib/tauri/commands';
import { cacheShare, daily, groupBy, namesOf, OTHERS, periodStart, totals, value } from './report';

const DAY = 24 * 60 * 60 * 1000;
const NOW = new Date(2026, 10, 10, 15, 0).getTime();

function metrics(
  input: number,
  output: number,
  extra: Partial<UsageMetricsDto> = {},
): UsageMetricsDto {
  return {
    inputTokens: input,
    outputTokens: output,
    totalTokens: input + output,
    cachedInputTokens: null,
    cost: null,
    currency: null,
    source: 'runtime_reported',
    ...extra,
  };
}

function run(over: Partial<UsageRecordDto> = {}): UsageRecordDto {
  return {
    executionId: Math.random().toString(36),
    workspaceId: 'w1',
    agentId: 'a1',
    runtimeId: 'claude',
    modelId: 'sonnet',
    startedAt: NOW - 1000,
    completedAt: NOW,
    succeeded: true,
    metrics: metrics(100, 10),
    ...over,
  };
}

const report = (records: UsageRecordDto[]): UsageReportDto => ({
  connected: [],
  records,
  agents: [
    { id: 'a1', name: 'Backend' },
    { id: 'a2', name: 'Reviewer' },
  ],
  workspaces: [{ id: 'w1', name: 'Atlas' }],
  runtimes: [
    { id: 'claude', name: 'Claude Code' },
    { id: 'codex', name: 'Codex' },
  ],
  quotas: {},
});

describe('usage totals', () => {
  it('keeps what no execution reported as unknown, never zero', () => {
    const t = totals([
      run({ metrics: null }),
      run({ metrics: metrics(5, 1, { totalTokens: null }) }),
    ]);

    expect(t.runs).toBe(2);
    expect(t.runsWithTokens).toBe(1);
    expect(t.total).toBe(6);
    expect(t.cost).toBeNull();
    expect(t.runsWithCost).toBe(0);
  });

  it('adds up cost only from the executions that reported one, and counts them', () => {
    const t = totals([
      run({ metrics: metrics(100, 10, { cost: 0.25, currency: 'USD' }) }),
      run({ runtimeId: 'codex', metrics: metrics(300, 30) }),
      run({ succeeded: false, metrics: metrics(50, 5, { cost: 0.05, currency: 'USD' }) }),
    ]);

    expect(t.cost).toBeCloseTo(0.3);
    expect(t.runsWithCost).toBe(2);
    expect(t.currency).toBe('USD');
    expect(t.failed).toBe(1);
    expect(t.total).toBe(495);
  });

  it('measures the cache share only over the executions that reported their cache', () => {
    const t = totals([
      run({ metrics: metrics(1000, 10, { cachedInputTokens: 800 }) }),
      run({ metrics: metrics(9000, 10) }),
    ]);

    expect(cacheShare(t)).toBeCloseTo(0.8);
    expect(cacheShare(totals([run()]))).toBeNull();
  });
});

describe('usage groups', () => {
  const names = namesOf(report([]));

  it('puts the largest first, with its share, and keeps a removed agent under its id', () => {
    const records = [
      run({ agentId: 'a1', metrics: metrics(100, 0) }),
      run({ agentId: 'a2', metrics: metrics(300, 0) }),
      run({ agentId: 'gone', metrics: metrics(600, 0) }),
    ];

    const groups = groupBy(records, 'agent', names);

    expect(groups.map((g) => g.label)).toEqual(['gone', 'Reviewer', 'Backend']);
    expect(groups[0]?.removed).toBe(true);
    expect(groups[0]?.share).toBeCloseTo(0.6);
    expect(groups[2]?.detail).toBe('Claude Code · sonnet');
  });

  it('tells models apart by runtime as well as by name', () => {
    const groups = groupBy(
      [run({ runtimeId: 'claude', modelId: 'auto' }), run({ runtimeId: 'codex', modelId: 'auto' })],
      'model',
      names,
    );

    expect(groups.map((g) => `${g.label} / ${g.detail ?? ''}`).sort()).toEqual([
      'auto / Claude Code',
      'auto / Codex',
    ]);
  });
});

describe('usage over time', () => {
  const names = namesOf(report([]));

  it('has one bar per day of the period, and the bars add up to the total', () => {
    const since = periodStart('week', NOW);
    const records = [
      run({ completedAt: NOW, metrics: metrics(100, 0) }),
      run({ completedAt: NOW - 2 * DAY, metrics: metrics(50, 0) }),
      run({ completedAt: NOW - 2 * DAY, modelId: 'opus', metrics: metrics(25, 0) }),
    ];

    const { days, series } = daily(records, since, NOW, 'model', names, 'tokens');

    expect(days).toHaveLength(7);
    expect(days.reduce((s, d) => s + d.total, 0)).toBe(value(totals(records), 'tokens'));
    expect(days.at(-1)?.total).toBe(100);
    expect(days.at(-3)?.runs).toBe(2);
    expect(series.map((s) => s.slot)).toEqual([0, 1]);
  });

  it('folds what is beyond the fifth series into one "others" bar', () => {
    const records = ['m1', 'm2', 'm3', 'm4', 'm5', 'm6', 'm7'].map((modelId, i) =>
      run({ modelId, metrics: metrics(100 - i, 0) }),
    );

    const { days, series } = daily(
      records,
      periodStart('today', NOW),
      NOW,
      'model',
      names,
      'tokens',
    );

    expect(series).toHaveLength(6);
    expect(series.at(-1)).toEqual({ key: OTHERS, label: '', slot: null });
    expect(days[0]?.values.get(OTHERS)).toBe(94 + 95);
  });

  it('starts a period at local midnight', () => {
    const start = new Date(periodStart('today', NOW));
    expect([start.getHours(), start.getMinutes(), start.getDate()]).toEqual([0, 0, 10]);
    expect(new Date(periodStart('week', NOW)).getDate()).toBe(4);
  });
});

describe('an agent that changed model', () => {
  it('names its latest model and how many others it used', () => {
    const names = namesOf(report([]));
    const [agent] = groupBy(
      [
        run({ runtimeId: 'codex', modelId: 'gpt-5.6-luna', completedAt: NOW - DAY }),
        run({ runtimeId: 'claude', modelId: 'sonnet', completedAt: NOW }),
      ],
      'agent',
      names,
    );
    expect(agent?.detail).toBe('Claude Code · sonnet +1');
  });
});
