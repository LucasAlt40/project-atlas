import type { ReactNode } from 'react';
import { useNow } from '@/features/agents/hooks/useNow';
import { formatCost, formatDuration, formatTokens } from '@/features/usage/model/format';
import { useI18n } from '@/i18n/I18nProvider';
import { failureMessage } from '@/i18n/messages';
import type { TranslationKey } from '@/i18n';
import type { PendingInteractionDto, RuntimeCapabilitiesDto } from '@/lib/tauri/commands';
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
  asked,
}: {
  facts: ExecutionFacts;
  context: ExecutionContext;
  /** The question this execution asked, as its workflow run kept it (with the answer, once given). */
  asked?: PendingInteractionDto;
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
      {facts.status === 'waiting_for_input' && facts.interaction && (
        <section className={styles.waiting} aria-label={t('interaction.inspector.waiting')}>
          <h3>{t('interaction.inspector.waiting').toUpperCase()}</h3>
          <dl className={styles.facts}>
            <Fact label={t('interaction.inspector.reason')}>
              {facts.interaction.kind
                ? t(`interaction.kind.${facts.interaction.kind}` as TranslationKey)
                : '—'}
            </Fact>
            <Fact label={t('interaction.inspector.question')}>{facts.interaction.question}</Fact>
            {facts.interaction.context && (
              <Fact label={t('interaction.context')}>{facts.interaction.context}</Fact>
            )}
            <Fact label={t('interaction.inspector.when')}>
              {time(facts.endedAt ?? facts.startedAt)}
            </Fact>
            <Fact label={t('interaction.inspector.how')}>
              {t(`interaction.source.${facts.interaction.source}` as TranslationKey)}
            </Fact>
            {asked?.status === 'answered' && (
              <Fact label={t('interaction.inspector.answer')}>
                {asked.answer ?? asked.choice ?? '—'}
                {asked.answeredAt !== null && ` (${time(asked.answeredAt)})`}
              </Fact>
            )}
            {asked?.status === 'cancelled' && (
              <Fact label={t('interaction.inspector.answer')}>{t('interaction.cancelled')}</Fact>
            )}
          </dl>
        </section>
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
        {facts.context && (
          <Fact label={t('details.context')}>
            <span>
              {facts.context.mode === 'fallback'
                ? t('details.context.fallback', { reason: facts.context.fallbackReason ?? '—' })
                : t('details.context.task_aware')}
            </span>
            <br />
            <span>
              {t('details.context.summary', {
                selected: facts.context.selectedContextCharacters.toLocaleString(language),
                total: facts.context.totalHarnessCharacters.toLocaleString(language),
                items: facts.context.selectedItems,
                omitted: facts.context.omittedItems,
              })}
            </span>
          </Fact>
        )}
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
