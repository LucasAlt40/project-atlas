import { invoke } from '@tauri-apps/api/core';

/**
 * Single choke point for frontend -> Rust calls.
 *
 * `CommandMap` is the typed contract of every Tauri command the frontend may call.
 * Add a command here (and register it in `src-tauri/src/lib.rs`, `build.rs` and the
 * capability file) before using it anywhere in the UI.
 */
export interface CommandMap {
  get_app_info: { args: undefined; result: AppInfoDto };
  list_personalities: { args: undefined; result: PersonalityDto[] };
  create_personality: { args: { request: CreatePersonalityRequestDto }; result: PersonalityDto };
  update_personality: {
    args: { id: string; request: CreatePersonalityRequestDto };
    result: PersonalityDto;
  };
  delete_personality: { args: { id: string }; result: null };
  restore_default_personalities: { args: undefined; result: PersonalityDto[] };
  list_runtimes: { args: undefined; result: RuntimeStatusDto[] };
  list_agents: { args: undefined; result: AgentDto[] };
  create_agent: { args: { request: CreateAgentRequestDto }; result: AgentDto };
  update_agent: { args: { id: string; request: CreateAgentRequestDto }; result: AgentDto };
  /** What the agent promises to say at the end of a step (for workflows to route on). */
  set_agent_result_contract: {
    args: { agentId: string; contract: ResultContractDto };
    result: AgentDto;
  };
  /** Also removes the agent from every workspace and discards its conversations. */
  delete_agent: { args: { id: string }; result: WorkspaceDto[] };
  list_workspaces: { args: undefined; result: WorkspaceDto[] };
  create_workspace: { args: { input: WorkspaceInputDto }; result: WorkspaceDto };
  update_workspace: { args: { id: string; input: WorkspaceInputDto }; result: WorkspaceDto };
  /** Also discards the workspace's conversations and usage; agents are kept. */
  delete_workspace: { args: { id: string }; result: WorkspaceDto[] };
  add_agent_to_workspace: {
    args: { workspaceId: string; agentId: string };
    result: WorkspaceDto;
  };
  remove_agent_from_workspace: {
    args: { workspaceId: string; agentId: string };
    result: WorkspaceDto;
  };
  get_project_context: { args: { workspaceId: string }; result: ProjectContextDto };
  get_settings: { args: undefined; result: AppSettingsDto };
  set_language: { args: { language: string }; result: AppSettingsDto };
  select_workspace: { args: { workspaceId: string | null }; result: AppSettingsDto };
  send_message: { args: { request: SendMessageRequestDto }; result: SentMessageDto };
  list_messages: { args: { workspaceId?: string; agentId?: string }; result: MessageDto[] };
  /** Executions that have ended, newest first. */
  list_executions: {
    args: { workspaceId?: string; agentId?: string };
    result: StoredExecutionDto[];
  };
  get_agent_usage: {
    args: { workspaceId: string; agentId: string; windows: UsageWindowsDto };
    result: AgentUsageSummaryDto;
  };
  get_workspace_usage: {
    args: { workspaceId: string; windows: UsageWindowsDto };
    result: WorkspaceUsageSummaryDto;
  };
  get_workspace_security: { args: { workspaceId: string }; result: WorkspaceSecurityDto };
  get_agent_permissions: {
    args: { workspaceId: string; agentId: string };
    result: AgentPermissionsDto;
  };
  set_agent_permission_profile: {
    args: { agentId: string; profileId: string };
    result: AgentDto;
  };
  list_pending_approvals: { args: { workspaceId?: string }; result: PendingApprovalDto[] };
  /** The user's explicit answer. Nothing else releases a command waiting for approval. */
  resolve_approval: { args: { approvalId: string; approve: boolean }; result: null };
  /** The terminal of an execution (live, or finished and still retained); `null` if none. */
  get_execution_terminal: { args: ExecutionRefArgs; result: TerminalSnapshotDto | null };
  /** Ctrl+C for the execution's process. The first thing to try. */
  execution_interrupt: { args: ExecutionRefArgs; result: null };
  /** Ends the process by force; for one that ignored the interrupt. */
  execution_terminate: { args: ExecutionRefArgs; result: null };
  /** Manual input; only for runtimes whose `terminalInput` capability is on. */
  execution_terminal_input: { args: ExecutionRefArgs & { data: string }; result: null };
  execution_terminal_resize: {
    args: ExecutionRefArgs & { cols: number; rows: number };
    result: null;
  };
  /** The Git worktrees of executions that ran isolated, oldest first. Metadata only. */
  list_execution_worktrees: {
    args: { workspaceId?: string; agentId?: string };
    result: ExecutionWorktreeDto[];
  };
  /**
   * The user's explicit "merge this execution into its base branch". It is the approval a
   * policy may ask for; it does not override a policy that denies, and a merge Git cannot do
   * cleanly comes back as `conflict` / `blocked`, never forced.
   */
  merge_execution: { args: ExecutionRefArgs; result: ExecutionWorktreeDto };
  /** Reads the project (structure and manifests only; nothing is run) and reports findings. */
  analyze_project: {
    args: { workspaceId: string; semantic?: SemanticRequestDto };
    result: ProjectAnalysisDto;
  };
  /** Creates or updates `.atlas/` from a fresh analysis and the user's review. */
  initialize_project: {
    args: { workspaceId: string; input: InitializeInputDto };
    result: InitializeOutcomeDto;
  };
  get_project_harness: { args: { workspaceId: string }; result: HarnessSummaryDto };
  /** Regenerates what Atlas generates, keeping the user's corrections. */
  refresh_project_harness: {
    args: { workspaceId: string; confirm?: boolean };
    result: RefreshOutcomeDto;
  };
  /**
   * What an agent would be told for a task, chosen from the Harness, with the reason for each
   * choice. Inspection only: nothing is written and nothing runs.
   */
  preview_task_context: {
    args: { request: TaskContextRequestDto };
    result: TaskContextPreviewDto;
  };
  /** The workflows of a workspace. */
  list_workflows: { args: { workspaceId: string }; result: WorkflowDto[] };
  get_workflow: { args: { workflowId: string }; result: WorkflowDto | null };
  list_workflow_templates: { args: undefined; result: WorkflowTemplateDto[] };
  /** The template Automatic mode proposes for a task. Deterministic: no model is asked. */
  select_workflow_template: { args: { task: string }; result: string };
  create_workflow: { args: { request: NewWorkflowDto }; result: WorkflowDto };
  /** Builds the workflow from the workspace's agents; roles without an agent are reported. */
  create_workflow_from_template: {
    args: { workspaceId: string; templateId: string; name?: string; mode: WorkflowModeDto };
    result: TemplateWorkflowDto;
  };
  /** Refused while the workflow has a run in progress. */
  update_workflow: { args: { workflow: WorkflowDto }; result: WorkflowDto };
  delete_workflow: { args: { workflowId: string }; result: null };
  /** Checks a definition (possibly unsaved) and lists everything wrong with it. */
  validate_workflow: { args: { workflow: WorkflowDto }; result: ValidationReportDto };
  /** Returns at once; progress arrives as `workflow:*` events. */
  start_workflow: { args: { workflowId: string; task: string }; result: WorkflowExecutionDto };
  pause_workflow: { args: { executionId: string }; result: null };
  /** Resumes a paused run, or picks up one the app's shutdown interrupted. */
  resume_workflow: { args: { executionId: string }; result: null };
  cancel_workflow: { args: { executionId: string }; result: null };
  /**
   * The person's answer to a question a step asked. Checked against the question; it is given back
   * to the agent as text and never changes a permission, a policy or the workflow.
   */
  answer_workflow_interaction: {
    args: { executionId: string; interactionId: string; answer: InteractionAnswerDto };
    result: null;
  };
  /** The questions waiting for a person in a workspace, across its runs. */
  list_pending_interactions: { args: { workspaceId: string }; result: PendingInteractionDto[] };
  get_workflow_execution: { args: { executionId: string }; result: WorkflowExecutionDto | null };
  list_workflow_executions: {
    args: { workspaceId: string; workflowId?: string };
    result: WorkflowExecutionDto[];
  };
  /** What the run changed in the code, from Git. `null` when it has no code worktree. */
  get_workflow_changes: { args: { executionId: string }; result: ChangeSetDto | null };
  /** The real diff of the run's code (of one file when `file` is given). */
  get_workflow_diff: { args: { executionId: string; file?: string }; result: string };
  /** The user's decision to apply the changes to the project. A merge that cannot happen is reported in the run. */
  apply_workflow_changes: { args: { executionId: string }; result: WorkflowExecutionDto };
  keep_workflow_changes: { args: { executionId: string }; result: WorkflowExecutionDto };
  /** Removes the worktree (its branch is kept). */
  discard_workflow_changes: { args: { executionId: string }; result: WorkflowExecutionDto };
  list_ides: { args: undefined; result: IdeDto[] };
  open_workflow_in_ide: { args: { executionId: string; ideId: string }; result: null };
}

/**
 * Names an execution's process. The core answers only when all three match the session it
 * opened; the webview never knows (or sends) a PID, a signal or a program.
 */
export interface ExecutionRefDto {
  workspaceId: string;
  agentId: string;
  executionId: string;
}

/** `ExecutionRefDto` as Tauri's invoke accepts it (an object with string keys). */
export type ExecutionRefArgs = ExecutionRefDto & Record<string, unknown>;

/** Wire format of `get_app_info`; mirrors `domain::app_info::AppInfo` in Rust. */
export interface AppInfoDto {
  name: string;
  version: string;
  platform: string;
}

/**
 * Every failure the core reports. The UI words each in the user's language (`src/i18n`); the
 * core never translates. Mirrors `application::errors::ErrorCode`.
 */
export type ErrorCodeDto =
  | 'name_required'
  | 'name_too_long'
  | 'instructions_required'
  | 'personality_required'
  | 'runtime_required'
  | 'runtime_not_supported'
  | 'model_required'
  | 'model_invalid'
  | 'personality_not_found'
  | 'personality_in_use'
  | 'agent_not_found'
  | 'agent_busy'
  | 'agent_already_placed'
  | 'workspace_not_found'
  | 'workspace_full'
  | 'workspace_busy'
  | 'project_folder_required'
  | 'project_folder_not_found'
  | 'message_empty'
  | 'approval_not_found'
  | 'execution_not_found'
  | 'execution_not_running'
  | 'terminal_input_unsupported'
  | 'terminal_input_invalid'
  | 'process_control_failed'
  | 'permission_profile_invalid'
  | 'git_repository_required'
  | 'git_unavailable'
  | 'worktree_base_unavailable'
  | 'worktree_not_found'
  | 'worktree_invalid_state'
  | 'worktree_busy'
  | 'worktree_failed'
  | 'merge_not_allowed'
  | 'language_unsupported'
  | 'project_not_found'
  | 'project_not_directory'
  | 'project_already_initialized'
  | 'harness_invalid'
  | 'harness_not_initialized'
  | 'semantic_analysis_failed'
  | 'semantic_unsupported'
  | 'project_analysis_failed'
  | 'harness_generation_failed'
  | 'unsafe_project_path'
  | 'storage_failed';

/**
 * How sure the analysis is. `high`: direct evidence; `medium`: several signals plus
 * interpretation; `low`: weak signals, never treated as established. Mirrors
 * `domain::harness::Confidence`.
 */
export type ConfidenceDto = 'low' | 'medium' | 'high';

/**
 * Where knowledge comes from: read from the repository (`fact`), concluded from it
 * (`inference`), or the user's word (`user_confirmed`, `user_corrected`). `generated` is Atlas's
 * own statement (such as "architecture: unknown"), not from evidence.
 */
export type FindingOriginDto =
  'fact' | 'inference' | 'user_confirmed' | 'user_corrected' | 'generated';

export type VerificationStatusDto = 'unverified' | 'verified' | 'stale';

export interface VerificationDto {
  status: VerificationStatusDto;
  /** Milliseconds since the epoch. */
  verifiedAt?: number;
  method?: 'repository_file' | 'command_execution';
}

export type FindingCategoryDto =
  | 'repository'
  | 'language'
  | 'runtime'
  | 'framework'
  | 'package_manager'
  | 'dependency'
  | 'database'
  | 'data'
  | 'integration'
  | 'infrastructure'
  | 'testing'
  | 'build'
  | 'tooling'
  | 'ci'
  | 'entry_point'
  | 'environment'
  | 'architecture'
  | 'module'
  | 'convention';

/** Where a statement comes from: a file or folder and, when it helps, the field in it. */
export interface EvidenceDto {
  source: string;
  field?: string;
}

/** Mirrors `domain::harness::Finding`. */
export interface FindingDto {
  /** `category:key`; corrections, confirmations and exclusions refer to it. */
  id: string;
  category: FindingCategoryDto;
  key: string;
  label: string;
  value: string;
  confidence: ConfidenceDto;
  origin: FindingOriginDto;
  /** What Atlas had made of it before the user's word replaced it (e.g. `inference`). */
  originalOrigin?: FindingOriginDto;
  /** Whether it was checked, as opposed to merely read or concluded. */
  verification: VerificationDto;
  /** Why it is concluded (inferences). */
  reason?: string;
  evidence: EvidenceDto[];
  /** Proposed by a model rather than by rules. */
  byModel: boolean;
}

export type HarnessStatusDto = 'not_initialized' | 'initialized' | 'needs_review';

export type HealthStateDto = 'healthy' | 'needs_review' | 'partial' | 'conflicted' | 'stale';

/** How far the Harness can be trusted, with machine-readable reasons the UI words. */
export interface HarnessHealthDto {
  state: HealthStateDto;
  reasons: string[];
}

/** Mirrors `domain::harness::HarnessSummary`. */
export interface HarnessSummaryDto {
  status: HarnessStatusDto;
  projectName: string | null;
  stack: string[];
  version: number | null;
  initializedAt: number | null;
  /** `.atlas/` exists in the project, with or without a valid manifest. */
  hasAtlasDir: boolean;
  problem: string | null;
  health: HarnessHealthDto | null;
  /** When the project was last analysed (ms since the epoch). */
  analyzedAt: number | null;
  /** Present when the project changed in relevant ways since then. */
  staleness: StalenessDto | null;
  /** Counts of what the Harness holds. */
  stats: KnowledgeStatsDto | null;
}

export interface RelevantChangeDto {
  path: string;
  kind: 'added' | 'removed' | 'modified';
}

/** The project changed since the analysis. Nothing is deleted because of it. */
export interface StalenessDto {
  analyzedAt: number;
  changes: RelevantChangeDto[];
  totalChanges: number;
}

export interface KnowledgeStatsDto {
  findings: number;
  verified: number;
  inferred: number;
  user: number;
  stale: number;
  unknown: number;
}

/** What is missing: looked for and not found (a bounded search), or unknown (not knowable). */
export interface GapDto {
  kind: 'not_found' | 'unknown';
  subject: string;
  statement: string;
}

export interface ClaimDto {
  value: string;
  /** The value stored if the user chooses this side. */
  choice: string;
  origin: FindingOriginDto;
  evidence: EvidenceDto[];
}

/** Two sources that disagree. Atlas records it; the user's decision resolves it. */
export interface ConflictDto {
  findingId: string;
  label: string;
  claims: ClaimDto[];
  resolution: string | null;
}

export interface FindingChangeDto {
  id: string;
  label: string;
  before: string | null;
  after: string | null;
}

export interface HarnessDiffDto {
  added: FindingChangeDto[];
  removed: FindingChangeDto[];
  changed: FindingChangeDto[];
  unchanged: string[];
}

export type SemanticStatusDto = 'not_run' | 'completed' | 'failed';

/** What the agent analysing a project is doing (the `harness:progress` event). */
export interface HarnessProgressDto {
  workspaceId: string;
  agentId: string;
  /** The execution doing the work: what the UI names to stop it. */
  executionId: string;
  kind: 'started' | 'step' | 'tool_started' | 'tool_completed' | 'output';
  text: string;
}

/** Which agent's model analyses the project, and how. */
export interface SemanticRequestDto {
  agentId: string;
  /** What the user wants the agent to know or focus on. */
  instructions: string;
  /** Show the model only a selected set of files, with every tool off (Claude only). */
  restricted: boolean;
}

export interface SemanticReportDto {
  status: SemanticStatusDto;
  /** An error code when it failed. */
  error: string | null;
  /** The technical reason: what the runtime said or what the answer looked like. */
  errorDetail: string | null;
  /** Exactly the files whose content was sent to the model. */
  sentFiles: string[];
  /** Statements dropped for lacking valid evidence. */
  rejected: number;
  /** The agent explored the project with its own read tools: no list of files is possible. */
  explored: boolean;
}

export interface AnalysisInfoDto {
  partial: boolean;
  scannedEntries: number;
  sampledFiles: string[];
  semantic: SemanticReportDto;
  analyzedAt: number;
}

/** What the user wrote about the project. Atlas never edits it unless they change it. */
export interface UserKnowledgeDto {
  purpose: string;
  users: string;
  concepts: string;
  businessRules: string;
  constraints: string;
  decisions: string;
}

/** Mirrors `domain::harness::ProjectAnalysis`. */
export interface ProjectAnalysisDto {
  projectName: string;
  path: string;
  findings: FindingDto[];
  conflicts: ConflictDto[];
  /** What is not known, and what was looked for and not found. */
  gaps: GapDto[];
  /** The scan or the sampling hit a limit: not everything was seen. */
  partial: boolean;
  scannedEntries: number;
  analysis: AnalysisInfoDto;
  existing: HarnessSummaryDto | null;
  /** How this analysis differs from the existing Harness's knowledge. */
  diff: HarnessDiffDto | null;
  previousCorrections: Record<string, string>;
  previousExcluded: string[];
  previousConfirmed: string[];
  user: UserKnowledgeDto;
  /** What a model drafted for the fields the user has not filled. Only a suggestion. */
  suggestedUser: UserKnowledgeDto;
  /** User files Atlas cannot rewrite without losing text; it leaves them alone. */
  unmanagedUserFiles: string[];
}

export type InitModeDto = 'create' | 'use_existing' | 'update_existing';

/** Mirrors `domain::harness::InitializeInput`. */
export interface InitializeInputDto {
  mode: InitModeDto;
  excluded: string[];
  corrections: Record<string, string>;
  confirmed: string[];
  purpose: string;
  users: string;
  concepts: string;
  businessRules: string;
  constraints: string;
  decisions: string;
}

export interface InitializeOutcomeDto {
  summary: HarnessSummaryDto;
  written: string[];
  backedUp: string[];
  leftUntouched: string[];
}

/** What a refresh would change (`applied` is null until the user confirms). */
export interface RefreshOutcomeDto {
  /** Why the Harness is stale, when it is. */
  staleness: StalenessDto | null;
  diff: HarnessDiffDto;
  conflicts: ConflictDto[];
  applied: InitializeOutcomeDto | null;
}

/** Mirrors `application::errors::AppError`. */
export interface AppErrorDto {
  code: ErrorCodeDto;
  params: Record<string, string>;
  detail: string | null;
}

export function isAppError(value: unknown): value is AppErrorDto {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    typeof value.code === 'string' &&
    'params' in value
  );
}

export type PersonalitySourceDto = 'builtin' | 'custom';

/** Mirrors `domain::personality::PersonalityProfile`. */
export interface PersonalityDto {
  id: string;
  name: string;
  description: string;
  systemInstructions: string;
  behavior: string[];
  tags: string[];
  source: PersonalitySourceDto;
  /** The contract a new agent of this personality starts from (a suggestion only). */
  suggestedContract: ResultContractDto;
}

/** Mirrors `application::personalities::CreatePersonalityRequest`. */
export interface CreatePersonalityRequestDto {
  name: string;
  description: string;
  systemInstructions: string;
  tags: string[];
}

/** Mirrors `domain::runtime::ProviderRef`: who provides the AI capability. */
export interface ProviderRefDto {
  id: string;
  name: string;
}

export type AuthKindDto = 'cli_session' | 'api_key' | 'environment_variable' | 'credential_store';

/** Mirrors `domain::runtime::RuntimeCapabilities`. */
export interface RuntimeCapabilitiesDto {
  modelDiscovery: boolean;
  streaming: boolean;
  systemPrompt: boolean;
  nonInteractiveExecution: boolean;
  authentication: AuthKindDto[];
  /** The runtime reports token counts for an execution. */
  usageMetrics: boolean;
  /** The runtime reports a cost for an execution. */
  costMetrics: boolean;
  /** The runtime or provider reports quota (how much of an allowance is used). */
  quotaMetrics: boolean;
  /** An execution runs attached to a real terminal (PTY) the UI can show. */
  interactiveTerminal: boolean;
  /** The user can interrupt (Ctrl+C) the running process. */
  interrupt: boolean;
  /** The process reads manual input from the terminal; otherwise the terminal is read-only. */
  terminalInput: boolean;
  /** The terminal's size can change while the process runs. */
  terminalResize: boolean;
  /** Can run with every tool off: usable for semantic analysis. */
  textOnly: boolean;
  /** Can be launched with file-editing tools (granted per execution by the agent's policy, in a worktree). */
  fileEdit: boolean;
  /** What the runtime's own tools can do as Atlas launches it (facts, not permissions). */
  toolAccess: ToolAccessDto;
}

/** Mirrors `domain::runtime::RuntimeInfo`: how Atlas reaches a provider on this machine. */
export interface RuntimeInfoDto {
  id: string;
  name: string;
  provider: ProviderRefDto;
  transport: 'cli' | 'api';
  capabilities: RuntimeCapabilitiesDto;
  /** A code the UI maps to a hint for the model field (for example `claude_alias`). */
  modelHint: string | null;
}

export type AuthStateDto = 'authenticated' | 'required' | 'unknown';

export interface AuthenticationDto {
  kind: AuthKindDto | null;
  state: AuthStateDto;
}

export type AvailabilityDto = 'not_installed' | 'unavailable' | 'authentication_required' | 'ready';

export interface ModelDto {
  id: string;
  name: string;
}

export type ModelDiscoveryDto = 'discovered' | 'unsupported' | 'unavailable' | 'failed';

export type RuntimeNoticeDto = 'sign_in_required' | 'execution_not_supported' | 'unavailable';

/** Mirrors `domain::runtime::RuntimeStatus`. */
export interface RuntimeStatusDto {
  runtime: RuntimeInfoDto;
  availability: AvailabilityDto;
  version: string | null;
  authentication: AuthenticationDto;
  availableModels: ModelDto[];
  modelDiscovery: ModelDiscoveryDto;
  /** Technical detail about a failed model listing. */
  discoveryError: string | null;
  notice: RuntimeNoticeDto | null;
}

/** Mirrors `domain::agent::Agent`. */
export type ContractKindDto = 'general' | 'validation' | 'review' | 'implementation' | 'custom';

/** One result an agent may declare. `id` is stable: workflow edges refer to it. */
export interface OutcomeDto {
  id: string;
  label: string;
  description?: string;
}

/** Mirrors `domain::result_contract::ResultContract`: the outcomes a step may end with. */
export interface ResultContractDto {
  kind: ContractKindDto;
  outcomes: OutcomeDto[];
}

export interface AgentDto {
  id: string;
  name: string;
  personalityId: string;
  runtimeId: string;
  modelId: string;
  instructions: string;
  /** Which permission profile bounds the agent; unset means the most restrictive. */
  permissionProfileId: string | null;
  /** Each execution works in its own Git worktree, not in the project's checkout. */
  worktreeIsolation: boolean;
  /** Agents saved before contracts existed are `general`: no outcome is required of them. */
  resultContract: ResultContractDto;
  createdAt: number;
}

/** Mirrors `application::agents::CreateAgentRequest`. */
export interface CreateAgentRequestDto {
  name: string;
  personalityId: string;
  runtimeId: string;
  modelId: string;
  instructions: string;
  /** Left out: a new agent is isolated, an edited one keeps its setting. */
  worktreeIsolation?: boolean;
  /** Left out: a new agent is `general`, an edited one keeps its contract. */
  resultContract?: ResultContractDto;
}

/** Mirrors `domain::workspace::GridPosition`. */
export interface GridPositionDto {
  row: number;
  column: number;
}

/** Mirrors `domain::workspace::AgentPlacement`. */
export interface AgentPlacementDto {
  agentId: string;
  position: GridPositionDto;
}

/** Mirrors `domain::workspace::WorkspaceLayout`. */
export interface WorkspaceLayoutDto {
  rows: number;
  columns: number;
  agentPlacements: AgentPlacementDto[];
}

/** Mirrors `domain::workspace::Workspace`: a project environment where agents work. */
export interface WorkspaceDto {
  id: string;
  name: string;
  projectPath: string;
  description: string | null;
  createdAt: number;
  updatedAt: number;
  layout: WorkspaceLayoutDto;
  /** What agents may do in this workspace. */
  security: SecurityPolicyDto;
}

/** Mirrors `application::workspace::WorkspaceInput`. */
export interface WorkspaceInputDto {
  name: string;
  projectPath: string;
  description?: string | null;
}

/** Mirrors `domain::project::ProjectContext`. */
export interface ProjectContextDto {
  name: string;
  path: string;
  /** Technologies recognised from marker files in the project folder. */
  technologies: string[];
}

/** Mirrors `application::config::AppSettings`: settings of the whole application. */
export interface AppSettingsDto {
  language: string;
  selectedWorkspaceId: string | null;
}

export type TaskStatusDto = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export type FailureKindDto =
  | 'runtime_not_installed'
  | 'runtime_unavailable'
  | 'authentication_required'
  | 'model_unavailable'
  | 'rate_limited'
  | 'timeout'
  | 'execution_failed'
  | 'invalid_request'
  | 'unexpected_response'
  | 'permission_denied'
  | 'cancelled'
  | 'app_closed'
  | 'git_repository_required'
  | 'worktree_failed';

/**
 * `output_chunk` (its `message` is a piece of the live answer), `tool_started` and
 * `tool_completed` appear only for runtimes whose tool really streams them.
 */
export type ExecutionEventKindDto =
  | 'started'
  | 'starting_runtime'
  | 'sending_prompt'
  | 'waiting_for_model'
  | 'output_chunk'
  | 'tool_started'
  | 'tool_completed'
  | 'permission'
  | 'completed'
  | 'failed'
  /** The agent stopped to ask a person; `message` is the question. */
  | 'interaction_detected'
  | 'terminal_connected'
  | 'user_interrupted'
  | 'user_terminated'
  | 'process_exited'
  | 'cancelled'
  | 'worktree_created'
  | 'worktree_finalized';

/** Mirrors `domain::execution::ExecutionEvent`. */
export interface ExecutionEventDto {
  executionId: string;
  workspaceId: string;
  taskId: string;
  agentId: string;
  kind: ExecutionEventKindDto;
  /** English text for logs; the UI words steps itself from `kind` and `metadata`. */
  message: string;
  /** Milliseconds since the Unix epoch. */
  timestamp: number;
  /**
   * `runtime` / `model` / `tool` names, `failureKind` on failure, runtime-reported values. For
   * `permission`: `decision`, `action`, `source`, `target` and, when present, `reason`, `cwd`,
   * `approvalId`, `notes`.
   */
  metadata: Record<string, string>;
}

/** `waiting_for_input` is neither completed nor failed: the agent asked a person and has no result. */
export type ExecutionStatusDto =
  'running' | 'waiting_for_input' | 'completed' | 'failed' | 'cancelled';

export type InteractionKindDto =
  'clarification' | 'approval' | 'permission' | 'runtime_confirmation';

export type InteractionStatusDto = 'pending' | 'answered' | 'cancelled' | 'expired';

export interface InteractionOptionDto {
  id: string;
  label: string;
}

/** How the question was recognised, from the most to the least reliable. */
export type DetectionSourceDto = 'structured' | 'adapter' | 'heuristic';

/** What an execution asked, while it waits for a person. */
export interface InteractionDetectionDto {
  detected: boolean;
  kind: InteractionKindDto | null;
  confidence: number;
  question: string;
  context: string;
  /** What the agent wrote before asking, as Markdown (a plan…), to be read in full. */
  document: string;
  options: InteractionOptionDto[];
  source: DetectionSourceDto;
}

/** Mirrors `domain::interaction::PendingInteraction`: a question that blocks one step of a run. */
export interface PendingInteractionDto {
  id: string;
  executionId: string;
  workflowId: string;
  workflowExecutionId: string;
  workspaceId: string;
  stepId: string;
  stepLabel: string;
  agentId: string;
  iteration: number;
  kind: InteractionKindDto;
  question: string;
  context: string;
  document: string;
  options: InteractionOptionDto[];
  source: DetectionSourceDto;
  confidence: number;
  status: InteractionStatusDto;
  createdAt: number;
  answeredAt: number | null;
  choice: string | null;
  answer: string | null;
}

/** A pick between the question's options and/or free text. */
export interface InteractionAnswerDto {
  choice?: string;
  text?: string;
}

/**
 * Mirrors `domain::execution::StoredExecution`: what is kept of an execution that ended. The
 * answer is the assistant message with the same `executionId`; terminal output is never kept.
 */
export interface StoredExecutionDto {
  id: string;
  workspaceId: string;
  agentId: string;
  status: ExecutionStatusDto;
  /** What the user asked. */
  task: string;
  startedAt: number;
  completedAt: number | null;
  runtimeId: string;
  modelId: string;
  failure: { kind: FailureKindDto; message: string; details: string | null } | null;
  metadata: Record<string, string>;
  usage: UsageMetricsDto | null;
  /** What it asked, when it ended waiting for a person. */
  interaction?: InteractionDetectionDto;
  /** How the Harness context of its prompt was chosen; absent without a Harness. */
  context?: ContextRecordDto;
  /** The workflow step this execution ran as, when it was one. */
  workflow?: WorkflowLinkDto;
  /** The timeline, without streamed answer text. */
  events: ExecutionEventDto[];
}

export type MessageRoleDto = 'user' | 'assistant';

/** Mirrors `domain::conversation::Message`. */
export interface MessageDto {
  id: string;
  workspaceId: string;
  agentId: string;
  executionId: string | null;
  role: MessageRoleDto;
  content: string;
  timestamp: number;
  /** The assistant could not answer. */
  failed: boolean;
  /** Why (a `FailureKindDto`), when it failed. */
  failureKind: string | null;
}

/** Mirrors `application::chat::SendMessageRequest`. */
export interface SendMessageRequestDto {
  workspaceId: string;
  agentId: string;
  content: string;
}

/** Mirrors `application::chat::SentMessage`. */
export interface SentMessageDto {
  userMessage: MessageDto;
  executionId: string;
}

/** Where a number came from. A missing number is `null`, never a source. */
export type UsageSourceDto = 'runtime_reported' | 'atlas_calculated' | 'provider_reported';

/** Mirrors `domain::usage::UsageMetrics`: `null` means "not reported", which is not zero. */
export interface UsageMetricsDto {
  inputTokens: number | null;
  outputTokens: number | null;
  totalTokens: number | null;
  cost: number | null;
  currency: string | null;
  source: UsageSourceDto;
}

/** Mirrors `domain::usage::UsageRecord`. */
export interface UsageRecordDto {
  executionId: string;
  workspaceId: string;
  agentId: string;
  runtimeId: string;
  modelId: string;
  startedAt: number;
  completedAt: number;
  succeeded: boolean;
  metrics: UsageMetricsDto | null;
}

/** Mirrors `domain::usage::UsageTotals`: a sum over executions Atlas observed. */
export interface UsageTotalsDto {
  inputTokens: number | null;
  outputTokens: number | null;
  totalTokens: number | null;
  cost: number | null;
  currency: string | null;
  /** Executions observed in the period. */
  runs: number;
  runsWithTokens: number;
  runsWithCost: number;
  source: UsageSourceDto;
}

/** Mirrors `domain::usage::QuotaWindow`. */
export interface QuotaWindowDto {
  id: string;
  /** 0 to 1, as the provider reported it. */
  usedFraction: number;
  /** Seconds since the Unix epoch. */
  resetsAt: number | null;
}

/** Mirrors `domain::usage::QuotaInfo`. */
export interface QuotaInfoDto {
  windows: QuotaWindowDto[];
  source: UsageSourceDto;
  observedAt: number;
}

/** Where "today", "this week" and "this month" start, in ms since the Unix epoch. */
export interface UsageWindowsDto {
  todayStart: number;
  weekStart: number;
  monthStart: number;
}

/** Mirrors `domain::usage::AgentUsageSummary`. */
export interface AgentUsageSummaryDto {
  latestExecution: UsageRecordDto | null;
  conversation: UsageTotalsDto;
  today: UsageTotalsDto;
  week: UsageTotalsDto;
  month: UsageTotalsDto;
  quota: QuotaInfoDto | null;
}

/** Mirrors `domain::usage::WorkspaceUsageSummary`. */
export interface WorkspaceUsageSummaryDto {
  today: UsageTotalsDto;
  week: UsageTotalsDto;
  month: UsageTotalsDto;
}

/** How much of something is granted; ordered from least to most. */
export type PermissionDto = 'denied' | 'approval_required' | 'allowed';

/** Mirrors `domain::security::SecurityPolicy`. */
export interface SecurityPolicyDto {
  filesystem: { scope: 'project_only'; write: PermissionDto };
  processes: { mode: PermissionDto; allowedCommands: string[] };
  network: { mode: PermissionDto };
  git: { read: PermissionDto; write: PermissionDto; destructive: PermissionDto };
}

/** Mirrors `domain::security::ToolAccess`. */
export interface ToolAccessDto {
  filesystemWrite: boolean;
  processExecution: boolean;
  network: boolean;
}

/** Mirrors `application::security::service::WorkspaceSecurity`. */
export interface WorkspaceSecurityDto {
  workspaceId: string;
  projectPath: string;
  policy: SecurityPolicyDto;
  label: 'secure' | 'developer';
}

/** Mirrors `application::security::service::AgentPermissions`. */
export interface AgentPermissionsDto {
  workspaceId: string;
  agentId: string;
  /** The profile id (`read_only`, `developer`). */
  profile: string;
  availableProfiles: string[];
  /** What the profile grants inside this workspace. */
  policy: SecurityPolicyDto;
  /** What applies once the runtime's own limits are counted; never wider than `policy`. */
  effective: SecurityPolicyDto;
  runtimeAccess: ToolAccessDto;
  /** Capabilities of the runtime's tools that exceed `policy`; Atlas can only report them. */
  unenforced: string[];
}

/** Mirrors `application::security::approvals::PendingApproval`. */
export interface PendingApprovalDto {
  id: string;
  executionId: string;
  workspaceId: string;
  taskId: string;
  agentId: string;
  agentName: string;
  action: 'launch_runtime' | 'run_process';
  executable: string;
  args: string[];
  cwd: string | null;
  reason: string;
  requestedAt: number;
}

export type CommandName = keyof CommandMap;

export class CommandError extends Error {
  /** The core's structured error, when it sent one. */
  readonly appError: AppErrorDto | undefined;

  constructor(
    readonly command: CommandName,
    cause: unknown,
  ) {
    super(
      `Command "${command}" failed: ${
        isAppError(cause) ? cause.code : cause instanceof Error ? cause.message : String(cause)
      }`,
      { cause },
    );
    this.name = 'CommandError';
    this.appError = isAppError(cause) ? cause : undefined;
  }
}

export async function invokeCommand<C extends CommandName>(
  command: C,
  ...args: CommandMap[C]['args'] extends undefined ? [] : [CommandMap[C]['args']]
): Promise<CommandMap[C]['result']> {
  try {
    return await invoke<CommandMap[C]['result']>(command, args[0]);
  } catch (error) {
    throw new CommandError(command, error);
  }
}

export type TerminalStreamDto = 'stdout' | 'stderr';

/** What the process is doing; `interrupting` / `terminating` last until it actually exits. */
export type SessionStatusDto = 'running' | 'interrupting' | 'terminating' | 'exited';

/** What the *user* did to the process. */
export type UserActionDto = 'interrupted' | 'terminated';

/** Mirrors `domain::terminal::TerminalChunk`: a piece of raw terminal output, in order. */
export interface TerminalChunkDto {
  executionId: string;
  processSessionId: string;
  workspaceId: string;
  agentId: string;
  timestamp: number;
  stream: TerminalStreamDto;
  /** Counts the chunks of one session. */
  seq: number;
  data: string;
}

/** Mirrors `domain::terminal::SessionStatusEvent`. */
export interface SessionStatusEventDto {
  executionId: string;
  processSessionId: string;
  workspaceId: string;
  agentId: string;
  status: SessionStatusDto;
  userAction: UserActionDto | null;
  exitCode: number | null;
  timestamp: number;
}

/** Mirrors `domain::terminal::TerminalSnapshot`. */
export interface TerminalSnapshotDto {
  executionId: string;
  processSessionId: string;
  workspaceId: string;
  agentId: string;
  status: SessionStatusDto;
  userAction: UserActionDto | null;
  /** The command as one line of text, for people to read. */
  command: string;
  startedAt: number;
  endedAt: number | null;
  exitCode: number | null;
  cols: number;
  rows: number;
  inputEnabled: boolean;
  /** Output retained so far (the newest part when it was cut). Ephemeral and bounded. */
  output: string;
  /** The `seq` the next chunk carries: chunks below it are already in `output`. */
  nextSeq: number;
  truncated: boolean;
}

export type WorktreeStatusDto =
  'creating' | 'active' | 'completed' | 'failed' | 'cleanup_pending' | 'cleaned';

export type MergeStatusDto =
  'not_evaluated' | 'nothing_to_merge' | 'pending' | 'merged' | 'conflict' | 'blocked';

export type BlockReasonDto =
  | 'policy_denied'
  | 'execution_not_completed'
  | 'validation_failed'
  | 'base_dirty'
  | 'base_branch_changed'
  | 'undetermined'
  | 'uncommitted_changes'
  | 'worktree_inconsistent'
  | 'conflict';

export type RecommendationDto = 'merge' | 'review' | 'resolve_conflicts' | 'inspect' | 'discard';

/** Mirrors `domain::worktree::WorktreeChanges`: measured by Git, never by the model. */
export interface WorktreeChangesDto {
  filesChanged: number;
  commitsAhead: number;
  commitsBehind: number;
  files: string[];
  conflicts: string[];
  /** `null`: Git could not say. */
  mergeable: boolean | null;
}

/**
 * Mirrors `domain::worktree::ExecutionWorktree`: the Git side of an execution that ran
 * isolated. Only metadata; the diff is always read again from Git.
 */
export interface ExecutionWorktreeDto {
  executionId: string;
  workspaceId: string;
  agentId: string;
  baseBranch: string;
  baseCommit: string;
  branchName: string;
  worktreePath: string;
  workingDir: string;
  repositoryPath: string;
  status: WorktreeStatusDto;
  mergeStatus: MergeStatusDto;
  blockReason: BlockReasonDto | null;
  baseDirtyAtStart: boolean;
  createdAt: number;
  changes: WorktreeChangesDto | null;
  validation: 'not_run' | 'passed' | 'failed';
  recommendation: RecommendationDto | null;
}

// ---- Task-aware context (mirrors `domain::task_context`) ----

export type ContextAreaDto =
  | 'architecture'
  | 'stack'
  | 'modules'
  | 'conventions'
  | 'business'
  | 'testing'
  | 'dependencies'
  | 'entry_points'
  | 'ci'
  | 'constraints'
  | 'decisions'
  | 'infrastructure'
  | 'general';

export type TaskIntentDto =
  'add' | 'change' | 'fix' | 'refactor' | 'test' | 'document' | 'investigate' | 'unknown';

export interface TaskContextRequestDto {
  task: string;
  workspaceId: string;
  agentId: string;
}

export interface TaskSignalsDto {
  keywords: string[];
  areas: ContextAreaDto[];
  baselineAreas: ContextAreaDto[];
  tags: string[];
  intent: TaskIntentDto;
  technologies: string[];
}

export type ContextBlockDto = 'know' | 'infer' | 'user' | 'unknown' | 'outdated';
export type ContextItemKindDto =
  | 'finding'
  | 'constraint'
  | 'decision'
  | 'business'
  | 'not_found'
  | 'unknown'
  | 'conflict'
  | 'notice';

export interface ContextItemDto {
  id: string;
  block: ContextBlockDto;
  kind: ContextItemKindDto;
  area: ContextAreaDto;
  category: FindingCategoryDto | null;
  label: string;
  content: string;
  provenance: FindingOriginDto | null;
  verification: VerificationStatusDto | null;
  confidence: ConfidenceDto | null;
  evidence: EvidenceDto[];
  sourceFindingId: string | null;
  tags: string[];
}

/** Why an item scored what it did. */
export interface SelectionReasonDto {
  score: number;
  matchedAreas: ContextAreaDto[];
  matchedTags: string[];
  matchedCategories: FindingCategoryDto[];
  matchedPaths: string[];
  matchedKeywords: string[];
  /** Set for constraints and decisions, which are never dropped for looking unrelated. */
  alwaysIncluded: 'constraint' | 'decision' | null;
  penalties: string[];
}

export type EntryOutcomeDto = 'included' | 'not_relevant' | 'over_budget';

export interface ContextEntryDto {
  item: ContextItemDto;
  reason: SelectionReasonDto;
  outcome: EntryOutcomeDto;
}

export type ContextModeDto = 'task_aware' | 'fallback' | 'none';

export interface TaskContextDto {
  mode: ContextModeDto;
  /** `snake_case` reason the whole Harness context was used instead. */
  fallbackReason: string | null;
  signals: TaskSignalsDto | null;
  /** The text the agent receives. */
  text: string;
  entries: ContextEntryDto[];
  includedAreas: ContextAreaDto[];
  excludedAreas: ContextAreaDto[];
  selectedChars: number;
  /** The size of the whole Harness context, for comparison. */
  fullHarnessChars: number;
  budgetChars: number;
  truncated: boolean;
  omitted: { area: ContextAreaDto; count: number }[];
}

export interface TaskContextPreviewDto {
  status: 'ready' | 'missing' | 'invalid';
  context: TaskContextDto | null;
}

/** What an execution keeps of how its context was chosen: counts, not the text. */
export interface ContextRecordDto {
  mode: ContextModeDto;
  totalHarnessCharacters: number;
  selectedContextCharacters: number;
  selectedItems: number;
  omittedItems: number;
  fallbackReason?: string;
}

// ---- Workflows -------------------------------------------------------------------------------
// Mirror `domain::workflow` and `domain::orchestration`. The graph library never appears here:
// positions are plain coordinates and ids are the workflow's own.

export type WorkflowModeDto = 'automatic' | 'custom';
export type WorkflowStatusDto = 'draft' | 'ready';

export interface PositionDto {
  x: number;
  y: number;
}

export interface ViewportDto {
  x: number;
  y: number;
  zoom: number;
}

export type ConditionOperatorDto = 'equals' | 'not_equals' | 'exists' | 'not_exists';

/** A closed test of one fact of a step's result: no expressions, no code. */
export interface ConditionDto {
  field: string;
  operator: ConditionOperatorDto;
  value?: string | null;
}

export interface RetryPolicyDto {
  maxRetries: number;
}

export type FailurePolicyDto =
  { type: 'stop_workflow' } | { type: 'route_to_node'; nodeId: string };

export interface ExecutionPolicyDto {
  isolation: 'from_agent';
}

export interface LoopPolicyDto {
  loopId: string;
  maxIterations: number;
}

export type EndOutcomeDto = 'done' | 'failed' | 'cancelled';

interface WorkflowNodeBaseDto {
  id: string;
  priority: number;
  loopPolicy: LoopPolicyDto | null;
  position: PositionDto | null;
}

export interface AgentNodeFieldsDto {
  type: 'agent';
  /** The agent is the source of truth: personality, runtime and model are never copied. */
  agentId: string;
  label: string;
  instructions: string;
  retryPolicy: RetryPolicyDto;
  failurePolicy: FailurePolicyDto;
  executionPolicy: ExecutionPolicyDto;
}

export interface ConditionNodeFieldsDto {
  type: 'condition';
  label: string;
  condition: ConditionDto;
}

export interface EndNodeFieldsDto {
  type: 'end';
  label: string;
  outcome: EndOutcomeDto;
}

export type WorkflowNodeDto = WorkflowNodeBaseDto &
  (AgentNodeFieldsDto | ConditionNodeFieldsDto | EndNodeFieldsDto);

export interface WorkflowEdgeDto {
  id: string;
  sourceNodeId: string;
  targetNodeId: string;
  condition: ConditionDto | null;
  label: string;
}

export interface WorkflowDto {
  id: string;
  workspaceId: string;
  name: string;
  description: string;
  mode: WorkflowModeDto;
  version: number;
  status: WorkflowStatusDto;
  templateId: string | null;
  nodes: WorkflowNodeDto[];
  edges: WorkflowEdgeDto[];
  viewport: ViewportDto | null;
  createdAt: number;
  updatedAt: number;
}

export interface NewWorkflowDto {
  workspaceId: string;
  name: string;
  description?: string;
  mode: WorkflowModeDto;
  nodes?: WorkflowNodeDto[];
  edges?: WorkflowEdgeDto[];
  viewport?: ViewportDto | null;
}

export type WorkflowRoleDto = 'architect' | 'developer' | 'validator' | 'qa' | 'bug_fixer';
export type TemplateCategoryDto = 'software_feature' | 'bug_fix' | 'refactoring' | 'code_review';

export interface WorkflowTemplateDto {
  id: string;
  category: TemplateCategoryDto;
  recommended: boolean;
  name: string;
  description: string;
  roles: WorkflowRoleDto[];
}

export interface TemplateWorkflowDto {
  workflow: WorkflowDto;
  /** Roles no agent was found for: their nodes have no agent yet. */
  missingRoles: WorkflowRoleDto[];
}

export type ValidationIssueCodeDto =
  | 'empty_workflow'
  | 'duplicate_node_id'
  | 'duplicate_edge_id'
  | 'edge_unknown_node'
  | 'failure_route_unknown_node'
  | 'end_has_outgoing'
  | 'no_start_node'
  | 'no_end_node'
  | 'isolated_node'
  | 'unreachable_node'
  | 'dead_end'
  | 'cycle_without_limit'
  | 'invalid_loop_policy'
  | 'duplicate_loop_id'
  | 'invalid_condition'
  | 'undeclared_outcome'
  | 'missing_agent'
  | 'unknown_agent'
  | 'workspace_mismatch'
  | 'too_many_retries';

export interface ValidationIssueDto {
  code: ValidationIssueCodeDto;
  nodeId: string | null;
  edgeId: string | null;
  params: Record<string, string>;
}

export interface ValidationReportDto {
  valid: boolean;
  issues: ValidationIssueDto[];
}

export type NodeStatusDto =
  | 'pending'
  | 'ready'
  | 'running'
  | 'waiting_approval'
  /** The agent asked a person; the step has no result and no outcome yet. */
  | 'waiting_for_input'
  | 'completed'
  | 'failed'
  | 'blocked'
  | 'cancelled'
  | 'skipped';

export type WorkflowExecutionStatusDto =
  | 'running'
  | 'paused'
  /** A step asked a person; nothing new starts until it is answered. */
  | 'waiting_for_input'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'interrupted';

export type AttemptStatusDto =
  'running' | 'completed' | 'failed' | 'cancelled' | 'interrupted' | 'waiting_for_input';

export interface NodeAttemptDto {
  attempt: number;
  iteration: number;
  executionId: string;
  status: AttemptStatusDto;
  startedAt: number;
  completedAt: number | null;
  summary: string | null;
  /** The outcome this attempt declared; belongs to this attempt alone. */
  outcome: string | null;
  failure: string | null;
}

export interface NodeStateDto {
  status: NodeStatusDto;
  iterations: number;
  failedAttempts: number;
  attempts: NodeAttemptDto[];
  facts: Record<string, string>;
  /** Why the node is blocked, skipped or failed (a stable code, or the failure message). */
  reason: string | null;
  routed: boolean;
  unrouted: boolean;
}

export type WorkflowFailureCodeDto =
  | 'max_iterations_reached'
  | 'node_failed'
  | 'no_path_to_completion'
  | 'no_route_matched'
  | 'ended_in_failure'
  | 'invalid_workflow'
  | 'internal_error'
  | 'worktree_unavailable';

export interface WorkflowFailureDto {
  code: WorkflowFailureCodeDto;
  nodeId: string | null;
  detail: string | null;
}

export type ArtifactTypeDto =
  | 'architecture_document'
  | 'api_contract'
  | 'database_migration'
  | 'implementation_summary'
  | 'test_report'
  | 'validation_report'
  | 'bug_report'
  | 'other';

export interface ArtifactDto {
  id: string;
  type: ArtifactTypeDto;
  name: string;
  producerNodeId: string;
  executionId: string;
  path: string | null;
  summary: string;
  metadata: Record<string, string>;
  createdAt: number;
}

export interface DecisionDto {
  id: string;
  title: string;
  decision: string;
  rationale: string;
  sourceNodeId: string;
  createdAt: number;
}

/** A finding a validating step reported (not a Harness finding). */
export interface ResultFindingDto {
  severity: string;
  category: string;
  title: string;
  /** A path inside the project, when the finding is about one file. */
  file: string | null;
  line: number | null;
  description: string;
  evidence: string;
  recommendation: string;
}

export interface ValidationEntryDto {
  nodeId: string;
  executionId: string;
  status: 'pass' | 'fail' | 'warning' | 'success' | 'unknown';
  outcome: string | null;
  summary: string;
  findings: ResultFindingDto[];
}

export interface OverlapWarningDto {
  nodeIds: [string, string];
  paths: string[];
}

export interface SharedExecutionStateDto {
  task: string;
  workflow: string;
  currentNodes: string[];
  completedNodes: string[];
  failedNodes: string[];
  artifacts: ArtifactDto[];
  decisions: DecisionDto[];
  activeAgents: string[];
  completedAgents: string[];
  touchedAreas: Record<string, string[]>;
  touchedFiles: Record<string, string[]>;
  validationResults: ValidationEntryDto[];
  warnings: OverlapWarningDto[];
  iterationCount: Record<string, number>;
}

export type WorkflowEventKindDto =
  | 'started'
  | 'paused'
  | 'resumed'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'interrupted'
  | 'node_ready'
  | 'node_started'
  | 'node_waiting_approval'
  | 'node_approval_resolved'
  | 'node_waiting_for_input'
  | 'node_input_resolved'
  | 'interaction_detected'
  | 'interaction_answered'
  | 'interaction_rejected'
  | 'interaction_cancelled'
  | 'node_completed'
  | 'node_failed'
  | 'node_blocked'
  | 'node_skipped'
  | 'node_retrying'
  | 'artifact_created'
  | 'decision_created'
  | 'overlap_detected'
  | 'handoff_created'
  | 'integration_changed';

/** Something that happened in a run; carries ids, not state: the UI reads the run again. */
export interface WorkflowEventDto {
  kind: WorkflowEventKindDto;
  workflowId: string;
  executionId: string;
  workspaceId: string;
  nodeId: string | null;
  message: string;
  timestamp: number;
  metadata: Record<string, string>;
}

/** One run of a workflow, with the snapshot of the definition it started from. */
export interface WorkflowExecutionDto {
  id: string;
  workflowId: string;
  workspaceId: string;
  workflowVersion: number;
  workflow: WorkflowDto;
  task: string;
  status: WorkflowExecutionStatusDto;
  failure: WorkflowFailureDto | null;
  cancelRequested: boolean;
  maxParallelSteps: number;
  nodes: Record<string, NodeStateDto>;
  state: SharedExecutionStateDto;
  /** What each step handed to the next, in order. */
  handoffs: AgentHandoffDto[];
  /** What the run changed in the code, by Git, once it ended. */
  changes: ChangeSetDto | null;
  /** Where the code stands: a run can complete with its code not integrated. */
  integration: WorkflowIntegrationDto;
  /** Every question the run's steps asked a person, answered or not: the audit trail. */
  interactions: PendingInteractionDto[];
  events: WorkflowEventDto[];
  startedAt: number;
  updatedAt: number;
  completedAt: number | null;
}

/** Where an execution sits in a workflow run (for the inspector's breadcrumb). */
export interface WorkflowLinkDto {
  workflowId: string;
  workflowName: string;
  workflowExecutionId: string;
  nodeId: string;
  nodeLabel: string;
  attempt: number;
  iteration: number;
}

// ---- Handoffs and the code of a run -------------------------------------------------------------

export type HandoffKindDto = 'result' | 'failure';
export type ResultStatusDto = 'pass' | 'fail' | 'warning' | 'success' | 'unknown';

export interface FileChangeDto {
  path: string;
  oldPath: string | null;
  status: 'added' | 'modified' | 'deleted' | 'renamed';
  /** `null` for a binary file. */
  additions: number | null;
  deletions: number | null;
  binary: boolean;
}

/** What one step handed to the next over one transition. Context, never authority. */
export interface AgentHandoffDto {
  id: string;
  workflowExecutionId: string;
  /** The edge, or `failure:<node>` for a failure route. */
  linkId: string;
  fromNodeId: string;
  toNodeId: string;
  fromExecutionId: string;
  iteration: number;
  kind: HandoffKindDto;
  createdAt: number;
  status: ResultStatusDto;
  /** What the step declared (`pass`, `fail`, `approved`…), when its agent has a result contract.
   * Not the execution's status: a step that completed can say `fail`. */
  outcome: string | null;
  summary: string;
  /** What the agent suggested next. Information: the workflow decides. */
  instructions: string | null;
  decisions: { title: string; decision: string; rationale: string }[];
  artifacts: {
    id: string;
    name: string;
    type: ArtifactTypeDto;
    path: string | null;
    summary: string;
  }[];
  /** Measured by Git in the shared worktree: the source of truth for the code. */
  changedFiles: FileChangeDto[];
  uncommittedFiles: string[];
  /** Only what the agent claimed. */
  reportedFiles: string[];
  validation: {
    status: ResultStatusDto;
    outcome: string | null;
    summary: string;
    findings: ResultFindingDto[];
  } | null;
  failure: string | null;
}

export interface ChangeSetDto {
  baseRevision: string;
  currentRevision: string;
  files: FileChangeDto[];
  filesChanged: number;
  additions: number;
  deletions: number;
  uncommitted: string[];
  capturedAt: number;
}

export type IntegrationStatusDto =
  | 'not_applicable'
  | 'in_progress'
  | 'no_changes'
  | 'changes_available'
  | 'conflicts'
  | 'blocked'
  | 'integrated'
  | 'kept_isolated'
  | 'discarded'
  | 'failed';

export interface WorkflowIntegrationDto {
  status: IntegrationStatusDto;
  worktreeExecutionId: string | null;
  branch: string | null;
  baseBranch: string | null;
  baseRevision: string | null;
  currentRevision: string | null;
  blockReason: BlockReasonDto | null;
  canApply: boolean;
  conflicts: string[];
  message: string | null;
  updatedAt: number;
}

export interface IdeDto {
  id: string;
  name: string;
}
