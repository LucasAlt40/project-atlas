import type { RuntimeStatus } from '@/features/agents/types';
import type { AgentRun, ProcessState } from './agentRuns';

/** What the card shows. Each value comes from real execution or runtime state. */
export type AgentStatus =
  | 'ready'
  | 'running'
  | 'waiting'
  | 'responding'
  | 'waiting_approval'
  | 'stopping'
  | 'completed'
  | 'cancelled'
  | 'failed'
  | 'authentication_required'
  | 'unavailable';

/**
 * - an execution in progress decides first ("waiting_approval" only while the core holds a
 *   command for the user's decision; "waiting" only once the core says the prompt was
 *   sent and it is waiting; "responding" only while answer text is really arriving);
 * - a finished execution decides next;
 * - otherwise the runtime's detected availability does.
 *
 * `process` is the live process of the run, when it has one: while the user's interrupt or
 * terminate has not yet ended it, the agent is "stopping".
 */
export function agentStatus(
  run: AgentRun | undefined,
  runtime: RuntimeStatus | undefined,
  process?: ProcessState,
): AgentStatus {
  if (run?.status === 'running') {
    if (process?.status === 'interrupting' || process?.status === 'terminating') return 'stopping';
    // Blocked on the user, which is a real state of the core, not a timer.
    if (run.pendingApprovalId) return 'waiting_approval';
    if (run.receiving) return 'responding';
    const last = run.activity.at(-1);
    return last?.kind === 'waiting_for_model' ? 'waiting' : 'running';
  }
  if (run?.status === 'failed') {
    return run.failureKind === 'authentication_required' ? 'authentication_required' : 'failed';
  }
  if (run?.status === 'completed') return 'completed';
  if (run?.status === 'cancelled') return 'cancelled';
  switch (runtime?.availability) {
    case 'authentication_required':
      return 'authentication_required';
    case 'not_installed':
    case 'unavailable':
      return 'unavailable';
    default:
      return 'ready';
  }
}
