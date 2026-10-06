import type { TranslationKey } from '@/i18n';
import type { BlockReasonDto } from '@/lib/tauri/commands';
import type { Handoff, WorkflowRun } from '../types';

/**
 * What the screen may say about the code of a run. A run completing says nothing about the code
 * reaching the project: that is `integrated`, and only that. Everything else says, in words, that
 * the changes are still in the isolated worktree (or kept, or discarded, or blocked).
 */
export interface Delivery {
  /** How the run itself ended. */
  runLine: TranslationKey;
  /** Where the code stands. */
  codeLine: TranslationKey;
  params: Record<string, string | number>;
  tone: 'neutral' | 'pending' | 'done' | 'problem';
  /** The code is in the project. The only state in which the screen may say so. */
  inProject: boolean;
  review: boolean;
  openInIde: boolean;
  apply: boolean;
  keep: boolean;
  discard: boolean;
}

const BLOCKS: Record<BlockReasonDto, TranslationKey> = {
  policy_denied: 'integration.block.policy_denied',
  execution_not_completed: 'integration.block.execution_not_completed',
  validation_failed: 'integration.block.validation_failed',
  base_dirty: 'integration.block.base_dirty',
  base_branch_changed: 'integration.block.base_branch_changed',
  undetermined: 'integration.block.undetermined',
  uncommitted_changes: 'integration.block.uncommitted_changes',
  worktree_inconsistent: 'integration.block.worktree_inconsistent',
  conflict: 'integration.block.conflict',
  protected_paths: 'integration.block.protected_paths',
  needs_review: 'integration.block.needs_review',
};

const RUN_LINES = {
  completed: 'integration.run.completed',
  failed: 'integration.run.failed',
  cancelled: 'integration.run.cancelled',
} as const;

const NONE = {
  review: false,
  openInIde: false,
  apply: false,
  keep: false,
  discard: false,
};

/** The delivery of a finished run that has code to decide about; `null` when there is none. */
export function deliveryOf(run: WorkflowRun): Delivery | null {
  const { integration: code, changes } = run;
  const runLine = run.status in RUN_LINES ? RUN_LINES[run.status as keyof typeof RUN_LINES] : null;
  if (!runLine || code.status === 'not_applicable') return null;
  const files = changes?.filesChanged ?? 0;
  const base = {
    runLine,
    params: { files, count: code.conflicts.length, message: code.message ?? '' },
    inProject: false,
    ...NONE,
  };
  switch (code.status) {
    case 'no_changes':
      return { ...base, codeLine: 'integration.line.no_changes', tone: 'neutral' };
    case 'changes_available':
      return {
        ...base,
        codeLine: code.canApply ? 'integration.line.available' : 'integration.line.unfinished',
        tone: 'pending',
        review: true,
        openInIde: true,
        apply: code.canApply,
        keep: true,
        discard: true,
      };
    case 'conflicts':
      return {
        ...base,
        codeLine: 'integration.line.conflicts',
        tone: 'problem',
        review: true,
        openInIde: true,
        apply: code.canApply,
        keep: true,
        discard: true,
      };
    case 'blocked':
      return {
        ...base,
        codeLine: code.blockReason ? BLOCKS[code.blockReason] : 'integration.block.undetermined',
        tone: 'problem',
        review: true,
        openInIde: true,
        apply: code.canApply,
        keep: true,
        discard: true,
      };
    case 'integrated':
      return {
        ...base,
        codeLine: 'integration.line.integrated',
        tone: 'done',
        inProject: true,
        review: true,
        openInIde: true,
        // "Keep isolated" after applying takes the applied changes back out of the project.
        keep: code.canUndo,
      };
    case 'kept_isolated':
      return {
        ...base,
        codeLine: 'integration.line.kept',
        tone: 'pending',
        review: true,
        openInIde: true,
        apply: code.canApply || run.status === 'completed',
        discard: true,
      };
    case 'discarded':
      return { ...base, codeLine: 'integration.line.discarded', tone: 'neutral' };
    case 'failed':
      return {
        ...base,
        codeLine: 'integration.line.failed',
        tone: 'problem',
        review: true,
        openInIde: true,
        apply: code.canApply,
        keep: true,
        discard: true,
      };
    default:
      // `in_progress` on a run that has ended: the core has not concluded it yet.
      return null;
  }
}

/** The handoffs that travelled over one transition (an edge, or `failure:<node>`), oldest first. */
export function handoffsOver(run: WorkflowRun, linkId: string): Handoff[] {
  return run.handoffs.filter((h) => h.linkId === linkId);
}

/** What a node was handed (`received`) and what it handed on (`sent`). */
export function handoffsOf(
  run: WorkflowRun,
  nodeId: string,
): { received: Handoff[]; sent: Handoff[] } {
  return {
    received: run.handoffs.filter((h) => h.toNodeId === nodeId),
    sent: run.handoffs.filter((h) => h.fromNodeId === nodeId),
  };
}
