import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { useT } from '@/i18n/I18nProvider';
import { errorDetail, errorMessage } from '@/i18n/messages';
import type { HarnessSummaryDto, RefreshOutcomeDto } from '@/lib/tauri/commands';
import { refreshProjectHarness } from '@/features/workspace/services/workspaceService';
import { ConflictsPanel } from './ConflictsPanel';
import { ErrorBox, type Problem } from './ErrorBox';
import { DiffView } from './DiffView';
import { StaleNotice } from './StaleNotice';
import styles from './Harness.module.css';

interface Props {
  workspaceId: string;
  onClose: () => void;
  onApplied: (summary: HarnessSummaryDto) => void;
}

type State =
  | { name: 'loading' }
  | { name: 'failed'; problem: Problem }
  | { name: 'preview'; outcome: RefreshOutcomeDto }
  | { name: 'applied'; outcome: RefreshOutcomeDto };

/**
 * Refresh: re-analyse, compare with the Harness and show what would change. Nothing is applied
 * until the user confirms; their own files and corrections are never touched.
 */
export function RefreshHarnessModal({ workspaceId, onClose, onApplied }: Props) {
  const t = useT();
  const [state, setState] = useState<State>({ name: 'loading' });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Problem | null>(null);

  useEffect(() => {
    let cancelled = false;
    refreshProjectHarness(workspaceId, false)
      .then((outcome) => {
        if (!cancelled) setState({ name: 'preview', outcome });
      })
      .catch((e: unknown) => {
        if (!cancelled)
          setState({
            name: 'failed',
            problem: { message: errorMessage(t, e), detail: errorDetail(e) },
          });
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, t]);

  function apply() {
    setBusy(true);
    setError(null);
    refreshProjectHarness(workspaceId, true)
      .then((outcome) => {
        if (outcome.applied) onApplied(outcome.applied.summary);
        setState({ name: 'applied', outcome });
      })
      .catch((e: unknown) => {
        setError({ message: errorMessage(t, e), detail: errorDetail(e) });
      })
      .finally(() => {
        setBusy(false);
      });
  }

  return (
    <Modal label={t('harness.refresh.title')} onClose={onClose}>
      <div className={styles.modal}>
        <h2 className={styles.title}>{t('harness.refresh.title')}</h2>
        {state.name === 'loading' && <p className={styles.muted}>{t('harness.analyzing')}</p>}
        {state.name === 'failed' && <ErrorBox problem={state.problem} />}
        {state.name === 'preview' && (
          <>
            {state.outcome.staleness && <StaleNotice staleness={state.outcome.staleness} />}
            <DiffView diff={state.outcome.diff} />
            <ConflictsPanel conflicts={state.outcome.conflicts} />
            <p className={styles.muted}>{t('harness.refresh.applyNote')}</p>
          </>
        )}
        {state.name === 'applied' && (
          <>
            <p>
              <span className={styles.ok}>✓</span> {t('harness.refresh.done')}
            </p>
            {state.outcome.applied && state.outcome.applied.backedUp.length > 0 && (
              <p className={styles.muted}>
                {t('harness.done.backedUp', { count: state.outcome.applied.backedUp.length })}
              </p>
            )}
          </>
        )}
        {error && <ErrorBox problem={error} />}
        <div className={styles.actions}>
          <Button variant="secondary" onClick={onClose}>
            {state.name === 'applied' ? t('common.close') : t('common.cancel')}
          </Button>
          {state.name === 'preview' && (
            <Button disabled={busy} onClick={apply}>
              {t('harness.refresh.apply')}
            </Button>
          )}
        </div>
      </div>
    </Modal>
  );
}
