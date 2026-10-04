import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import { deliveryOf } from '../model/integration';
import { isActiveRun } from '../types';
import type { Ide, WorkflowRun } from '../types';
import styles from './Workflow.module.css';

export interface CodeActions {
  review: () => void;
  openInIde: (ideId: string) => void;
  apply: () => void;
  keep: () => void;
  discard: () => void;
}

/**
 * What became of the run's code, kept apart from how the run ended. "Workflow completed" is one
 * line; "the code is in your project" is another, and it is only ever said once the changes were
 * applied. Until then the panel says where the code is and offers the user's decisions.
 */
export function DeliveryPanel({
  run,
  ides,
  busy,
  actions,
}: {
  run: WorkflowRun;
  ides: Ide[];
  busy: boolean;
  actions: CodeActions;
}) {
  const { t } = useI18n();
  const [choosing, setChoosing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const delivery = deliveryOf(run);
  const { integration, changes } = run;

  if (!delivery) {
    if (integration.status === 'in_progress' && isActiveRun(run)) {
      return (
        <p className={styles.muted} role="status">
          {t('integration.line.in_progress')}
        </p>
      );
    }
    return null;
  }

  const stats =
    changes && changes.filesChanged > 0
      ? t('integration.stats', {
          files: changes.filesChanged,
          additions: changes.additions,
          deletions: changes.deletions,
        })
      : null;

  return (
    <section
      className={styles.delivery}
      aria-label={t('integration.title')}
      data-tone={delivery.tone}
    >
      <p className={styles.deliveryRun}>
        <span aria-hidden="true">
          {run.status === 'completed' ? '✓' : run.status === 'failed' ? '✕' : '⊘'}
        </span>{' '}
        <strong>{t(delivery.runLine)}</strong>
      </p>
      <p className={styles.deliveryCode} role="status">
        {t(delivery.codeLine, delivery.params)}
      </p>
      {stats && <p className={styles.deliveryStats}>{stats}</p>}
      {changes && changes.uncommitted.length > 0 && (
        <p className={styles.muted}>
          {t('integration.uncommitted', { count: changes.uncommitted.length })}
        </p>
      )}
      {!delivery.inProject && delivery.tone !== 'neutral' && (
        <p className={styles.muted}>{t('integration.note')}</p>
      )}
      {integration.status === 'conflicts' && integration.conflicts.length > 0 && (
        <ul className={styles.fileList}>
          {integration.conflicts.map((file) => (
            <li key={file}>
              <span className={styles.filePath}>{file}</span>
            </li>
          ))}
        </ul>
      )}
      <div className={styles.deliveryActions}>
        {delivery.review && (
          <Button disabled={busy} onClick={actions.review}>
            {t('integration.review')}
          </Button>
        )}
        {delivery.openInIde && (
          <div className={styles.popoverHost}>
            <Button
              disabled={busy || ides.length === 0}
              aria-expanded={choosing}
              title={ides.length === 0 ? t('integration.noIde') : undefined}
              onClick={() => {
                if (ides.length === 1 && ides[0]) actions.openInIde(ides[0].id);
                else setChoosing((value) => !value);
              }}
            >
              {t('integration.openIde')}
            </Button>
            {choosing && ides.length > 1 && (
              <ul className={styles.popover} aria-label={t('integration.chooseIde')}>
                {ides.map((ide) => (
                  <li key={ide.id}>
                    <button
                      type="button"
                      className={styles.popoverItem}
                      onClick={() => {
                        setChoosing(false);
                        actions.openInIde(ide.id);
                      }}
                    >
                      {ide.name}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
        {delivery.apply && (
          <Button disabled={busy} onClick={actions.apply}>
            {t('integration.apply')}
          </Button>
        )}
        {delivery.keep && (
          <Button disabled={busy} onClick={actions.keep}>
            {t('integration.keep')}
          </Button>
        )}
        {delivery.discard &&
          (confirming ? (
            <Button
              disabled={busy}
              onClick={() => {
                setConfirming(false);
                actions.discard();
              }}
            >
              {t('integration.confirmDiscard')}
            </Button>
          ) : (
            <Button
              disabled={busy}
              onClick={() => {
                setConfirming(true);
              }}
            >
              {t('integration.discard')}
            </Button>
          ))}
      </div>
      {confirming && <p className={styles.warning}>{t('integration.discardNote')}</p>}
    </section>
  );
}
