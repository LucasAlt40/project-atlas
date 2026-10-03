import { invokeCommand } from '@/lib/tauri/commands';
import type {
  Agent,
  CreateAgentInput,
  CreatePersonalityInput,
  Personality,
  RuntimeStatus,
} from '../types';

/** The UI's only door to the Rust core for personalities, agents and runtimes (global data). */

export function listPersonalities(): Promise<Personality[]> {
  return invokeCommand('list_personalities');
}

export function createPersonality(request: CreatePersonalityInput): Promise<Personality> {
  return invokeCommand('create_personality', { request });
}

/** Editing a built-in personality saves your own version of it. */
export function updatePersonality(
  id: string,
  request: CreatePersonalityInput,
): Promise<Personality> {
  return invokeCommand('update_personality', { id, request });
}

/** Refused while agents use the personality; a built-in is hidden, not destroyed. */
export async function deletePersonality(id: string): Promise<void> {
  await invokeCommand('delete_personality', { id });
}

/** Brings back removed built-in personalities and undoes edits to them. */
export function restoreDefaultPersonalities(): Promise<Personality[]> {
  return invokeCommand('restore_default_personalities');
}

/** Inspects the machine for AI runtimes; can take a few seconds. */
export function listRuntimes(): Promise<RuntimeStatus[]> {
  return invokeCommand('list_runtimes');
}

export function listAgents(): Promise<Agent[]> {
  return invokeCommand('list_agents');
}

export function createAgent(request: CreateAgentInput): Promise<Agent> {
  return invokeCommand('create_agent', { request });
}

export function updateAgent(id: string, request: CreateAgentInput): Promise<Agent> {
  return invokeCommand('update_agent', { id, request });
}

/** Also removes the agent from every workspace and discards its conversations. */
export async function deleteAgent(id: string): Promise<void> {
  await invokeCommand('delete_agent', { id });
}
