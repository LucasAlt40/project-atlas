import { useI18n } from '@/i18n/I18nProvider';
import { clock, duration } from './format';
import { STEP_LOOK } from './labels';
import { StepChip } from './StatusChip';
import type { EdgeView, StepView } from './types';
import styles from './Mobile.module.css';

/** How many times the step really ran: a pause to ask a person does not count as a try. */
function tries(step: StepView): number {
  return step.attempts.filter((attempt) => attempt.status !== 'waiting_for_input').length;
}

/**
 * The steps in the order they began, then the ones still to come in the order the workflow was
 * drawn. It shows the workflow that really exists: steps that run side by side, run again in a loop
 * or are skipped by a condition appear as they are, and "what comes next" is read from the graph.
 */
export function Timeline({
  steps,
  edges,
  now,
}: {
  steps: StepView[];
  edges: EdgeView[];
  now: number;
}) {
  const { t, language } = useI18n();
  const labels = new Map(steps.map((step) => [step.nodeId, step.label]));
  const running = steps.filter((s) => s.status === 'running' || s.status === 'waiting_for_input');
  const next = (step: StepView) =>
    edges
      .filter((edge) => edge.from === step.nodeId)
      .map((edge) => labels.get(edge.to))
      .filter((label): label is string => label !== undefined);

  return (
    <div>
      {running.length > 1 && (
        <p className={styles.fine}>{t('mobile.timeline.parallel', { count: running.length })}</p>
      )}
      <ol className={styles.timeline} aria-label={t('mobile.run.timeline')}>
        {steps
          .filter((step) => step.kind !== 'condition' || step.status !== 'pending')
          .map((step) => {
            const look = STEP_LOOK[step.status];
            const last = step.attempts.at(-1);
            const upcoming = next(step);
            return (
              <li key={step.nodeId} className={styles.step} data-tone={look.tone}>
                <span className={styles.marker} aria-hidden="true">
                  {look.glyph}
                </span>
                <div className={styles.stepBody}>
                  <div className={styles.stepTop}>
                    <span className={styles.stepLabel}>{step.label}</span>
                    <StepChip status={step.status} />
                  </div>
                  {step.agent && step.kind === 'agent' && (
                    <span className={styles.muted}>{step.agent}</span>
                  )}
                  <span className={styles.fine}>
                    {step.startedAt !== null &&
                      `${clock(step.startedAt, language, now)} · ${duration(
                        (step.completedAt ?? now) - step.startedAt,
                      )}`}
                    {/* A pause for a question and its answer is not a second try. */}
                    {tries(step) > 1 &&
                      ` · ${t('mobile.timeline.attempts', { count: tries(step) })}`}
                    {step.iterations > 1 &&
                      ` · ${t('mobile.timeline.pass', { count: step.iterations })}`}
                  </span>
                  {last?.summary && step.status !== 'running' && (
                    <details className={styles.details}>
                      <summary>{t('mobile.timeline.summary')}</summary>
                      <p className={styles.pre}>{last.summary}</p>
                    </details>
                  )}
                  {last?.failure && (
                    <p className={styles.error} role="note">
                      {last.failure}
                    </p>
                  )}
                  {step.status !== 'completed' &&
                    step.status !== 'skipped' &&
                    step.status !== 'cancelled' &&
                    upcoming.length > 0 && (
                      <span className={styles.fine}>
                        {t('mobile.timeline.then', { steps: upcoming.join(', ') })}
                      </span>
                    )}
                </div>
              </li>
            );
          })}
      </ol>
    </div>
  );
}
