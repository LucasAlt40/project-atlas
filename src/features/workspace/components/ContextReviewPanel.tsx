import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type {
  ContextHealthDto,
  ContextReviewDto,
  GuardrailMetricsDto,
  PromptSectionKindDto,
} from '@/lib/tauri/commands';
import styles from './ContextReview.module.css';

const MARK: Record<ContextHealthDto, string> = {
  healthy: '✓',
  partial: '⚠',
  needs_review: '⚠',
  invalid: '✕',
};

/**
 * What Atlas found when it reviewed the context an agent was about to receive: whole, coherent,
 * safe to send. Not a code review and not a security decision: it says what is in the context and
 * where, and the guardrail below it says what that meant for the execution. Every problem opens to
 * show where it is (and, for a conflict, where the other side is).
 */
export function ContextReviewPanel({
  review,
  guardrails,
}: {
  review: ContextReviewDto;
  guardrails: GuardrailMetricsDto | null;
}) {
  const { t } = useI18n();
  const section = (kind: PromptSectionKindDto) =>
    t(`contextReview.section.${kind}` as TranslationKey);
  const count = (severity: string) => review.issues.filter((i) => i.severity === severity).length;
  const problems = review.issues.filter((issue) => issue.severity !== 'info');
  const information = review.issues.filter((issue) => issue.severity === 'info');

  return (
    <section className={styles.panel} aria-label={t('contextReview.title')}>
      <h3>{t('contextReview.title').toUpperCase()}</h3>
      <p className={styles.status} data-health={review.health}>
        <span aria-hidden="true">{MARK[review.health]}</span>{' '}
        <strong>{t(`contextReview.status.${review.health}` as TranslationKey)}</strong>
      </p>
      <p className={styles.line}>
        {t('contextReview.items', {
          required: review.requiredItems,
          high: review.highItems,
          normal: review.normalItems,
          optional: review.optionalItems,
        })}
      </p>
      <p className={styles.line}>
        {t('contextReview.counts', {
          warnings: count('warning'),
          errors: count('error'),
          blocking: count('blocking'),
          stale: review.staleItems,
        })}
      </p>
      {guardrails && (
        <p className={styles.line}>
          {t('contextReview.guardrails', {
            allowed: guardrails.allowed,
            asked: guardrails.asked,
            denied: guardrails.denied,
            transformed: guardrails.transformed,
          })}
        </p>
      )}
      {problems.length + information.length === 0 ? (
        <p className={styles.line}>{t('contextReview.noIssues')}</p>
      ) : (
        <ul className={styles.issues}>
          {[...problems, ...information].map((issue, index) => (
            <li key={`${issue.code}-${String(index)}`}>
              <details>
                <summary>
                  <span className={styles.severity} data-severity={issue.severity}>
                    {t(`contextReview.severity.${issue.severity}` as TranslationKey)}
                  </span>{' '}
                  {t(`contextReview.issue.${issue.code}` as TranslationKey)}
                </summary>
                <dl className={styles.detail}>
                  <dt>{t('contextReview.sources')}</dt>
                  <dd>
                    {issue.otherSource
                      ? t('contextReview.between', {
                          a: section(issue.source),
                          b: section(issue.otherSource),
                        })
                      : section(issue.source)}
                  </dd>
                  <dt>{t('contextReview.issue.reason')}</dt>
                  <dd>{issue.message}</dd>
                  {issue.excerpt && (
                    <>
                      <dt>{t('contextReview.issue.excerpt')}</dt>
                      <dd>
                        <code>{issue.excerpt}</code>
                      </dd>
                    </>
                  )}
                </dl>
              </details>
            </li>
          ))}
        </ul>
      )}
      <h4 className={styles.heading}>{t('contextReview.sources')}</h4>
      <ul className={styles.sources}>
        {review.sources.map((source) => (
          <li key={source.source}>
            <span aria-hidden="true">✓</span> {section(source.source)}{' '}
            <span className={styles.trust}>
              ({t(`contextReview.trust.${source.trust}` as TranslationKey)})
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}
