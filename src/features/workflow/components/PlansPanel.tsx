import { Icon } from '@/components/ui/Icon';
import { Markdown } from '@/components/ui/Markdown';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { PendingInteraction, WorkflowRun } from '../types';
import styles from './Workflow.module.css';

/** What became of a plan, in the words the person sees. */
function planState(plan: PendingInteraction): TranslationKey {
  if (plan.status === 'pending') return 'workflow.plans.waiting';
  if (plan.choice === 'revise') return 'workflow.plans.revise';
  if (plan.choice === 'redo') return 'workflow.plans.redo';
  return plan.auto ? 'workflow.plans.auto' : 'workflow.plans.approved';
}

/**
 * Every plan the steps of a run drew up, each with what was decided about it, so a plan that was
 * approved without asking (or long ago) can still be read.
 */
export function PlansPanel({ run }: { run: WorkflowRun }) {
  const { t } = useI18n();
  const plans = run.interactions.filter((i) => i.kind === 'plan' && i.status !== 'cancelled');
  if (plans.length === 0) return null;
  return (
    <section className={styles.card} aria-label={t('workflow.plans')}>
      <h4>
        <Icon name="viewList" size={14} />
        {t('workflow.plans')}
      </h4>
      <ul>
        {plans.map((plan) => (
          <li key={plan.id}>
            <details className={styles.planItem}>
              <summary>
                <strong>{plan.stepLabel}</strong>
                <span className={styles.tag} data-kind={plan.auto ? 'custom' : undefined}>
                  {t(planState(plan))}
                </span>
              </summary>
              {plan.document ? <Markdown>{plan.document}</Markdown> : <p>{plan.question}</p>}
              {plan.answer && <p className={styles.muted}>“{plan.answer}”</p>}
            </details>
          </li>
        ))}
      </ul>
    </section>
  );
}
