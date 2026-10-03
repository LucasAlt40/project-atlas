import type { RuntimeStatus } from '@/features/agents/types';
import type { AgentRun } from './agentRuns';

/** What the card shows. Each value comes from real execution or runtime state. */
export type AgentStatus =
  | 'ready'
  | 'running'
  | 'waiting'
  | 'responding'
  | 'completed'
  | 'failed'
  | 'authentication_required'
  | 'unavailable';

/**
 * - an execution in progress decides first ("waiting" only once the core says the prompt was
 *   sent and it is waiting; "responding" only while answer text is really arriving);
 * - a finished execution decides next;
 * - otherwise the runtime's detected availability does.
 */
export function agentStatus(
  run: AgentRun | undefined,
  runtime: RuntimeStatus | undefined,
): AgentStatus {
  if (run?.status === 'running') {
    if (run.receiving) return 'responding';
    const last = run.activity.at(-1);
    return last?.kind === 'waiting_for_model' ? 'waiting' : 'running';
  }
  if (run?.status === 'failed') {
    return run.failureKind === 'authentication_required' ? 'authentication_required' : 'failed';
  }
  if (run?.status === 'completed') return 'completed';
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
