import { useState, type SyntheticEvent } from 'react';
import { Callout } from '@/components/ui/Callout';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { CreatePersonalityInput, Personality } from '../types';
import styles from './Form.module.css';

interface Props {
  /** The form's id: the drawer's footer submits it from outside. */
  id: string;
  /** The personality being edited; omit to create a new one. */
  initial?: Personality;
  onSubmit: (input: CreatePersonalityInput) => Promise<Personality>;
  onSaved: (personality: Personality) => void;
}

/** The fields of a personality, drawn in the edit drawer; its actions live in the drawer's footer. */
export function PersonalityForm({ id, initial, onSubmit, onSaved }: Props) {
  const t = useT();
  const [name, setName] = useState(initial?.name ?? '');
  const [description, setDescription] = useState(initial?.description ?? '');
  const [instructions, setInstructions] = useState(initial?.systemInstructions ?? '');
  const [tags, setTags] = useState(initial?.tags.join(', ') ?? '');
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    if (saving) return;
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
      id={id}
      className={styles.panelForm}
      onSubmit={submit}
      aria-busy={saving}
      aria-label={
        initial ? t('workspace.editAgent', { name: initial.name }) : t('personalities.form.new')
      }
    >
      {initial?.source === 'builtin' && (
        <Callout tone="info">{t('personalities.form.builtinNote')}</Callout>
      )}
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
        <textarea
          id="personality-description"
          className={styles.textarea}
          rows={2}
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
          className={[styles.textarea, styles.mono].join(' ')}
          rows={10}
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
          aria-describedby="personality-tags-hint"
          onChange={(e) => {
            setTags(e.target.value);
          }}
        />
        <p id="personality-tags-hint" className={styles.hint}>
          {t('personalities.form.tagsHint')}
        </p>
      </div>
      {error && (
        <Callout tone="danger" role="alert">
          {error}
        </Callout>
      )}
    </form>
  );
}
