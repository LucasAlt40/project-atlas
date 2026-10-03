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
  get_agent_usage: {
    args: { workspaceId: string; agentId: string; windows: UsageWindowsDto };
    result: AgentUsageSummaryDto;
  };
  get_workspace_usage: {
    args: { workspaceId: string; windows: UsageWindowsDto };
    result: WorkspaceUsageSummaryDto;
  };
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
  | 'language_unsupported'
  | 'storage_failed';

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
export interface AgentDto {
  id: string;
  name: string;
  personalityId: string;
  runtimeId: string;
  modelId: string;
  instructions: string;
  createdAt: number;
}

/** Mirrors `application::agents::CreateAgentRequest`. */
export interface CreateAgentRequestDto {
  name: string;
  personalityId: string;
  runtimeId: string;
  modelId: string;
  instructions: string;
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

export type TaskStatusDto = 'pending' | 'running' | 'completed' | 'failed';

export type FailureKindDto =
  | 'runtime_not_installed'
  | 'runtime_unavailable'
  | 'authentication_required'
  | 'model_unavailable'
  | 'timeout'
  | 'execution_failed'
  | 'invalid_request'
  | 'unexpected_response';

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
  | 'completed'
  | 'failed';

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
  /** `runtime` / `model` / `tool` names, `failureKind` on failure, runtime-reported values. */
  metadata: Record<string, string>;
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
