import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { shortId } from '@/features/workspace/model/inspection';
import {
  failureText,
  NODE_STATUS_VIEW,
  RUN_STATUS_LABEL,
  nodeLabel,
  runProgress,
} from '../model/status';
import { isRoutingIssue, issueMessage } from '../model/validation';
import { useState } from 'react';
import type { Ide, RecoveryPlan, ValidationReport, Workflow, WorkflowRun } from '../types';
import { DeliveryPanel, type CodeActions } from './DeliveryPanel';
import { FileLine, HandoffList } from './HandoffView';
import { RecoveryPanel, VerdictList } from './RecoveryPanel';
import { SharedStatePanel } from './SharedStatePanel';
import styles from './Workflow.module.css';

interface RunProps {
  run: WorkflowRun;
  agentName: (agentId: string) => string;
  onOpenExecution: (executionId: string) => void;
  ides: Ide[];
  /** An action on the code is going on. */
  busy: boolean;
  code: CodeActions;
  /** Where a failed run would go on from (`null` when it did not fail, or nothing is known). */
  recovery: RecoveryPlan | null;
  onResume: () => void;
}

type Tab = 'overview' | 'handoffs' | 'changes';
const TABS: readonly Tab[] = ['overview', 'handoffs', 'changes'];

/** What is going on in a run right now, and what has come out of it so far. */
export function RunOverview({
  run,
  agentName,
  onOpenExecution,
  ides,
  busy,
  code,
  recovery,
  onResume,
}: RunProps) {
  const { t } = useI18n();
  const [tab, setTab] = useState<Tab>('overview');
  const progress = runProgress(run);
  const failure = run.failure;
  const counted = run.workflow.nodes.filter((n) => n.type !== 'end');
  const looping = counted.flatMap((node) => {
    const state = run.nodes[node.id];
    return node.loopPolicy && state && state.iterations > 0
      ? [{ node, iteration: state.iterations, max: node.loopPolicy.maxIterations }]
      : [];
  });
  const current = run.state.currentNodes.map((id) => nodeLabel(run, id));
  const agents = run.state.activeAgents.map(agentName);

  const tabs = (
    <div className={styles.tabs} role="tablist" aria-label={t('workflow.tabs')}>
      {TABS.map((name) => (
        <button
          key={name}
          type="button"
          role="tab"
          className={styles.tab}
          aria-selected={tab === name}
          onClick={() => {
            setTab(name);
          }}
        >
          {t(`workflow.tab.${name}` as TranslationKey)}
          {name === 'handoffs' && run.handoffs.length > 0
            ? ` (${String(run.handoffs.length)})`
            : ''}
        </button>
      ))}
    </div>
  );

  if (tab === 'handoffs') {
    return (
      <section className={styles.overview} aria-label={t('workflow.overview')}>
        {tabs}
        <div role="tabpanel">
          <HandoffList run={run} handoffs={run.handoffs} onOpenExecution={onOpenExecution} />
        </div>
      </section>
    );
  }
  if (tab === 'changes') {
    const files = run.changes?.files ?? [];
    return (
      <section className={styles.overview} aria-label={t('workflow.overview')}>
        {tabs}
        <div role="tabpanel" className={styles.overview}>
          {files.length === 0 ? (
            <p className={styles.muted}>{t('integration.noFiles')}</p>
          ) : (
            <ul className={styles.fileList}>
              {files.map((file) => (
                <li key={file.path}>
                  <FileLine file={file} />
                </li>
              ))}
            </ul>
          )}
          {files.length > 0 && (
            <p>
              <button type="button" className={styles.link} onClick={code.review}>
                {t('integration.review')}
              </button>
            </p>
          )}
        </div>
      </section>
    );
  }

  return (
    <section className={styles.overview} aria-label={t('workflow.overview')}>
      <h3 className={styles.inspectorTitle}>{t('workflow.overview')}</h3>
      {tabs}
      <DeliveryPanel run={run} ides={ides} busy={busy} actions={code} />
      <p className={styles.runStatus} data-status={run.status}>
        {t('workflow.overview.status')}: <strong>{t(RUN_STATUS_LABEL[run.status])}</strong>
      </p>
      {run.status === 'interrupted' && (
        <p role="status" className={styles.warning}>
          {t('workflow.interrupted')}
        </p>
      )}
      {failure && (
        <p role="alert" className={styles.failure}>
          {failureText(t, run)}
        </p>
      )}
      <VerdictList run={run} />
      {run.status === 'failed' && recovery && (
        <RecoveryPanel run={run} plan={recovery} busy={busy} onResume={onResume} />
      )}
      {run.recoveries.length > 0 && (
        <p className={styles.muted}>{t('workflow.recovery.count', { n: run.recoveries.length })}</p>
      )}
      <p>
        <strong>
          {t('workflow.progress', { done: progress.completed, total: progress.total })}
        </strong>
      </p>
      <div className={styles.progressBar} aria-hidden="true">
        <span
          style={{
            width: `${String(progress.total === 0 ? 0 : Math.round((progress.completed / progress.total) * 100))}%`,
          }}
        />
      </div>
      <ul className={styles.progressList}>
        {counted.map((node) => {
          const state = run.nodes[node.id];
          const status = state?.status ?? 'pending';
          const view = NODE_STATUS_VIEW[status];
          const loop = looping.find((l) => l.node.id === node.id);
          const outcome = state?.facts['result.outcome'];
          const lastRecovery = run.recoveries.at(-1);
          const completedAt = state?.attempts.at(-1)?.completedAt;
          const reused =
            status === 'completed' &&
            lastRecovery !== undefined &&
            completedAt !== null &&
            completedAt !== undefined &&
            completedAt <= lastRecovery.at;
          return (
            <li key={node.id}>
              <span aria-hidden="true">{view.symbol}</span> {node.label}{' '}
              <span className={styles.muted}>
                {t(view.label)}
                {outcome && ` · ${t('workflow.node.outcome', { outcome: outcome.toUpperCase() })}`}
                {loop && ` · ${t('workflow.node.loop', { n: loop.iteration, max: loop.max })}`}
                {reused && ` · ${t('workflow.node.reused')}`}
              </span>
            </li>
          );
        })}
      </ul>
      <dl className={styles.facts}>
        <div>
          <dt>{t('workflow.overview.current')}</dt>
          <dd>{current.join(', ') || '—'}</dd>
        </div>
        <div>
          <dt>{t('workflow.overview.agents')}</dt>
          <dd>{agents.join(', ') || '—'}</dd>
        </div>
        <div>
          <dt>{t('workflow.overview.running')}</dt>
          <dd>{progress.running + progress.waiting}</dd>
        </div>
        <div>
          <dt>{t('workflow.overview.pending')}</dt>
          <dd>{progress.pending}</dd>
        </div>
        <div>
          <dt>{t('workflow.overview.failed')}</dt>
          <dd>{progress.failed}</dd>
        </div>
        <div>
          <dt>{t('workflow.overview.blocked')}</dt>
          <dd>{progress.blocked}</dd>
        </div>
      </dl>
      <SharedStatePanel run={run} onOpenExecution={onOpenExecution} />
      <p className={styles.muted}>
        {t('workflow.overview.task', { task: run.task })} ·{' '}
        {t('workflow.version', { version: run.workflowVersion })}
        {run.state.artifacts[0] && ` · ${shortId(run.state.artifacts[0].executionId)}`}
      </p>
    </section>
  );
}

interface EditProps {
  workflow: Workflow;
  validation: ValidationReport | null;
  editable: boolean;
  onRename: (name: string) => void;
  /** Whether the routes that cannot match a contract have a repair to review, and why not yet. */
  repair?: { available: boolean; unsaved: boolean; onReview: () => void };
}

/** The workflow as defined, and whether it can run. */
export function EditorOverview({ workflow, validation, editable, onRename, repair }: EditProps) {
  const { t } = useI18n();
  const errors = validation?.issues.filter((i) => i.severity === 'error') ?? [];
  const warnings = validation?.issues.filter((i) => i.severity === 'warning') ?? [];
  const routing = errors.filter(isRoutingIssue);
  const others = errors.filter((i) => !isRoutingIssue(i));
  return (
    <section className={styles.overview} aria-label={t('workflow.overview')}>
      <h3 className={styles.inspectorTitle}>{t('workflow.overview')}</h3>
      <label className={styles.field}>
        <span className={styles.fieldLabel}>{t('workflow.name')}</span>
        <input
          className={styles.control}
          value={workflow.name}
          disabled={!editable}
          onChange={(e) => {
            onRename(e.target.value);
          }}
        />
      </label>
      {workflow.description && <p className={styles.muted}>{workflow.description}</p>}
      <p className={styles.muted}>
        {t(`workflow.mode.${workflow.mode}` as TranslationKey)} ·{' '}
        {t('workflow.version', { version: workflow.version })} ·{' '}
        {t('workflow.nodeCount', { n: workflow.nodes.length })}
      </p>
      {!editable && (
        <p role="status" className={styles.warning}>
          {t('workflow.readOnly')}
        </p>
      )}
      {validation && validation.valid && (
        <p role="status" className={styles.ok}>
          ✓ {t('workflow.validation.valid')}
        </p>
      )}
      {validation && !validation.valid && (
        <div role="alert" className={styles.failure}>
          <strong>{t('workflow.validation.cannotStart')}</strong>
          {routing.length > 0 && (
            <>
              <p>
                <strong>⚠ {t('workflow.validation.routing')}</strong>
              </p>
              <ul>
                {routing.map((issue, index) => (
                  <li key={`${issue.code}-${issue.edgeId ?? issue.nodeId ?? ''}-${String(index)}`}>
                    {issueMessage(t, issue, workflow)}
                  </li>
                ))}
              </ul>
              <p className={styles.muted}>{t('workflow.validation.routingHelp')}</p>
              {repair?.available && (
                <>
                  <Button disabled={!editable || repair.unsaved} onClick={repair.onReview}>
                    {t('workflow.validation.reviewRoutes')}
                  </Button>
                  {repair.unsaved && (
                    <p className={styles.muted}>{t('workflow.validation.saveFirst')}</p>
                  )}
                </>
              )}
            </>
          )}
          {others.length > 0 && (
            <ul>
              {others.map((issue, index) => (
                <li key={`${issue.code}-${issue.nodeId ?? issue.edgeId ?? String(index)}`}>
                  {issueMessage(t, issue, workflow)}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      {warnings.length > 0 && (
        <div role="status" className={styles.warning}>
          <strong>{t('workflow.validation.warnings')}</strong>
          <ul>
            {warnings.map((issue, index) => (
              <li key={`${issue.code}-${issue.nodeId ?? String(index)}`}>
                {issueMessage(t, issue, workflow)}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
