import { invokeCommand } from '@/lib/tauri/commands';
import type {
  InteractionAnswerDto,
  NewWorkflowDto,
  ScopeRequestDto,
  WorkflowModeDto,
} from '@/lib/tauri/commands';
import { WORKFLOW_EVENT_NAMES, listenToEvent } from '@/lib/tauri/events';
import type {
  ChangeSet,
  Ide,
  PendingInteraction,
  RecoveryPlan,
  RepairChoice,
  RepairProposal,
  TemplateWorkflow,
  ToolPlanItem,
  ValidationReport,
  Workflow,
  WorkflowEvent,
  WorkflowRun,
  WorkflowTemplate,
} from '../types';

/** The UI's only door to the Rust core for workflows. */

export function listWorkflows(workspaceId: string): Promise<Workflow[]> {
  return invokeCommand('list_workflows', { workspaceId });
}

export function listTemplates(): Promise<WorkflowTemplate[]> {
  return invokeCommand('list_workflow_templates');
}

/** The template Automatic mode proposes for a task (deterministic; no model is asked). */
export function selectTemplate(task: string): Promise<string> {
  return invokeCommand('select_workflow_template', { task });
}

export function createWorkflow(request: NewWorkflowDto): Promise<Workflow> {
  return invokeCommand('create_workflow', { request });
}

export function createFromTemplate(
  workspaceId: string,
  templateId: string,
  mode: WorkflowModeDto,
  name?: string,
  applyTools?: boolean,
): Promise<TemplateWorkflow> {
  return invokeCommand('create_workflow_from_template', {
    workspaceId,
    templateId,
    mode,
    ...(name ? { name } : {}),
    ...(applyTools ? { applyTools } : {}),
  });
}

/** What a template's suggested tools come to in this workspace. Grants nothing. */
export function planTemplateTools(
  workspaceId: string,
  templateId: string,
): Promise<ToolPlanItem[]> {
  return invokeCommand('plan_workflow_template_tools', { workspaceId, templateId });
}

/** Saves the workflow's steps, routes and granted tools as a template of the person's own. */
export function saveAsTemplate(
  workflowId: string,
  name: string,
  description?: string,
): Promise<WorkflowTemplate> {
  return invokeCommand('save_workflow_as_template', {
    workflowId,
    name,
    ...(description ? { description } : {}),
  });
}

export async function deleteTemplate(templateId: string): Promise<void> {
  await invokeCommand('delete_workflow_template', { templateId });
}

export function updateWorkflow(workflow: Workflow): Promise<Workflow> {
  return invokeCommand('update_workflow', { workflow });
}

export async function deleteWorkflow(workflowId: string): Promise<void> {
  await invokeCommand('delete_workflow', { workflowId });
}

export function validateWorkflow(workflow: Workflow): Promise<ValidationReport> {
  return invokeCommand('validate_workflow', { workflow });
}

export function suggestRouteRepairs(workflow: Workflow): Promise<RepairProposal[]> {
  return invokeCommand('suggest_route_repairs', { workflow });
}

export function repairWorkflowRoutes(
  workflowId: string,
  choices: RepairChoice[],
): Promise<Workflow> {
  return invokeCommand('repair_workflow_routes', { workflowId, choices });
}

export function startWorkflow(
  workflowId: string,
  task: string,
  autoApprovePlans = false,
  scope?: ScopeRequestDto,
  specId?: string,
): Promise<WorkflowRun> {
  return invokeCommand('start_workflow', {
    workflowId,
    task,
    ...(autoApprovePlans ? { autoApprovePlans } : {}),
    ...(scope ? { scope } : {}),
    ...(specId ? { specId } : {}),
  });
}

export async function setAutoApprovePlans(executionId: string, enabled: boolean): Promise<void> {
  await invokeCommand('set_auto_approve_plans', { executionId, enabled });
}

export async function pauseWorkflow(executionId: string): Promise<void> {
  await invokeCommand('pause_workflow', { executionId });
}

export async function resumeWorkflow(executionId: string): Promise<void> {
  await invokeCommand('resume_workflow', { executionId });
}

/** Where a failed run would go on from; `null` for a run that did not fail. */
export function getRecovery(executionId: string): Promise<RecoveryPlan | null> {
  return invokeCommand('get_workflow_recovery', { executionId });
}

export async function cancelWorkflow(executionId: string): Promise<void> {
  await invokeCommand('cancel_workflow', { executionId });
}

/** The person's answer to a question a step asked. Only checked and handed back to the agent. */
export async function answerInteraction(
  executionId: string,
  interactionId: string,
  answer: InteractionAnswerDto,
): Promise<void> {
  await invokeCommand('answer_workflow_interaction', { executionId, interactionId, answer });
}

/** The questions waiting for a person in a workspace. */
export function listPendingInteractions(workspaceId: string): Promise<PendingInteraction[]> {
  return invokeCommand('list_pending_interactions', { workspaceId });
}

export function getRun(executionId: string): Promise<WorkflowRun | null> {
  return invokeCommand('get_workflow_execution', { executionId });
}

export function listRuns(workspaceId: string, workflowId?: string): Promise<WorkflowRun[]> {
  return invokeCommand('list_workflow_executions', {
    workspaceId,
    ...(workflowId ? { workflowId } : {}),
  });
}

export function getChanges(executionId: string): Promise<ChangeSet | null> {
  return invokeCommand('get_workflow_changes', { executionId });
}

/** The real diff of the run's code, or of one file. */
export function getDiff(executionId: string, file?: string): Promise<string> {
  return invokeCommand('get_workflow_diff', { executionId, ...(file ? { file } : {}) });
}

export function applyChanges(executionId: string): Promise<WorkflowRun> {
  return invokeCommand('apply_workflow_changes', { executionId });
}

export function keepChanges(executionId: string): Promise<WorkflowRun> {
  return invokeCommand('keep_workflow_changes', { executionId });
}

export function discardChanges(executionId: string): Promise<WorkflowRun> {
  return invokeCommand('discard_workflow_changes', { executionId });
}

export function listIdes(): Promise<Ide[]> {
  return invokeCommand('list_ides');
}

export async function openInIde(executionId: string, ideId: string): Promise<void> {
  await invokeCommand('open_workflow_in_ide', { executionId, ideId });
}

/** Hears every workflow event of every run. Resolves with the way to stop listening. */
export async function subscribeToWorkflowEvents(
  handler: (event: WorkflowEvent) => void,
): Promise<() => void> {
  const opened = await Promise.allSettled(
    WORKFLOW_EVENT_NAMES.map((name) => listenToEvent(name, handler)),
  );
  const stops = opened.flatMap((o) => (o.status === 'fulfilled' ? [o.value] : []));
  const stopAll = () => {
    for (const stop of stops) stop();
  };
  // All or nothing: a listener that did open must not be left behind when another did not.
  const failed = opened.find((o) => o.status === 'rejected');
  if (failed) {
    stopAll();
    throw failed.reason;
  }
  return stopAll;
}
