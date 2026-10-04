import type {
  AgentHandoffDto,
  AgentNodeFieldsDto,
  ChangeSetDto,
  FileChangeDto,
  IdeDto,
  PendingInteractionDto,
  WorkflowIntegrationDto,
  ArtifactDto,
  ConditionDto,
  DecisionDto,
  NodeAttemptDto,
  NodeStateDto,
  NodeStatusDto,
  TemplateWorkflowDto,
  ValidationIssueDto,
  ValidationReportDto,
  WorkflowDto,
  WorkflowEdgeDto,
  WorkflowEventDto,
  WorkflowExecutionDto,
  WorkflowExecutionStatusDto,
  WorkflowNodeDto,
  WorkflowTemplateDto,
} from '@/lib/tauri/commands';

// The wire types are the domain types; aliased so the feature does not import DTOs by name.
// Nothing here knows about the graph library: it is only a way to draw these.
export type Workflow = WorkflowDto;
export type WorkflowNode = WorkflowNodeDto;
export type WorkflowEdge = WorkflowEdgeDto;
export type WorkflowRun = WorkflowExecutionDto;
export type WorkflowRunStatus = WorkflowExecutionStatusDto;
export type NodeStatus = NodeStatusDto;
export type NodeState = NodeStateDto;
export type NodeAttempt = NodeAttemptDto;
export type WorkflowTemplate = WorkflowTemplateDto;
export type TemplateWorkflow = TemplateWorkflowDto;
export type ValidationReport = ValidationReportDto;
export type ValidationIssue = ValidationIssueDto;
export type WorkflowEvent = WorkflowEventDto;
export type Artifact = ArtifactDto;
export type Decision = DecisionDto;
export type Condition = ConditionDto;
export type Handoff = AgentHandoffDto;
export type ChangeSet = ChangeSetDto;
export type FileChange = FileChangeDto;
export type Integration = WorkflowIntegrationDto;
export type Ide = IdeDto;
export type PendingInteraction = PendingInteractionDto;
export type AgentNode = Extract<WorkflowNodeDto, { type: 'agent' }>;
export type AgentNodePatch = Partial<Omit<AgentNodeFieldsDto, 'type'>>;

/** A run that still holds on to its workflow: nothing about the definition may change. */
export function isActiveRun(run: WorkflowRun | undefined): boolean {
  return (
    run !== undefined &&
    ['running', 'paused', 'waiting_for_input', 'interrupted'].includes(run.status)
  );
}
