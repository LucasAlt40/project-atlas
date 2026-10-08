/**
 * Single choke point for frontend -> Rust calls.
 *
 * `CommandMap` is the typed contract of every Tauri command the frontend may call.
 * Add a command here (and register it in `src-tauri/src/lib.rs`, `build.rs` and the
 * capability file) before using it anywhere in the UI.
 */
export interface CommandMap {
  get_app_info: { args: undefined; result: AppInfoDto };
  check_for_update: { args: undefined; result: UpdateInfoDto | null };
  install_update: { args: undefined; result: null };
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
  /** The workspace's MCP connections and who may use them. No secret is in them. */
  list_mcp_connections: { args: { workspaceId: string }; result: McpOverviewDto };
  add_mcp_connection: {
    args: { workspaceId: string; name: string; transport: McpTransportDto; required: boolean };
    result: McpConnectionDto;
  };
  update_mcp_connection: {
    args: { connectionId: string; transport: McpTransportDto; required: boolean };
    result: McpConnectionDto;
  };
  set_mcp_connection_enabled: {
    args: { connectionId: string; enabled: boolean };
    result: McpConnectionDto;
  };
  remove_mcp_connection: { args: { connectionId: string }; result: undefined };
  /** The value goes to the OS credential store and is never returned. */
  set_mcp_secret: {
    args: { connectionId: string; name: string; value: string };
    result: McpConnectionDto;
  };
  clear_mcp_secret: { args: { connectionId: string; name: string }; result: McpConnectionDto };
  /** Starts the server through the runtime only to see what it reports; no model is asked. */
  probe_mcp_connection: {
    args: { connectionId: string; runtimeId: string };
    result: McpConnectionDto;
  };
  grant_mcp_connection: {
    args: {
      connectionId: string;
      agentId?: string;
      workflowId?: string;
      nodeId?: string;
      tools: McpToolSelectionDto;
    };
    result: McpGrantDto;
  };
  revoke_mcp_grant: { args: { grantId: string }; result: undefined };
  /** The integrations Atlas knows by name. Nothing is started to answer. */
  list_mcp_catalog: { args: undefined; result: McpPresetDto[] };
  /** Adds an entry as an ordinary connection: off, granted to nobody, not started. */
  add_mcp_preset: {
    args: { workspaceId: string; presetId: string };
    result: McpConnectionDto;
  };
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
  /** What Atlas proposes for routes that cannot match their agent's contract. Changes nothing. */
  suggest_route_repairs: { args: { workflow: WorkflowDto }; result: RepairProposalDto[] };
  /** Applies what the user confirmed to the saved workflow, as a new version with its record. */
  repair_workflow_routes: {
    args: { workflowId: string; choices: RepairChoiceDto[] };
    result: WorkflowDto;
  };
  /** Returns at once; progress arrives as `workflow:*` events. */
  start_workflow: { args: { workflowId: string; task: string }; result: WorkflowExecutionDto };
  pause_workflow: { args: { executionId: string }; result: null };
  /**
   * Resumes a paused run, picks up one the app's shutdown interrupted, or picks a failed one up
   * at its Recovery Point (steps that completed are not run again).
   */
  resume_workflow: { args: { executionId: string }; result: null };
  /** Where a failed run would go on from. `null` for a run that did not fail. Changes nothing. */
  get_workflow_recovery: { args: { executionId: string }; result: RecoveryPlanDto | null };
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
  /**
   * The files of a run's isolated worktree as they are now, against the commit the run started from
   * (read from Git, never from what an agent said). `null` when the run has no code worktree.
   */
  get_live_workspace: { args: { executionId: string }; result: LiveWorkspaceStateDto | null };
  /** Compares the whole worktree with the baseline now. */
  refresh_live_workspace: { args: { executionId: string }; result: LiveWorkspaceStateDto };
  /**
   * One file of the run's worktree as it is now. The run and a relative path are all the webview
   * says; the folder is the one Atlas stored for the run.
   */
  get_live_file: { args: { executionId: string; path: string }; result: LiveFileDto };
  /** The diff of the run's worktree as it is now against its baseline (of one file when `path` is given). */
  get_live_diff: { args: { executionId: string; path?: string }; result: string };
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

/** A newer version of Atlas; mirrors `commands::updater::UpdateInfo` in Rust. */
export interface UpdateInfoDto {
  version: string;
  notes: string | null;
}

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
  | 'storage_failed'
  | 'update_check_failed'
  | 'update_install_failed';

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
  /** Add `.atlas/` to the project's `.gitignore` so it is not pushed. */
  ignoreInGit: boolean;
}

export type GitIgnoreStatusDto =
  'skipped' | 'added' | 'already_ignored' | 'no_repository' | 'failed';

export interface InitializeOutcomeDto {
  summary: HarnessSummaryDto;
  written: string[];
  backedUp: string[];
  leftUntouched: string[];
  gitIgnore: GitIgnoreStatusDto;
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
  /** The permission profile a new agent of this personality starts from (a suggestion only). */
  suggestedPermissionProfile: string;
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

/** Mirrors `domain::runtime::SystemPromptChannel`. */
export type SystemPromptChannelDto = 'unsupported' | 'native' | 'appended';

/** Mirrors `domain::mcp::McpSupport`. */
export type McpSupportDto = 'unsupported' | 'not_investigated' | 'supported';

export type AuthKindDto = 'cli_session' | 'api_key' | 'environment_variable' | 'credential_store';

/** Mirrors `domain::runtime::RuntimeCapabilities`. */
export interface RuntimeCapabilitiesDto {
  modelDiscovery: boolean;
  streaming: boolean;
  /** How Atlas's system instructions reach the runtime. `unsupported`: inside the prompt body. */
  systemPrompt: SystemPromptChannelDto;
  /** Whether Atlas can give this runtime MCP servers of its own (an adapter that was measured). */
  mcp: McpSupportDto;
  /** How its MCP support filters tools and probes (meaningful when `mcp` is `supported`). */
  mcpFeatures: {
    toolFilter: 'unsupported' | 'deny_list' | 'allow_list';
    probe: 'none' | 'status_only' | 'tools';
    /** It loads only the servers Atlas gives it; `false`: the user's own load as well. */
    strict: boolean;
  };
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
  /** Left out: a new agent starts from its personality's suggestion, an edited one keeps its profile. */
  permissionProfileId?: string;
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
  /** `optimization.metrics.enabled`: measure prompts and runs. Never changes what is sent. */
  optimization?: {
    metricsEnabled: boolean;
    /** `optimization.context.enabled`: the Context Engine removes what a prompt says twice. */
    contextEnabled?: boolean;
    /** `optimization.context.maxTokens`: an estimated-token budget; only reported, never cut to. */
    contextMaxTokens?: number | null;
    /** `optimization.skills.enabled`: Atlas sends the skills a task calls for. */
    skillsEnabled?: boolean;
    /** `optimization.guardrails.enabled`: review the context before an agent starts. */
    guardrailsEnabled?: boolean;
  };
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
  | 'worktree_finalized'
  /** Observability only (Optimization Layer): what was measured is in `metadata`. */
  | 'optimization_context_built'
  | 'optimization_prompt_built'
  | 'optimization_metrics_recorded'
  /** The Context Engine removed what the prompt said twice; counts are in `metadata`. */
  | 'optimization_context_optimized'
  /** The prompt is over its budget after everything that may be shortened was; nothing was cut. */
  | 'optimization_budget_warning'
  /** The skills layer looked at the task; the skills it chose and why are in `metadata`. */
  | 'optimization_skills_selected'
  /** The context an agent is about to receive was reviewed; `metadata` has the health and counts. */
  | 'optimization_context_reviewed'
  /** The context was not fit to send, so the agent was not started. */
  | 'optimization_context_review_blocked'
  /** A guardrail asked a person before the agent started (a pending interaction). */
  | 'optimization_guardrail_asked'
  /** A guardrail refused something; `metadata` has the rule and the reason. */
  | 'optimization_guardrail_denied'
  /** Everything the guardrails decided for this execution, counted. */
  | 'optimization_guardrail_evaluated';

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
export type DetectionSourceDto = 'structured' | 'adapter' | 'heuristic' | 'guardrail';

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
  /** What Atlas observed about its prompt and run; absent from before the Optimization Layer. */
  optimization?: OptimizationMetricsDto;
  /** What Atlas meant to send. Present whether or not metrics were on. */
  plan?: ContextPlanDto;
  /** What Atlas prepared and delivered, with the hash of the payload. Present whether or not metrics were on. */
  manifest?: ContextManifestDto;
  /** The workflow step this execution ran as, when it was one. */
  workflow?: WorkflowLinkDto;
  /** The timeline, without streamed answer text. */
  events: ExecutionEventDto[];
}

/** `estimated` is Atlas's own approximation (chars / 4), never a count by a tokenizer. */
export type TokenSourceDto = 'exact' | 'estimated' | 'unavailable';

/** Mirrors `domain::optimization::SectionKind`. */
export type PromptSectionKindDto =
  | 'personality'
  | 'atlas_rules'
  | 'live_narration'
  | 'plan_rule'
  | 'rules'
  | 'harness'
  | 'task_context'
  | 'skills'
  | 'project_context'
  | 'agent_instructions'
  | 'brief_workflow_context'
  | 'brief_handoff'
  | 'brief_protocols'
  | 'task'
  | 'framing';

/** Mirrors `domain::optimization::OptimizationMetrics`. `null` means "not observable". */
export interface OptimizationMetricsDto {
  prompt: {
    totalBytes: number;
    estimatedTokens: number;
    tokenSource: TokenSourceDto;
    sections: {
      section: PromptSectionKindDto;
      bytes: number;
      estimatedTokens: number;
      tokenSource: TokenSourceDto;
    }[];
  };
  context: {
    selectedItems: number;
    omittedItems: number;
    selectedCharacters: number;
    totalHarnessCharacters: number;
  } | null;
  latency: {
    contextBuildMs: number | null;
    promptBuildMs: number | null;
    runtimeStartupMs: number | null;
    runtimeExecutionMs: number | null;
    runtimeMs: number | null;
    totalMs: number | null;
    instrumentationMs: number | null;
    /** Preparing the delivery: building the payload, hashing it, writing the manifest. */
    deliveryMs?: number | null;
  };
  tools: {
    calls: number | null;
    totalOutputBytes: number | null;
    /** The tools the model could call, as the runtime listed them at start (MCP tools included). */
    exposed?: string[];
    /** The tools it did call, most used first. */
    used?: { name: string; calls: number }[];
  };
  handoff: { bytes: number | null };
  optimization: {
    cacheHits: number | null;
    cacheMisses: number | null;
    deduplicatedItems: number;
    compressedItems: number;
    droppedItems: number | null;
  };
  /** What the Context Engine did; absent when it is off. Sizes are estimates. */
  contextEngine?: ContextEngineMetricsDto;
  /** What the skills layer did; absent when it is off. Token figures are estimates. */
  skills?: SkillMetricsDto;
  /** What the runtime loaded around the model on its own (MCP servers, skills, plugins). */
  extensions?: {
    mcpServers: string[];
    skills: number;
    slashCommands: number;
    plugins: number;
  };
  /** What the guardrails decided before the agent started; absent when they are off. */
  guardrails?: GuardrailMetricsDto;
  /** The review of the context the agent was given; absent when the guardrails are off. */
  contextReview?: ContextReviewDto;
  /** What this execution may use: the limits of its runtime + model and how much the prompt uses. */
  budget?: ExecutionBudgetDto;
  /** What the rules did for this execution; absent when none applied. */
  rules?: RuleMetricsDto;
  /** The prompt's size by what its text may do (estimates). */
  authority?: AuthorityMetricsDto;
  /** What the step had to do with MCP; absent when the workspace has no connection. */
  mcp?: McpMetricsDto;
}

/** Mirrors `domain::optimization::McpMetrics`. `null`: the runtime did not report (not zero). */
export interface McpMetricsDto {
  connections: number;
  serversExposed: number;
  serversLeftOut: number;
  toolsAuthorized: number;
  toolsHeldBack: number;
  toolsReported: number | null;
  toolsUnauthorized: number;
  toolsUsed: number | null;
  serversFailed: number | null;
}

export type McpServerStatusDto = 'connected' | 'failed' | 'needs_auth' | 'pending' | 'unknown';

/** Mirrors `domain::mcp::McpTransport`. HTTP is part of the contract and refused for now. */
export type McpTransportDto =
  | {
      kind: 'stdio';
      executable: string;
      args?: string[];
      env?: { name: string; value: { kind: 'plain'; value: string } | { kind: 'secret' } }[];
    }
  | { kind: 'http'; url: string };

export type McpToolSelectionDto = { kind: 'server' } | { kind: 'only'; tools: string[] };

/** Mirrors `domain::mcp::McpConnection`. */
export interface McpConnectionDto {
  id: string;
  workspaceId: string;
  name: string;
  transport: McpTransportDto;
  enabled: boolean;
  required: boolean;
  /** When each secret was stored, by variable name. Never a value. */
  secrets: Record<string, number>;
  discovery?: {
    discoveredAt: number;
    runtimeId: string;
    status: McpServerStatusDto;
    tools: string[];
  };
  createdAt: number;
}

/** Mirrors `domain::mcp::McpGrant`. */
export interface McpGrantDto {
  id: string;
  connectionId: string;
  agentId: string | null;
  workflowId: string | null;
  nodeId: string | null;
  tools: McpToolSelectionDto;
}

/** Mirrors `application::mcp::McpPresetInfo`. */
export interface McpPresetDto {
  id: string;
  connectionName: string;
  /** `needs_http_and_oauth`: listed for what it is, cannot be added. */
  availability: 'ready' | 'needs_http_and_oauth';
  risk: 'high';
  sourceUrl: string;
  pinnedVersion: string | null;
  /** What would be started, shown as text. */
  command: string | null;
  /** What Atlas looked for on this machine; "not found" is not "missing". */
  requirements: { requirement: 'node' | 'npx' | 'chrome'; found: boolean }[];
}

export interface McpOverviewDto {
  connections: McpConnectionDto[];
  grants: McpGrantDto[];
}

/** Why a connection was not given to a step (`domain::mcp::McpProblem`). */
export type McpProblemDto =
  | { kind: 'not_enabled' }
  | { kind: 'no_grant' }
  | { kind: 'policy_denied' }
  | { kind: 'runtime_unsupported' }
  | { kind: 'invalid_configuration'; reason: string }
  | { kind: 'secret_missing'; name: string }
  | { kind: 'needs_discovery' }
  | { kind: 'tool_filter_unsupported' };

/** Mirrors `domain::mcp::McpRecord`: the six things about a tool, kept apart. */
export interface McpRecordDto {
  servers: {
    connectionId: string;
    name: string;
    enabled: boolean;
    required: boolean;
    authorized: boolean;
    exposed: boolean;
    problem?: McpProblemDto;
    discoveredStatus?: McpServerStatusDto;
    reportedStatus?: McpServerStatusDto;
  }[];
  tools: {
    server: string;
    tool: string;
    discovered: boolean;
    enabled: boolean;
    authorized: boolean;
    exposed: boolean;
    /** What the runtime listed; `null`: it did not say. */
    reportedExposed: boolean | null;
    used: boolean | null;
  }[];
  heldBack: string[];
  /** Tools the runtime listed that nobody authorized: the step was stopped. */
  unauthorized: string[];
}

/** Mirrors `domain::optimization::RuleMetrics`. */
export interface RuleMetricsDto {
  applied: number;
  mandatory: number;
  preference: number;
  informational: number;
  excluded: number;
  conflicts: number;
  omittedForBudget: number;
  warnings: number;
  estimatedTokens: number;
  tokenSource: TokenSourceDto;
}

/** Mirrors `domain::optimization::AuthorityMetrics`. */
export interface AuthorityMetricsDto {
  authoritativeTokens: number;
  instructionalTokens: number;
  informationalTokens: number;
  untrustedTokens: number;
  tokenSource: TokenSourceDto;
}

/** What a piece of context may do (`domain::context::ContextAuthority`). */
export type ContextAuthorityDto = 'authoritative' | 'instructional' | 'informational' | 'untrusted';
export type RuleScopeDto = 'global' | 'project' | 'workspace' | 'workflow' | 'agent' | 'task';
export type RuleStrengthDto = 'mandatory' | 'preference' | 'informational';
export type RuleOriginDto = 'user' | 'project_file' | 'generated' | 'external';
export type RuleStatusDto =
  'applied' | 'omitted_for_budget' | 'disabled' | 'duplicate' | 'overridden';

/** Mirrors `domain::context::ManifestRule`. */
export interface ManifestRuleDto {
  reference: string;
  title: string;
  scope: RuleScopeDto;
  strength: RuleStrengthDto;
  authority: ContextAuthorityDto;
  origin: RuleOriginDto;
  source: string;
  status: RuleStatusDto;
  /** The rule that governs it (overridden) or that it repeats (duplicate). */
  by?: string;
  inConflict: boolean;
  downgraded: boolean;
}

/** Who stated a figure (`domain::context::FigureSource`). */
export type FigureSourceDto = 'reported' | 'configured' | 'default' | 'unknown';
/** How far a figure can be trusted as a measurement (`domain::context::Precision`). */
export type PrecisionDto = 'exact' | 'estimated' | 'unknown';

/** Mirrors `domain::context::Figure`: a token count (or `null`: not known) with where it came from. */
export interface FigureDto {
  value: number | null;
  source: FigureSourceDto;
  precision: PrecisionDto;
  /** How an estimate was made. */
  method?: 'chars_div_4';
}

/** Mirrors `domain::context::ExecutionBudget`. */
export interface ExecutionBudgetDto {
  runtimeId: string;
  modelId: string;
  limits: { input: FigureDto; output: FigureDto; total: FigureDto };
  outputReserve: FigureDto;
  safetyMargin: FigureDto;
  /** What the MCP tools' definitions weigh: unknown, the runtime counts them in its own request. */
  toolDefinitions?: FigureDto;
  input: { limit: FigureDto; used: FigureDto; remaining: FigureDto };
  resolution: {
    layer: 'settings' | 'workspace' | 'agent';
    dimension: 'input' | 'output' | 'total';
    requested: number;
    outcome: { kind: 'applied' | 'narrowed' | 'kept' | 'clamped'; limit?: number };
  }[];
}

/** Mirrors `domain::context::ContextPlan`. */
export interface ContextPlanDto {
  sections: { section: PromptSectionKindDto; bytes: number; tokens: FigureDto }[];
  totalBytes: number;
  totalTokens: FigureDto;
  fingerprint: string;
  engineOmittedItems: number;
}

export type ContextWarningDto =
  | 'tokens_estimated'
  | 'model_limit_unknown'
  | 'limit_clamped'
  | 'over_budget'
  | 'not_delivered'
  | 'diverged_from_plan'
  | 'surface_partly_unobserved';

export type SurfaceControlDto =
  'atlas_controlled' | 'runtime_controlled' | 'user_controlled' | 'unknown';
export type SurfaceObservationDto = 'reported' | 'declared' | 'not_observed';
export type SurfaceKindDto =
  | 'prompt'
  | 'launch_flags'
  | 'tools'
  | 'mcp_servers'
  | 'skills'
  | 'plugins'
  | 'system_prompt'
  | 'system_channel'
  | 'user_instructions'
  | 'hooks'
  | 'user_settings'
  | 'auto_memory'
  | 'other';

/** Mirrors `domain::context::RuntimeSurface`. */
export interface RuntimeSurfaceDto {
  runtimeId: string;
  entries: {
    kind: SurfaceKindDto;
    control: SurfaceControlDto;
    observation: SurfaceObservationDto;
    /** Only the prompt Atlas delivered is confirmed to have reached the model. */
    reachesModel: boolean;
    detail?: string;
  }[];
}

/** Mirrors `domain::context::ContextManifest`. */
export interface ContextManifestDto {
  executionId: string;
  workspaceId: string;
  taskId: string;
  agentId: string;
  runtimeId: string;
  modelId: string;
  createdAt: number;
  planFingerprint: string;
  divergedFromPlan: boolean;
  sections: PromptPartDto[];
  delivery: {
    delivered: boolean;
    /** `sha256:<hex>` of the exact payload handed to the runtime. */
    promptHash: string;
    bytes: number;
    chars: number;
    estimatedTokens: FigureDto;
    /** How Atlas's system instructions travelled. */
    systemChannel?: SystemPromptChannelDto;
    systemBytes?: number;
  };
  /** The rules that applied to this execution and what became of each. */
  rules?: ManifestRuleDto[];
  /** The workspace's MCP connections as this step saw them. */
  mcp?: McpRecordDto;
  surface: RuntimeSurfaceDto;
  warnings: ContextWarningDto[];
}

interface PromptPartDto {
  section: PromptSectionKindDto;
  bytes: number;
  estimatedTokens: number;
  tokenSource: TokenSourceDto;
}

/** Mirrors `domain::guardrail::GuardrailMetrics`. */
export interface GuardrailMetricsDto {
  evaluations: number;
  allowed: number;
  asked: number;
  denied: number;
  transformed: number;
  /** Denials that stopped the execution before its runtime started. */
  blocked: number;
}

export type ContextHealthDto = 'healthy' | 'partial' | 'needs_review' | 'invalid';
export type IssueSeverityDto = 'info' | 'warning' | 'error' | 'blocking';
export type IssueCodeDto =
  | 'missing_required'
  | 'altered_required'
  | 'conflicting_instructions'
  | 'stale_context'
  | 'duplicated_context'
  | 'budget_exceeded'
  | 'context_trimmed'
  | 'skill_issues'
  | 'secret_in_context'
  | 'authority_claim'
  | 'rule_conflict'
  | 'possible_rule_conflict'
  | 'rule_over_budget'
  | 'mcp_unavailable'
  | 'mcp_approval_required'
  | 'unknown_provenance';

/** What a text tried to claim for itself (`domain::guardrail::ClaimKind`). */
export type ClaimKindDto =
  'override_rules' | 'grant_permission' | 'disable_security' | 'false_approval' | 'elevation';

/** Mirrors `domain::guardrail::ContextReviewResult`. */
export interface ContextReviewDto {
  health: ContextHealthDto;
  issues: {
    code: IssueCodeDto;
    severity: IssueSeverityDto;
    source: PromptSectionKindDto;
    /** The other side of a conflict. */
    otherSource: PromptSectionKindDto | null;
    message: string;
    excerpt: string;
    /** For an authority claim: what it tried to claim. */
    claim?: ClaimKindDto;
  }[];
  sources: {
    source: PromptSectionKindDto;
    trust: 'atlas' | 'configured' | 'untrusted';
    /** What the source's text may do. */
    authority?: ContextAuthorityDto;
    estimatedTokens: number;
  }[];
  requiredItems: number;
  highItems: number;
  normalItems: number;
  optionalItems: number;
  staleItems: number;
}

/** Mirrors `domain::optimization::SkillMetrics`. */
export interface SkillMetricsDto {
  discovered: number;
  usable: number;
  issues: number;
  /** What every skill's name and description would cost if all were sent (they are not). */
  level1Tokens: number;
  candidates: number;
  activated: string[];
  level2Tokens: number;
  resourcesAvailable: number;
  resourcesLoaded: number;
  level3Tokens: number;
  cacheHits: number;
  cacheMisses: number;
  selectMs: number | null;
  tokenSource: TokenSourceDto;
}

export type ContextDecisionKindDto =
  'duplicate_exact' | 'duplicate_normalized' | 'duplicate_overlap' | 'compressed' | 'omitted';

/** Mirrors `domain::optimization::ContextEngineMetrics`. */
export interface ContextEngineMetricsDto {
  rawBytes: number;
  finalBytes: number;
  rawEstimatedTokens: number;
  finalEstimatedTokens: number;
  tokenSource: TokenSourceDto;
  deduplicatedLines: number;
  compressedItems: number;
  omittedItems: number;
  decisions: {
    kind: ContextDecisionKindDto;
    source: PromptSectionKindDto;
    keptIn: PromptSectionKindDto | null;
    bytesSaved: number;
    preview: string;
  }[];
  overBudget: { budgetTokens: number; estimatedTokens: number; requiredTokens: number } | null;
  skipped: string | null;
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
  /** The part of `inputTokens` served from the provider's prompt cache, when reported. */
  cachedInputTokens?: number | null;
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
  'not_evaluated' | 'nothing_to_merge' | 'pending' | 'merged' | 'applied' | 'conflict' | 'blocked';

export type BlockReasonDto =
  | 'policy_denied'
  | 'execution_not_completed'
  | 'validation_failed'
  | 'base_dirty'
  | 'base_branch_changed'
  | 'undetermined'
  | 'uncommitted_changes'
  | 'worktree_inconsistent'
  | 'conflict'
  | 'protected_paths'
  | 'needs_review';

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
  /** Items the project has changed under since they were written. */
  staleItems?: number;
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

/** A route the user repaired: what the edge tested before and what it tests now. */
export interface RouteRepairDto {
  edgeId: string;
  nodeId: string;
  agentId: string;
  targetNodeId: string;
  previous: ConditionDto | null;
  current: ConditionDto;
  /** The version of the workflow the repair produced. */
  version: number;
  at: number;
}

/** An edge that cannot match its agent's contract, and the outcome Atlas would pair it with. */
export interface EdgeRepairProposalDto {
  edgeId: string;
  targetNodeId: string;
  current: ConditionDto;
  suggested: string | null;
}

export interface RepairProposalDto {
  nodeId: string;
  agentId: string;
  agent: string;
  /** The outcomes the agent's contract declares, in the contract's order. */
  declared: string[];
  edges: EdgeRepairProposalDto[];
}

export interface RepairChoiceDto {
  edgeId: string;
  outcome: string;
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
  routeRepairs: RouteRepairDto[];
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
  | 'outcome_without_route'
  | 'status_route_on_contract'
  | 'cannot_reach_end'
  | 'missing_agent'
  | 'unknown_agent'
  | 'workspace_mismatch'
  | 'too_many_retries';

/** An error keeps the workflow from running; a warning is only shown. */
export type ValidationSeverityDto = 'error' | 'warning';

export interface ValidationIssueDto {
  code: ValidationIssueCodeDto;
  severity: ValidationSeverityDto;
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
  | 'integration_changed'
  /** A guardrail took secrets out of a step's result before it was handed on. */
  | 'guardrail_transformed';

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
  /** Each time the run was picked up again after failing. */
  recoveries: RecoveryRecordDto[];
  events: WorkflowEventDto[];
  startedAt: number;
  updatedAt: number;
  completedAt: number | null;
}

/** A retry runs the same step again; a resume goes on from a step that completed. */
export type RecoveryKindDto = 'retry' | 'resume';

export type RecoveryProblemDto =
  'not_recoverable' | 'no_route' | 'loop_limit' | 'recovery_required' | 'code_settled';

export interface RecoveryPlanDto {
  kind: RecoveryKindDto;
  failureNodeId: string | null;
  lastCompletedNodeId: string | null;
  /** The Recovery Point: the steps that run next. */
  restartNodeIds: string[];
  /** Steps that completed and are kept as they are. */
  reusedNodeIds: string[];
  problem: RecoveryProblemDto | null;
}

export interface RecoveryRecordDto {
  at: number;
  kind: RecoveryKindDto;
  failure: WorkflowFailureDto | null;
  restartedNodeIds: string[];
  reusedNodeIds: string[];
  workflowVersion: number;
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

export type WorktreeAvailabilityDto = 'available' | 'missing' | 'invalid';

export type LivePhaseDto =
  'idle' | 'running' | 'waiting_for_input' | 'cancelled' | 'ended' | 'stopped';

export type ObservationModeDto = 'events' | 'polling' | 'stopped';

/** Where the run's worktree stands right now. Apply updates whose `revision` is the next one. */
export interface LiveWorkspaceStateDto {
  runId: string;
  worktreeExecutionId: string;
  branch: string;
  /** The commit the workflow started from: what every file is compared with. */
  baselineRevision: string;
  currentRevision: string | null;
  availability: WorktreeAvailabilityDto;
  phase: LivePhaseDto;
  observation: ObservationModeDto;
  files: FileChangeDto[];
  filesChanged: number;
  additions: number;
  deletions: number;
  revision: number;
  updatedAt: number;
  lastReconciledAt: number | null;
}

/** `live_workspace:changed`. With `full`, `changed` is every file and anything else is gone. */
export interface LiveWorkspaceUpdateDto {
  runId: string;
  worktreeExecutionId: string;
  revision: number;
  full: boolean;
  changed: FileChangeDto[];
  removed: string[];
  currentRevision: string | null;
  availability: WorktreeAvailabilityDto;
  phase: LivePhaseDto;
  observation: ObservationModeDto;
  filesChanged: number;
  additions: number;
  deletions: number;
  updatedAt: number;
}

export type LiveFileKindDto = 'text' | 'binary' | 'deleted' | 'symlink' | 'not_file';

export interface LiveFileDto {
  path: string;
  kind: LiveFileKindDto;
  content: string | null;
  size: number | null;
  /** `content` is only the beginning of the file. */
  truncated: boolean;
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
  /** The changes are in the project's working tree, uncommitted, and may be taken back out. */
  canUndo: boolean;
  conflicts: string[];
  message: string | null;
  /** What the ChangeSet review found the last time Apply was asked for. */
  review: ChangeSetReviewDto | null;
  /** The review that needs a person: Apply again, having seen it, is their decision. */
  reviewPending: string | null;
  updatedAt: number;
}

/** Mirrors `domain::guardrail::ChangeSetReview`: names and paths, never file content. */
export type ChangeSetHealthDto = 'healthy' | 'needs_review' | 'invalid';

export interface ChangeSetReviewDto {
  health: ChangeSetHealthDto;
  issues: { code: string; path: string }[];
  filesReviewed: number;
  fingerprint: string;
}

export interface IdeDto {
  id: string;
  name: string;
}
