import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import type { StalenessDto } from '@/lib/tauri/commands';
import styles from './Harness.module.css';

const SHOWN = 5;

/** The date a Harness was analysed, in the user's language. */
export function formatAnalyzed(ms: number, language: string): string {
  return new Date(ms).toLocaleString(language, { dateStyle: 'medium', timeStyle: 'short' });
}

/**
 * Says the project changed in relevant ways since the Harness was analysed, and what changed.
 * It never deletes anything: it only tells the user (and offers the way to refresh).
 */
export function StaleNotice({
  staleness,
  limit = SHOWN,
  fix,
}: {
  staleness: StalenessDto;
  limit?: number;
  /** Offers to bring the Harness up to date: automatically, or by reviewing the changes first. */
  fix?: { busy: boolean; error: string | null; onFix: () => void; onReview: () => void };
}) {
  const { t, language } = useI18n();
  const date = formatAnalyzed(staleness.analyzedAt, language);
  const shown = staleness.changes.slice(0, limit);
  const more = staleness.totalChanges - shown.length;
  return (
    <section className={styles.stale} role="status" aria-label={t('harness.stale.title')}>
      <h3 className={styles.staleTitle}>⚠ {t('harness.stale.title')}</h3>
      <p className={styles.staleSummary}>
        {staleness.totalChanges === 1
          ? t('harness.stale.summaryOne', { date })
          : t('harness.stale.summary', { count: staleness.totalChanges, date })}
      </p>
      <ul className={styles.staleChanges}>
        {shown.map((change) => (
          <li key={`${change.kind}:${change.path}`}>
            <code>{change.path}</code>
            <span>{t(`harness.stale.change.${change.kind}`)}</span>
          </li>
        ))}
        {more > 0 && (
          <li className={styles.staleMore}>{t('harness.stale.more', { count: more })}</li>
        )}
      </ul>
      <p className={styles.staleNote}>{t('harness.stale.keeps')}</p>
      {fix && (
        <div className={styles.staleActions}>
          <Button disabled={fix.busy} onClick={fix.onFix}>
            {fix.busy ? t('harness.stale.fixing') : t('harness.stale.fix')}
          </Button>
          <button type="button" className={styles.staleReview} onClick={fix.onReview}>
            {t('harness.stale.review')}
          </button>
          {fix.error && (
            <span role="alert" className={styles.staleError}>
              {fix.error}
            </span>
          )}
        </div>
      )}
    </section>
  );
}
