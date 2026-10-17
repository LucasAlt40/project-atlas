import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import { stageName } from '../model/messages';
import type { SpecView } from '../types';
import styles from './Specs.module.css';

interface Props {
  view: SpecView;
  busy: boolean;
  /** The person says to go on after the budget was reached. */
  onContinue: () => void;
  /** The alert of a reached limit (on top of the page), the figures (in the side column) or both. */
  part?: 'banner' | 'panel' | 'all';
}

function tokens(language: string, value: number): string {
  return new Intl.NumberFormat(language, { notation: 'compact', maximumFractionDigits: 1 }).format(
    value,
  );
}

function money(language: string, value: number, currency: string): string {
  try {
    return new Intl.NumberFormat(language, { style: 'currency', currency }).format(value);
  } catch {
    return `${value.toFixed(2)} ${currency}`;
  }
}

/**
 * What the spec has cost so far, as the runtimes reported it, and where it stands against its
 * token limit. Only what was reported is shown: a run that said nothing is counted as one that
 * said nothing, never as zero.
 */
export function UsageBar({ view, busy, onContinue, part = 'all' }: Props) {
  const { t, language } = useI18n();
  const { usage, budget } = view;
  const measured = usage.runsMeasured > 0;
  if (!measured && budget.limit === null) return null;
  if (part === 'banner' && !(budget.reached && budget.limit !== null)) return null;
  const used = (usage.inputTokens ?? 0) + (usage.outputTokens ?? 0);
  const share =
    budget.limit !== null && budget.limit > 0 ? Math.min(100, (used / budget.limit) * 100) : null;
  return (
    <section className={styles.usage} aria-label={t('specs.usage.title')}>
      {part !== 'panel' && budget.reached && budget.limit !== null && (
        <div className={styles.banner} data-tone="warning" role="alert">
          <Icon name="warning" size={16} />
          <div className={styles.bannerText}>
            <strong>
              {t('specs.budget.reached', {
                spent: tokens(language, used),
                limit: tokens(language, budget.limit),
              })}
            </strong>
            <span>{t('specs.budget.hint')}</span>
          </div>
          <Button disabled={busy} onClick={onContinue}>
            {t('specs.budget.continue', { limit: tokens(language, budget.limit) })}
          </Button>
        </div>
      )}
      {part !== 'banner' && (
        <details className={styles.usageDetails}>
          <summary>
            <span className={styles.usageLine}>
              <strong>{t('specs.usage.title')}</strong>
              {measured ? (
                <>
                  <span>
                    {t('specs.usage.summary', {
                      input: tokens(language, usage.inputTokens ?? 0),
                      output: tokens(language, usage.outputTokens ?? 0),
                    })}
                  </span>
                  {usage.cachedInputTokens !== null && usage.cachedInputTokens > 0 && (
                    <span className={styles.muted}>
                      {t('specs.usage.cached', {
                        cached: tokens(language, usage.cachedInputTokens),
                      })}
                    </span>
                  )}
                  {usage.cost !== null && usage.currency !== null && (
                    <span>{money(language, usage.cost, usage.currency)}</span>
                  )}
                </>
              ) : (
                <span className={styles.muted}>{t('specs.usage.none')}</span>
              )}
              {share !== null && budget.limit !== null && (
                <span
                  className={styles.usageMeter}
                  data-tone={budget.reached ? 'danger' : budget.near ? 'warning' : 'ok'}
                  role="img"
                  aria-label={t('specs.budget.share', {
                    spent: tokens(language, used),
                    limit: tokens(language, budget.limit),
                  })}
                >
                  <span style={{ width: `${String(share)}%` }} />
                </span>
              )}
            </span>
          </summary>
          <div className={styles.usageBody}>
            <p className={styles.muted}>
              {t('specs.usage.coverage', {
                measured: usage.runsMeasured,
                total: usage.runsTotal,
              })}
            </p>
            <ul className={styles.plain}>
              {view.spec.runs.map((run) => {
                const own = usage.runs.find((u) => u.runId === run.id);
                const what =
                  run.kind === 'review'
                    ? t('specs.usage.review')
                    : stageName(t, run.artifactId ?? 'proposal');
                return (
                  <li key={run.id}>
                    <span>
                      {what}
                      {own ? ` · ${own.modelId}` : ''}
                    </span>
                    <span className={styles.muted}>
                      {own && (own.inputTokens !== null || own.outputTokens !== null)
                        ? t('specs.usage.summary', {
                            input: tokens(language, own.inputTokens ?? 0),
                            output: tokens(language, own.outputTokens ?? 0),
                          })
                        : t('specs.usage.notReported')}
                    </span>
                  </li>
                );
              })}
            </ul>
            <p className={styles.muted}>{t('specs.usage.source')}</p>
          </div>
        </details>
      )}
    </section>
  );
}
