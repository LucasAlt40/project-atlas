import type { Agent, Personality, RuntimeStatus } from '@/features/agents/types';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { AgentRun } from '../model/agentRuns';
import { agentStatus } from '../model/agentStatus';
import type { Message } from '../types';
import { AgentActivity } from './AgentActivity';
import styles from './AgentCard.module.css';
import { AgentComposer } from './AgentComposer';
import { AgentHeader } from './AgentHeader';
import { AgentMessageList } from './AgentMessageList';

interface Props {
  agent: Agent;
  personality: Personality | undefined;
  runtime: RuntimeStatus | undefined;
  messages: Message[];
  run: AgentRun | undefined;
  liveText: string | undefined;
  /** Why the last send was rejected, if it was. */
  sendError: unknown;
  onSend: (content: string) => void;
  onOpenDetails: () => void;
  onEdit: () => void;
  onRemove: () => void;
}

/**
 * One workspace item. It knows nothing about positions or other agents, so it can later be
 * dragged, resized, pinned or maximized by whatever lays it out.
 */
export function AgentCard({
  agent,
  personality,
  runtime,
  messages,
  run,
  liveText,
  sendError,
  onSend,
  onOpenDetails,
  onEdit,
  onRemove,
}: Props) {
  const t = useT();
  const status = agentStatus(run, runtime);
  return (
    <article className={styles.card} aria-label={agent.name} data-status={status}>
      <AgentHeader
        name={agent.name}
        personalityName={personality?.name ?? agent.personalityId}
        runtimeName={runtime?.runtime.name ?? agent.runtimeId}
        providerName={runtime?.runtime.provider.name ?? null}
        modelId={agent.modelId}
        status={status}
        onOpenDetails={onOpenDetails}
        onEdit={onEdit}
        onRemove={onRemove}
      />
      <AgentMessageList messages={messages} liveText={liveText} />
      {run && <AgentActivity run={run} />}
      {sendError !== undefined && (
        <p role="alert" className={styles.error}>
          {errorMessage(t, sendError)}
        </p>
      )}
      <AgentComposer agentName={agent.name} disabled={run?.status === 'running'} onSend={onSend} />
    </article>
  );
}
