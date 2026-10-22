import { useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
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

/** A compact card shows this many tags; the open card shows them all. */
const MAX_TAGS = 3;

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

  const tags = expanded ? personality.tags : personality.tags.slice(0, MAX_TAGS);

  return (
    <article className={styles.card} aria-labelledby={headingId} data-expanded={expanded}>
      <header className={styles.header}>
        <h3 id={headingId} className={styles.name}>
          {personality.name}
        </h3>
        <Badge>{t(`personalities.source.${personality.source}`)}</Badge>
      </header>
      {description && <p className={styles.description}>{description}</p>}

      {tags.length > 0 && (
        <ul className={styles.tags} aria-label={t('personalities.tags')}>
          {tags.map((tag) => (
            <li key={tag}>{tag}</li>
          ))}
          {tags.length < personality.tags.length && (
            <li className={styles.moreTags}>+{personality.tags.length - tags.length}</li>
          )}
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

      <div className={styles.toggleRow}>
        <button
          type="button"
          className={styles.toggle}
          aria-expanded={expanded}
          aria-controls={detailsId}
          onClick={onToggle}
        >
          {t(expanded ? 'common.showLess' : 'common.showMore')}
        </button>
      </div>

      {error && (
        <Callout tone="danger" role="alert">
          {error}
        </Callout>
      )}
      <div className={styles.cardActions}>
        {confirming ? (
          <span
            className={styles.confirm}
            role="group"
            aria-label={t('personalities.deleteQuestion', { name: personality.name })}
          >
            {t('personalities.deleteQuestion', { name: personality.name })}
            <Button variant="danger" size="sm" onClick={confirmDelete}>
              {t('common.confirmDelete')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
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
              variant="secondary"
              size="sm"
              aria-label={t('personalities.useNamed', { name: personality.name })}
              onClick={() => {
                onUse(personality.id);
              }}
            >
              {t('personalities.use')}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              aria-label={t('workspace.editAgent', { name: personality.name })}
              onClick={() => {
                onEdit(personality.id);
              }}
            >
              {t('common.edit')}
            </Button>
            <span className={styles.spacer} />
            <OverflowMenu
              label={t('agent.moreActions', { name: personality.name })}
              items={[
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
