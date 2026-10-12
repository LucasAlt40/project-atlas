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
  TaskContextPreviewDto,
  TerminalChunkDto,
  TerminalSnapshotDto,
  PendingApprovalDto,
  ProjectInfoDto,
  ProjectInitUpdateDto,
  ProjectInputDto,
  ProjectPatchDto,
  ScopeRequestDto,
  WorkspaceContextInfoDto,
  WorkspaceHarnessOverviewDto,
  WorkspaceInitReportDto,
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

/** The workspace's projects with the state of each folder. */
export function listWorkspaceProjects(workspaceId: string): Promise<ProjectInfoDto[]> {
  return invokeCommand('list_workspace_projects', { workspaceId });
}

/** Registers another folder in the workspace. Nothing in the folder is read or changed. */
export function addWorkspaceProject(
  workspaceId: string,
  input: ProjectInputDto,
): Promise<Workspace> {
  return invokeCommand('add_workspace_project', { workspaceId, input });
}

export function updateWorkspaceProject(
  workspaceId: string,
  projectId: string,
  patch: ProjectPatchDto,
): Promise<Workspace> {
  return invokeCommand('update_workspace_project', { workspaceId, projectId, patch });
}

/** Takes the project out of the workspace. Its folder and its `.atlas/` are not touched. */
export function removeWorkspaceProject(workspaceId: string, projectId: string): Promise<Workspace> {
  return invokeCommand('remove_workspace_project', { workspaceId, projectId });
}

/**
 * Reads the project and reports what it found. Writes nothing; no project code is run. With
 * `semantic`, an agent also explores the project (or, when `restricted`, reads only selected
 * evidence) and proposes findings. That can take minutes. Without `projectId`: the primary one.
 */
export function analyzeProject(
  workspaceId: string,
  semantic?: SemanticRequestDto,
  projectId?: string,
): Promise<ProjectAnalysisDto> {
  return invokeCommand('analyze_project', {
    workspaceId,
    ...(projectId ? { projectId } : {}),
    ...(semantic ? { semantic } : {}),
  });
}

/** Creates or updates the project's `.atlas/` Harness from the user's review. */
export function initializeProject(
  workspaceId: string,
  input: InitializeInputDto,
  projectId?: string,
): Promise<InitializeOutcomeDto> {
  return invokeCommand('initialize_project', {
    workspaceId,
    ...(projectId ? { projectId } : {}),
    input,
  });
}

/** Whether the project's Harness exists, is valid, or needs a look. */
export function getProjectHarness(
  workspaceId: string,
  projectId?: string,
): Promise<HarnessSummaryDto> {
  return invokeCommand('get_project_harness', {
    workspaceId,
    ...(projectId ? { projectId } : {}),
  });
}

/**
 * Compares a new analysis with the Harness. Without `confirm` nothing is written: the result is
 * the diff and the conflicts. With it, generated files are updated and the user's own are kept.
 */
export function refreshProjectHarness(
  workspaceId: string,
  confirm = false,
  projectId?: string,
): Promise<RefreshOutcomeDto> {
  return invokeCommand('refresh_project_harness', {
    workspaceId,
    ...(projectId ? { projectId } : {}),
    confirm,
  });
}

/** Where each project stands (its Harness) and whether the workspace's own context is current. */
export function getWorkspaceHarness(workspaceId: string): Promise<WorkspaceHarnessOverviewDto> {
  return invokeCommand('get_workspace_harness', { workspaceId });
}

/**
 * Starts the Harness of the projects that have none (all of them, or the ones named). One that
 * fails does not stop the others; one that already has a Harness is left as it is.
 */
export function initializeWorkspaceHarness(
  workspaceId: string,
  projectIds?: string[],
): Promise<WorkspaceInitReportDto> {
  return invokeCommand('initialize_workspace_harness', {
    workspaceId,
    ...(projectIds ? { projectIds } : {}),
  });
}

export function subscribeToWorkspaceHarnessProgress(
  handler: (update: ProjectInitUpdateDto) => void,
): Promise<() => void> {
  return listenToEvent('harness:workspace-progress', handler);
}

/** Rebuilds the generated part of the workspace context. The user's notes are untouched. */
export function buildWorkspaceContext(workspaceId: string): Promise<WorkspaceContextInfoDto> {
  return invokeCommand('build_workspace_context', { workspaceId });
}

export function getWorkspaceNotes(workspaceId: string): Promise<string> {
  return invokeCommand('get_workspace_notes', { workspaceId });
}

export async function saveWorkspaceNotes(workspaceId: string, notes: string): Promise<void> {
  await invokeCommand('save_workspace_notes', { workspaceId, notes });
}

/**
 * What an agent would be told for this task, chosen from the project's Harness, with the reason
 * for each choice. Inspection only.
 */
export function previewTaskContext(
  workspaceId: string,
  agentId: string,
  task: string,
): Promise<TaskContextPreviewDto> {
  return invokeCommand('preview_task_context', { request: { task, workspaceId, agentId } });
}

export interface SendMessageInput {
  workspaceId: string;
  agentId: string;
  content: string;
  /** The projects the agent works in and may consult. Absent: the primary project alone. */
  scope?: ScopeRequestDto;
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
