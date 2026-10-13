import type { Agent } from '@/features/agents/types';
import type { Workspace } from '@/features/workspace/types';

/** The agents placed in the workspace, in the order the workspace lists them. */
export function agentsOf(workspace: Workspace, agents: Agent[]): Agent[] {
  return workspace.layout.agentPlacements
    .map((placement) => agents.find((agent) => agent.id === placement.agentId))
    .filter((agent): agent is Agent => agent !== undefined);
}

/**
 * Who should write a spec when the person has not said: an architect if the workspace has one,
 * else whoever is first. A suggestion the form shows, never a choice made for them.
 */
export function suggestedAuthor(agents: Agent[]): Agent | undefined {
  return agents.find((agent) => agent.personalityId === 'architect') ?? agents[0];
}
