import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { nodeLabel } from '../model/status';
import { clock } from '../model/time';
import type { RecoveryPlan, WorkflowRun } from '../types';
import { FileRow, FindingCard } from './HandoffView';
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
        const attempt = run.nodes[entry.nodeId]?.attempts.find(
          (a) => a.executionId === entry.executionId,
        );
        const took = attempt?.completedAt ? clock(attempt.completedAt - attempt.startedAt) : null;
        const decisions = run.state.decisions.filter((d) => d.sourceNodeId === entry.nodeId);
        const artifacts = run.state.artifacts.filter((a) => a.producerNodeId === entry.nodeId);
        const files =
          run.handoffs.filter((h) => h.fromNodeId === entry.nodeId).at(-1)?.changedFiles ?? [];
        const added = files.reduce((sum, f) => sum + (f.additions ?? 0), 0);
        const removed = files.reduce((sum, f) => sum + (f.deletions ?? 0), 0);
        const delivered = artifacts.length + files.length;
        return (
          <section key={entry.nodeId} className={styles.result} aria-label={t('workflow.verdict')}>
            <header className={styles.resultHead}>
              <span className={styles.chip} data-tone="finished">
                {t('workflow.result.finished')}
              </span>
              {took && <span className={styles.muted}>{took}</span>}
              {run.nodes[entry.nodeId]?.status === 'completed' && (
                <span className={styles.resultCheck}>
                  <Icon name="check" size={18} />
                </span>
              )}
            </header>
            <h4 className={styles.resultTitle}>
              {nodeLabel(run, entry.nodeId)} · {t('workflow.verdict.outcome', { outcome })}
            </h4>
            {entry.summary && (
              <>
                <h5 className={styles.cardLabel}>{t('workflow.result.summary')}</h5>
                <p className={styles.cardBlock}>{entry.summary}</p>
              </>
            )}
            {decisions.length > 0 && (
              <>
                <h5 className={styles.cardLabel}>{t('workflow.result.decisions')}</h5>
                {decisions.map((d) => (
                  <div key={d.id} className={styles.decisionRow} title={d.rationale}>
                    <Icon name="branch" size={14} />
                    <span>
                      <strong>{d.title ? `${d.title}: ` : ''}</strong>
                      {d.decision}
                    </span>
                  </div>
                ))}
              </>
            )}
            {delivered > 0 && (
              <>
                <h5 className={styles.cardLabel}>
                  <span>{t('workflow.result.artifacts', { n: delivered })}</span>
                  {files.length > 0 && (
                    <span className={styles.cardLabelMeta}>
                      <span data-sign="add">+{String(added)}</span>{' '}
                      <span data-sign="del">−{String(removed)}</span> {t('workflow.result.lines')}
                    </span>
                  )}
                </h5>
                <ul className={styles.rowList}>
                  {artifacts.map((a) => (
                    <li key={a.id} className={styles.fileRow} title={a.summary}>
                      <Icon name="stack" size={14} />
                      <span className={styles.fileRowText}>
                        <span className={styles.fileRowName}>{a.name}</span>
                        {a.path && <span className={styles.fileRowFolder}>{a.path}</span>}
                      </span>
                    </li>
                  ))}
                  {files.map((file) => (
                    <FileRow key={file.path} file={file} />
                  ))}
                </ul>
              </>
            )}
            {entry.findings.length > 0 && (
              <>
                <h5 className={styles.cardLabel}>
                  <span>{t('workflow.result.findings')}</span>
                </h5>
                <p className={styles.muted}>
                  {t('workflow.verdict.findings', { n: entry.findings.length })}
                </p>
                <ul className={styles.rowList}>
                  {entry.findings.map((finding, index) => (
                    <FindingCard key={`${finding.title}-${String(index)}`} finding={finding} />
                  ))}
                </ul>
              </>
            )}
            {unrouted ? (
              <p className={styles.cardBlock}>{t('workflow.verdict.noRoute')}</p>
            ) : (
              next && (
                <p className={styles.nextStep}>
                  <Icon name="chevronRight" size={14} />
                  {t('workflow.verdict.next', { node: nodeLabel(run, next) })}
                </p>
              )
            )}
          </section>
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
