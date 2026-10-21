import { useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { ExportDialog } from '@/features/portability';
import type { TranslationKey } from '@/i18n';
import type { Personality } from '../types';
import styles from './Personality.module.css';

interface Props {
  personality: Personality;
  /** Open: behavior and the full instructions are shown. Closed: the card stays compact. */
  expanded: boolean;
  onToggle: () => void;
  onUse: (personalityId: string) => void;
  onEdit: (personalityId: string) => void;
  /** Rejects when the personality cannot be deleted (for example agents use it). */
  onDelete: (personalityId: string) => Promise<void>;
}

const BUILTIN_IDS = [
  'architect',
  'spec-architect',
  'developer',
  'qa',
  'architecture-validator',
  'bug-fixer',
] as const;

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

/**
 * A personality at a glance: who it is, what it is for and its tags. Opening the card shows
 * everything it does, including its full instructions, before it is used.
 */
export function PersonalityCard({
  personality,
  expanded,
  onToggle,
  onUse,
  onEdit,
  onDelete,
}: Props) {
  const t = useT();
  const headingId = `personality-${personality.id}`;
  const detailsId = `${headingId}-details`;
  const [confirming, setConfirming] = useState(false);
  const [exporting, setExporting] = useState(false);
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
    <article className={styles.card} aria-labelledby={headingId} data-expanded={expanded}>
      <header className={styles.header}>
        <span className={styles.avatar} aria-hidden="true">
          <Icon name={personality.source === 'builtin' ? 'shield' : 'personalities'} size={20} />
        </span>
        <h3 id={headingId} className={styles.name}>
          {personality.name}
        </h3>
        <Badge mono dot tone={personality.source === 'builtin' ? 'info' : 'policy'}>
          {t(`personalities.source.${personality.source}`)}
        </Badge>
        <button
          type="button"
          className={styles.toggle}
          aria-expanded={expanded}
          aria-controls={detailsId}
          aria-label={t(expanded ? 'agent.collapse' : 'agent.expand', { name: personality.name })}
          title={t(expanded ? 'agent.collapse' : 'agent.expand', { name: personality.name })}
          onClick={onToggle}
        >
          <Icon name={expanded ? 'chevronUp' : 'chevronDown'} size={16} />
        </button>
      </header>
      {description && <p className={styles.description}>{description}</p>}

      {personality.tags.length > 0 && (
        <ul className={styles.tags} aria-label={t('personalities.tags')}>
          {personality.tags.map((tag) => (
            <li key={tag}>{tag}</li>
          ))}
        </ul>
      )}

      {expanded && (
        <div id={detailsId} className={styles.details}>
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
        </div>
      )}

      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <div className={styles.cardActions}>
        {confirming ? (
          <span
            className={styles.confirm}
            role="group"
            aria-label={t('personalities.deleteQuestion', { name: personality.name })}
          >
            {t('personalities.deleteQuestion', { name: personality.name })}
            <Button variant="danger" onClick={confirmDelete}>
              {t('common.confirmDelete')}
            </Button>
            <Button
              variant="ghost"
              onClick={() => {
                setConfirming(false);
              }}
            >
              {t('common.cancel')}
            </Button>
          </span>
        ) : (
          <>
            <Button
              onClick={() => {
                onUse(personality.id);
              }}
            >
              {t('personalities.use')}
            </Button>
            <OverflowMenu
              label={t('agent.moreActions', { name: personality.name })}
              items={[
                {
                  label: t('common.edit'),
                  icon: 'edit',
                  onSelect: () => {
                    onEdit(personality.id);
                  },
                },
                {
                  label: t('portability.export'),
                  icon: 'upload',
                  onSelect: () => {
                    setExporting(true);
                  },
                },
                {
                  label: t('common.delete'),
                  icon: 'trash',
                  danger: true,
                  onSelect: () => {
                    setConfirming(true);
                  },
                },
              ]}
            />
          </>
        )}
      </div>
      {exporting && (
        <ExportDialog
          targets={[{ kind: 'personality', id: personality.id }]}
          title={t('portability.export.title', { name: personality.name })}
          onClose={() => {
            setExporting(false);
          }}
        />
      )}
    </article>
  );
}
