import { useCallback, useEffect, useMemo, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { Drawer } from '@/components/ui/Drawer';
import { Icon } from '@/components/ui/Icon';
import { PageHeader } from '@/components/ui/PageHeader';
import { SegmentedControl } from '@/components/ui/SegmentedControl';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { UsageRecordDto, UsageReportDto } from '@/lib/tauri/commands';
import { UsageChart, cx, seriesClass } from '../components/UsageChart';
import { formatCost, formatDuration, formatReset, formatTokens } from '../model/format';
import {
  cacheShare,
  daily,
  groupBy,
  labelOf,
  namesOf,
  periodStart,
  runtimeName,
  totals,
  value,
  type Dimension,
  type Group,
  type Metric,
  type Names,
  type Period,
  type Totals,
} from '../model/report';
import { getUsageReport } from '../services/usageService';
import styles from '../components/Usage.module.css';

type Load =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready'; report: UsageReportDto };

/** What the detail of one group is split by: an agent by its models, a model by its agents. */
const INNER: Record<Dimension, Dimension> = {
  agent: 'model',
  model: 'agent',
  workspace: 'agent',
  runtime: 'model',
};

/**
 * Usage: what every agent and model Atlas runs has consumed, all together and one by one. The
 * numbers are the ones the runtimes reported for executions Atlas observed; nothing is estimated.
 */
export function UsagePage() {
  const { t, language } = useI18n();
  const [load, setLoad] = useState<Load>({ status: 'loading' });
  const [period, setPeriod] = useState<Period>('month');
  const [workspace, setWorkspace] = useState<string>('all');
  const [metric, setMetric] = useState<Metric>('tokens');
  const [by, setBy] = useState<Dimension>('agent');
  const [open, setOpen] = useState<{ by: Dimension; key: string } | null>(null);
  const [now, setNow] = useState(() => Date.now());

  // Bumped by "Refresh"; every load also moves "now", so "today" stays today.
  const [reloads, setReloads] = useState(0);
  useEffect(() => {
    let cancelled = false;
    const at = Date.now();
    getUsageReport(periodStart('all', at))
      .then((report) => {
        if (cancelled) return;
        setNow(at);
        setLoad({ status: 'ready', report });
      })
      .catch((error: unknown) => {
        if (!cancelled) setLoad({ status: 'error', message: errorMessage(t, error) });
      });
    return () => {
      cancelled = true;
    };
  }, [reloads, t]);
  const refresh = useCallback(() => {
    setReloads((n) => n + 1);
  }, []);

  const report = load.status === 'ready' ? load.report : null;
  const names = useMemo(() => (report ? namesOf(report) : null), [report]);
  const since = periodStart(period, now);
  const records = useMemo(
    () =>
      (report?.records ?? []).filter(
        (r) => r.completedAt >= since && (workspace === 'all' || r.workspaceId === workspace),
      ),
    [report, since, workspace],
  );

  const fmt = useMemo(
    () => ({
      tokens: (v: number) => formatTokens(v, language),
      cost: (v: number) => formatCost(v, 'USD', language),
      value: (v: number) =>
        metric === 'tokens' ? formatTokens(v, language) : formatCost(v, 'USD', language),
    }),
    [language, metric],
  );

  const workspaceIds = useMemo(
    () => [...new Set((report?.records ?? []).map((r) => r.workspaceId))],
    [report],
  );

  return (
    <div className={styles.page}>
      <PageHeader
        title={t('usagePage.title')}
        description={t('usagePage.description')}
        actions={
          <Button variant="secondary" onClick={refresh}>
            <Icon name="refresh" size={14} />
            {t('usagePage.refresh')}
          </Button>
        }
      />

      <div className={styles.filters}>
        <SegmentedControl<Period>
          label={t('usagePage.period')}
          value={period}
          onChange={setPeriod}
          segments={(['today', 'week', 'month', 'all'] as const).map((p) => ({
            value: p,
            label: t(`usagePage.period.${p}`),
          }))}
        />
        <SegmentedControl<Metric>
          label={t('usagePage.metric')}
          value={metric}
          onChange={setMetric}
          segments={(['tokens', 'cost'] as const).map((m) => ({
            value: m,
            label: t(`usagePage.metric.${m}`),
          }))}
        />
        <label className={styles.select}>
          <span className={styles.srOnly}>{t('usagePage.workspace')}</span>
          <select
            value={workspace}
            onChange={(e) => {
              setWorkspace(e.target.value);
            }}
          >
            <option value="all">{t('usagePage.workspace.all')}</option>
            {names &&
              workspaceIds.map((id) => (
                <option key={id} value={id}>
                  {labelOf(id, 'workspace', names).label}
                </option>
              ))}
          </select>
        </label>
      </div>

      {report && names && <Remaining report={report} names={names} records={records} now={now} />}

      {load.status === 'loading' && (
        <p className={styles.muted} role="status">
          {t('usagePage.loading')}
        </p>
      )}
      {load.status === 'error' && (
        <Callout tone="danger">{t('usagePage.error', { message: load.message })}</Callout>
      )}
      {report && names && records.length === 0 && (
        <p className={styles.empty}>{t('usagePage.empty')}</p>
      )}
      {report && names && records.length > 0 && (
        <>
          <Kpis totals={totals(records)} />
          <Coverage totals={totals(records)} metric={metric} />

          <section className={styles.panel} aria-labelledby="usage-chart">
            <div className={styles.panelHead}>
              <h2 id="usage-chart" className={styles.heading}>
                {t('usagePage.chart.title')}
              </h2>
              <span className={styles.muted}>
                {t('usagePage.chart.by', { dimension: t(`usagePage.dimension.${by}`) })}
              </span>
            </div>
            <UsageChart
              {...daily(records, since, now, by, names, metric)}
              format={fmt.value}
              othersLabel={t('usagePage.chart.others')}
              label={t('usagePage.chart.aria', {
                metric: t(`usagePage.metric.${metric}`),
                dimension: t(`usagePage.dimension.${by}`),
              })}
            />
          </section>

          <section className={styles.panel} aria-labelledby="usage-by">
            <div className={styles.panelHead}>
              <h2 id="usage-by" className={styles.heading}>
                {t('usagePage.by')}
              </h2>
              <SegmentedControl<Dimension>
                label={t('usagePage.by')}
                value={by}
                onChange={setBy}
                segments={(['agent', 'model', 'workspace', 'runtime'] as const).map((d) => ({
                  value: d,
                  label: t(`usagePage.by.${d}`),
                }))}
              />
            </div>
            <GroupTable
              groups={groupBy(records, by, names, metric)}
              metric={metric}
              onOpen={(key) => {
                setOpen({ by, key });
              }}
            />
          </section>
        </>
      )}

      {open && names && (
        <Detail
          by={open.by}
          group={groupBy(records, open.by, names, metric).find((g) => g.key === open.key)}
          names={names}
          metric={metric}
          since={since}
          now={now}
          onClose={() => {
            setOpen(null);
          }}
        />
      )}
    </div>
  );
}

function Kpis({ totals: t }: { totals: Totals }) {
  const { t: tr, language } = useI18n();
  const tokens = (v: number | null) =>
    v === null ? tr('usagePage.notReported') : formatTokens(v, language);
  const share = cacheShare(t);
  const tiles = [
    {
      label: tr('usagePage.kpi.tokens'),
      value: tokens(t.total),
      detail: tr('usagePage.kpi.tokensDetail', {
        input: tokens(t.input),
        output: tokens(t.output),
      }),
    },
    {
      label: tr('usagePage.kpi.cost'),
      value:
        t.cost === null || t.mixedCurrency
          ? tr('usagePage.notReported')
          : formatCost(t.cost, t.currency, language),
      detail:
        t.cost === null
          ? tr('usagePage.kpi.costNone')
          : tr('usagePage.kpi.costDetail', { reported: t.runsWithCost, runs: t.runs }),
    },
    {
      label: tr('usagePage.kpi.runs'),
      value: String(t.runs),
      detail: tr('usagePage.kpi.runsDetail', { failed: t.failed }),
    },
    {
      label: tr('usagePage.kpi.average'),
      value:
        t.total === null || t.runsWithTokens === 0
          ? tr('usagePage.notReported')
          : tokens(t.total / t.runsWithTokens),
      detail: tr('usagePage.kpi.averageDetail'),
    },
    {
      label: tr('usagePage.kpi.cache'),
      value:
        share === null
          ? tr('usagePage.kpi.cacheNone')
          : new Intl.NumberFormat(language, { style: 'percent' }).format(share),
      detail: tr('usagePage.kpi.cacheDetail'),
    },
  ];
  return (
    <dl className={styles.kpis}>
      {tiles.map((tile) => (
        <div key={tile.label} className={styles.kpi}>
          <dt>{tile.label}</dt>
          <dd className={styles.kpiValue}>{tile.value}</dd>
          <dd className={styles.kpiDetail}>{tile.detail}</dd>
        </div>
      ))}
    </dl>
  );
}

/** Says, plainly, which part of the numbers is missing, so a sum is never read as complete. */
function Coverage({ totals: t, metric }: { totals: Totals; metric: Metric }) {
  const { t: tr } = useI18n();
  const missing = t.runs - t.runsWithTokens;
  const notes: string[] = [];
  if (missing > 0) notes.push(tr('usagePage.coverage', { missing, runs: t.runs }));
  if (metric === 'cost' || t.runsWithCost < t.runs) notes.push(tr('usagePage.costNote'));
  if (notes.length === 0) return null;
  return (
    <Callout tone="info">
      {notes.map((note) => (
        <p key={note} className={styles.note}>
          {note}
        </p>
      ))}
    </Callout>
  );
}

function GroupTable({
  groups,
  metric,
  onOpen,
}: {
  groups: Group[];
  metric: Metric;
  onOpen: (key: string) => void;
}) {
  const { t, language } = useI18n();
  const tokens = (v: number | null) =>
    v === null ? t('usagePage.notReported') : formatTokens(v, language);
  const percent = (v: number) => new Intl.NumberFormat(language, { style: 'percent' }).format(v);
  const when = (ms: number | null) =>
    ms === null
      ? t('usagePage.notReported')
      : new Intl.DateTimeFormat(language, {
          day: '2-digit',
          month: 'short',
          hour: '2-digit',
          minute: '2-digit',
        }).format(ms);
  return (
    <div className={styles.tableWrap}>
      <table className={styles.table}>
        <thead>
          <tr>
            <th scope="col">{t('usagePage.col.name')}</th>
            <th scope="col">{t('usagePage.col.share')}</th>
            <th scope="col" className={styles.num}>
              {t('usagePage.col.tokens')}
            </th>
            <th scope="col" className={styles.num}>
              {t('usagePage.col.inOut')}
            </th>
            <th scope="col" className={styles.num}>
              {t('usagePage.col.cache')}
            </th>
            <th scope="col" className={styles.num}>
              {t('usagePage.col.cost')}
            </th>
            <th scope="col" className={styles.num}>
              {t('usagePage.col.runs')}
            </th>
            <th scope="col">{t('usagePage.col.last')}</th>
          </tr>
        </thead>
        <tbody>
          {groups.map((g) => {
            const share = cacheShare(g.totals);
            return (
              <tr key={g.key}>
                <th scope="row">
                  <button
                    type="button"
                    className={styles.rowButton}
                    onClick={() => {
                      onOpen(g.key);
                    }}
                    aria-label={t('usagePage.open', { name: g.label })}
                  >
                    <span className={styles.rowName}>
                      {g.label}
                      {g.removed && <span className={styles.badge}>{t('usagePage.removed')}</span>}
                    </span>
                    {g.detail && <span className={styles.rowDetail}>{g.detail}</span>}
                  </button>
                </th>
                <td>
                  <span className={styles.share}>
                    <span className={styles.meter} aria-hidden="true">
                      <span
                        style={{
                          width: `${String(Math.max(g.share * 100, g.share > 0 ? 1.5 : 0))}%`,
                        }}
                      />
                    </span>
                    {percent(g.share)}
                  </span>
                </td>
                <td className={styles.num}>{tokens(g.totals.total)}</td>
                <td className={styles.num}>
                  {tokens(g.totals.input)} / {tokens(g.totals.output)}
                </td>
                <td className={styles.num}>
                  {share === null ? t('usagePage.notReported') : percent(share)}
                </td>
                <td className={styles.num}>
                  {g.totals.cost === null
                    ? t('usagePage.notReported')
                    : formatCost(g.totals.cost, g.totals.currency, language)}
                  {g.totals.cost !== null && g.totals.runsWithCost < g.totals.runs && (
                    <span className={styles.partial}>
                      {t('usagePage.col.costPartial', {
                        reported: g.totals.runsWithCost,
                        runs: g.totals.runs,
                      })}
                    </span>
                  )}
                </td>
                <td className={styles.num}>
                  {g.totals.runs}
                  {g.totals.failed > 0 && (
                    <span className={styles.failed}> · {g.totals.failed}✕</span>
                  )}
                </td>
                <td className={styles.when}>{when(g.totals.lastAt)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {metric === 'cost' && groups.every((g) => value(g.totals, 'cost') === 0) && (
        <p className={styles.muted}>{t('usagePage.kpi.costNone')}</p>
      )}
    </div>
  );
}

function Detail({
  by,
  group,
  names,
  metric,
  since,
  now,
  onClose,
}: {
  by: Dimension;
  group: Group | undefined;
  names: Names;
  metric: Metric;
  since: number;
  now: number;
  onClose: () => void;
}) {
  const { t, language } = useI18n();
  if (!group) return null;
  const inner = INNER[by];
  const format = (v: number) =>
    metric === 'tokens' ? formatTokens(v, language) : formatCost(v, 'USD', language);
  const chart = daily(group.records, since, now, inner, names, metric);
  const latest = [...group.records].sort((a, b) => b.completedAt - a.completedAt).slice(0, 12);
  return (
    <Drawer
      label={t('usagePage.detail.title', { name: group.label })}
      title={group.label}
      subtitle={group.detail ?? undefined}
      width="lg"
      onClose={onClose}
    >
      <div className={styles.detail}>
        <Kpis totals={group.totals} />
        <section className={styles.panel}>
          <h3 className={styles.heading}>{t('usagePage.chart.title')}</h3>
          <UsageChart
            {...chart}
            format={format}
            othersLabel={t('usagePage.chart.others')}
            label={t('usagePage.chart.aria', {
              metric: t(`usagePage.metric.${metric}`),
              dimension: t(`usagePage.dimension.${inner}`),
            })}
          />
        </section>
        <section className={styles.panel}>
          <h3 className={styles.heading}>
            {inner === 'model' ? t('usagePage.detail.models') : t('usagePage.detail.agents')}
          </h3>
          <ul className={styles.split}>
            {groupBy(group.records, inner, names, metric).map((g, i) => (
              <li key={g.key}>
                <span
                  className={cx(styles.swatch, seriesClass(i < 5 ? i : null))}
                  aria-hidden="true"
                />
                <span className={styles.rowName}>
                  {g.label}
                  {g.detail && inner === 'model' && (
                    <span className={styles.rowDetail}> · {g.detail}</span>
                  )}
                </span>
                <span className={styles.num}>{format(value(g.totals, metric))}</span>
                <span className={styles.muted}>{g.totals.runs}×</span>
              </li>
            ))}
          </ul>
        </section>
        <section className={styles.panel}>
          <h3 className={styles.heading}>{t('usagePage.detail.recent')}</h3>
          <ul className={styles.recent}>
            {latest.map((r) => (
              <Recent key={r.executionId} record={r} names={names} />
            ))}
          </ul>
        </section>
      </div>
    </Drawer>
  );
}

function Recent({ record: r, names }: { record: UsageRecordDto; names: Names }) {
  const { t, language } = useI18n();
  const m = r.metrics;
  return (
    <li className={styles.recentRow}>
      <span className={styles.when}>
        {new Intl.DateTimeFormat(language, {
          day: '2-digit',
          month: 'short',
          hour: '2-digit',
          minute: '2-digit',
        }).format(r.completedAt)}
      </span>
      <span className={styles.rowName}>
        {labelOf(r.agentId, 'agent', names).label}
        <span className={styles.rowDetail}>
          {' '}
          · {runtimeName(names, r.runtimeId)} · {r.modelId}
        </span>
        {!r.succeeded && <span className={styles.failedBadge}>{t('usagePage.detail.failed')}</span>}
      </span>
      <span className={styles.num}>
        {m?.totalTokens === null || m?.totalTokens === undefined
          ? t('usagePage.notReported')
          : formatTokens(m.totalTokens, language)}
      </span>
      <span className={styles.num}>
        {m?.cost === null || m?.cost === undefined
          ? t('usagePage.notReported')
          : formatCost(m.cost, m.currency, language)}
      </span>
      <span className={styles.num}>{formatDuration(r.completedAt - r.startedAt)}</span>
    </li>
  );
}

/** A provider limit window's name ("5 hours", "7 days"), the raw id when Atlas has no name for it. */
function useWindowName(): (id: string) => string {
  const { t } = useI18n();
  return (id) => {
    const key = `usagePage.window.${id}`;
    const known = [
      'usagePage.window.five_hour',
      'usagePage.window.seven_day',
      'usagePage.window.thirty_day',
      'usagePage.window.seven_day_opus',
      'usagePage.window.seven_day_sonnet',
    ] as const;
    const found = known.find((k) => k === key);
    return found ? t(found) : id.replaceAll('_', ' ');
  };
}

/**
 * How much of each connected runtime's limits is still free, as the provider last said for the
 * whole account. A runtime that reports no limit says so, with what Atlas itself used of it.
 */
function Remaining({
  report,
  names,
  records,
  now,
}: {
  report: UsageReportDto;
  names: Names;
  records: UsageRecordDto[];
  now: number;
}) {
  const { t, language } = useI18n();
  const windowName = useWindowName();
  const ids = [
    ...new Set([
      ...report.connected,
      ...Object.keys(report.quotas),
      ...records.map((r) => r.runtimeId),
    ]),
  ];
  if (ids.length === 0) return null;
  const percent = (v: number) =>
    new Intl.NumberFormat(language, { style: 'percent', maximumFractionDigits: 0 }).format(v);
  const ago = (ms: number) => formatReset(ms / 1000, now, language);
  // Runtimes with limits first, the most used up first.
  const rows = ids
    .map((id) => {
      const quota = report.quotas[id];
      const windows = quota?.windows ?? [];
      const worst = windows.reduce((m, w) => Math.max(m, w.usedFraction), -1);
      return { id, quota: windows.length > 0 ? quota : undefined, worst };
    })
    .sort(
      (a, b) =>
        b.worst - a.worst || runtimeName(names, a.id).localeCompare(runtimeName(names, b.id)),
    );
  return (
    <section className={styles.panel} aria-labelledby="usage-remaining">
      <div className={styles.panelHead}>
        <h2 id="usage-remaining" className={styles.heading}>
          {t('usagePage.remaining')}
        </h2>
      </div>
      <p className={styles.muted}>{t('usagePage.remaining.description')}</p>
      <ul className={styles.remaining}>
        {rows.map(({ id, quota }) => {
          const name = runtimeName(names, id);
          const own = totals(records.filter((r) => r.runtimeId === id));
          return (
            <li key={id} className={styles.remainingCard}>
              <div className={styles.remainingHead}>
                <span className={styles.rowName}>{name}</span>
                {quota?.plan && (
                  <span className={styles.badge}>
                    {t('usagePage.remaining.plan', { plan: quota.plan })}
                  </span>
                )}
              </div>
              {quota ? (
                <>
                  {quota.windows.map((w) => {
                    const reset = w.resetsAt !== null && w.resetsAt * 1000 < now;
                    const left = Math.max(0, 1 - w.usedFraction);
                    return (
                      <div key={w.id} className={styles.window} data-stale={reset}>
                        <div className={styles.windowHead}>
                          <span className={styles.quotaName}>{windowName(w.id)}</span>
                          <span className={styles.left}>
                            {t('usagePage.remaining.left', { percent: percent(left) })}
                          </span>
                        </div>
                        <span
                          className={styles.quotaBar}
                          data-level={
                            w.usedFraction >= 0.9 ? 'high' : w.usedFraction >= 0.7 ? 'mid' : 'low'
                          }
                          role="meter"
                          aria-valuemin={0}
                          aria-valuemax={1}
                          aria-valuenow={left}
                          aria-label={`${name} · ${windowName(w.id)}`}
                        >
                          <span
                            style={{ width: `${String(Math.min(100, w.usedFraction * 100))}%` }}
                          />
                        </span>
                        <span className={reset ? styles.stale : styles.muted}>
                          {reset
                            ? t('usagePage.remaining.reset')
                            : w.resetsAt !== null
                              ? t('usagePage.remaining.resets', {
                                  when: formatReset(w.resetsAt, now, language),
                                })
                              : ''}
                        </span>
                      </div>
                    );
                  })}
                  <p className={styles.source}>
                    {quota.origin === 'cli_session'
                      ? t('usagePage.remaining.fromSession', {
                          runtime: name,
                          when: ago(quota.observedAt),
                        })
                      : t('usagePage.remaining.fromRun', {
                          runtime: name,
                          when: ago(quota.observedAt),
                        })}
                  </p>
                </>
              ) : (
                <p className={styles.muted}>{t('usagePage.remaining.none')}</p>
              )}
              <p className={styles.source}>
                {own.runs > 0
                  ? t('usagePage.remaining.atlasUse', {
                      tokens:
                        own.total === null
                          ? t('usagePage.notReported')
                          : formatTokens(own.total, language),
                      runs: own.runs,
                    })
                  : t('usagePage.remaining.atlasNone')}
              </p>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
