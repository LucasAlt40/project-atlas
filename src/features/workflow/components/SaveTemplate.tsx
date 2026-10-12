import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import styles from './Workflow.module.css';

interface Props {
  /** The workflow has edits that are not saved: the template is made from the saved version. */
  dirty: boolean;
  defaultName: string;
  onSave: (name: string, description: string) => void;
  onCancel: () => void;
}

/** Keeps a workflow's steps, instructions, routes and granted tools as a template of one's own. */
export function SaveTemplate({ dirty, defaultName, onSave, onCancel }: Props) {
  const { t } = useI18n();
  const [name, setName] = useState(defaultName);
  const [description, setDescription] = useState('');
  return (
    <form
      className={styles.preview}
      aria-label={t('workflow.saveTemplate')}
      onSubmit={(event) => {
        event.preventDefault();
        if (!dirty && name.trim()) onSave(name.trim(), description.trim());
      }}
    >
      <label className={styles.field}>
        <span className={styles.fieldLabel}>{t('workflow.saveTemplate.name')}</span>
        <input
          className={styles.control}
          value={name}
          onChange={(e) => {
            setName(e.target.value);
          }}
        />
      </label>
      <label className={styles.field}>
        <span className={styles.fieldLabel}>{t('workflow.saveTemplate.description')}</span>
        <input
          className={styles.control}
          value={description}
          onChange={(e) => {
            setDescription(e.target.value);
          }}
        />
      </label>
      <p className={styles.muted}>{t('workflow.saveTemplate.hint')}</p>
      {dirty && (
        <p role="status" className={styles.failure}>
          {t('workflow.saveTemplate.unsaved')}
        </p>
      )}
      <div className={styles.actions}>
        <Button type="button" variant="secondary" onClick={onCancel}>
          {t('common.cancel')}
        </Button>
        <Button type="submit" disabled={dirty || name.trim() === ''}>
          {t('workflow.saveTemplate.confirm')}
        </Button>
      </div>
    </form>
  );
}
