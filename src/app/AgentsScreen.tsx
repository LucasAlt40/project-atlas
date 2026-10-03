import { useState } from 'react';
import { AgentsPage } from '@/features/agents/pages/AgentsPage';
import type { Agent } from '@/features/agents/types';
import { useWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import { errorMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import { useNavigation } from './NavigationContext';

/**
 * The Agents screen as a step of the workspace flow: an agent created here is placed in the
 * active workspace and the workspace opens, so the user never configures it twice. The agent
 * itself stays global; only its placement belongs to the workspace.
 */
export function AgentsScreen() {
  const t = useT();
  const { intent, navigate } = useNavigation();
  const { addAgent, active } = useWorkspace();
  const [notice, setNotice] = useState<string | null>(null);

  function onAgentCreated(agent: Agent) {
    if (!active) {
      setNotice(t('agents.noWorkspace', { name: agent.name }));
      return;
    }
    void addAgent(agent.id).then((result) => {
      if (result.ok) {
        navigate('workspace');
      } else {
        const reason = result.capacity
          ? t('workspace.capacity', { max: 4 })
          : errorMessage(t, result.error);
        setNotice(t('agents.createdNotPlaced', { name: agent.name, reason }));
      }
    });
  }

  return (
    <AgentsPage
      startCreating={
        intent?.type === 'create-agent'
          ? intent.personalityId === undefined
            ? {}
            : { personalityId: intent.personalityId }
          : undefined
      }
      onAgentCreated={onAgentCreated}
      notice={notice}
    />
  );
}
