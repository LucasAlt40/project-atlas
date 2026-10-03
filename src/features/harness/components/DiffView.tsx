import { useT } from '@/i18n/I18nProvider';
import type { HarnessDiffDto } from '@/lib/tauri/commands';
import styles from './Harness.module.css';

/** What a new analysis changes in an existing Harness: changed, added, removed, unchanged. */
export function DiffView({ diff }: { diff: HarnessDiffDto }) {
  const t = useT();
  const empty = diff.added.length + diff.removed.length + diff.changed.length === 0;
  return (
    <section aria-label={t('harness.diff.title')}>
      <h3 className={styles.heading}>{t('harness.diff.title')}</h3>
      {empty && <p className={styles.muted}>{t('harness.diff.none')}</p>}
      {diff.changed.length > 0 && (
        <ul className={styles.list} aria-label={t('harness.diff.changed')}>
          {diff.changed.map((c) => (
            <li key={c.id}>
              <span className={styles.diffRemoved}>− {c.before}</span>{' '}
              <span className={styles.diffAdded}>+ {c.after}</span>
            </li>
          ))}
        </ul>
      )}
      {diff.added.length > 0 && (
        <ul className={styles.list} aria-label={t('harness.diff.added')}>
          {diff.added.map((c) => (
            <li key={c.id} className={styles.diffAdded}>
              + {c.after}
            </li>
          ))}
        </ul>
      )}
      {diff.removed.length > 0 && (
        <ul className={styles.list} aria-label={t('harness.diff.removed')}>
          {diff.removed.map((c) => (
            <li key={c.id} className={styles.diffRemoved}>
              − {c.before}
            </li>
          ))}
        </ul>
      )}
      {diff.unchanged.length > 0 && (
        <p className={styles.muted}>
          {t('harness.diff.unchanged', { count: diff.unchanged.length })}
        </p>
      )}
    </section>
  );
}
