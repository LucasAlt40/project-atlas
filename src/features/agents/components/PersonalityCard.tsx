import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { Personality } from '../types';
import styles from './Personality.module.css';

interface Props {
  personality: Personality;
  onUse: (personalityId: string) => void;
  onEdit: (personalityId: string) => void;
  /** Rejects when the personality cannot be deleted (for example agents use it). */
  onDelete: (personalityId: string) => Promise<void>;
}

const BUILTIN_IDS = ['architect', 'developer', 'qa'] as const;

/**
 * An untouched built-in has its description and behavior list translated. Editing a built-in
 * clears its behavior list, which is how an edited copy is told apart from the original.
 */
function useDisplayText(personality: Personality): { description: string; behavior: string[] } {
  const t = useT();
  const id = BUILTIN_IDS.find((builtin) => builtin === personality.id);
  const untouched = personality.source === 'builtin' && personality.behavior.length > 0;
  if (!id || !untouched) {
    return { description: personality.description, behavior: personality.behavior };
  }
  const behavior = t(`personalities.builtin.${id}.behavior` as TranslationKey).split('|');
  return { description: t(`personalities.builtin.${id}.description` as TranslationKey), behavior };
}

/** Shows everything a personality does, including its full instructions, before it is used. */
export function PersonalityCard({ personality, onUse, onEdit, onDelete }: Props) {
  const t = useT();
  const headingId = `personality-${personality.id}`;
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { description, behavior } = useDisplayText(personality);

  function confirmDelete() {
    setError(null);
    onDelete(personality.id).catch((e: unknown) => {
      setError(errorMessage(t, e));
      setConfirming(false);
    });
  }

  return (
    <article className={styles.card} aria-labelledby={headingId}>
      <header className={styles.header}>
        <span className={styles.avatar} aria-hidden="true">
          <Icon name={personality.source === 'builtin' ? 'shield' : 'personalities'} size={20} />
        </span>
        <h3 id={headingId} className={styles.name}>
          {personality.name}
        </h3>
        <span className={styles.badge}>{t(`personalities.source.${personality.source}`)}</span>
      </header>
      {description && <p className={styles.description}>{description}</p>}

      {behavior.length > 0 && (
        <>
          <h4 className={styles.subheading}>{t('personalities.behavior')}</h4>
          <ul className={styles.behavior}>
            {behavior.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </>
      )}

      <h4 className={styles.subheading}>{t('personalities.instructions')}</h4>
      <pre
        className={styles.instructions}
        aria-label={t('personalities.instructionsLabel', { name: personality.name })}
      >
        {personality.systemInstructions}
      </pre>

      {personality.tags.length > 0 && (
        <ul className={styles.tags} aria-label={t('personalities.tags')}>
          {personality.tags.map((tag) => (
            <li key={tag}>{tag}</li>
          ))}
        </ul>
      )}

      <p className={styles.note}>{t('personalities.note')}</p>

      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <div className={styles.actions}>
        <Button
          onClick={() => {
            onUse(personality.id);
          }}
        >
          {t('personalities.use')}
        </Button>
        <button
          type="button"
          className={styles.linkButton}
          aria-label={t('workspace.editAgent', { name: personality.name })}
          onClick={() => {
            onEdit(personality.id);
          }}
        >
          {t('common.edit')}
        </button>
        {confirming ? (
          <span
            className={styles.confirm}
            role="group"
            aria-label={t('personalities.deleteQuestion', { name: personality.name })}
          >
            {t('personalities.deleteQuestion', { name: personality.name })}
            <button type="button" className={styles.danger} onClick={confirmDelete}>
              {t('common.confirmDelete')}
            </button>
            <button
              type="button"
              className={styles.linkButton}
              onClick={() => {
                setConfirming(false);
              }}
            >
              {t('common.cancel')}
            </button>
          </span>
        ) : (
          <button
            type="button"
            className={styles.linkButton}
            aria-label={t('workspace.manage.deleteLabel', { name: personality.name })}
            onClick={() => {
              setConfirming(true);
            }}
          >
            {t('common.delete')}
          </button>
        )}
      </div>
    </article>
  );
}
