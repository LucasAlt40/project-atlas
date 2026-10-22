import { useI18n } from '@/i18n/I18nProvider';
import { activityLabel, isUserAction, permissionTone, type ActivityEntry } from '../model/activity';
import type { AgentRun } from '../model/agentRuns';
import styles from './AgentCard.module.css';

type State = 'done' | 'active' | 'failed' | 'attention' | 'stopped';

/**
 * A compact, factual timeline of what the core reported for the latest execution. What the
 * user did (interrupting, terminating) is marked as theirs, apart from what the agent and its
 * process did.
 */
export function AgentActivity({ run }: { run: AgentRun | undefined }) {
  const { t, language } = useI18n();
  if (!run) return <p className={styles.empty}>{t('agent.noActivity')}</p>;
  const lastIndex = run.activity.length - 1;
  const stateOf = (index: number, entry: ActivityEntry): State => {
    if (entry.kind === 'failed') return 'failed';
    if (entry.kind === 'cancelled') return 'stopped';
    if (entry.kind === 'permission') {
      const tone = permissionTone(entry, run.pendingApprovalId);
      if (tone === 'refused') return 'failed';
      if (tone === 'attention') return 'attention';
    }
    if (run.status === 'running' && index === lastIndex) return 'active';
    return 'done';
  };
  const time = (at: number) =>
    new Date(at).toLocaleTimeString(language, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });

  return (
    <div className={styles.activity}>
      <ol aria-label={t('agent.activity')}>
        {run.activity.map((entry, index) => {
          const state = stateOf(index, entry);
          return (
            <li
              key={entry.id}
              data-state={state}
              data-actor={isUserAction(entry.kind) ? 'user' : 'agent'}
            >
              <span className={styles.marker} aria-hidden="true" />
              <time className={styles.when} dateTime={new Date(entry.timestamp).toISOString()}>
                {time(entry.timestamp)}
              </time>
              <span className={styles.what}>
                {isUserAction(entry.kind) && (
                  <strong className={styles.actor}>{t('agent.activity.byUser')}: </strong>
                )}
                {activityLabel(t, entry)}
              </span>
            </li>
          );
        })}
        {run.status === 'running' && (
          <li data-state="pending">
            <span className={styles.marker} aria-hidden="true" />
            <span className={styles.when} />
            <span className={styles.what}>{t('agent.activity.completed')}</span>
          </li>
        )}
      </ol>
    </div>
  );
}
