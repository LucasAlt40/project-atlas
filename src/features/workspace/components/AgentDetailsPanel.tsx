import { useState, type ReactNode } from 'react';
import { Button } from '@/components/ui/Button';
import { Drawer } from '@/components/ui/Drawer';
import { StateBadge } from '@/components/ui/status';
import { Tabs } from '@/components/ui/Tabs';
import { useNow } from '@/features/agents/hooks/useNow';
import { useProjectHarness } from '@/features/harness/hooks/useProjectHarness';
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
import type { TranslationKey } from '@/i18n';
import { errorMessage } from '@/i18n/messages';
import type { AgentUsageSummaryDto, QuotaWindowDto, UsageMetricsDto } from '@/lib/tauri/commands';
import { activityLabel } from '../model/activity';
import type { AgentRun } from '../model/agentRuns';
import type { AgentStatus } from '../model/agentStatus';
import type { StoredExecution } from '../types';
import styles from './Details.module.css';
import { AgentPermissions } from './AgentPermissions';
import { ContextPlatformPanel } from './ContextPlatformPanel';
import { AGENT_STATE, StatusBadge } from './StatusBadge';

interface Props {
  workspaceId: string;
  agent: Agent;
  personality: Personality | undefined;
  runtime: RuntimeStatus | undefined;
  run: AgentRun | undefined;
  status: AgentStatus;
  usageVersion: number;
  /** The agent's latest ended execution in this workspace, if any. */
  latest?: StoredExecution | undefined;
  /** Shows the latest execution in the agent's card (Executions tab). */
  onOpenExecution?: () => void;
  onEdit: () => void;
  onClose: () => void;
}

type Tab = 'summary' | 'permissions' | 'context' | 'usage';
const TABS: readonly Tab[] = ['summary', 'permissions', 'context', 'usage'];

/**
 * The agent in detail, as a drawer over the workspace: a summary first (last execution, today's
 * usage, what it runs on, the project context), then its permissions, the context its last
 * execution was given and its usage over time. Every number is what the runtime reported or a sum
 * of those; anything not reported says so instead of showing zero.
 */
export function AgentDetailsPanel({
  workspaceId,
  agent,
  personality,
  runtime,
  run,
  status,
  usageVersion,
  latest,
  onOpenExecution,
  onEdit,
  onClose,
}: Props) {
  const { t, language } = useI18n();
  const [tab, setTab] = useState<Tab>('summary');
  const usage = useAgentUsage(workspaceId, agent.id, usageVersion);
  const now = useNow(1000);
  const harness = useProjectHarness(workspaceId);
  const capabilities = runtime?.runtime.capabilities;
  const runtimeName = runtime?.runtime.name ?? agent.runtimeId;
  const running = run?.status === 'running';
  const profile = agent.permissionProfileId ?? 'read_only';
  const outcomes = agent.resultContract.outcomes.map((o) => o.id);
  const contract = `${t(`contract.kind.${agent.resultContract.kind}`)}${
    outcomes.length > 0 ? ` (${outcomes.join(' / ')})` : ''
  }`;
  const idPrefix = `agent-details-${agent.id}`;
  const lineage = [
    personality?.name ?? agent.personalityId,
    runtimeName,
    runtime?.runtime.provider.name,
  ]
    .filter(Boolean)
    .join(' · ');

  return (
    <Drawer
      label={t('workspace.agentDetails', { name: agent.name })}
      onClose={onClose}
      title={
        <>
          {agent.name}
          <StatusBadge status={status} />
        </>
      }
      subtitle={
        <>
          <span>{lineage}</span>
          <span aria-hidden="true">·</span>
          <span className={styles.metaModel}>{agent.modelId}</span>
        </>
      }
      tabs={
        <Tabs
          label={t('details.tabs')}
          idPrefix={idPrefix}
          active={tab}
          onChange={setTab}
          items={TABS.map((name) => ({ id: name, label: t(`details.tab.${name}`) }))}
        />
      }
      footer={
        <Button variant="secondary" onClick={onEdit}>
          {t('details.editAgent')}
        </Button>
      }
    >
      <div
        role="tabpanel"
        id={`${idPrefix}-panel-${tab}`}
        aria-labelledby={`${idPrefix}-tab-${tab}`}
        className={styles.tabBody}
      >
        {tab === 'summary' && (
          <>
            <section
              className={styles.group}
              aria-label={running ? t('details.currentExecution') : t('details.lastExecution')}
            >
              <h3 className={styles.groupTitle}>
                {running ? t('details.currentExecution') : t('details.lastExecution')}
              </h3>
              {running ? (
                <div className={styles.box}>
                  <p className={styles.running}>
                    {t('details.runningFor', { duration: formatDuration(now - run.startedAt) })}
                  </p>
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
                </div>
              ) : latest ? (
                <div className={styles.box} data-row>
                  <div className={styles.lastText}>
                    <p className={styles.lastTask}>{latest.task}</p>
                    <p className={styles.lastMeta}>
                      <StateBadge state={AGENT_STATE[latest.status]}>
                        {t(`agent.status.${latest.status}`)}
                      </StateBadge>
                      <span aria-hidden="true">·</span>
                      <span>
                        {new Date(latest.startedAt).toLocaleString(language, {
                          dateStyle: 'short',
                          timeStyle: 'short',
                        })}
                        {latest.completedAt !== null &&
                          ` · ${formatDuration(latest.completedAt - latest.startedAt)}`}
                      </span>
                    </p>
                  </div>
                  {onOpenExecution && (
                    <button type="button" className={styles.link} onClick={onOpenExecution}>
                      {t('details.openExecution')}
                    </button>
                  )}
                </div>
              ) : (
                <p className={styles.muted}>{t('details.noExecution')}</p>
              )}
            </section>

            {usage.status === 'ready' && (
              <section className={styles.group} aria-label={t('details.usageTodayTitle')}>
                <h3 className={styles.groupTitle}>{t('details.usageTodayTitle')}</h3>
                <dl className={styles.tiles} data-columns="3">
                  <Tile label={t('details.cost')}>
                    <TotalsView totals={usage.summary.today} showTokens={false} />
                  </Tile>
                  <Tile label={t('details.tokens')}>
                    {usage.summary.today.totalTokens !== null
                      ? formatTokens(usage.summary.today.totalTokens, language)
                      : t('details.notReported')}
                  </Tile>
                  <Tile label={t('agent.tab.executions')}>
                    {t('details.runs', { runs: usage.summary.today.runs })}
                  </Tile>
                </dl>
                <p className={styles.note}>{t('details.reportedNote')}</p>
              </section>
            )}
            {usage.status === 'error' && (
              <p role="alert" className={styles.error}>
                {errorMessage(t, usage.error)}
              </p>
            )}

            <section className={styles.group} aria-label={t('details.agent')}>
              <h3 className={styles.groupTitle}>{t('details.agent')}</h3>
              <dl className={styles.facts}>
                <Row label={t('details.runtime')} mono>
                  {runtimeName}
                  {runtime?.version ? ` ${runtime.version}` : ''}
                </Row>
                <Row label={t('details.model')}>
                  <span className={styles.metaModel}>{agent.modelId}</span>
                </Row>
                <Row label={t('details.permissionProfile')}>
                  {t(`security.profile.${profile}` as TranslationKey)}
                </Row>
                <Row
                  label={t('details.isolation')}
                  title={t(
                    agent.worktreeIsolation
                      ? 'agent.gitIsolation.onHint'
                      : 'agent.gitIsolation.offHint',
                  )}
                >
                  {t(agent.worktreeIsolation ? 'details.isolation.on' : 'details.isolation.off')}
                </Row>
                <Row label={t('details.contract')}>{contract}</Row>
                <Row label={t('details.agentId')} mono>
                  {agent.id}
                </Row>
              </dl>
            </section>

            <section className={styles.group} aria-label={t('harness.agent.title')}>
              <h3 className={styles.groupTitle}>{t('harness.agent.title')}</h3>
              <div className={styles.box} data-row>
                {harness.state.status === 'ready' && (
                  <>
                    <StateBadge
                      state={harness.state.summary.status === 'initialized' ? 'completed' : 'stale'}
                    >
                      {t(`harness.agent.${harness.state.summary.status}`)}
                    </StateBadge>
                    <p className={styles.boxText}>{t('harness.agent.note')}</p>
                  </>
                )}
                {harness.state.status === 'loading' && <p className={styles.muted}>…</p>}
                {harness.state.status === 'error' && (
                  <p className={styles.muted}>{t('harness.loadFailed')}</p>
                )}
              </div>
            </section>
          </>
        )}

        {tab === 'permissions' && (
          <AgentPermissions
            workspaceId={workspaceId}
            agentId={agent.id}
            runtimeName={runtimeName}
          />
        )}

        {tab === 'context' && (
          <section className={styles.group} aria-label={t('details.lastContext')}>
            <h3 className={styles.groupTitle}>{t('details.lastContext')}</h3>
            {latest?.manifest ? (
              <ContextPlatformPanel execution={latest} />
            ) : (
              <p className={styles.muted}>{t('inspector.context.none')}</p>
            )}
          </section>
        )}

        {tab === 'usage' && (
          <>
            {usage.status === 'ready' && (
              <section className={styles.group} aria-label={t('details.usage')}>
                <h3 className={styles.groupTitle}>{t('details.usage')}</h3>
                <dl className={styles.usageRows}>
                  {(
                    [
                      ['details.thisConversation', usage.summary.conversation],
                      ['details.today', usage.summary.today],
                      ['details.thisWeek', usage.summary.week],
                    ] as const
                  ).map(([label, totals]) => (
                    <div key={label} className={styles.usageRow}>
                      <dt>{t(label)}</dt>
                      <dd>
                        <TotalsView totals={totals} stacked />
                        <span className={styles.runs}>
                          {t('details.runs', { runs: totals.runs })}
                        </span>
                      </dd>
                    </div>
                  ))}
                </dl>
                <p className={styles.note}>
                  <strong>{t('details.atlasTracked')}.</strong> {t('details.atlasTrackedNote')}
                </p>
              </section>
            )}
            {!running && usage.status === 'ready' && (
              <section className={styles.group} aria-label={t('details.lastExecutionUsage')}>
                <h3 className={styles.groupTitle}>{t('details.lastExecutionUsage')}</h3>
                <LastExecution
                  summary={usage.summary}
                  costAvailable={capabilities?.costMetrics ?? true}
                  tokensAvailable={capabilities?.usageMetrics ?? true}
                />
              </section>
            )}
            {usage.status === 'loading' && <p className={styles.muted}>…</p>}
            {usage.status === 'error' && (
              <p role="alert" className={styles.error}>
                {errorMessage(t, usage.error)}
              </p>
            )}
            <section className={styles.group} aria-label={t('details.quota')}>
              <h3 className={styles.groupTitle}>{t('details.quota')}</h3>
              {!capabilities?.quotaMetrics ? (
                <p className={styles.muted}>{t('details.quotaNotForRuntime')}</p>
              ) : usage.status === 'ready' && usage.summary.quota ? (
                <>
                  {usage.summary.quota.windows.map((window) => (
                    <QuotaBar key={window.id} window={window} now={now} language={language} />
                  ))}
                  <p className={styles.note}>
                    {t('details.quotaProvider', { runtime: runtimeName })}
                  </p>
                </>
              ) : (
                <p className={styles.muted}>{t('details.quotaNotReportedYet')}</p>
              )}
            </section>
          </>
        )}
      </div>
    </Drawer>
  );
}

function Row({
  label,
  mono = false,
  title,
  children,
}: {
  label: string;
  mono?: boolean;
  title?: string;
  children: ReactNode;
}) {
  return (
    <div className={styles.row} data-mono={mono ? true : undefined} {...(title ? { title } : {})}>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function Tile({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles.tile}>
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
      <dl className={styles.tiles}>
        <Tile label={t('details.input')}>{tokens(metrics?.inputTokens)}</Tile>
        <Tile label={t('details.output')}>{tokens(metrics?.outputTokens)}</Tile>
        <Tile label={t('details.total')}>{tokens(metrics?.totalTokens)}</Tile>
        <Tile label={t('details.cost')}>{cost}</Tile>
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
    <div
      className={styles.quota}
      data-level={percent >= 90 ? 'high' : percent >= 70 ? 'mid' : 'low'}
    >
      <div className={styles.quotaHead}>
        <span className={styles.quotaLabel}>{label}</span>
        <span className={styles.quotaPercent}>{t('details.quotaUsed', { percent })}</span>
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
      {window.resetsAt !== null && (
        <span className={styles.quotaReset}>
          {t('details.quotaResets', { when: formatReset(window.resetsAt, now, language) })}
        </span>
      )}
    </div>
  );
}
