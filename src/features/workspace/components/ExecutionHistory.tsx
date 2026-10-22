import { StateBadge } from '@/components/ui/status';
import { useNow } from '@/features/agents/hooks/useNow';
import { formatDuration } from '@/features/usage/model/format';
import { useI18n } from '@/i18n/I18nProvider';
import { failureMessage } from '@/i18n/messages';
import { shortId, splitByDay } from '../model/inspection';
import type { StoredExecution } from '../types';
import { AGENT_STATE } from './StatusBadge';
import styles from './Inspector.module.css';

interface Props {
  /** Newest first. */
  executions: StoredExecution[];
  runtimeName: (runtimeId: string) => string;
  /** The execution shown beside the list. */
  selectedId?: string | null;
  onOpen: (execution: StoredExecution) => void;
}

/** What the agent has done in this workspace, most recent first, grouped by day. */
export function ExecutionHistory({ executions, runtimeName, selectedId, onOpen }: Props) {
  const { t, language } = useI18n();
  const now = useNow(60_000);
  if (executions.length === 0) return <p className={styles.note}>{t('executions.empty')}</p>;
  const { today, earlier } = splitByDay(executions, now);

  const outcome = (execution: StoredExecution) =>
    execution.status === 'cancelled'
      ? t('executions.cancelledByUser')
      : execution.status === 'waiting_for_input'
        ? t('executions.waitingForInput')
        : execution.status === 'failed'
          ? `${t('executions.failed')} · ${failureMessage(t, execution.failure?.kind)}${execution.failure?.details ? ` — ${execution.failure.details}` : ''}`
          : t('executions.completed');
  const group = (title: string, items: StoredExecution[]) =>
    items.length > 0 && (
      <section className={styles.dayGroup} aria-label={title}>
        <h4 className={styles.dayTitle}>{title}</h4>
        <ul className={styles.list} aria-label={title}>
          {items.map((execution) => (
            <li key={execution.id}>
              <button
                type="button"
                className={styles.row}
                aria-label={t('executions.open', { id: shortId(execution.id) })}
                aria-current={execution.id === selectedId ? 'true' : undefined}
                title={`${execution.task} · ${outcome(execution)}`}
                onClick={() => {
                  onOpen(execution);
                }}
              >
                <span
                  className={styles.rowDot}
                  data-tone={
                    execution.status === 'running' ? 'running' : AGENT_STATE[execution.status]
                  }
                  aria-hidden="true"
                />
                <span className={styles.rowTitle}>{execution.task}</span>
                <span className={styles.rowTime}>
                  {execution.completedAt === null
                    ? ''
                    : formatDuration(execution.completedAt - execution.startedAt)}
                </span>
                <span className={styles.rowMeta}>
                  {runtimeName(execution.runtimeId)} · {execution.modelId || '—'} ·{' '}
                  {new Date(execution.startedAt).toLocaleTimeString(language, {
                    hour: '2-digit',
                    minute: '2-digit',
                  })}
                </span>
                {execution.status !== 'completed' && (
                  <span className={styles.rowOutcome}>
                    <StateBadge
                      state={
                        execution.status === 'running' ? 'running' : AGENT_STATE[execution.status]
                      }
                    >
                      {outcome(execution)}
                    </StateBadge>
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      </section>
    );

  return (
    <div className={styles.history}>
      {group(t('executions.today'), today)}
      {group(t('executions.earlier'), earlier)}
    </div>
  );
}
