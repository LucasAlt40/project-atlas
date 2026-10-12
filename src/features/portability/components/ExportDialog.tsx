import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { Modal } from '@/components/ui/Modal';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import type { ExportedFileDto, ExportTargetDto } from '@/lib/tauri/commands';
import { portabilityError } from '../model/messages';
import { exportConfigs, saveConfigFile } from '../services/portabilityService';
import { KIND_ICON } from './ImportItemCard';
import styles from './Portability.module.css';

interface Props {
  /** What is being exported, by the ids this installation knows. */
  targets: ExportTargetDto[];
  /** What the dialog calls it: the name of the one thing, or of the set. */
  title: string;
  onClose: () => void;
}

/**
 * Exporting: what goes into the file is read from the core first (and shown, with anything
 * worth a second look), then the person says where to keep it, in the operating system's own
 * dialog. The file never holds credentials, conversations, logs or paths of this machine.
 */
export function ExportDialog({ targets, title, onClose }: Props) {
  const { t } = useI18n();
  const [withDependencies, setWithDependencies] = useState(true);
  /** What was read (or why not) for the targets and option it was read for. */
  const [read, setRead] = useState<{
    key: string;
    file: ExportedFileDto | null;
    error: string | null;
  }>();
  /** The outcome of the last save, for the file it was a save of. */
  const [saving, setSaving] = useState<{ key: string; name: string | null } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const key = JSON.stringify([targets, withDependencies]);
  const file = read?.key === key ? read.file : null;
  const readError = read?.key === key ? read.error : null;
  const saved = saving?.key === key ? saving : null;

  useEffect(() => {
    let stale = false;
    exportConfigs(targets, withDependencies)
      .then((exported) => {
        if (!stale) setRead({ key, file: exported, error: null });
      })
      .catch((cause: unknown) => {
        if (!stale) setRead({ key, file: null, error: portabilityError(t, cause) });
      });
    return () => {
      stale = true;
    };
  }, [key, targets, withDependencies, t]);

  function save() {
    if (!file) return;
    setBusy(true);
    setError(null);
    saveConfigFile(file.fileName, file.text)
      .then((name) => {
        setSaving({ key, name });
      })
      .catch((cause: unknown) => {
        setError(portabilityError(t, cause));
      })
      .finally(() => {
        setBusy(false);
      });
  }

  return (
    <Modal label={title} onClose={onClose} size="analyze">
      <div className={styles.dialog}>
        <h2 className={styles.title}>{title}</h2>
        <p className={styles.muted}>{t('portability.export.intro')}</p>

        <label className={styles.check}>
          <input
            type="checkbox"
            checked={withDependencies}
            onChange={(e) => {
              setWithDependencies(e.target.checked);
            }}
          />
          <span>
            {t('portability.export.deps')}
            <span className={styles.helpText}>{t('portability.export.depsHelp')}</span>
          </span>
        </label>

        {file && (
          <>
            <section className={styles.section} aria-label={t('portability.export.contents')}>
              <h3 className={styles.sectionTitle}>{t('portability.export.contents')}</h3>
              <ul className={styles.list}>
                {file.items.map((entry) => (
                  <li key={`${entry.kind}-${entry.name}`} className={styles.cardHead}>
                    <Icon name={KIND_ICON[entry.kind]} size={14} />
                    <span className={styles.name}>{entry.name}</span>
                    <span className={styles.chip}>{t(`portability.kind.${entry.kind}`)}</span>
                    {!entry.chosen && (
                      <span className={styles.chip}>{t('portability.item.needed')}</span>
                    )}
                  </li>
                ))}
              </ul>
            </section>
            {file.external.length > 0 && (
              <section className={styles.section} aria-label={t('portability.export.external')}>
                <h3 className={styles.sectionTitle}>{t('portability.export.external')}</h3>
                <p className={styles.muted}>{t('portability.export.externalHelp')}</p>
                <ul className={styles.list}>
                  {file.external.map((entry) => (
                    <li key={`${entry.kind}-${entry.name}`} className={styles.cardHead}>
                      <Icon name={KIND_ICON[entry.kind]} size={14} />
                      <span className={styles.name}>{entry.name}</span>
                      <span className={styles.chip}>{t(`portability.kind.${entry.kind}`)}</span>
                    </li>
                  ))}
                </ul>
              </section>
            )}
            {file.warnings.length > 0 && (
              <ul className={styles.issues} aria-label={t('portability.export.warnings')}>
                {file.warnings.map((warning) => (
                  <li key={`${warning.code}-${warning.name}`} data-severity="warning">
                    {t(`portability.export.warn.${warning.code}` as TranslationKey, {
                      name: warning.name,
                    })}
                  </li>
                ))}
              </ul>
            )}
          </>
        )}

        {(readError ?? error) && (
          <p role="alert" className={styles.error}>
            {readError ?? error}
          </p>
        )}
        {saved?.name && (
          <p role="status" className={styles.ok}>
            {t('portability.export.saved', { file: saved.name })}
          </p>
        )}
        {saved?.name === null && (
          <p className={styles.muted}>{t('portability.export.cancelled')}</p>
        )}

        <footer className={styles.footer}>
          <div className={styles.footerRow}>
            <span className={styles.summary}>{file?.fileName ?? ''}</span>
            <span className={styles.rowEnd}>
              <Button variant="secondary" onClick={onClose}>
                {t('common.close')}
              </Button>
              <Button disabled={!file || busy} onClick={save}>
                {t('portability.export.save')}
              </Button>
            </span>
          </div>
        </footer>
      </div>
    </Modal>
  );
}
