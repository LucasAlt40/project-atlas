import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { ImportReportDto } from '@/lib/tauri/commands';
import { issueText } from '../model/messages';
import styles from './Portability.module.css';

/** What the import did, item by item, with what the person should still look at. */
export function ImportResult({
  report,
  onClose,
}: {
  report: ImportReportDto;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const written = report.items.filter((i) => i.result === 'created' || i.result === 'replaced');
  return (
    <>
      <p role="status" className={styles.ok}>
        {t('portability.done.title', { n: written.length })}
      </p>
      <ul className={styles.list} aria-label={t('portability.done.list')}>
        {report.items.map((item) => (
          <li key={item.ref} className={styles.card}>
            <header className={styles.cardHead}>
              <h4 className={styles.name}>{item.renamedTo ?? item.name}</h4>
              <span className={styles.chip}>{t(`portability.kind.${item.kind}`)}</span>
              <span
                className={styles.chip}
                data-tone={item.result === 'skipped' ? undefined : 'accent'}
              >
                {t(`portability.result.${item.result}` as TranslationKey)}
              </span>
            </header>
            {item.renamedTo && (
              <p className={styles.conflict}>
                {t('portability.result.renamed', { original: item.name, name: item.renamedTo })}
              </p>
            )}
            {item.model && (
              <p className={styles.muted}>
                {item.model.originalRuntime === item.model.runtime &&
                item.model.originalModel === item.model.model
                  ? t('portability.result.modelKept', {
                      runtime: item.model.runtime,
                      model: item.model.model,
                    })
                  : t('portability.result.modelChanged', {
                      original: `${item.model.originalRuntime} · ${item.model.originalModel}`,
                      now: `${item.model.runtime} · ${item.model.model}`,
                    })}
              </p>
            )}
            {item.issues.length > 0 && (
              <ul className={styles.issues}>
                {item.issues.map((issue) => (
                  <li key={issue.code} data-severity={issue.severity}>
                    {issueText(t, issue)}
                  </li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>
      <p className={styles.muted}>{t('portability.done.next')}</p>
      <footer className={styles.footer}>
        <div className={styles.footerRow}>
          <span />
          <Button onClick={onClose}>{t('portability.done.close')}</Button>
        </div>
      </footer>
    </>
  );
}
