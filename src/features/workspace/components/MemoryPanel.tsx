import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { ManifestMemoryDto } from '@/lib/tauri/commands';
import styles from './ContextReview.module.css';

/**
 * The notes of earlier work that could have gone into one execution, and what became of each. A
 * note is a hint the agent may use, never an instruction; notes that had nothing to do with the
 * task are only counted.
 */
export function MemoryPanel({
  memories,
  notRelevant,
}: {
  memories: ManifestMemoryDto[];
  notRelevant: number;
}) {
  const { t } = useI18n();

  return (
    <section className={styles.panel} aria-label={t('context.memory.title')}>
      <h3>{t('context.memory.title').toUpperCase()}</h3>
      <p className={styles.line}>{t('context.memory.note')}</p>
      <ul className={styles.sources}>
        {memories.map((memory) => (
          <li key={memory.id} data-status={memory.status}>
            <span aria-hidden="true">{memory.status === 'sent' ? '✓' : '✗'}</span>{' '}
            <strong>{memory.title}</strong>{' '}
            <span className={styles.trust}>
              ({t(`context.memory.status.${memory.status}` as TranslationKey)}
              {memory.automatic && ` · ${t('memory.automatic')}`}
              {memory.reason && ` · ${memory.reason}`})
            </span>
          </li>
        ))}
      </ul>
      {notRelevant > 0 && (
        <p className={styles.line}>{t('context.memory.notRelevant', { count: notRelevant })}</p>
      )}
    </section>
  );
}
