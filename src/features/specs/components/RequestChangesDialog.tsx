import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import styles from './Specs.module.css';

interface Props {
  mode: 'revise' | 'redo';
  /** The file(s) the agent will change. */
  targets: string[];
  busy: boolean;
  onSend: (note: string) => void;
  onClose: () => void;
}

/**
 * What the person wants different. For changes the words are required (the agent has nothing else
 * to go on); for a rewrite they are welcome but not needed.
 */
export function RequestChangesDialog({ mode, targets, busy, onSend, onClose }: Props) {
  const { t } = useI18n();
  const [note, setNote] = useState('');
  const required = mode === 'revise';
  const ready = !required || note.trim() !== '';
  return (
    <Modal
      label={t(mode === 'revise' ? 'specs.changes.title' : 'specs.redo.title')}
      onClose={onClose}
      placement="side"
    >
      <form
        className={styles.form}
        onSubmit={(event) => {
          event.preventDefault();
          if (ready && !busy) onSend(note.trim());
        }}
      >
        <h2 className={styles.dialogTitle}>
          {t(mode === 'revise' ? 'specs.changes.title' : 'specs.redo.title')}
        </h2>
        <p className={styles.muted}>
          {t(mode === 'revise' ? 'specs.changes.intro' : 'specs.redo.intro')}
        </p>
        <p className={styles.mono}>{targets.join(' · ')}</p>
        <label className={styles.field}>
          <span>{t(required ? 'specs.changes.note' : 'specs.changes.noteOptional')}</span>
          <textarea
            rows={7}
            value={note}
            maxLength={2000}
            autoFocus
            onChange={(event) => {
              setNote(event.target.value);
            }}
          />
        </label>
        <div className={styles.dialogActions}>
          <Button variant="secondary" onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={!ready || busy}>
            {t(mode === 'revise' ? 'specs.changes.send' : 'specs.redo.send')}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
