import { render } from '@testing-library/react';
import type { ReactElement } from 'react';
import { NavigationContext, type Navigation } from '@/app/NavigationContext';
import { CatalogProvider } from '@/features/agents/hooks/useCatalog';
import {
  createAgent,
  createPersonality,
  deleteAgent,
  deletePersonality,
  listAgents,
  listPersonalities,
  listRuntimes,
  restoreDefaultPersonalities,
  updateAgent,
  updatePersonality,
} from '@/features/agents/services/catalogService';
import type { Agent, Personality, RuntimeStatus } from '@/features/agents/types';
import { SettingsProvider } from '@/features/settings/hooks/SettingsProvider';
import {
  getSettings,
  selectWorkspace,
  setLanguage,
} from '@/features/settings/services/settingsService';
import { getAgentUsage, getWorkspaceUsage } from '@/features/usage/services/usageService';
import { WorkspaceProvider } from '@/features/workspace/hooks/WorkspaceProvider';
import {
  addAgentToWorkspace,
  createWorkspace,
  deleteWorkspace,
  getAgentPermissions,
  getExecutionTerminal,
  getProjectContext,
  getProjectHarness,
  analyzeProject,
  initializeProject,
  refreshProjectHarness,
  listExecutions,
  listExecutionWorktrees,
  listMessages,
  listWorkspaces,
  previewTaskContext,
  removeAgentFromWorkspace,
  sendMessage,
  setAgentPermissionProfile,
  subscribeToExecutionEvents,
  subscribeToHarnessProgress,
  subscribeToMessages,
  subscribeToSessionStatus,
  subscribeToTerminalOutput,
  updateWorkspace,
} from '@/features/workspace/services/workspaceService';
import type {
  ExecutionEvent,
  Message,
  StoredExecution,
  Workspace,
} from '@/features/workspace/types';
import type {
  AgentPermissionsDto,
  AgentUsageSummaryDto,
  SecurityPolicyDto,
  ExecutionWorktreeDto,
  HarnessProgressDto,
  HarnessSummaryDto,
  InitializeOutcomeDto,
  ProjectAnalysisDto,
  SessionStatusEventDto,
  TaskContextDto,
  TaskContextPreviewDto,
  TerminalChunkDto,
  TerminalSnapshotDto,
  UsageTotalsDto,
  WorkspaceUsageSummaryDto,
} from '@/lib/tauri/commands';

/**
 * Test harness: sample data, a way to script the whole backend (every service module is mocked
 * in the test file with `vi.mock`), and a render that mounts the real providers.
 */

const validationOutcomes = [
  { id: 'pass', label: 'Pass', description: 'Validation passed.' },
  { id: 'fail', label: 'Fail', description: 'Validation failed.' },
];
const reviewOutcomes = [
  { id: 'approved', label: 'Approved', description: 'The work is approved.' },
  { id: 'changes_requested', label: 'Changes requested', description: 'The work needs changes.' },
];

export const architectPersonality: Personality = {
  id: 'architect',
  name: 'Architect',
  description: 'Architecture-focused agent profile.',
  systemInstructions: 'You are an experienced software architect.',
  behavior: ['Analyzes architecture', 'Identifies boundaries'],
  tags: ['architecture'],
  source: 'builtin',
  suggestedContract: { kind: 'review', outcomes: reviewOutcomes },
};

export const qaPersonality: Personality = {
  id: 'qa',
  name: 'QA',
  description: 'Quality-focused agent profile.',
  systemInstructions: 'You are a meticulous QA engineer.',
  behavior: ['Validates behavior'],
  tags: [],
  source: 'builtin',
  suggestedContract: { kind: 'validation', outcomes: validationOutcomes },
};

export const personalities: Personality[] = [architectPersonality, qaPersonality];

const capabilities = {
  modelDiscovery: false,
  streaming: false,
  systemPrompt: false,
  nonInteractiveExecution: true,
  authentication: ['cli_session' as const],
  usageMetrics: true,
  costMetrics: true,
  quotaMetrics: false,
  interactiveTerminal: true,
  interrupt: true,
  terminalInput: false,
  terminalResize: true,
  textOnly: false,
  fileEdit: false,
  toolAccess: { filesystemWrite: false, processExecution: false, network: false },
};

export const opencode: RuntimeStatus = {
  runtime: {
    id: 'opencode',
    name: 'OpenCode CLI',
    provider: { id: 'opencode', name: 'OpenCode' },
    transport: 'cli',
    capabilities: { ...capabilities, modelDiscovery: true },
    modelHint: null,
  },
  availability: 'ready',
  version: '1.18.34',
  authentication: { kind: 'cli_session', state: 'unknown' },
  availableModels: [{ id: 'opencode/big-pickle', name: 'opencode/big-pickle' }],
  modelDiscovery: 'discovered',
  discoveryError: null,
  notice: null,
};

export const claude: RuntimeStatus = {
  runtime: {
    id: 'claude',
    name: 'Claude CLI',
    provider: { id: 'anthropic', name: 'Anthropic' },
    transport: 'cli',
    capabilities: { ...capabilities, quotaMetrics: true, textOnly: true },
    modelHint: 'claude_alias',
  },
  availability: 'ready',
  version: '2.1.285',
  authentication: { kind: 'cli_session', state: 'authenticated' },
  availableModels: [],
  modelDiscovery: 'unsupported',
  discoveryError: null,
  notice: null,
};

export function agent(id: string, name: string, runtimeId = 'claude', modelId = 'sonnet'): Agent {
  return {
    id,
    name,
    personalityId: 'architect',
    runtimeId,
    modelId,
    instructions: '',
    permissionProfileId: null,
    worktreeIsolation: true,
    resultContract: { kind: 'general', outcomes: [] },
    createdAt: 1,
  };
}

export function workspace(
  id: string,
  name: string,
  projectPath: string,
  placed: string[] = [],
): Workspace {
  return {
    id,
    name,
    projectPath,
    description: null,
    createdAt: 1,
    updatedAt: 1,
    security: {
      filesystem: { scope: 'project_only', write: 'allowed' },
      processes: { mode: 'allowed', allowedCommands: ['git', 'npm'] },
      network: { mode: 'denied' },
      git: { read: 'allowed', write: 'allowed', destructive: 'approval_required' },
    },
    layout: {
      rows: 2,
      columns: 2,
      agentPlacements: placed.map((agentId, i) => ({
        agentId,
        position: { row: Math.floor(i / 2), column: i % 2 },
      })),
    },
  };
}

export function totals(overrides: Partial<UsageTotalsDto> = {}): UsageTotalsDto {
  return {
    inputTokens: null,
    outputTokens: null,
    totalTokens: null,
    cost: null,
    currency: null,
    runs: 0,
    runsWithTokens: 0,
    runsWithCost: 0,
    source: 'atlas_calculated',
    ...overrides,
  };
}

export function emptyAgentUsage(): AgentUsageSummaryDto {
  return {
    latestExecution: null,
    conversation: totals(),
    today: totals(),
    week: totals(),
    month: totals(),
    quota: null,
  };
}

export function emptyWorkspaceUsage(): WorkspaceUsageSummaryDto {
  return { today: totals(), week: totals(), month: totals() };
}

export interface Backend {
  language?: string;
  selectedWorkspaceId?: string | null;
  personalities?: Personality[];
  agents?: Agent[];
  runtimes?: RuntimeStatus[];
  workspaces?: Workspace[];
  messages?: Message[];
  /** Executions that ended before the app opened (newest first). */
  executions?: StoredExecution[];
  agentUsage?: AgentUsageSummaryDto;
  workspaceUsage?: WorkspaceUsageSummaryDto;
  /** The Git worktrees of executions that ran isolated. */
  worktrees?: ExecutionWorktreeDto[];
  /** What `getExecutionTerminal` answers (by default: no terminal). */
  terminal?: TerminalSnapshotDto | null;
  /** The project's Harness state (by default: not initialized). */
  harness?: HarnessSummaryDto;
  /** What analysing the project finds (by default: Angular on a Git repository). */
  analysis?: ProjectAnalysisDto;
  /** What previewing a task's context answers (by default: the project has no Harness). */
  taskContext?: TaskContextPreviewDto;
  /** The permission profile of every agent (by default: `read_only`, as a new agent has). */
  profile?: 'read_only' | 'developer';
}

/** A task context chosen for "Add password recovery endpoint": three items in, two out. */
export function taskContext(overrides: Partial<TaskContextDto> = {}): TaskContextDto {
  const item = (id: string, label: string, area: TaskContextDto['includedAreas'][number]) => ({
    id,
    block: 'infer' as const,
    kind: 'finding' as const,
    area,
    category: null,
    label,
    content: label,
    provenance: 'inference' as const,
    verification: 'unverified' as const,
    confidence: 'medium' as const,
    evidence: [{ source: 'src/auth/password-reset.ts' }],
    sourceFindingId: id,
    tags: [],
  });
  const reason = {
    score: 0,
    matchedAreas: [],
    matchedTags: [],
    matchedCategories: [],
    matchedPaths: [],
    matchedKeywords: [],
    alwaysIncluded: null,
    penalties: [],
  };
  return {
    mode: 'task_aware',
    fallbackReason: null,
    signals: {
      keywords: ['password', 'recovery', 'endpoint'],
      areas: ['business', 'modules'],
      baselineAreas: ['architecture', 'conventions', 'testing'],
      tags: ['authentication', 'password', 'api', 'backend'],
      intent: 'add',
      technologies: [],
    },
    text: 'This context was selected from the project’s Harness based on the current task.',
    entries: [
      {
        item: item('module:auth', 'Authentication module', 'modules'),
        reason: {
          ...reason,
          score: 120,
          matchedTags: ['authentication'],
          matchedAreas: ['modules'],
          matchedPaths: ['src/auth/password-reset.ts'],
        },
        outcome: 'included',
      },
      {
        item: item('architecture:clean', 'Clean Architecture', 'architecture'),
        reason: { ...reason, score: 25, matchedAreas: ['architecture'] },
        outcome: 'included',
      },
      {
        item: {
          ...item('constraints', 'Constraints', 'constraints'),
          kind: 'constraint' as const,
          block: 'user' as const,
        },
        reason: { ...reason, score: 100, alwaysIncluded: 'constraint' },
        outcome: 'included',
      },
      {
        item: item('module:billing', 'Billing module', 'modules'),
        reason,
        outcome: 'not_relevant',
      },
      {
        item: item('module:maps', 'Maps module', 'modules'),
        reason: { ...reason, penalties: ['layer_mismatch'] },
        outcome: 'not_relevant',
      },
    ],
    includedAreas: ['architecture', 'constraints', 'modules'],
    excludedAreas: [],
    selectedChars: 1984,
    fullHarnessChars: 5800,
    budgetChars: 6000,
    truncated: false,
    omitted: [],
    ...overrides,
  };
}

export function harnessSummary(overrides: Partial<HarnessSummaryDto> = {}): HarnessSummaryDto {
  return {
    status: 'not_initialized',
    projectName: null,
    stack: [],
    version: null,
    initializedAt: null,
    hasAtlasDir: false,
    analyzedAt: null,
    staleness: null,
    stats: null,
    problem: null,
    health: null,
    ...overrides,
  };
}

export function outcome(summary: HarnessSummaryDto, extra: Partial<InitializeOutcomeDto> = {}) {
  return { summary, written: [], backedUp: [], leftUntouched: [], ...extra };
}

export function projectAnalysis(overrides: Partial<ProjectAnalysisDto> = {}): ProjectAnalysisDto {
  const finding = (
    category: ProjectAnalysisDto['findings'][number]['category'],
    key: string,
    label: string,
    value = 'true',
    extra: Partial<ProjectAnalysisDto['findings'][number]> = {},
  ): ProjectAnalysisDto['findings'][number] => ({
    id: `${category}:${key}`,
    category,
    key,
    label,
    value,
    confidence: 'high',
    origin: 'fact',
    verification: { status: 'verified', method: 'repository_file' },
    evidence: [{ source: 'package.json', field: key }],
    byModel: false,
    ...extra,
  });
  return {
    projectName: 'transport-erp',
    path: '/dev/transport-erp',
    findings: [
      finding('repository', 'git', 'Git'),
      finding('repository', 'current_branch', 'Current branch main', 'main'),
      finding('language', 'typescript', 'TypeScript'),
      finding('framework', 'angular', 'Angular 21', '21'),
      finding('testing', 'vitest', 'Vitest', '3'),
      finding('infrastructure', 'docker', 'Docker'),
      finding('ci', 'github_actions', 'GitHub Actions'),
      finding('architecture', 'layered', 'Layered', 'possible', {
        confidence: 'medium',
        origin: 'inference',
        verification: { status: 'unverified' },
      }),
    ],
    conflicts: [],
    gaps: [],
    partial: false,
    scannedEntries: 42,
    analysis: {
      partial: false,
      scannedEntries: 42,
      sampledFiles: [],
      semantic: {
        status: 'not_run',
        error: null,
        errorDetail: null,
        sentFiles: [],
        rejected: 0,
        explored: false,
      },
      analyzedAt: 1,
    },
    existing: harnessSummary(),
    diff: null,
    previousCorrections: {},
    previousExcluded: [],
    previousConfirmed: [],
    user: {
      purpose: '',
      users: '',
      concepts: '',
      businessRules: '',
      constraints: '',
      decisions: '',
    },
    suggestedUser: {
      purpose: '',
      users: '',
      concepts: '',
      businessRules: '',
      constraints: '',
      decisions: '',
    },
    unmanagedUserFiles: [],
    ...overrides,
  };
}

let emitEvent: (event: ExecutionEvent) => void = () => undefined;
let emitMessage: (message: Message) => void = () => undefined;
let emitOutput: (chunk: TerminalChunkDto) => void = () => undefined;
let emitHarnessProgress: (update: HarnessProgressDto) => void = () => undefined;
let emitStatus: (event: SessionStatusEventDto) => void = () => undefined;

/** Delivers a progress event to the app, as the core would. */
export function emit(event: ExecutionEvent): void {
  emitEvent(event);
}

/** Delivers a conversation message to the app, as the core would. */
export function emitConversationMessage(message: Message): void {
  emitMessage(message);
}

/** Delivers a piece of terminal output to the app, as the core would. */
export function emitTerminalOutput(chunk: TerminalChunkDto): void {
  emitOutput(chunk);
}

/** Delivers a harness-analysis progress update to the app, as the core would. */
export function emitHarness(update: HarnessProgressDto): void {
  emitHarnessProgress(update);
}

/** Delivers a process state change to the app, as the core would. */
export function emitSessionStatus(event: SessionStatusEventDto): void {
  emitStatus(event);
}

/** Scripts every service. Anything not given is empty / default. */
/** What the core answers for an agent's permissions: read only, or the developer profile. */
export function agentPermissions(
  workspaceId: string,
  agentId: string,
  profile: 'read_only' | 'developer',
): AgentPermissionsDto {
  const write = profile === 'developer' ? 'allowed' : 'denied';
  const policy: SecurityPolicyDto = {
    filesystem: { scope: 'project_only', write },
    processes: { mode: write, allowedCommands: [] },
    network: { mode: 'denied' },
    git: { read: 'allowed', write, destructive: 'denied' },
  };
  return {
    workspaceId,
    agentId,
    profile,
    availableProfiles: ['read_only', 'developer'],
    policy,
    effective: policy,
    runtimeAccess: { filesystemWrite: true, processExecution: true, network: true },
    unenforced: [],
  };
}

export function mockBackend(backend: Backend = {}): void {
  const settings = {
    language: backend.language ?? 'en-US',
    selectedWorkspaceId: backend.selectedWorkspaceId ?? null,
  };
  vi.mocked(getSettings).mockResolvedValue(settings);
  vi.mocked(setLanguage).mockImplementation((language) =>
    Promise.resolve({ ...settings, language }),
  );
  vi.mocked(selectWorkspace).mockImplementation((workspaceId) =>
    Promise.resolve({ ...settings, selectedWorkspaceId: workspaceId }),
  );
  vi.mocked(listPersonalities).mockResolvedValue(backend.personalities ?? personalities);
  vi.mocked(listAgents).mockResolvedValue(backend.agents ?? []);
  vi.mocked(listRuntimes).mockResolvedValue(backend.runtimes ?? [claude, opencode]);
  vi.mocked(listWorkspaces).mockResolvedValue(backend.workspaces ?? []);
  vi.mocked(listMessages).mockResolvedValue(backend.messages ?? []);
  vi.mocked(listExecutions).mockResolvedValue(backend.executions ?? []);
  vi.mocked(listExecutionWorktrees).mockResolvedValue(backend.worktrees ?? []);
  vi.mocked(getProjectContext).mockImplementation((workspaceId) => {
    const found = (backend.workspaces ?? []).find((w) => w.id === workspaceId);
    return Promise.resolve({
      name: found?.projectPath.split('/').filter(Boolean).at(-1) ?? 'project',
      path: found?.projectPath ?? '/project',
      technologies: ['Angular', 'Git'],
    });
  });
  vi.mocked(getProjectHarness).mockResolvedValue(backend.harness ?? harnessSummary());
  vi.mocked(analyzeProject).mockResolvedValue(backend.analysis ?? projectAnalysis());
  vi.mocked(previewTaskContext).mockResolvedValue(
    backend.taskContext ?? { status: 'missing', context: null },
  );
  vi.mocked(getAgentPermissions).mockImplementation((workspaceId, agentId) =>
    Promise.resolve(agentPermissions(workspaceId, agentId, backend.profile ?? 'read_only')),
  );
  vi.mocked(setAgentPermissionProfile).mockResolvedValue(agent('a', 'a'));
  vi.mocked(getAgentUsage).mockResolvedValue(backend.agentUsage ?? emptyAgentUsage());
  vi.mocked(getWorkspaceUsage).mockResolvedValue(backend.workspaceUsage ?? emptyWorkspaceUsage());
  vi.mocked(subscribeToExecutionEvents).mockImplementation((handler) => {
    emitEvent = handler;
    return Promise.resolve(() => undefined);
  });
  vi.mocked(subscribeToMessages).mockImplementation((handler) => {
    emitMessage = handler;
    return Promise.resolve(() => undefined);
  });
  vi.mocked(subscribeToTerminalOutput).mockImplementation((handler) => {
    emitOutput = handler;
    return Promise.resolve(() => undefined);
  });
  vi.mocked(subscribeToHarnessProgress).mockImplementation((handler) => {
    emitHarnessProgress = handler;
    return Promise.resolve(() => undefined);
  });
  vi.mocked(subscribeToSessionStatus).mockImplementation((handler) => {
    emitStatus = handler;
    return Promise.resolve(() => undefined);
  });
  vi.mocked(getExecutionTerminal).mockResolvedValue(backend.terminal ?? null);
  for (const fn of [
    createAgent,
    updateAgent,
    deleteAgent,
    createPersonality,
    updatePersonality,
    deletePersonality,
    restoreDefaultPersonalities,
    createWorkspace,
    updateWorkspace,
    deleteWorkspace,
    addAgentToWorkspace,
    removeAgentFromWorkspace,
    sendMessage,
    initializeProject,
    refreshProjectHarness,
  ]) {
    vi.mocked(fn).mockReset();
  }
}

export const navigate = vi.fn();

/** Mounts the real providers (settings, catalog, workspaces) around `ui`. */
export function renderWithProviders(ui: ReactElement) {
  const navigation: Navigation = { navigate, intent: undefined };
  return render(
    <SettingsProvider>
      <CatalogProvider>
        <WorkspaceProvider>
          <NavigationContext.Provider value={navigation}>{ui}</NavigationContext.Provider>
        </WorkspaceProvider>
      </CatalogProvider>
    </SettingsProvider>,
  );
}

export function executionEvent(
  workspaceId: string,
  agentId: string,
  executionId: string,
  kind: ExecutionEvent['kind'],
  metadata: Record<string, string> = {},
  message = '',
): ExecutionEvent {
  return {
    executionId,
    workspaceId,
    taskId: 't',
    agentId,
    kind,
    message,
    timestamp: Date.now(),
    metadata,
  };
}

export function chatMessage(
  workspaceId: string,
  agentId: string,
  executionId: string,
  role: 'user' | 'assistant',
  content: string,
  extra: Partial<Message> = {},
): Message {
  return {
    id: `m-${role}-${executionId}`,
    workspaceId,
    agentId,
    executionId,
    role,
    content,
    timestamp: Date.now() + (role === 'assistant' ? 1 : 0),
    failed: false,
    failureKind: null,
    ...extra,
  };
}

/** An execution the core kept after it ended. */
export function storedExecution(
  workspaceId: string,
  agentId: string,
  id: string,
  overrides: Partial<StoredExecution> = {},
): StoredExecution {
  const startedAt = Date.now() - 60_000;
  return {
    id,
    workspaceId,
    agentId,
    status: 'completed',
    task: `Task ${id}`,
    startedAt,
    completedAt: startedAt + 4_000,
    runtimeId: 'claude',
    modelId: 'sonnet',
    failure: null,
    metadata: {},
    usage: null,
    events: [
      { ...executionEvent(workspaceId, agentId, id, 'started'), timestamp: startedAt },
      { ...executionEvent(workspaceId, agentId, id, 'completed'), timestamp: startedAt + 4_000 },
    ],
    ...overrides,
  };
}

export function terminalChunk(
  executionId: string,
  seq: number,
  data: string,
  ids: { workspaceId?: string; agentId?: string } = {},
): TerminalChunkDto {
  return {
    executionId,
    processSessionId: `ps-${executionId}`,
    workspaceId: ids.workspaceId ?? 'w1',
    agentId: ids.agentId ?? 'a1',
    timestamp: Date.now(),
    stream: 'stdout',
    seq,
    data,
  };
}

export function sessionStatus(
  executionId: string,
  status: SessionStatusEventDto['status'],
  extra: Partial<SessionStatusEventDto> = {},
): SessionStatusEventDto {
  return {
    executionId,
    processSessionId: `ps-${executionId}`,
    workspaceId: 'w1',
    agentId: 'a1',
    status,
    userAction: null,
    exitCode: null,
    timestamp: Date.now(),
    ...extra,
  };
}

export function terminalSnapshot(
  executionId: string,
  extra: Partial<TerminalSnapshotDto> = {},
): TerminalSnapshotDto {
  return {
    executionId,
    processSessionId: `ps-${executionId}`,
    workspaceId: 'w1',
    agentId: 'a1',
    status: 'running',
    userAction: null,
    command: 'claude -p --output-format stream-json',
    startedAt: 1,
    endedAt: null,
    exitCode: null,
    cols: 120,
    rows: 30,
    inputEnabled: false,
    output: '',
    nextSeq: 0,
    truncated: false,
    ...extra,
  };
}

/** The Git worktree of an execution that ran isolated, merged into nothing yet. */
export function worktreeOf(
  workspaceId: string,
  agentId: string,
  executionId: string,
  overrides: Partial<ExecutionWorktreeDto> = {},
): ExecutionWorktreeDto {
  return {
    executionId,
    workspaceId,
    agentId,
    baseBranch: 'main',
    baseCommit: 'a'.repeat(40),
    branchName: `atlas/${executionId}`,
    worktreePath: `/data/worktrees/${workspaceId}/${executionId}`,
    workingDir: `/data/worktrees/${workspaceId}/${executionId}`,
    repositoryPath: '/dev/atlas',
    status: 'completed',
    mergeStatus: 'pending',
    blockReason: null,
    baseDirtyAtStart: false,
    createdAt: 1,
    changes: {
      filesChanged: 7,
      commitsAhead: 2,
      commitsBehind: 0,
      files: ['a.ts'],
      conflicts: [],
      mergeable: true,
    },
    validation: 'not_run',
    recommendation: 'merge',
    ...overrides,
  };
}
