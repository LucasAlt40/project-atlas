import type { ReactNode } from 'react';
import { useNow } from '@/features/agents/hooks/useNow';
import { formatCost, formatDuration, formatTokens } from '@/features/usage/model/format';
import { useI18n } from '@/i18n/I18nProvider';
import { failureMessage } from '@/i18n/messages';
import type { RuntimeCapabilitiesDto } from '@/lib/tauri/commands';
import type { ExecutionFacts } from '../model/inspection';
import { shortId } from '../model/inspection';
import { GitDetails } from './GitDetails';
import styles from './Inspector.module.css';

export interface ExecutionContext {
  workspaceName: string;
  agentName: string;
  personalityName: string;
  runtimeName: string;
  capabilities: RuntimeCapabilitiesDto | undefined;
  /** The agent's Git isolation setting: whether its executions run in their own worktree. */
  worktreeIsolation: boolean;
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles.fact}>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

/**
 * Everything known about one execution: where and how it ran, how long, what became of its
 * process, and what the runtime reported it consumed. A number the runtime did not report says
 * so; it is never shown as zero.
 */
export function ExecutionDetails({
  facts,
  context,
}: {
  facts: ExecutionFacts;
  context: ExecutionContext;
}) {
  const { t, language } = useI18n();
  const now = useNow(1000);
  const running = facts.status === 'running';
  const end = facts.endedAt ?? (running ? now : null);
  const time = (ms: number) => new Date(ms).toLocaleTimeString(language);
  const { capabilities } = context;

  const tokens = (value: number | null | undefined) =>
    capabilities && !capabilities.usageMetrics
      ? t('details.notAvailableForRuntime')
      : value === null || value === undefined
        ? t('details.notReported')
        : formatTokens(value, language);
  const cost =
    capabilities && !capabilities.costMetrics
      ? t('details.notAvailableForRuntime')
      : facts.usage?.cost === null || facts.usage?.cost === undefined
        ? t('details.notReported')
        : formatCost(facts.usage.cost, facts.usage.currency, language);

  const processText =
    facts.process === 'exited'
      ? facts.exitCode === null
        ? t('inspector.process.exitedUnknown')
        : t('inspector.process.exited', { code: facts.exitCode })
      : t(`inspector.process.${facts.process}`);
  const status =
    facts.status === 'failed'
      ? `${t('executions.failed')}: ${failureMessage(t, facts.failureKind)}`
      : facts.status === 'cancelled'
        ? t('executions.cancelledByUser')
        : t(`agent.status.${facts.status === 'running' ? 'running' : facts.status}`);

  return (
    <div aria-label={t('inspector.title', { id: shortId(facts.executionId) })}>
      {facts.task && (
        <p className={styles.task}>
          <strong>{t('inspector.task')}: </strong>
          {facts.task}
        </p>
      )}
      <dl className={styles.facts}>
        <Fact label={t('inspector.status')}>{status}</Fact>
        <Fact label={t('inspector.workspace')}>{context.workspaceName}</Fact>
        <Fact label={t('inspector.agent')}>{context.agentName}</Fact>
        <Fact label={t('inspector.personality')}>{context.personalityName}</Fact>
        <Fact label={t('inspector.runtime')}>{context.runtimeName}</Fact>
        <Fact label={t('inspector.model')}>{facts.modelId || t('details.notReported')}</Fact>
        <Fact label={t('inspector.started')}>{time(facts.startedAt)}</Fact>
        {facts.endedAt !== null && (
          <Fact label={t('inspector.finished')}>{time(facts.endedAt)}</Fact>
        )}
        {end !== null && (
          <Fact label={t('inspector.duration')}>{formatDuration(end - facts.startedAt)}</Fact>
        )}
        <Fact label={t('inspector.process')}>{processText}</Fact>
        {facts.stoppedBy && (
          <Fact label={t('inspector.status')}>{t(`inspector.stoppedBy.${facts.stoppedBy}`)}</Fact>
        )}
        {!facts.usagePending && (
          <>
            <Fact label={t('details.input')}>{tokens(facts.usage?.inputTokens)}</Fact>
            <Fact label={t('details.output')}>{tokens(facts.usage?.outputTokens)}</Fact>
            <Fact label={t('details.total')}>{tokens(facts.usage?.totalTokens)}</Fact>
            <Fact label={t('details.cost')}>{cost}</Fact>
          </>
        )}
      </dl>
      {facts.usagePending && <p className={styles.note}>{t('inspector.usagePending')}</p>}
      <GitDetails
        executionId={facts.executionId}
        isolated={context.worktreeIsolation}
        running={running}
      />
    </div>
  );
}
