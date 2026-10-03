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
  getProjectContext,
  listMessages,
  listWorkspaces,
  removeAgentFromWorkspace,
  sendMessage,
  subscribeToExecutionEvents,
  subscribeToMessages,
  updateWorkspace,
} from '@/features/workspace/services/workspaceService';
import type { ExecutionEvent, Message, Workspace } from '@/features/workspace/types';
import type {
  AgentUsageSummaryDto,
  UsageTotalsDto,
  WorkspaceUsageSummaryDto,
} from '@/lib/tauri/commands';

/**
 * Test harness: sample data, a way to script the whole backend (every service module is mocked
 * in the test file with `vi.mock`), and a render that mounts the real providers.
 */

export const architectPersonality: Personality = {
  id: 'architect',
  name: 'Architect',
  description: 'Architecture-focused agent profile.',
  systemInstructions: 'You are an experienced software architect.',
  behavior: ['Analyzes architecture', 'Identifies boundaries'],
  tags: ['architecture'],
  source: 'builtin',
};

export const qaPersonality: Personality = {
  id: 'qa',
  name: 'QA',
  description: 'Quality-focused agent profile.',
  systemInstructions: 'You are a meticulous QA engineer.',
  behavior: ['Validates behavior'],
  tags: [],
  source: 'builtin',
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
    capabilities: { ...capabilities, quotaMetrics: true },
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
  agentUsage?: AgentUsageSummaryDto;
  workspaceUsage?: WorkspaceUsageSummaryDto;
}

let emitEvent: (event: ExecutionEvent) => void = () => undefined;
let emitMessage: (message: Message) => void = () => undefined;

/** Delivers a progress event to the app, as the core would. */
export function emit(event: ExecutionEvent): void {
  emitEvent(event);
}

/** Delivers a conversation message to the app, as the core would. */
export function emitConversationMessage(message: Message): void {
  emitMessage(message);
}

/** Scripts every service. Anything not given is empty / default. */
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
  vi.mocked(getProjectContext).mockImplementation((workspaceId) => {
    const found = (backend.workspaces ?? []).find((w) => w.id === workspaceId);
    return Promise.resolve({
      name: found?.projectPath.split('/').filter(Boolean).at(-1) ?? 'project',
      path: found?.projectPath ?? '/project',
      technologies: ['Angular', 'Git'],
    });
  });
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
