import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { nodeLabel } from '../model/status';
import type { RecoveryPlan, WorkflowRun } from '../types';
import styles from './Workflow.module.css';

const names = (run: WorkflowRun, ids: readonly string[]) =>
  ids.map((id) => nodeLabel(run, id)).join(', ');

/**
 * What a step concluded, apart from whether it ran well: its result (`FAIL`, `approved`…), what
 * it found and where the workflow goes from there. A step that concluded `fail` and ran fine is
 * not a failed step.
 */
export function VerdictList({ run }: { run: WorkflowRun }) {
  const { t } = useI18n();
  const latest = new Map<string, WorkflowRun['state']['validationResults'][number]>();
  for (const entry of run.state.validationResults) latest.set(entry.nodeId, entry);
  const shown = [...latest.values()].filter(
    (entry) => entry.outcome !== null || entry.findings.length > 0,
  );
  if (shown.length === 0) return null;
  return (
    <>
      {shown.map((entry) => {
        const outcome = (entry.outcome ?? entry.status).toUpperCase();
        const unrouted =
          run.failure?.code === 'no_route_matched' && run.failure.nodeId === entry.nodeId;
        const next = run.handoffs.filter((h) => h.fromNodeId === entry.nodeId).at(-1)?.toNodeId;
        return (
          <div key={entry.nodeId} className={styles.verdict} aria-label={t('workflow.verdict')}>
            <strong>
              {nodeLabel(run, entry.nodeId)} · {t('workflow.verdict.outcome', { outcome })}
            </strong>
            {entry.summary && <p>{entry.summary}</p>}
            {entry.findings.length > 0 && (
              <>
                <p>{t('workflow.verdict.findings', { n: entry.findings.length })}</p>
                <ul>
                  {entry.findings.map((finding, index) => (
                    <li key={`${finding.title}-${String(index)}`}>
                      {finding.title || finding.description}
                      {finding.file && (
                        <span className={styles.muted}>
                          {' '}
                          — {finding.file}
                          {finding.line === null ? '' : `:${String(finding.line)}`}
                        </span>
                      )}
                    </li>
                  ))}
                </ul>
              </>
            )}
            {unrouted ? (
              <p>{t('workflow.verdict.noRoute')}</p>
            ) : (
              next && <p>{t('workflow.verdict.next', { node: nodeLabel(run, next) })}</p>
            )}
          </div>
        );
      })}
    </>
  );
}

/** Where a failed run would go on from, and the way to do it. Nothing is redone that finished. */
export function RecoveryPanel({
  run,
  plan,
  busy,
  onResume,
}: {
  run: WorkflowRun;
  plan: RecoveryPlan;
  busy: boolean;
  onResume: () => void;
}) {
  const { t } = useI18n();
  if (plan.problem) {
    return (
      <div className={styles.warning} role="status">
        <strong>{t('workflow.recovery.cannot')}</strong>
        <p>{t(`workflow.recovery.problem.${plan.problem}` as TranslationKey)}</p>
      </div>
    );
  }
  return (
    <div className={styles.ok} role="status">
      <strong>{t('workflow.recovery.title')}</strong>
      {plan.lastCompletedNodeId && (
        <p>
          {t('workflow.recovery.lastCompleted', { node: nodeLabel(run, plan.lastCompletedNodeId) })}
        </p>
      )}
      {plan.failureNodeId && (
        <p>{t('workflow.recovery.stoppedAt', { node: nodeLabel(run, plan.failureNodeId) })}</p>
      )}
      <p>
        {t(`workflow.recovery.${plan.kind}` as TranslationKey, {
          nodes: names(run, plan.restartNodeIds),
        })}
      </p>
      {plan.reusedNodeIds.length > 0 && (
        <p className={styles.muted}>
          {t('workflow.recovery.kept', { nodes: names(run, plan.reusedNodeIds) })}
        </p>
      )}
      <button type="button" className={styles.link} disabled={busy} onClick={onResume}>
        {t('workflow.recovery.button')}
      </button>
    </div>
  );
}
