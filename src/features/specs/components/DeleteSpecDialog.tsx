import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import styles from './Specs.module.css';

/** Asks before a spec is removed from Atlas; what was already published to the project stays. */
export function DeleteSpecDialog({
  title,
  busy,
  onClose,
  onConfirm,
}: {
  title: string;
  busy: boolean;
  onClose: () => void;
  onConfirm: () => void;
}) {
  const { t } = useI18n();
  return (
    <Modal label={t('specs.delete.title')} onClose={onClose}>
      <div className={styles.form}>
        <h2 className={styles.dialogTitle}>{t('specs.delete.title')}</h2>
        <p>{t('specs.delete.body', { title })}</p>
        <p className={styles.muted}>{t('specs.delete.files')}</p>
        <div className={styles.dialogActions}>
          <Button variant="secondary" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button variant="danger" disabled={busy} onClick={onConfirm}>
            {t('common.confirmDelete')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
