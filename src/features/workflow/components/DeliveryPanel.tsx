import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import { deliveryOf } from '../model/integration';
import { reviewStateOf } from '../model/review';
import { isActiveRun } from '../types';
import type { Ide, WorkflowRun } from '../types';
import styles from './Workflow.module.css';

export interface CodeActions {
  /** Opens the diff, of one file or of all of them. */
  review: (file?: string) => void;
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
  worktree = 'unknown',
}: {
  run: WorkflowRun;
  ides: Ide[];
  busy: boolean;
  actions: CodeActions;
  /** Whether the run's worktree can still be read. Once it is gone only the saved state is left. */
  worktree?: 'available' | 'missing' | 'invalid' | 'unknown';
}) {
  const { t } = useI18n();
  const [choosing, setChoosing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [applying, setApplying] = useState(false);
  const base = deliveryOf(run);
  const { integration, changes } = run;
  // A worktree that is gone cannot be opened, applied or kept: only what was saved is left.
  const gone =
    (worktree === 'missing' || worktree === 'invalid') && integration.status !== 'integrated';
  const delivery = base && gone ? { ...base, openInIde: false, apply: false, keep: false } : base;

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

  const state = reviewStateOf(run);
  const noChanges = integration.status === 'no_changes';
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
      {run.status === 'failed' && <p className={styles.warning}>{t('review.failedNote')}</p>}
      {run.status === 'cancelled' && <p className={styles.warning}>{t('review.cancelledNote')}</p>}
      <dl className={styles.stateRows}>
        <div>
          <dt>{t('review.reviewStatus')}</dt>
          <dd>{t(state.review)}</dd>
        </div>
        <div>
          <dt>{t('review.integrationStatus')}</dt>
          <dd data-applied={state.applied}>{t(state.integration)}</dd>
        </div>
      </dl>
      <p className={styles.deliveryCode} role="status">
        {t(delivery.codeLine, delivery.params)}
      </p>
      {noChanges && <p className={styles.muted}>{t('review.noChangesToApply')}</p>}
      {integration.status === 'integrated' && (
        <p className={styles.applied}>
          <strong>✓ {t('review.applied.title')}</strong> {t('review.applied.body')}{' '}
          {integration.canUndo
            ? t('review.applied.headUnchanged')
            : t('review.applied.headUnknown')}
        </p>
      )}
      {(integration.status === 'conflicts' || integration.status === 'blocked') &&
        integration.conflicts.length > 0 && (
          <p role="alert" className={styles.warning}>
            <strong>{t('review.blocked.title')}</strong> {t('review.blocked.files')}
          </p>
        )}
      {stats && <p className={styles.deliveryStats}>{stats}</p>}
      {changes && changes.uncommitted.length > 0 && (
        <p className={styles.muted}>
          {t('integration.uncommitted', { count: changes.uncommitted.length })}
        </p>
      )}
      {!delivery.inProject && delivery.tone !== 'neutral' && (
        <p className={styles.muted}>{t('integration.note')}</p>
      )}
      {integration.status === 'integrated' && integration.canUndo && (
        <p className={styles.muted}>{t('integration.undoNote')}</p>
      )}
      {integration.status === 'integrated' && integration.blockReason === 'conflict' && (
        <p role="alert" className={styles.warning}>
          {t('integration.undoConflict', { count: integration.conflicts.length })}
        </p>
      )}
      {integration.status === 'integrated' && integration.message === 'project_moved' && (
        <p role="alert" className={styles.warning}>
          {t('integration.undoMoved')}
        </p>
      )}
      {integration.conflicts.length > 0 &&
        (integration.status === 'conflicts' || integration.status === 'integrated') && (
          <ul className={styles.fileList}>
            {integration.conflicts.map((file) => (
              <li key={file}>
                <span className={styles.filePath}>{file}</span>
              </li>
            ))}
          </ul>
        )}
      {integration.status === 'conflicts' && (
        <p className={styles.muted}>{t('review.blocked.resolve')}</p>
      )}
      <div className={styles.deliveryActions}>
        {delivery.review && (
          <Button
            variant="secondary"
            disabled={busy}
            onClick={() => {
              actions.review();
            }}
          >
            {t('integration.review')}
          </Button>
        )}
        {delivery.openInIde && (
          <div className={styles.popoverHost}>
            <Button
              variant="secondary"
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
          <Button
            disabled={busy}
            onClick={() => {
              setApplying(true);
            }}
          >
            {t('integration.apply')}
          </Button>
        )}
        {delivery.keep && (
          <Button variant="secondary" disabled={busy} onClick={actions.keep}>
            {t('integration.keep')}
          </Button>
        )}
        {delivery.discard &&
          (confirming ? (
            <Button
              variant="danger"
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
              variant="secondary"
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
      {applying && (
        <Modal
          label={t('review.apply.title')}
          onClose={() => {
            setApplying(false);
          }}
        >
          <h2>{t('review.apply.title')}</h2>
          <p>{t('review.apply.body', { files: changes?.filesChanged ?? 0 })}</p>
          <p>{t('review.apply.willNot')}</p>
          <ul>
            <li>{t('review.apply.noCommit')}</li>
            <li>{t('review.apply.noPush')}</li>
            <li>{t('review.apply.noMerge')}</li>
          </ul>
          <p>{t('review.apply.result')}</p>
          {integration.conflicts.length > 0 && (
            <>
              <p role="alert" className={styles.warning}>
                {t('review.apply.knownConflicts')}
              </p>
              <ul className={styles.fileList}>
                {integration.conflicts.map((file) => (
                  <li key={file}>
                    <span className={styles.filePath}>{file}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
          <p className={styles.muted}>{t('review.apply.neverOverwrites')}</p>
          <div className={styles.deliveryActions}>
            <Button
              variant="secondary"
              onClick={() => {
                setApplying(false);
              }}
            >
              {t('common.cancel')}
            </Button>
            <Button
              disabled={busy}
              onClick={() => {
                setApplying(false);
                actions.apply();
              }}
            >
              {t('review.apply.confirm')}
            </Button>
          </div>
        </Modal>
      )}
    </section>
  );
}
