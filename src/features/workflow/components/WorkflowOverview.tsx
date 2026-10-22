import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useNow } from '@/features/agents/hooks/useNow';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { shortId } from '@/features/workspace/model/inspection';
import { failureText, NODE_STATUS_VIEW, nodeLabel, runProgress } from '../model/status';
import { isFinished } from '../model/review';
import { clock } from '../model/time';
import { isActiveRun } from '../types';
import { isRoutingIssue, issueMessage } from '../model/validation';
import { useState, type ReactNode } from 'react';
import { Tabs } from '@/components/ui/Tabs';
import type { Ide, RecoveryPlan, ValidationReport, Workflow, WorkflowRun } from '../types';
import { DeliveryPanel, type CodeActions } from './DeliveryPanel';
import { FileLine, HandoffList } from './HandoffView';
import { RecoveryPanel, VerdictList } from './RecoveryPanel';
import { PlansPanel } from './PlansPanel';
import { SharedStatePanel } from './SharedStatePanel';
import { WorkflowTools } from './WorkflowTools';
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
  /** Where the MCP connections of the workspace are read and managed. */
  tools?: ToolsProps;
  /** The files and code being written right now (the Live tab), while the run is not over. */
  live?: ReactNode;
}

interface ToolsProps {
  workspaceId: string;
  onOpenHub: () => void;
}

type Tab = 'overview' | 'handoffs' | 'changes' | 'live';

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
  tools,
  live,
}: RunProps) {
  const { t } = useI18n();
  const [tab, setTab] = useState<Tab>('overview');
  const progress = runProgress(run);
  // The clock of a running step ticks; nothing else on the screen needs the time.
  const now = useNow(isActiveRun(run) ? 1000 : 60_000);
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

  const fileCount = run.changes?.files.length ?? 0;
  const head = (
    <div className={styles.dockTabs}>
      <Tabs
        label={t('workflow.tabs')}
        active={tab}
        onChange={setTab}
        items={[
          { id: 'overview' as const, label: t('workflow.tab.overview') },
          {
            id: 'handoffs' as const,
            label: t('workflow.tab.handoffs'),
            ...(run.handoffs.length > 0 ? { count: run.handoffs.length } : {}),
          },
          {
            id: 'changes' as const,
            label: t('workflow.tab.changes'),
            ...(fileCount > 0 ? { count: fileCount } : {}),
          },
          ...(live ? [{ id: 'live' as const, label: t('workflow.tab.live') }] : []),
        ]}
      />
    </div>
  );

  const changes = run.changes;
  const footer = (
    <footer className={styles.dockFoot}>
      {tab === 'overview' && changes && changes.filesChanged > 0 ? (
        <>
          <span className={styles.dockFootStats}>
            {t('workflow.foot.files', { n: changes.filesChanged })}
            <span data-sign="add">+{String(changes.additions)}</span>
            <span data-sign="del">−{String(changes.deletions)}</span>
          </span>
          <Button
            variant="secondary"
            onClick={() => {
              code.review();
            }}
          >
            {t('workflow.foot.review')}
          </Button>
        </>
      ) : (
        <span className={styles.dockFootStats}>
          <span className={styles.dockDot} data-tone="done" />
          {t('workflow.foot.done', { n: progress.completed })}
          <span className={styles.dockDot} data-tone="failed" />
          {t('workflow.foot.failed', { n: progress.failed })}
        </span>
      )}
    </footer>
  );

  if (tab === 'handoffs') {
    return (
      <section className={styles.overview} aria-label={t('workflow.overview')}>
        {head}
        <div role="tabpanel" className={styles.dockBody}>
          <HandoffList
            run={run}
            handoffs={run.handoffs}
            agentName={agentName}
            onOpenExecution={onOpenExecution}
          />
        </div>
        {footer}
      </section>
    );
  }
  if (tab === 'live' && live) {
    return (
      <section className={styles.overview} aria-label={t('workflow.overview')}>
        {head}
        <div role="tabpanel" className={styles.dockBody}>
          {live}
        </div>
      </section>
    );
  }
  if (tab === 'changes') {
    const files = run.changes?.files ?? [];
    return (
      <section className={styles.overview} aria-label={t('workflow.overview')}>
        {head}
        <div role="tabpanel" className={styles.dockBody}>
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
              <button
                type="button"
                className={styles.link}
                onClick={() => {
                  code.review();
                }}
              >
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
      {head}
      <div className={styles.dockBody}>
        {/* Once the run is over, its result and code are the Review Workspace's. */}
        {!isFinished(run) && <DeliveryPanel run={run} ides={ides} busy={busy} actions={code} />}
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
        <section className={styles.statusCard} aria-label={t('workflow.overview.status')}>
          <p className={styles.progressCount}>
            <span>
              {t('workflow.progress', { done: progress.completed, total: progress.total })}
            </span>
            <span>
              {String(
                progress.total === 0 ? 0 : Math.round((progress.completed / progress.total) * 100),
              )}
              %
            </span>
          </p>
          <div
            className={styles.progressBar}
            role="progressbar"
            aria-label={t('workflow.overview.status')}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={
              progress.total === 0 ? 0 : Math.round((progress.completed / progress.total) * 100)
            }
          >
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
              const lastAttempt = state?.attempts.at(-1);
              const completedAt = lastAttempt?.completedAt;
              // Real time only: a step that never started has none.
              const elapsed = lastAttempt ? (completedAt ?? now) - lastAttempt.startedAt : null;
              const reused =
                status === 'completed' &&
                lastRecovery !== undefined &&
                completedAt !== null &&
                completedAt !== undefined &&
                completedAt <= lastRecovery.at;
              return (
                <li key={node.id} data-status={status}>
                  <span className={styles.stepName}>
                    <span className={styles.stepDot} data-status={status} aria-hidden="true" />
                    <span title={node.label}>
                      <span className={styles.srOnly}>{view.symbol} </span>
                      {node.label}
                    </span>
                  </span>
                  <span className={styles.stepState}>
                    <span>
                      {t(view.label)}
                      {outcome &&
                        ` · ${t('workflow.node.outcome', { outcome: outcome.toUpperCase() })}`}
                      {loop &&
                        ` · ${t('workflow.node.loop', { n: loop.iteration, max: loop.max })}`}
                      {reused && ` · ${t('workflow.node.reused')}`}
                    </span>
                    {elapsed !== null && <span className={styles.stepTime}>{clock(elapsed)}</span>}
                  </span>
                </li>
              );
            })}
          </ul>
          <p className={styles.counters}>
            {t('workflow.overview.counters', {
              running: progress.running + progress.waiting,
              pending: progress.pending,
              failed: progress.failed,
              blocked: progress.blocked,
            })}
          </p>
        </section>
        <section className={styles.overviewGroup} aria-label={t('workflow.overview.taskTitle')}>
          <h4 className={styles.groupTitle}>{t('workflow.overview.taskTitle')}</h4>
          <p className={styles.taskText}>{run.task}</p>
        </section>
        <VerdictList run={run} />
        {run.status === 'failed' && recovery && (
          <RecoveryPanel run={run} plan={recovery} busy={busy} onResume={onResume} />
        )}
        {run.recoveries.length > 0 && (
          <p className={styles.muted}>
            {t('workflow.recovery.count', { n: run.recoveries.length })}
          </p>
        )}
        <dl className={styles.facts}>
          <div>
            <dt>{t('workflow.overview.current')}</dt>
            <dd>{current.join(', ') || '—'}</dd>
          </div>
          <div>
            <dt>{t('workflow.overview.agents')}</dt>
            <dd>{agents.join(', ') || '—'}</dd>
          </div>
        </dl>
        <PlansPanel run={run} />
        {tools && (
          <WorkflowTools
            workspaceId={tools.workspaceId}
            workflow={run.workflow}
            onOpenHub={tools.onOpenHub}
          />
        )}
        <SharedStatePanel run={run} onOpenExecution={onOpenExecution} />
        <p className={styles.codeBlock}>
          {t('workflow.version', { version: run.workflowVersion })}
          {run.state.artifacts[0] && ` · ${shortId(run.state.artifacts[0].executionId)}`}
        </p>
      </div>
      {footer}
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
  tools?: ToolsProps;
}

/** The workflow as defined, and whether it can run. */
export function EditorOverview({
  workflow,
  validation,
  editable,
  onRename,
  repair,
  tools,
}: EditProps) {
  const { t } = useI18n();
  const errors = validation?.issues.filter((i) => i.severity === 'error') ?? [];
  const warnings = validation?.issues.filter((i) => i.severity === 'warning') ?? [];
  const routing = errors.filter(isRoutingIssue);
  const others = errors.filter((i) => !isRoutingIssue(i));
  return (
    <section className={styles.overview} aria-label={t('workflow.overview')}>
      <div className={styles.dockHead}>
        <h3 className={styles.inspectorTitle}>
          <Icon name="workflow" size={16} />
          {t('workflow.overview')}
        </h3>
      </div>
      <div className={styles.dockBody}>
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
                    <li
                      key={`${issue.code}-${issue.edgeId ?? issue.nodeId ?? ''}-${String(index)}`}
                    >
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
        {tools && (
          <WorkflowTools
            workspaceId={tools.workspaceId}
            workflow={workflow}
            onOpenHub={tools.onOpenHub}
          />
        )}
      </div>
    </section>
  );
}
