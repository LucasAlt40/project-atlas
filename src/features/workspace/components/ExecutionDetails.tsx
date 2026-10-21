import type { ReactNode } from 'react';
import { useNow } from '@/features/agents/hooks/useNow';
import { formatCost, formatDuration, formatTokens } from '@/features/usage/model/format';
import { useI18n } from '@/i18n/I18nProvider';
import { failureMessage } from '@/i18n/messages';
import type { TranslationKey } from '@/i18n';
import type { PendingInteractionDto, RuntimeCapabilitiesDto } from '@/lib/tauri/commands';
import type { ExecutionFacts } from '../model/inspection';
import { shortId } from '../model/inspection';
import { ContextReviewPanel } from './ContextReviewPanel';
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

function Fact({
  label,
  mono = false,
  wide = false,
  children,
}: {
  label: string;
  /** A technical value (a time, a model, a number): set in mono. */
  mono?: boolean;
  /** Takes the whole row (a long explanation). */
  wide?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      className={styles.fact}
      data-mono={mono ? true : undefined}
      data-wide={wide ? true : undefined}
    >
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
  showTask = true,
}: {
  facts: ExecutionFacts;
  context: ExecutionContext;
  /** The question this execution asked, as its workflow run kept it (with the answer, once given). */
  asked?: PendingInteractionDto;
  /** The inspector already names the task in its header. */
  showTask?: boolean;
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
      ? `${t('executions.failed')}: ${failureMessage(t, facts.failureKind)}${facts.failureDetails ? ` ${facts.failureDetails}` : ''}`
      : facts.status === 'cancelled'
        ? t('executions.cancelledByUser')
        : t(`agent.status.${facts.status === 'running' ? 'running' : facts.status}`);

  return (
    <div
      className={styles.details}
      aria-label={t('inspector.title', { id: shortId(facts.executionId) })}
    >
      {showTask && facts.task && (
        <p className={styles.task}>
          <strong>{t('inspector.task')}: </strong>
          {facts.task}
        </p>
      )}
      {facts.status === 'waiting_for_input' && facts.interaction && (
        <section className={styles.waiting} aria-label={t('interaction.inspector.waiting')}>
          <h4 className={styles.groupTitle}>{t('interaction.inspector.waiting')}</h4>
          <dl className={styles.facts}>
            <Fact label={t('interaction.inspector.reason')}>
              {facts.interaction.kind
                ? t(`interaction.kind.${facts.interaction.kind}` as TranslationKey)
                : '—'}
            </Fact>
            <Fact label={t('interaction.inspector.question')} wide>
              {facts.interaction.question}
            </Fact>
            {facts.interaction.context && (
              <Fact label={t('interaction.context')} wide>
                {facts.interaction.context}
              </Fact>
            )}
            <Fact label={t('interaction.inspector.when')} mono>
              {time(facts.endedAt ?? facts.startedAt)}
            </Fact>
            <Fact label={t('interaction.inspector.how')}>
              {t(`interaction.source.${facts.interaction.source}` as TranslationKey)}
            </Fact>
            {asked?.status === 'answered' && (
              <Fact label={t('interaction.inspector.answer')} wide>
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
      <section className={styles.group} aria-label={t('inspector.group.execution')}>
        <h4 className={styles.groupTitle}>{t('inspector.group.execution')}</h4>
        <dl className={styles.facts}>
          {showTask && <Fact label={t('inspector.status')}>{status}</Fact>}
          <Fact label={t('inspector.workspace')}>{context.workspaceName}</Fact>
          <Fact label={t('inspector.agent')}>{context.agentName}</Fact>
          <Fact label={t('inspector.personality')}>{context.personalityName}</Fact>
          <Fact label={t('inspector.runtime')}>{context.runtimeName}</Fact>
          <Fact label={t('inspector.model')}>
            {facts.modelId ? (
              <span className={styles.chip}>{facts.modelId}</span>
            ) : (
              t('details.notReported')
            )}
          </Fact>
          <Fact label={t('inspector.process')}>
            <span className={styles.chip}>{processText}</span>
          </Fact>
          <Fact label={t('inspector.started')} mono>
            {time(facts.startedAt)}
          </Fact>
          {facts.endedAt !== null && (
            <Fact label={t('inspector.finished')} mono>
              {time(facts.endedAt)}
            </Fact>
          )}
          {end !== null && (
            <Fact label={t('inspector.duration')} mono>
              {formatDuration(end - facts.startedAt)}
            </Fact>
          )}
          {!showTask && facts.status !== 'completed' && (
            <Fact label={t('inspector.status')} wide>
              {status}
            </Fact>
          )}
          {facts.stoppedBy && (
            <Fact label={t('inspector.status')}>{t(`inspector.stoppedBy.${facts.stoppedBy}`)}</Fact>
          )}
          {facts.context && (
            <Fact label={t('details.context')} wide>
              <span>
                {facts.context.mode === 'fallback'
                  ? t('details.context.fallback', { reason: facts.context.fallbackReason ?? '—' })
                  : t('details.context.task_aware')}
              </span>
              <br />
              <span className={styles.factNote}>
                {t('details.context.summary', {
                  selected: facts.context.selectedContextCharacters.toLocaleString(language),
                  total: facts.context.totalHarnessCharacters.toLocaleString(language),
                  items: facts.context.selectedItems,
                  omitted: facts.context.omittedItems,
                })}
              </span>
            </Fact>
          )}
        </dl>
      </section>
      <section className={styles.group} aria-label={t('details.usage')}>
        <h4 className={styles.groupTitle}>{t('details.usage')}</h4>
        {facts.usagePending ? (
          <p className={styles.note}>{t('inspector.usagePending')}</p>
        ) : (
          <dl className={styles.facts} data-columns="4">
            <Fact label={t('details.input')} mono>
              {tokens(facts.usage?.inputTokens)}
            </Fact>
            <Fact label={t('details.output')} mono>
              {tokens(facts.usage?.outputTokens)}
            </Fact>
            <Fact label={t('details.total')} mono>
              {tokens(facts.usage?.totalTokens)}
            </Fact>
            <Fact label={t('details.cost')} mono>
              <span className={styles.cost}>{cost}</span>
            </Fact>
          </dl>
        )}
      </section>
      {facts.contextReview && (
        <ContextReviewPanel review={facts.contextReview} guardrails={facts.guardrails} />
      )}
      <GitDetails
        executionId={facts.executionId}
        isolated={context.worktreeIsolation}
        running={running}
      />
    </div>
  );
}
