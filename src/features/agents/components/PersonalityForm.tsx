import { useState, type SyntheticEvent } from 'react';
import { Button } from '@/components/ui/Button';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { CreatePersonalityInput, Personality } from '../types';
import styles from './Form.module.css';

interface Props {
  /** The personality being edited; omit to create a new one. */
  initial?: Personality;
  onSubmit: (input: CreatePersonalityInput) => Promise<Personality>;
  onSaved: (personality: Personality) => void;
  onCancel: () => void;
}

export function PersonalityForm({ initial, onSubmit, onSaved, onCancel }: Props) {
  const t = useT();
  const [name, setName] = useState(initial?.name ?? '');
  const [description, setDescription] = useState(initial?.description ?? '');
  const [instructions, setInstructions] = useState(initial?.systemInstructions ?? '');
  const [tags, setTags] = useState(initial?.tags.join(', ') ?? '');
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    onSubmit({ name, description, systemInstructions: instructions, tags: tags.split(',') })
      .then(onSaved)
      .catch((e: unknown) => {
        setError(errorMessage(t, e));
        setSaving(false);
      });
  }

  return (
    <form
      className={styles.form}
      onSubmit={submit}
      aria-label={
        initial ? t('workspace.editAgent', { name: initial.name }) : t('personalities.form.new')
      }
    >
      <div className={styles.field}>
        <label htmlFor="personality-name" className={styles.label}>
          {t('personalities.form.name')}
        </label>
        <input
          id="personality-name"
          className={styles.control}
          value={name}
          placeholder={t('personalities.form.namePlaceholder')}
          onChange={(e) => {
            setName(e.target.value);
          }}
        />
      </div>
      <div className={styles.field}>
        <label htmlFor="personality-description" className={styles.label}>
          {t('personalities.form.description')}
        </label>
        <input
          id="personality-description"
          className={styles.control}
          value={description}
          placeholder={t('personalities.form.descriptionPlaceholder')}
          onChange={(e) => {
            setDescription(e.target.value);
          }}
        />
      </div>
      <div className={styles.field}>
        <label htmlFor="personality-instructions" className={styles.label}>
          {t('personalities.form.instructions')}
        </label>
        <textarea
          id="personality-instructions"
          className={styles.textarea}
          value={instructions}
          placeholder={t('personalities.form.instructionsPlaceholder')}
          onChange={(e) => {
            setInstructions(e.target.value);
          }}
        />
      </div>
      <div className={styles.field}>
        <label htmlFor="personality-tags" className={styles.label}>
          {t('personalities.form.tags')}
        </label>
        <input
          id="personality-tags"
          className={styles.control}
          value={tags}
          placeholder={t('personalities.form.tagsPlaceholder')}
          onChange={(e) => {
            setTags(e.target.value);
          }}
        />
      </div>
      {initial?.source === 'builtin' && (
        <p className={styles.hint}>{t('personalities.form.builtinNote')}</p>
      )}
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <div className={styles.actions}>
        <Button type="button" onClick={onCancel}>
          {t('common.cancel')}
        </Button>
        <Button type="submit" disabled={saving}>
          {t('personalities.form.save')}
        </Button>
      </div>
    </form>
  );
}
