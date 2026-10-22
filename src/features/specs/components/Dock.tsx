import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import type { Dock as DockModel, DockActionId } from '../model/dock';
import styles from './Specs.module.css';

interface Props {
  dock: DockModel;
  busy: boolean;
  /** The name of what comes after this document, for the sentence about the approval. */
  next: string;
  stage: string;
  onAction: (id: DockActionId) => void;
}

const LABELS: Record<DockActionId, TranslationKey> = {
  approve: 'specs.action.approve',
  revise: 'specs.action.revise',
  redo: 'specs.action.redo',
  generate: 'specs.action.generate',
  cancel: 'specs.action.cancel',
  keep: 'specs.action.keep',
};

const VARIANT = {
  approve: 'primary',
  generate: 'primary',
  revise: 'secondary',
  redo: 'ghost',
  keep: 'secondary',
  cancel: 'secondary',
} as const;

/**
 * Where the person decides. Only what makes sense for the document's state is offered, and what
 * cannot be done now says why, beside it, instead of simply not working.
 */
export function Dock({ dock, busy, next, stage, onAction }: Props) {
  const { t } = useI18n();
  const reasons = dock.actions.filter((a) => !a.enabled && a.reason);
  return (
    <div className={styles.dock}>
      <div className={styles.dockMessage}>
        <Icon name="shield" size={16} />
        <div>
          <strong>{t('specs.dock.title')}</strong>
          <span>{t(dock.message, { stage, next })}</span>
          {reasons.length > 0 && (
            <ul className={styles.reasons}>
              {[...new Set(reasons.map((a) => a.reason))].map((reason) =>
                reason ? <li key={reason}>{t(reason)}</li> : null,
              )}
            </ul>
          )}
        </div>
      </div>
      <div className={styles.dockActions}>
        {dock.actions.map((action) => (
          <Button
            key={action.id}
            variant={VARIANT[action.id]}
            disabled={!action.enabled || busy}
            aria-disabled={!action.enabled || busy}
            onClick={() => {
              onAction(action.id);
            }}
          >
            {t(LABELS[action.id])}
          </Button>
        ))}
      </div>
    </div>
  );
}
