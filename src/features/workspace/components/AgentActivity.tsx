import { useT } from '@/i18n/I18nProvider';
import { activityLabel } from '../model/activity';
import type { AgentRun } from '../model/agentRuns';
import styles from './AgentCard.module.css';

type State = 'done' | 'active' | 'failed';

/** A compact, factual timeline of what the core reported for the latest execution. */
export function AgentActivity({ run }: { run: AgentRun }) {
  const t = useT();
  const lastIndex = run.activity.length - 1;
  const stateOf = (index: number, kind: string): State => {
    if (kind === 'failed') return 'failed';
    if (run.status === 'running' && index === lastIndex) return 'active';
    return 'done';
  };
  const SYMBOL: Record<State, string> = { done: '✓', active: '●', failed: '✕' };

  return (
    <details className={styles.activity} open={run.status !== 'completed'}>
      <summary>{t('agent.activity')}</summary>
      <ol aria-label={t('agent.activity')}>
        {run.activity.map((entry, index) => {
          const state = stateOf(index, entry.kind);
          return (
            <li key={entry.id} data-state={state}>
              <span aria-hidden="true">{SYMBOL[state]}</span> {activityLabel(t, entry)}
            </li>
          );
        })}
        {run.status === 'running' && (
          <li data-state="pending">
            <span aria-hidden="true">○</span> {t('agent.activity.completed')}
          </li>
        )}
      </ol>
    </details>
  );
}
