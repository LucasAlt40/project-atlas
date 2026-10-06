import type { TranslationKey } from '@/i18n';
import type { AttemptStatusDto, ResultFindingDto, ValidationEntryDto } from '@/lib/tauri/commands';
import { nodeLabel } from './status';
import type { WorkflowRun } from '../types';

/**
 * What the Review Workspace says about a finished run. Everything is read from what the run
 * already kept (its steps, results, handoffs, shared state, change set and integration); nothing
 * is measured again and nothing is invented: what the run does not know is absent.
 */

export const isFinished = (run: WorkflowRun): boolean =>
  run.status === 'completed' || run.status === 'failed' || run.status === 'cancelled';

// ---- timeline ---------------------------------------------------------------------------------

/** One pass of one agent step. The execution status and the outcome are two different facts. */
export interface TimelineStep {
  key: string;
  nodeId: string;
  label: string;
  agentId: string;
  executionId: string;
  attempt: number;
  iteration: number;
  /** How the execution ended. */
  status: AttemptStatusDto;
  /** What the agent declared (`pass`, `approved`…), when its agent has a result contract. */
  outcome: string | null;
  startedAt: number;
  completedAt: number | null;
  durationMs: number | null;
  failure: string | null;
}

/** Every attempt of every agent step, in the order they started. */
export function timelineOf(run: WorkflowRun): TimelineStep[] {
  const steps: { step: TimelineStep; order: number }[] = [];
  run.workflow.nodes.forEach((node, order) => {
    if (node.type !== 'agent') return;
    for (const attempt of run.nodes[node.id]?.attempts ?? []) {
      steps.push({
        step: {
          key: `${node.id}:${attempt.executionId}`,
          nodeId: node.id,
          label: node.label,
          agentId: node.agentId,
          executionId: attempt.executionId,
          attempt: attempt.attempt,
          iteration: attempt.iteration,
          status: attempt.status,
          outcome: attempt.outcome,
          startedAt: attempt.startedAt,
          completedAt: attempt.completedAt,
          durationMs:
            attempt.completedAt === null
              ? null
              : Math.max(0, attempt.completedAt - attempt.startedAt),
          failure: attempt.failure,
        },
        order,
      });
    }
  });
  return steps
    .sort(
      (a, b) =>
        a.step.startedAt - b.step.startedAt || a.order - b.order || a.step.attempt - b.step.attempt,
    )
    .map(({ step }) => step);
}

export function formatDuration(ms: number): string {
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${String(seconds)}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${String(minutes)}m ${String(seconds % 60)}s`;
  return `${String(Math.floor(minutes / 60))}h ${String(minutes % 60)}m`;
}

// ---- findings ---------------------------------------------------------------------------------

export type Severity = 'error' | 'warning' | 'info';

const ERROR_SEVERITIES = new Set(['error', 'critical', 'blocker', 'high', 'major', 'fatal']);
const WARNING_SEVERITIES = new Set(['warning', 'warn', 'medium', 'moderate', 'minor']);

/** The three kinds the Review tells apart. A severity it does not know is shown as information. */
export function severityOf(raw: string): Severity {
  const value = raw.trim().toLowerCase();
  if (ERROR_SEVERITIES.has(value)) return 'error';
  if (WARNING_SEVERITIES.has(value)) return 'warning';
  return 'info';
}

export interface ReviewFinding {
  key: string;
  severity: Severity;
  /** The severity as the agent wrote it. */
  rawSeverity: string;
  finding: ResultFindingDto;
  /** The step that reported it. */
  nodeId: string;
  label: string;
  executionId: string;
  /** What that step concluded in the same validation. */
  outcome: string | null;
  /** A later validation by the same step passed: the finding was dealt with, and is kept. */
  addressed: boolean;
}

const passed = (entry: ValidationEntryDto) => entry.status === 'pass' || entry.status === 'success';

/** Every finding of every validation, oldest first: a finding is never dropped once fixed. */
export function findingsOf(run: WorkflowRun): ReviewFinding[] {
  const entries = run.state.validationResults;
  const out: ReviewFinding[] = [];
  entries.forEach((entry, index) => {
    const addressed = entries
      .slice(index + 1)
      .some((later) => later.nodeId === entry.nodeId && passed(later));
    entry.findings.forEach((finding, n) => {
      out.push({
        key: `${entry.executionId}:${String(n)}`,
        severity: severityOf(finding.severity),
        rawSeverity: finding.severity,
        finding,
        nodeId: entry.nodeId,
        label: nodeLabel(run, entry.nodeId),
        executionId: entry.executionId,
        outcome: entry.outcome,
        addressed,
      });
    });
  });
  return out;
}

/** One step of the story a finding leads to: who ran, how it ended and, if it validated, what it found. */
export interface TrailStep {
  step: TimelineStep;
  validation: ValidationEntryDto | null;
}

/**
 * The sequence from the first validation that did not pass to the end of the run: the
 * validator's finding, whoever worked on it, and the validation that came after. Empty when no
 * validation ever failed or found anything.
 */
export function trailOf(run: WorkflowRun): TrailStep[] {
  const byExecution = new Map(run.state.validationResults.map((v) => [v.executionId, v]));
  const steps = timelineOf(run).map((step) => ({
    step,
    validation: byExecution.get(step.executionId) ?? null,
  }));
  const start = steps.findIndex(
    ({ validation }) =>
      validation !== null && (!passed(validation) || validation.findings.length > 0),
  );
  return start < 0 ? [] : steps.slice(start);
}

// ---- summary ----------------------------------------------------------------------------------

export interface Summary {
  steps: number;
  agents: number;
  files: number;
  additions: number;
  deletions: number;
  /** `null` while the run has no change set (nothing is known, not zero). */
  hasChangeSet: boolean;
  durationMs: number | null;
  /** The last outcome each agent step declared. */
  outcomes: { nodeId: string; label: string; outcome: string }[];
  findings: number;
  errors: number;
  warnings: number;
}

export function summaryOf(run: WorkflowRun): Summary {
  const timeline = timelineOf(run);
  const findings = findingsOf(run);
  const outcomes: Summary['outcomes'] = [];
  for (const node of run.workflow.nodes) {
    if (node.type !== 'agent') continue;
    const last = run.nodes[node.id]?.attempts.filter((a) => a.outcome !== null).at(-1);
    if (last?.outcome) outcomes.push({ nodeId: node.id, label: node.label, outcome: last.outcome });
  }
  return {
    steps: timeline.length,
    agents: new Set(timeline.map((s) => s.agentId)).size,
    files: run.changes?.filesChanged ?? 0,
    additions: run.changes?.additions ?? 0,
    deletions: run.changes?.deletions ?? 0,
    hasChangeSet: run.changes !== null,
    durationMs: run.completedAt === null ? null : Math.max(0, run.completedAt - run.startedAt),
    outcomes,
    findings: findings.length,
    errors: findings.filter((f) => f.severity === 'error').length,
    warnings: findings.filter((f) => f.severity === 'warning').length,
  };
}

// ---- review status x integration status -------------------------------------------------------

export interface ReviewState {
  review: TranslationKey;
  integration: TranslationKey;
  /** The code is in the project's working tree and nothing says it was committed. */
  applied: boolean;
}

/**
 * Reviewing and integrating are two things. Review is where the user stands in front of the
 * result (derived: nothing is saved about it); integration is what Atlas did with the code.
 * "Applied" only ever means "in the working tree, not committed".
 */
export function reviewStateOf(run: WorkflowRun): ReviewState {
  const incomplete = run.status !== 'completed';
  const ready: TranslationKey = incomplete ? 'review.state.incomplete' : 'review.state.ready';
  switch (run.integration.status) {
    case 'not_applicable':
      return {
        review: 'review.state.noCode',
        integration: 'review.integration.none',
        applied: false,
      };
    case 'in_progress':
      return {
        review: 'review.state.pending',
        integration: 'review.integration.pending',
        applied: false,
      };
    case 'no_changes':
      return {
        review: 'review.state.nothing',
        integration: 'review.integration.nothing',
        applied: false,
      };
    case 'integrated':
      return {
        review: 'review.state.reviewed',
        integration: 'review.integration.applied',
        applied: true,
      };
    case 'discarded':
      return {
        review: 'review.state.discarded',
        integration: 'review.integration.discarded',
        applied: false,
      };
    case 'kept_isolated':
      return { review: ready, integration: 'review.integration.kept', applied: false };
    case 'conflicts':
    case 'blocked':
      return { review: ready, integration: 'review.integration.blocked', applied: false };
    case 'failed':
      return { review: ready, integration: 'review.integration.failed', applied: false };
    case 'changes_available':
      return { review: ready, integration: 'review.integration.notApplied', applied: false };
  }
}

/** The files Git measured the step changing that the agent did not report, and the reverse. */
export function divergence(handoff: {
  changedFiles: { path: string }[];
  reportedFiles: string[];
}): { claimedOnly: string[]; detectedOnly: string[] } {
  const detected = new Set(handoff.changedFiles.map((f) => f.path));
  const claimed = new Set(handoff.reportedFiles);
  return {
    claimedOnly: handoff.reportedFiles.filter((p) => !detected.has(p)),
    detectedOnly: handoff.changedFiles.map((f) => f.path).filter((p) => !claimed.has(p)),
  };
}
