import { useState, type ReactNode } from 'react';
import { Icon } from '@/components/ui/Icon';
import { useI18n } from '@/i18n/I18nProvider';
import type { TranslationKey } from '@/i18n';
import { useLiveWorkspace } from '../hooks/useLiveWorkspace';
import {
  findingsOf,
  formatDuration,
  isFinished,
  summaryOf,
  timelineOf,
  trailOf,
  type ReviewFinding,
  type TimelineStep,
} from '../model/review';
import { nodeLabel } from '../model/status';
import type { Ide, WorkflowRun } from '../types';
import { DeliveryPanel, type CodeActions } from './DeliveryPanel';
import { FileLine, FindingText, HandoffList } from './HandoffView';
import { LiveWorkspaceView } from './LiveWorkspacePanel';
import styles from './Review.module.css';
import flow from './Workflow.module.css';

/** What the page knows about an agent and an execution that the run itself does not keep. */
export interface ReviewContext {
  personality: (agentId: string) => string;
  agentName: (agentId: string) => string;
  /** The runtime and model an execution ran with, when the execution is still stored. */
  executionOf: (executionId: string) => { runtime: string; model: string } | undefined;
}

interface Props {
  run: WorkflowRun;
  ides: Ide[];
  busy: boolean;
  code: CodeActions;
  context: ReviewContext;
  onOpenExecution: (executionId: string) => void;
}

/**
 * The Live Workspace once the run is over: what the agents did, what they found and decided,
 * what changed in the run's worktree and what happens if the user applies it. It evolves the live
 * view (the same panel, in REVIEW) rather than replacing it, and decides nothing: Apply, keep and
 * discard are the delivery panel's, and each one is the user's.
 */
export function ReviewWorkspace({ run, ides, busy, code, context, onOpenExecution }: Props) {
  const { t } = useI18n();
  const hasWorktree = run.integration.worktreeExecutionId !== null;
  // The worktree is followed once, here: the panel below and the actions both read from it.
  const live = useLiveWorkspace(hasWorktree ? run.id : null);
  const availability =
    live.load.status === 'ready' && live.load.model !== null
      ? live.load.model.state.availability
      : 'unknown';
  const summary = summaryOf(run);
  const findings = findingsOf(run);
  const trail = trailOf(run);
  const { decisions, artifacts } = run.state;

  if (!isFinished(run)) return null;

  return (
    <section className={styles.review} aria-label={t('review.title')} data-status={run.status}>
      <header className={styles.head}>
        <span className={styles.mode}>{t('live.mode.review')}</span>
        <h3 className={styles.title}>{t('review.title')}</h3>
        <span className={styles.headline} data-status={run.status}>
          {t(`review.run.${run.status as 'completed' | 'failed' | 'cancelled'}` as TranslationKey)}
        </span>
      </header>

      <ul className={styles.stats} aria-label={t('review.summary')}>
        <li>{t('review.stat.steps', { n: summary.steps })}</li>
        <li>{t('review.stat.agents', { n: summary.agents })}</li>
        <li>
          {summary.hasChangeSet
            ? t('review.stat.files', { n: summary.files })
            : t('review.stat.noChangeSet')}
          {summary.hasChangeSet && summary.files > 0 && (
            <span className={styles.lines}>
              {' '}
              <span data-sign="add">+{summary.additions}</span>{' '}
              <span data-sign="del">−{summary.deletions}</span>
            </span>
          )}
        </li>
        {summary.durationMs !== null && <li>{formatDuration(summary.durationMs)}</li>}
        <li data-alert={summary.errors > 0}>
          {summary.findings === 0
            ? t('review.stat.noFindings')
            : t('review.stat.findings', {
                n: summary.findings,
                errors: summary.errors,
                warnings: summary.warnings,
              })}
        </li>
      </ul>
      {summary.outcomes.length > 0 && (
        <ul className={styles.outcomes} aria-label={t('review.outcomes')}>
          {summary.outcomes.map((o) => (
            <li key={o.nodeId}>
              <span>{o.label}</span>
              <strong data-outcome={o.outcome}>{o.outcome.toUpperCase()}</strong>
            </li>
          ))}
        </ul>
      )}

      {(availability === 'missing' || availability === 'invalid') && (
        <p className={styles.notice} role="status">
          {t('review.worktreeGone')}
        </p>
      )}

      <DeliveryPanel run={run} ides={ides} busy={busy} actions={code} worktree={availability} />

      <Section title={t('review.timeline')} count={summary.steps} defaultOpen>
        <Timeline run={run} context={context} onOpenExecution={onOpenExecution} />
      </Section>

      <Section
        title={t('review.findings')}
        count={findings.length}
        defaultOpen={findings.length > 0}
      >
        <FindingsView run={run} findings={findings} trail={trail} context={context} />
      </Section>

      <Section title={t('review.handoffs')} count={run.handoffs.length}>
        <HandoffList run={run} handoffs={run.handoffs} onOpenExecution={onOpenExecution} />
      </Section>

      <Section title={t('review.decisions')} count={decisions.length}>
        {decisions.length === 0 ? (
          <p className={flow.muted}>{t('review.decisions.none')}</p>
        ) : (
          <ul className={styles.cards}>
            {decisions.map((d) => (
              <li key={d.id} className={styles.card}>
                <strong>{d.title || d.decision}</strong>
                {d.title && <p className={styles.body}>{d.decision}</p>}
                {d.rationale && (
                  <p className={flow.muted}>
                    {t('review.decision.context')}: {d.rationale}
                  </p>
                )}
                <p className={flow.muted}>
                  {t('review.decision.by', { agent: nodeLabel(run, d.sourceNodeId) })}
                  {' · '}
                  {new Date(d.createdAt).toLocaleString()}
                </p>
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title={t('review.artifacts')} count={artifacts.length}>
        {artifacts.length === 0 ? (
          <p className={flow.muted}>{t('review.artifacts.none')}</p>
        ) : (
          <ul className={styles.cards}>
            {artifacts.map((a) => (
              <li key={a.id} className={styles.card}>
                <strong>{a.name}</strong>{' '}
                <span className={flow.muted}>
                  {t(`workflow.artifact.${a.type}` as TranslationKey)} ·{' '}
                  {nodeLabel(run, a.producerNodeId)}
                </span>
                {a.summary && <p className={styles.body}>{a.summary}</p>}
                {a.path && <p className={flow.filePath}>{a.path}</p>}
              </li>
            ))}
          </ul>
        )}
      </Section>

      <Section title={t('review.changeSet')} count={run.changes?.filesChanged ?? 0} defaultOpen>
        <ChangeSetView run={run} onOpen={code.review} />
      </Section>

      <LiveWorkspaceView run={run} live={live} />
    </section>
  );
}

function Section({
  title,
  count,
  defaultOpen = false,
  children,
}: {
  title: string;
  count: number;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className={styles.section}>
      <button
        type="button"
        className={styles.sectionHead}
        aria-expanded={open}
        onClick={() => {
          setOpen((value) => !value);
        }}
      >
        <Icon name={open ? 'chevronDown' : 'chevronRight'} size={14} />
        <span>{title}</span>
        <span className={styles.count}>{count}</span>
      </button>
      {open && <div className={styles.sectionBody}>{children}</div>}
    </div>
  );
}

// ---- timeline ---------------------------------------------------------------------------------

function StepCard({
  step,
  context,
  onOpenExecution,
}: {
  step: TimelineStep;
  context: ReviewContext;
  onOpenExecution?: (executionId: string) => void;
}) {
  const { t } = useI18n();
  const personality = context.personality(step.agentId);
  const ran = context.executionOf(step.executionId);
  return (
    <div className={styles.step} data-status={step.status}>
      <div className={styles.stepHead}>
        <strong>{step.label}</strong>
        {step.iteration > 1 && (
          <span className={flow.muted}>{t('handoff.pass', { n: step.iteration })}</span>
        )}
        <span className={styles.status} data-status={step.status}>
          {t(`review.attempt.${step.status}` as TranslationKey)}
        </span>
        {step.outcome && (
          <span className={styles.outcome} data-outcome={step.outcome}>
            {step.outcome.toUpperCase()}
          </span>
        )}
      </div>
      <p className={styles.stepMeta}>
        {[
          context.agentName(step.agentId),
          personality,
          ran ? `${ran.runtime} / ${ran.model}` : '',
          step.durationMs === null ? '' : formatDuration(step.durationMs),
        ]
          .filter((part) => part !== '')
          .join(' · ')}
      </p>
      {step.failure && <p className={flow.failure}>{step.failure}</p>}
      {onOpenExecution && (
        <button
          type="button"
          className={flow.link}
          onClick={() => {
            onOpenExecution(step.executionId);
          }}
        >
          {t('review.openExecution')}
        </button>
      )}
    </div>
  );
}

function Timeline({
  run,
  context,
  onOpenExecution,
}: {
  run: WorkflowRun;
  context: ReviewContext;
  onOpenExecution: (executionId: string) => void;
}) {
  const { t } = useI18n();
  const steps = timelineOf(run);
  if (steps.length === 0) return <p className={flow.muted}>{t('review.timeline.none')}</p>;
  return (
    <ol className={styles.timeline} aria-label={t('review.timeline')}>
      {steps.map((step) => (
        <li key={step.key}>
          <StepCard step={step} context={context} onOpenExecution={onOpenExecution} />
        </li>
      ))}
    </ol>
  );
}

// ---- findings ---------------------------------------------------------------------------------

function FindingsView({
  run,
  findings,
  trail,
  context,
}: {
  run: WorkflowRun;
  findings: ReviewFinding[];
  trail: ReturnType<typeof trailOf>;
  context: ReviewContext;
}) {
  const { t } = useI18n();
  if (findings.length === 0 && trail.length === 0) {
    return <p className={flow.muted}>{t('review.findings.none')}</p>;
  }
  return (
    <>
      {findings.length > 0 && (
        <ul className={styles.findings} aria-label={t('review.findings')}>
          {findings.map((f) => (
            <li key={f.key} className={styles.finding} data-severity={f.severity}>
              <div className={styles.findingHead}>
                <span className={styles.severity} data-severity={f.severity}>
                  {t(`review.severity.${f.severity}` as TranslationKey)}
                </span>
                <span className={flow.muted}>
                  {t('review.finding.by', { agent: f.label })}
                  {f.outcome ? ` · ${f.outcome.toUpperCase()}` : ''}
                </span>
                <span className={styles.status} data-addressed={f.addressed}>
                  {t(f.addressed ? 'review.finding.addressed' : 'review.finding.open')}
                </span>
              </div>
              <p className={styles.body}>
                <FindingText finding={f.finding} />
              </p>
              {f.finding.recommendation && (
                <p className={flow.muted}>
                  {t('review.finding.recommendation')}: {f.finding.recommendation}
                </p>
              )}
            </li>
          ))}
        </ul>
      )}
      {trail.length > 0 && (
        <>
          <h5 className={styles.subtitle}>{t('review.trail')}</h5>
          <ol className={styles.timeline} aria-label={t('review.trail')}>
            {trail.map(({ step, validation }) => (
              <li key={step.key}>
                <StepCard step={step} context={context} />
                {validation && (
                  <p className={styles.trailNote}>
                    {validation.findings.length > 0
                      ? t('review.trail.found', { n: validation.findings.length })
                      : t('review.trail.clean')}
                    {validation.summary ? ` — ${validation.summary}` : ''}
                  </p>
                )}
              </li>
            ))}
          </ol>
          <p className={flow.muted}>{t('review.trail.note', { run: run.id.slice(0, 8) })}</p>
        </>
      )}
    </>
  );
}

// ---- change set -------------------------------------------------------------------------------

function ChangeSetView({ run, onOpen }: { run: WorkflowRun; onOpen: (file?: string) => void }) {
  const { t } = useI18n();
  const files = run.changes?.files ?? [];
  if (files.length === 0) {
    return <p className={flow.muted}>{t('review.noChangesToApply')}</p>;
  }
  const by = (path: string) => [
    ...new Set(
      run.handoffs
        .filter((h) => h.changedFiles.some((f) => f.path === path))
        .map((h) => nodeLabel(run, h.fromNodeId)),
    ),
  ];
  return (
    <>
      <p className={flow.muted}>{t('review.changeSet.scope')}</p>
      <ul className={flow.fileList}>
        {files.map((file) => {
          const agents = by(file.path);
          return (
            <li key={file.path}>
              <button
                type="button"
                className={flow.fileButton}
                aria-label={t('changes.open', { path: file.path })}
                onClick={() => {
                  onOpen(file.path);
                }}
              >
                <FileLine file={file} />
              </button>
              {agents.length > 0 && (
                <span className={flow.muted}>
                  {' '}
                  · {t('review.changeSet.by', { agents: agents.join(', ') })}
                </span>
              )}
            </li>
          );
        })}
      </ul>
      <p>
        <button
          type="button"
          className={flow.link}
          onClick={() => {
            onOpen();
          }}
        >
          {t('review.changeSet.all')}
        </button>
      </p>
    </>
  );
}
