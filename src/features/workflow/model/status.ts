import type { Translate, TranslationKey } from '@/i18n';
import type { NodeStatusDto, WorkflowFailureDto } from '@/lib/tauri/commands';
import type { WorkflowRun, WorkflowRunStatus } from '../types';

/**
 * How each state is shown. Never by colour alone: every state has a symbol and a word as well.
 */
export const NODE_STATUS_VIEW: Record<NodeStatusDto, { symbol: string; label: TranslationKey }> = {
  pending: { symbol: '○', label: 'workflow.status.pending' },
  ready: { symbol: '◔', label: 'workflow.status.ready' },
  running: { symbol: '●', label: 'workflow.status.running' },
  waiting_approval: { symbol: '⏸', label: 'workflow.status.waiting_approval' },
  waiting_for_input: { symbol: '?', label: 'workflow.status.waiting_for_input' },
  completed: { symbol: '✓', label: 'workflow.status.completed' },
  failed: { symbol: '✕', label: 'workflow.status.failed' },
  blocked: { symbol: '⛔', label: 'workflow.status.blocked' },
  cancelled: { symbol: '⊘', label: 'workflow.status.cancelled' },
  skipped: { symbol: '↷', label: 'workflow.status.skipped' },
};

export const RUN_STATUS_LABEL: Record<WorkflowRunStatus, TranslationKey> = {
  running: 'workflow.run.running',
  paused: 'workflow.run.paused',
  waiting_for_input: 'workflow.run.waiting_for_input',
  completed: 'workflow.run.completed',
  failed: 'workflow.run.failed',
  cancelled: 'workflow.run.cancelled',
  interrupted: 'workflow.run.interrupted',
};

export const FAILURE_LABEL: Record<WorkflowFailureDto['code'], TranslationKey> = {
  max_iterations_reached: 'workflow.failure.max_iterations_reached',
  node_failed: 'workflow.failure.node_failed',
  no_path_to_completion: 'workflow.failure.no_path_to_completion',
  no_route_matched: 'workflow.failure.no_route_matched',
  ended_in_failure: 'workflow.failure.ended_in_failure',
  invalid_workflow: 'workflow.failure.invalid_workflow',
  internal_error: 'workflow.failure.internal_error',
  worktree_unavailable: 'workflow.failure.worktree_unavailable',
};

export interface Progress {
  /** Steps that are agent or condition work (End nodes are not counted). */
  total: number;
  completed: number;
  running: number;
  waiting: number;
  pending: number;
  failed: number;
  blocked: number;
}

/** `3 / 5 nodes completed`, and what the rest are doing. */
export function runProgress(run: WorkflowRun): Progress {
  const counted = run.workflow.nodes.filter((n) => n.type !== 'end');
  const count = (...statuses: NodeStatusDto[]) =>
    counted.filter((n) => statuses.includes(run.nodes[n.id]?.status ?? 'pending')).length;
  return {
    total: counted.length,
    completed: count('completed'),
    running: count('running'),
    waiting: count('waiting_approval', 'waiting_for_input'),
    pending: count('pending', 'ready'),
    failed: count('failed'),
    blocked: count('blocked'),
  };
}

/** The label of a node in the snapshot the run is on. */
export function nodeLabel(run: WorkflowRun, nodeId: string | null | undefined): string {
  if (!nodeId) return '';
  return run.workflow.nodes.find((n) => n.id === nodeId)?.label ?? nodeId;
}

/** The agents working right now, by node. */
export function activeNodes(run: WorkflowRun): string[] {
  return run.state.currentNodes;
}

/** Why a run failed, in words (the same text the run overview shows). Empty if it did not fail. */
export function failureText(t: Translate, run: WorkflowRun): string {
  const failure = run.failure;
  if (!failure) return '';
  return t(
    failure.code === 'no_route_matched' && failure.detail
      ? 'workflow.failure.no_route_outcome'
      : FAILURE_LABEL[failure.code],
    {
      node: nodeLabel(run, failure.nodeId),
      loop: failure.detail ?? '',
      outcome: failure.detail ?? '',
    },
  );
}
