import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import type { SpecFinding } from '../types';
import styles from './Specs.module.css';

interface Props {
  /** The findings that have an answer, in the order they are shown. */
  answered: { finding: SpecFinding; answer: string }[];
  unanswered: number;
  busy: boolean;
  onChange: (findingId: string, answer: string) => void;
  onSend: () => void;
  onClose: () => void;
}

/**
 * The last look before the answers go: every one next to its question, still editable. Nothing is
 * sent to an agent until this is confirmed, and all of it goes together.
 */
export function SendAnswersDialog({
  answered,
  unanswered,
  busy,
  onChange,
  onSend,
  onClose,
}: Props) {
  const { t } = useI18n();
  return (
    <Modal label={t('specs.send.title')} onClose={onClose} size="wide">
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          if (!busy && answered.length > 0) onSend();
        }}
      >
        <h2 className={styles.dialogTitle}>{t('specs.send.title')}</h2>
        <p className={styles.muted}>{t('specs.send.intro')}</p>
        <div className={styles.formBody}>
          {answered.map(({ finding, answer }, index) => (
            <label key={finding.id} className={styles.field}>
              <span className={styles.questionText}>
                {String(index + 1)}. {finding.question ?? finding.title}
              </span>
              <small className={styles.hint}>{finding.title}</small>
              <textarea
                rows={2}
                value={answer}
                maxLength={2000}
                onChange={(event) => {
                  onChange(finding.id, event.target.value);
                }}
              />
            </label>
          ))}
        </div>
        {unanswered > 0 && (
          <p className={styles.note}>{t('specs.send.unanswered', { count: unanswered })}</p>
        )}
        <div className={styles.dialogActions}>
          <Button variant="secondary" onClick={onClose}>
            {t('specs.send.back')}
          </Button>
          <Button type="submit" disabled={busy || answered.length === 0}>
            {t('specs.send.confirm', { count: answered.length })}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
