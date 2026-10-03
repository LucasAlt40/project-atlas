import type { ReactNode } from 'react';
import { Modal } from '@/components/ui/Modal';
import { useNow } from '@/features/agents/hooks/useNow';
import type { Agent, Personality, RuntimeStatus } from '@/features/agents/types';
import { TotalsView } from '@/features/usage/components/TotalsView';
import { useAgentUsage } from '@/features/usage/hooks/useUsage';
import {
  formatCost,
  formatDuration,
  formatReset,
  formatTokens,
} from '@/features/usage/model/format';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { AgentUsageSummaryDto, QuotaWindowDto, UsageMetricsDto } from '@/lib/tauri/commands';
import { activityLabel } from '../model/activity';
import type { AgentRun } from '../model/agentRuns';
import type { AgentStatus } from '../model/agentStatus';
import styles from './Details.module.css';
import { StatusBadge } from './StatusBadge';

interface Props {
  workspaceId: string;
  agent: Agent;
  personality: Personality | undefined;
  runtime: RuntimeStatus | undefined;
  run: AgentRun | undefined;
  status: AgentStatus;
  usageVersion: number;
  onClose: () => void;
}

/**
 * Quick inspection of an agent: who it is, what it is doing, and what it consumed. Every number
 * is either what the runtime reported or a sum of those over executions Atlas observed; anything
 * not reported says so instead of showing zero.
 */
export function AgentDetailsPanel({
  workspaceId,
  agent,
  personality,
  runtime,
  run,
  status,
  usageVersion,
  onClose,
}: Props) {
  const { t, language } = useI18n();
  const usage = useAgentUsage(workspaceId, agent.id, usageVersion);
  const now = useNow(1000);
  const capabilities = runtime?.runtime.capabilities;
  const runtimeName = runtime?.runtime.name ?? agent.runtimeId;
  const running = run?.status === 'running';

  return (
    <Modal
      label={t('workspace.agentDetails', { name: agent.name })}
      onClose={onClose}
      placement="side"
    >
      <header className={styles.header}>
        <h2 className={styles.name}>{agent.name}</h2>
        <p className={styles.muted}>
          {personality?.name ?? agent.personalityId} · {runtimeName}
          {runtime ? ` · ${runtime.runtime.provider.name}` : ''}
        </p>
        <p className={styles.muted}>{agent.modelId}</p>
        <StatusBadge status={status} />
        {running && (
          <p className={styles.running}>
            {t('details.runningFor', { duration: formatDuration(now - run.startedAt) })}
          </p>
        )}
      </header>

      <section aria-label={running ? t('details.currentExecution') : t('details.lastExecution')}>
        <h3 className={styles.heading}>
          {running ? t('details.currentExecution') : t('details.lastExecution')}
        </h3>
        {running && (
          <>
            <ol className={styles.activity} aria-label={t('details.activity')}>
              {run.activity.map((entry) => (
                <li key={entry.id}>{activityLabel(t, entry)}</li>
              ))}
            </ol>
            <p className={styles.muted}>
              {capabilities?.usageMetrics
                ? t('details.metricsAtEnd')
                : t('details.notAvailableForRuntime')}
            </p>
          </>
        )}
        {!running && usage.status === 'ready' && (
          <LastExecution
            summary={usage.summary}
            costAvailable={capabilities?.costMetrics ?? true}
            tokensAvailable={capabilities?.usageMetrics ?? true}
          />
        )}
        {!running && usage.status === 'loading' && <p className={styles.muted}>…</p>}
        {usage.status === 'error' && (
          <p role="alert" className={styles.error}>
            {errorMessage(t, usage.error)}
          </p>
        )}
      </section>

      {usage.status === 'ready' && (
        <section aria-label={t('details.usage')}>
          <h3 className={styles.heading}>{t('details.usage')}</h3>
          <dl className={styles.rows}>
            {(
              [
                ['details.thisConversation', usage.summary.conversation],
                ['details.today', usage.summary.today],
                ['details.thisWeek', usage.summary.week],
              ] as const
            ).map(([label, totals]) => (
              <Row key={label} label={t(label)}>
                <TotalsView totals={totals} />
                <span className={styles.muted}> · {t('details.runs', { runs: totals.runs })}</span>
              </Row>
            ))}
          </dl>
          <p className={styles.note}>
            <strong>{t('details.atlasTracked')}.</strong> {t('details.atlasTrackedNote')}
          </p>
        </section>
      )}

      <section aria-label={t('details.quota')}>
        <h3 className={styles.heading}>{t('details.quota')}</h3>
        {!capabilities?.quotaMetrics ? (
          <p className={styles.muted}>{t('details.quotaNotForRuntime')}</p>
        ) : usage.status === 'ready' && usage.summary.quota ? (
          <>
            {usage.summary.quota.windows.map((window) => (
              <QuotaBar key={window.id} window={window} now={now} language={language} />
            ))}
            <p className={styles.note}>{t('details.quotaProvider', { runtime: runtimeName })}</p>
          </>
        ) : (
          <p className={styles.muted}>{t('details.quotaNotReportedYet')}</p>
        )}
      </section>

      <dl className={styles.facts}>
        <Row label={t('details.runtime')}>{runtimeName}</Row>
        <Row label={t('details.model')}>{agent.modelId}</Row>
        <Row label={t('details.status')}>{t(`agent.status.${status}`)}</Row>
      </dl>
    </Modal>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles.row}>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function LastExecution({
  summary,
  costAvailable,
  tokensAvailable,
}: {
  summary: AgentUsageSummaryDto;
  costAvailable: boolean;
  tokensAvailable: boolean;
}) {
  const { t, language } = useI18n();
  const latest = summary.latestExecution;
  if (!latest) return <p className={styles.muted}>{t('details.noExecution')}</p>;
  const metrics: UsageMetricsDto | null = latest.metrics;
  const tokens = (value: number | null | undefined) =>
    !tokensAvailable
      ? t('details.notAvailableForRuntime')
      : value === null || value === undefined
        ? t('details.notReported')
        : formatTokens(value, language);
  const cost = !costAvailable
    ? t('details.notAvailableForRuntime')
    : metrics?.cost === null || metrics?.cost === undefined
      ? t('details.notReported')
      : formatCost(metrics.cost, metrics.currency, language);
  return (
    <>
      <p className={styles.muted}>
        {formatDuration(latest.completedAt - latest.startedAt)}
        {metrics ? ` · ${t('details.runtimeReported')}` : ''}
      </p>
      <dl className={styles.rows}>
        <Row label={t('details.input')}>{tokens(metrics?.inputTokens)}</Row>
        <Row label={t('details.output')}>{tokens(metrics?.outputTokens)}</Row>
        <Row label={t('details.total')}>{tokens(metrics?.totalTokens)}</Row>
        <Row label={t('details.cost')}>{cost}</Row>
      </dl>
    </>
  );
}

function QuotaBar({
  window,
  now,
  language,
}: {
  window: QuotaWindowDto;
  now: number;
  language: string;
}) {
  const { t } = useI18n();
  const percent = Math.round(window.usedFraction * 100);
  const label =
    window.id === 'five_hour'
      ? t('details.window.five_hour')
      : window.id === 'seven_day'
        ? t('details.window.seven_day')
        : t('details.window.other', { id: window.id });
  return (
    <div className={styles.quota}>
      <div className={styles.quotaHead}>
        <span>{label}</span>
        <span>
          {t('details.quotaUsed', { percent })}
          {window.resetsAt !== null
            ? ` · ${t('details.quotaResets', { when: formatReset(window.resetsAt, now, language) })}`
            : ''}
        </span>
      </div>
      <div
        className={styles.bar}
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
      >
        <div className={styles.barFill} style={{ width: `${String(Math.min(100, percent))}%` }} />
      </div>
    </div>
  );
}
