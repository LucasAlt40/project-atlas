import { useT } from '@/i18n/I18nProvider';
import type { ConflictDto } from '@/lib/tauri/commands';
import styles from './Harness.module.css';

interface Props {
  conflicts: ConflictDto[];
  /** What the user decided so far, by finding id. Without `onChoose` the panel is read-only. */
  decisions?: Record<string, string>;
  onChoose?: (findingId: string, value: string) => void;
}

/**
 * Evidence that contradicts itself. Atlas shows both sides with their sources and does not pick
 * one: the user's decision resolves it and has precedence.
 */
export function ConflictsPanel({ conflicts, decisions = {}, onChoose }: Props) {
  const t = useT();
  if (conflicts.length === 0) return null;
  return (
    <section className={styles.warning} aria-label={t('harness.conflict.title')}>
      <h3 className={styles.heading}>{t('harness.conflict.title')}</h3>
      <p className={styles.muted}>{t('harness.conflict.body')}</p>
      <ul className={styles.list}>
        {conflicts.map((conflict) => {
          const decided = decisions[conflict.findingId] ?? conflict.resolution;
          return (
            <li key={conflict.findingId} className={styles.conflict}>
              <strong>{conflict.label}</strong>{' '}
              <span className={styles.muted}>
                {decided
                  ? t('harness.conflict.resolved', { value: decided })
                  : t('harness.conflict.unresolved')}
              </span>
              <ul className={styles.claims}>
                {conflict.claims.map((claim) => (
                  <li key={`${claim.origin}\u0000${claim.value}`}>
                    <span>
                      {claim.value}{' '}
                      <span className={styles.muted}>
                        ({t(`harness.origin.${claim.origin}`)}:{' '}
                        {claim.evidence.map((e) => e.source).join(', ')})
                      </span>
                    </span>
                    {onChoose && (
                      <button
                        type="button"
                        className={styles.linkButton}
                        aria-pressed={decided === claim.choice}
                        aria-label={t('harness.conflict.useLabel', {
                          value: claim.value,
                          label: conflict.label,
                        })}
                        onClick={() => {
                          onChoose(conflict.findingId, claim.choice);
                        }}
                      >
                        {t('harness.conflict.use', { value: claim.value })}
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
