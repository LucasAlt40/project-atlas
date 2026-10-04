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
}: {
  staleness: StalenessDto;
  limit?: number;
}) {
  const { t, language } = useI18n();
  const date = formatAnalyzed(staleness.analyzedAt, language);
  const shown = staleness.changes.slice(0, limit);
  const more = staleness.totalChanges - shown.length;
  return (
    <section className={styles.warning} role="status" aria-label={t('harness.stale.title')}>
      <h3 className={styles.heading}>⚠ {t('harness.stale.title')}</h3>
      <p>
        {staleness.totalChanges === 1
          ? t('harness.stale.summaryOne', { date })
          : t('harness.stale.summary', { count: staleness.totalChanges, date })}
      </p>
      <ul className={styles.list}>
        {shown.map((change) => (
          <li key={`${change.kind}:${change.path}`}>
            <code>{change.path}</code>{' '}
            <span className={styles.muted}>{t(`harness.stale.change.${change.kind}`)}</span>
          </li>
        ))}
        {more > 0 && <li className={styles.muted}>{t('harness.stale.more', { count: more })}</li>}
      </ul>
      <p className={styles.muted}>{t('harness.stale.keeps')}</p>
    </section>
  );
}
