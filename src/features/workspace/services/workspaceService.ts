import { invokeCommand } from '@/lib/tauri/commands';
import { listenToEvent } from '@/lib/tauri/events';
import type {
  ExecutionEvent,
  Message,
  ProjectContext,
  SentMessage,
  Workspace,
  WorkspaceInput,
} from '../types';

/** The UI's only door to the Rust core for workspaces and agent conversations. */

export function listWorkspaces(): Promise<Workspace[]> {
  return invokeCommand('list_workspaces');
}

export function createWorkspace(input: WorkspaceInput): Promise<Workspace> {
  return invokeCommand('create_workspace', { input });
}

/** Renames the workspace and/or points it at another project folder. */
export function updateWorkspace(id: string, input: WorkspaceInput): Promise<Workspace> {
  return invokeCommand('update_workspace', { id, input });
}

/** Also discards the workspace's conversations and usage; agents are kept. */
export function deleteWorkspace(id: string): Promise<Workspace[]> {
  return invokeCommand('delete_workspace', { id });
}

export function addAgentToWorkspace(workspaceId: string, agentId: string): Promise<Workspace> {
  return invokeCommand('add_agent_to_workspace', { workspaceId, agentId });
}

/** Frees the agent's position. The agent itself is kept. */
export function removeAgentFromWorkspace(workspaceId: string, agentId: string): Promise<Workspace> {
  return invokeCommand('remove_agent_from_workspace', { workspaceId, agentId });
}

/** Folder name, path and the technologies recognised in the project folder. */
export function getProjectContext(workspaceId: string): Promise<ProjectContext> {
  return invokeCommand('get_project_context', { workspaceId });
}

export interface SendMessageInput {
  workspaceId: string;
  agentId: string;
  content: string;
}

/**
 * Returns as soon as the message is recorded and the execution started; the answer
 * arrives through the events below.
 */
export function sendMessage(input: SendMessageInput): Promise<SentMessage> {
  return invokeCommand('send_message', { request: input });
}

/** Every conversation of every workspace. */
export function listMessages(): Promise<Message[]> {
  return invokeCommand('list_messages', {});
}

export function subscribeToExecutionEvents(
  handler: (event: ExecutionEvent) => void,
): Promise<() => void> {
  return listenToEvent('execution:progress', handler);
}

export function subscribeToMessages(handler: (message: Message) => void): Promise<() => void> {
  return listenToEvent('conversation:message', handler);
}
