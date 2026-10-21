import { StateBadge, type StateKind } from '@/components/ui/status';
import { useT } from '@/i18n/I18nProvider';
import type { AgentStatus } from '../model/agentStatus';

/** Where each state of an agent falls in the app's one vocabulary of states. */
export const AGENT_STATE: Record<AgentStatus, StateKind> = {
  ready: 'ready',
  running: 'running',
  waiting: 'running',
  responding: 'running',
  stopping: 'running',
  waiting_approval: 'waiting',
  waiting_for_input: 'waiting',
  completed: 'completed',
  cancelled: 'cancelled',
  failed: 'failed',
  authentication_required: 'unavailable',
  unavailable: 'unavailable',
};

export function StatusBadge({ status }: { status: AgentStatus }) {
  const t = useT();
  return (
    <span role="status" style={{ display: 'inline-flex' }}>
      <StateBadge state={AGENT_STATE[status]}>{t(`agent.status.${status}`)}</StateBadge>
    </span>
  );
}
