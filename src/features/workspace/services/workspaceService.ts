import { invokeCommand } from '@/lib/tauri/commands';
import { listenToEvent } from '@/lib/tauri/events';
import type {
  AgentPermissionsDto,
  ExecutionRefDto,
  ExecutionWorktreeDto,
  HarnessProgressDto,
  HarnessSummaryDto,
  InitializeInputDto,
  InitializeOutcomeDto,
  ProjectAnalysisDto,
  RefreshOutcomeDto,
  SemanticRequestDto,
  SessionStatusEventDto,
  TerminalChunkDto,
  TerminalSnapshotDto,
  PendingApprovalDto,
  WorkspaceSecurityDto,
} from '@/lib/tauri/commands';
import type {
  ExecutionEvent,
  Message,
  ProjectContext,
  SentMessage,
  StoredExecution,
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

/**
 * Reads the project and reports what it found. Writes nothing; no project code is run. With
 * `semantic`, an agent also explores the project (or, when `restricted`, reads only selected
 * evidence) and proposes findings. That can take minutes.
 */
export function analyzeProject(
  workspaceId: string,
  semantic?: SemanticRequestDto,
): Promise<ProjectAnalysisDto> {
  return invokeCommand('analyze_project', semantic ? { workspaceId, semantic } : { workspaceId });
}

/** Creates or updates the project's `.atlas/` Harness from the user's review. */
export function initializeProject(
  workspaceId: string,
  input: InitializeInputDto,
): Promise<InitializeOutcomeDto> {
  return invokeCommand('initialize_project', { workspaceId, input });
}

/** Whether the project's Harness exists, is valid, or needs a look. */
export function getProjectHarness(workspaceId: string): Promise<HarnessSummaryDto> {
  return invokeCommand('get_project_harness', { workspaceId });
}

/**
 * Compares a new analysis with the Harness. Without `confirm` nothing is written: the result is
 * the diff and the conflicts. With it, generated files are updated and the user's own are kept.
 */
export function refreshProjectHarness(
  workspaceId: string,
  confirm = false,
): Promise<RefreshOutcomeDto> {
  return invokeCommand('refresh_project_harness', { workspaceId, confirm });
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

/** Every execution that has ended, newest first, in every workspace. */
export function listExecutions(): Promise<StoredExecution[]> {
  return invokeCommand('list_executions', {});
}

/** What agents may do in the workspace, as the core enforces it. Read-only. */
export function getWorkspaceSecurity(workspaceId: string): Promise<WorkspaceSecurityDto> {
  return invokeCommand('get_workspace_security', { workspaceId });
}

/** An agent's profile and permissions in a workspace, including its runtime's own limits. */
export function getAgentPermissions(
  workspaceId: string,
  agentId: string,
): Promise<AgentPermissionsDto> {
  return invokeCommand('get_agent_permissions', { workspaceId, agentId });
}

/** The workspace's policy still bounds whatever profile is chosen. */
export function setAgentPermissionProfile(agentId: string, profileId: string) {
  return invokeCommand('set_agent_permission_profile', { agentId, profileId });
}

/** Commands waiting for the user's decision (to recover them after a reload). */
export function listPendingApprovals(workspaceId?: string): Promise<PendingApprovalDto[]> {
  return invokeCommand('list_pending_approvals', workspaceId ? { workspaceId } : {});
}

/**
 * The user's explicit answer to an approval request. Only UI buttons call this: nothing the
 * model writes is ever turned into an answer.
 */
export async function resolveApproval(approvalId: string, approve: boolean): Promise<void> {
  await invokeCommand('resolve_approval', { approvalId, approve });
}

/** The Git worktrees of every execution that ran isolated, oldest first. */
export function listExecutionWorktrees(): Promise<ExecutionWorktreeDto[]> {
  return invokeCommand('list_execution_worktrees', {});
}

/**
 * The user's explicit "merge this execution's work". Only a button calls this: nothing the
 * model writes is ever turned into a merge. Resolves with what became of the worktree; a merge
 * Git could not do (conflict, blocked) is a result, not an error.
 */
export function mergeExecution(ref: ExecutionRefDto): Promise<ExecutionWorktreeDto> {
  return invokeCommand('merge_execution', { ...ref });
}

export function subscribeToExecutionEvents(
  handler: (event: ExecutionEvent) => void,
): Promise<() => void> {
  return listenToEvent('execution:progress', handler);
}

export function subscribeToHarnessProgress(
  handler: (update: HarnessProgressDto) => void,
): Promise<() => void> {
  return listenToEvent('harness:progress', handler);
}

export function subscribeToMessages(handler: (message: Message) => void): Promise<() => void> {
  return listenToEvent('conversation:message', handler);
}

/** The terminal of an execution: its output so far and its state; `null` if there is none. */
export function getExecutionTerminal(ref: ExecutionRefDto): Promise<TerminalSnapshotDto | null> {
  return invokeCommand('get_execution_terminal', { ...ref });
}

/** Ctrl+C for the execution's process. */
export async function interruptExecution(ref: ExecutionRefDto): Promise<void> {
  await invokeCommand('execution_interrupt', { ...ref });
}

/** Ends the process by force. For one that ignored the interrupt. */
export async function terminateExecution(ref: ExecutionRefDto): Promise<void> {
  await invokeCommand('execution_terminate', { ...ref });
}

/** Manual input, only for runtimes that read it. */
export async function sendTerminalInput(ref: ExecutionRefDto, data: string): Promise<void> {
  await invokeCommand('execution_terminal_input', { ...ref, data });
}

export async function resizeTerminal(
  ref: ExecutionRefDto,
  cols: number,
  rows: number,
): Promise<void> {
  await invokeCommand('execution_terminal_resize', { ...ref, cols, rows });
}

export function subscribeToTerminalOutput(
  handler: (chunk: TerminalChunkDto) => void,
): Promise<() => void> {
  return listenToEvent('execution:output', handler);
}

export function subscribeToSessionStatus(
  handler: (event: SessionStatusEventDto) => void,
): Promise<() => void> {
  return listenToEvent('execution:status', handler);
}
