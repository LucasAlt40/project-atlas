import type { Agent } from '@/features/agents/types';
import type { Translate } from '@/i18n';
import type { Workspace } from '@/features/workspace/types';

/** The agents placed in the workspace, in the order the workspace lists them. */
export function agentsOf(workspace: Workspace, agents: Agent[]): Agent[] {
  return workspace.layout.agentPlacements
    .map((placement) => agents.find((agent) => agent.id === placement.agentId))
    .filter((agent): agent is Agent => agent !== undefined);
}

/** The built-in personality made for writing specs: its agents are the recommended authors. */
export const SPEC_AUTHOR_PERSONALITY = 'spec-architect';

/** Whether the agent is a recommended spec author. A recommendation, never a requirement. */
export function isRecommendedAuthor(agent: Agent): boolean {
  return agent.personalityId === SPEC_AUTHOR_PERSONALITY;
}

/**
 * Who should write a spec when the person has not said: a spec architect if the workspace has
 * one, else an architect, else whoever is first. A suggestion the form shows, never a choice
 * made for them.
 */
export function suggestedAuthor(agents: Agent[]): Agent | undefined {
  return (
    agents.find(isRecommendedAuthor) ??
    agents.find((agent) => agent.personalityId === 'architect') ??
    agents[0]
  );
}

/** The agent's name as the author choice shows it, marking the recommended ones. */
export function authorName(t: Translate, agent: Agent): string {
  return isRecommendedAuthor(agent) ? t('specs.new.recommended', { name: agent.name }) : agent.name;
}
