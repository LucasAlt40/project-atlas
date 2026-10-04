import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import type { Translate, TranslationKey } from '@/i18n';
import type { ContextAreaDto, ContextEntryDto, TaskContextDto } from '@/lib/tauri/commands';
import {
  areaKey,
  included,
  includedBecause,
  leftOut,
  leftOutBecause,
  type ReasonLine,
} from '../model/taskContext';
import styles from './Harness.module.css';

function reasonText(t: Translate, line: ReasonLine): string {
  const params = { ...line.params };
  if (typeof params.area === 'string') params.area = t(areaKey(params.area as ContextAreaDto));
  return t(line.key, params);
}

function Entry({ entry, lines }: { entry: ContextEntryDto; lines: ReasonLine[] }) {
  const { t } = useI18n();
  const shown: ReasonLine[] = lines.length > 0 ? lines : [{ key: 'taskContext.reason.nothing' }];
  return (
    <li className={styles.contextEntry}>
      <strong>{entry.item.label}</strong>{' '}
      <span className={styles.muted}>
        {t(areaKey(entry.item.area))} ·{' '}
        {t('taskContext.reason.score', { score: entry.reason.score })}
      </span>
      <ul className={styles.reasons}>
        {shown.map((line) => (
          <li key={`${line.key}:${JSON.stringify(line.params ?? {})}`}>✓ {reasonText(t, line)}</li>
        ))}
      </ul>
    </li>
  );
}

/**
 * The context an agent would be told for a task, and why: what Atlas read in the task, each
 * item chosen with the matches that chose it, what was left out and the exact text. Inspection
 * only: nothing here edits the context.
 */
export function TaskContextModal({
  context,
  task,
  onClose,
}: {
  context: TaskContextDto;
  task: string;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const signals = context.signals;
  const none = t('taskContext.modal.none');
  return (
    <Modal label={t('taskContext.modal.title')} onClose={onClose}>
      <div className={styles.modal}>
        <h2 className={styles.title}>{t('taskContext.modal.title')}</h2>
        <p className={styles.muted}>{task}</p>
        {signals && (
          <section>
            <h3 className={styles.heading}>{t('taskContext.modal.read')}</h3>
            <dl className={styles.signals}>
              <dt>{t('taskContext.modal.intent')}</dt>
              <dd>{t(`taskContext.intent.${signals.intent}` as TranslationKey)}</dd>
              <dt>{t('taskContext.modal.tags')}</dt>
              <dd>{signals.tags.join(', ') || none}</dd>
              <dt>{t('taskContext.modal.keywords')}</dt>
              <dd>{signals.keywords.join(', ') || none}</dd>
              {signals.technologies.length > 0 && (
                <>
                  <dt>{t('taskContext.modal.technologies')}</dt>
                  <dd>{signals.technologies.join(', ')}</dd>
                </>
              )}
            </dl>
          </section>
        )}
        {context.mode === 'task_aware' && (
          <>
            <section>
              <h3 className={styles.heading}>{t('taskContext.modal.includedBecause')}</h3>
              <ul className={styles.contextList}>
                {included(context).map((entry) => (
                  <Entry key={entry.item.id} entry={entry} lines={includedBecause(entry)} />
                ))}
              </ul>
            </section>
            <section>
              <h3 className={styles.heading}>{t('taskContext.modal.excludedBecause')}</h3>
              <ul className={styles.contextList}>
                {leftOut(context).map((entry) => (
                  <li key={entry.item.id} className={styles.contextEntry}>
                    <span>○ {entry.item.label}</span>{' '}
                    <span className={styles.muted}>
                      {t(areaKey(entry.item.area))} ·{' '}
                      {leftOutBecause(entry)
                        .map((line) => reasonText(t, line))
                        .join(' · ')}
                    </span>
                  </li>
                ))}
              </ul>
            </section>
          </>
        )}
        <section>
          <h3 className={styles.heading}>{t('taskContext.modal.text')}</h3>
          <pre className={styles.contextText}>{context.text}</pre>
        </section>
      </div>
    </Modal>
  );
}
