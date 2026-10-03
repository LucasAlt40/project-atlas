import { useNow } from '@/features/agents/hooks/useNow';
import { formatDuration } from '@/features/usage/model/format';
import { useI18n } from '@/i18n/I18nProvider';
import { failureMessage } from '@/i18n/messages';
import { shortId, splitByDay } from '../model/inspection';
import type { StoredExecution } from '../types';
import styles from './Inspector.module.css';

const SYMBOL = { completed: '✓', failed: '✕', cancelled: '⊘', running: '●' } as const;

interface Props {
  /** Newest first. */
  executions: StoredExecution[];
  runtimeName: (runtimeId: string) => string;
  onOpen: (execution: StoredExecution) => void;
}

/** What the agent has done in this workspace, most recent first. Opening one inspects it. */
export function ExecutionHistory({ executions, runtimeName, onOpen }: Props) {
  const { t, language } = useI18n();
  const now = useNow(60_000);
  if (executions.length === 0) return <p className={styles.note}>{t('executions.empty')}</p>;
  const { today, earlier } = splitByDay(executions, now);

  const outcome = (execution: StoredExecution) =>
    execution.status === 'cancelled'
      ? t('executions.cancelledByUser')
      : execution.status === 'failed'
        ? `${t('executions.failed')} · ${failureMessage(t, execution.failure?.kind)}`
        : t('executions.completed');
  const group = (title: string, items: StoredExecution[]) =>
    items.length > 0 && (
      <section aria-label={title}>
        <h4 className={styles.groupTitle}>{title}</h4>
        <ul className={styles.list}>
          {items.map((execution) => (
            <li key={execution.id}>
              <button
                type="button"
                className={styles.row}
                aria-label={t('executions.open', { id: shortId(execution.id) })}
                onClick={() => {
                  onOpen(execution);
                }}
              >
                <span className={styles.symbol} data-status={execution.status} aria-hidden="true">
                  {SYMBOL[execution.status]}
                </span>
                <span className={styles.rowTitle}>{execution.task}</span>
                <span className={styles.rowTime}>
                  {execution.completedAt === null
                    ? ''
                    : formatDuration(execution.completedAt - execution.startedAt)}
                </span>
                <span className={styles.rowMeta}>
                  {runtimeName(execution.runtimeId)} · {execution.modelId || '—'} ·{' '}
                  {outcome(execution)} ·{' '}
                  {new Date(execution.startedAt).toLocaleTimeString(language)}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </section>
    );

  return (
    <div>
      {group(t('executions.today'), today)}
      {group(t('executions.earlier'), earlier)}
    </div>
  );
}
