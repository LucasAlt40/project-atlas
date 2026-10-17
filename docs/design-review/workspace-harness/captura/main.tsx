// Design-review capture harness: runs the real app in a browser with a scripted Tauri backend.
// Not part of the product; lives in the git-ignored tmp/ folder.
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { App } from '@/app/App';
import '@/styles/global.css';

const now = Date.now();
const MIN = 60_000;
const params = new URLSearchParams(location.search);
// ?harness=none|review|ready|stale  ?agents=0..3  ?runs=0|1
const harnessMode = params.get('harness') ?? 'ready';
const agentCount = Number(params.get('agents') ?? 3);
const withRuns = params.get('runs') !== '0';
const emptyWorkspace = params.get('workspaces') === '0';

const caps = {
  modelDiscovery: false,
  streaming: false,
  systemPrompt: 'unsupported',
  mcp: 'not_investigated',
  mcpFeatures: { toolFilter: 'unsupported', probe: 'none', strict: false },
  nonInteractiveExecution: true,
  authentication: ['cli_session'],
  usageMetrics: true,
  costMetrics: true,
  quotaMetrics: true,
  interactiveTerminal: true,
  interrupt: true,
  terminalInput: false,
  terminalResize: true,
  textOnly: false,
  fileEdit: false,
  toolAccess: { filesystemWrite: false, processExecution: false, network: false },
};
const runtimes = [
  {
    runtime: {
      id: 'claude',
      name: 'Claude CLI',
      provider: { id: 'anthropic', name: 'Anthropic' },
      transport: 'cli',
      capabilities: caps,
      modelHint: 'claude_alias',
    },
    availability: 'ready',
    version: '2.1.285',
    authentication: { kind: 'cli_session', state: 'authenticated' },
    availableModels: [],
    modelDiscovery: 'unsupported',
    discoveryError: null,
    notice: null,
  },
  {
    runtime: {
      id: 'codex',
      name: 'Codex CLI',
      provider: { id: 'openai', name: 'OpenAI' },
      transport: 'cli',
      capabilities: caps,
      modelHint: null,
    },
    availability: 'ready',
    version: '0.9.2',
    authentication: { kind: 'cli_session', state: 'authenticated' },
    availableModels: [{ id: 'gpt-5', name: 'gpt-5' }],
    modelDiscovery: 'discovered',
    discoveryError: null,
    notice: null,
  },
];
const outcomes = [
  { id: 'approved', label: 'Aprovado', description: 'O trabalho está aprovado.' },
  { id: 'changes_requested', label: 'Mudanças pedidas', description: 'Precisa de ajustes.' },
];
const personalities = [
  {
    id: 'architect',
    name: 'Arquiteto',
    description: 'Perfil focado em arquitetura.',
    systemInstructions: 'Você é um arquiteto de software experiente.',
    behavior: ['Analisa arquitetura', 'Identifica fronteiras'],
    tags: ['arquitetura'],
    source: 'builtin',
    suggestedContract: { kind: 'review', outcomes },
    suggestedPermissionProfile: 'developer',
  },
  {
    id: 'qa',
    name: 'QA',
    description: 'Perfil focado em qualidade.',
    systemInstructions: 'Você é um engenheiro de QA meticuloso.',
    behavior: ['Valida comportamento'],
    tags: [],
    source: 'builtin',
    suggestedContract: { kind: 'validation', outcomes },
    suggestedPermissionProfile: 'read_only',
  },
];
const allAgents = [
  ['a1', 'Arquiteto Backend', 'architect', 'claude', 'sonnet'],
  ['a2', 'QA Regressão', 'qa', 'claude', 'sonnet'],
  ['a3', 'Implementador API', 'architect', 'codex', 'gpt-5'],
].map(([id, name, personalityId, runtimeId, modelId]) => ({
  id,
  name,
  personalityId,
  runtimeId,
  modelId,
  instructions: '',
  permissionProfileId: null,
  worktreeIsolation: true,
  resultContract: { kind: 'general', outcomes: [] },
  createdAt: 1,
}));
const agents = allAgents.slice(0, agentCount);

const security = {
  filesystem: { scope: 'project_only', write: 'allowed' },
  processes: { mode: 'allowed', allowedCommands: ['git', 'npm'] },
  network: { mode: 'denied' },
  git: { read: 'allowed', write: 'allowed', destructive: 'approval_required' },
};
const projects = [
  {
    id: 'w1-main',
    name: 'transport-erp',
    path: '/dev/transport-erp',
    role: 'backend',
    dependsOn: [],
  },
  {
    id: 'w1-web',
    name: 'transport-web',
    path: '/dev/transport-web',
    role: 'frontend',
    dependsOn: ['w1-main'],
  },
];
const workspaces = emptyWorkspace
  ? []
  : [
      {
        id: 'w1',
        name: 'Transport ERP',
        projectPath: '/dev/transport-erp',
        projects,
        description: null,
        createdAt: 1,
        updatedAt: 1,
        security,
        layout: {
          rows: 2,
          columns: 2,
          agentPlacements: agents.map((a, i) => ({
            agentId: a.id,
            position: { row: Math.floor(i / 2), column: i % 2 },
          })),
        },
      },
    ];

const stats = { findings: 42, verified: 31, inferred: 9, user: 2, stale: 0, unknown: 0 };
function harnessSummary(): unknown {
  if (harnessMode === 'none')
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
    };
  const base = {
    status: harnessMode === 'review' ? 'needs_review' : 'initialized',
    projectName: 'transport-erp',
    stack: ['Angular', 'TypeScript', 'Docker'],
    version: 3,
    initializedAt: now - 3 * 86_400_000,
    hasAtlasDir: true,
    analyzedAt: now - 2 * 86_400_000,
    staleness:
      harnessMode === 'stale'
        ? {
            analyzedAt: now - 2 * 86_400_000,
            totalChanges: 4,
            changes: [
              { path: 'package.json', kind: 'modified' },
              { path: 'src/billing/invoice.ts', kind: 'added' },
              { path: 'src/legacy/old.ts', kind: 'removed' },
            ],
          }
        : null,
    stats,
    problem: null,
    health: {
      state:
        harnessMode === 'stale' ? 'stale' : harnessMode === 'review' ? 'needs_review' : 'healthy',
      reasons: [],
    },
  };
  return base;
}
const projectState =
  harnessMode === 'stale'
    ? 'outdated'
    : harnessMode === 'review'
      ? 'needs_review'
      : harnessMode === 'none'
        ? 'not_initialized'
        : 'ready';

const totals = (t: number, runs: number) => ({
  inputTokens: t,
  outputTokens: Math.round(t / 4),
  totalTokens: t + Math.round(t / 4),
  cost: runs ? runs * 0.31 : null,
  currency: runs ? 'USD' : null,
  runs,
  runsWithTokens: runs,
  runsWithCost: runs,
  source: 'runtime_reported',
});
const usage = {
  latestExecution: null,
  conversation: totals(52000, 4),
  today: totals(52000, 4),
  week: totals(180000, 12),
  month: totals(640000, 41),
  quota: null,
};

const ev = (
  executionId: string,
  agentId: string,
  kind: string,
  message: string,
  at: number,
  metadata: Record<string, string> = {},
) => ({
  executionId,
  workspaceId: 'w1',
  taskId: executionId,
  agentId,
  kind,
  message,
  timestamp: at,
  metadata,
});
const timeline = (id: string, agentId: string, start: number, end: string) => [
  ev(id, agentId, 'started', 'Execution started', start),
  ev(id, agentId, 'starting_runtime', 'Starting runtime', start + 1000, { runtime: 'claude' }),
  ev(id, agentId, 'sending_prompt', 'Sending prompt', start + 3000),
  ev(id, agentId, 'waiting_for_model', 'Waiting for model', start + 4000, { model: 'sonnet' }),
  ev(id, agentId, 'tool_started', 'Tool started', start + 20000, { tool: 'Read' }),
  ev(id, agentId, 'tool_completed', 'Tool completed', start + 22000, { tool: 'Read' }),
  ...(end === 'completed' ? [ev(id, agentId, 'completed', 'Completed', start + 9 * MIN)] : []),
  ...(end === 'failed'
    ? [ev(id, agentId, 'failed', 'Failed', start + 3 * MIN, { failureKind: 'runtime_error' })]
    : []),
];
const executions = withRuns
  ? [
      {
        id: 'e1',
        workspaceId: 'w1',
        agentId: 'a1',
        status: 'completed',
        task: 'Revisar a camada de faturamento',
        startedAt: now - 40 * MIN,
        completedAt: now - 31 * MIN,
        runtimeId: 'claude',
        modelId: 'sonnet',
        failure: null,
        metadata: {},
        usage: {
          inputTokens: 12000,
          outputTokens: 3100,
          totalTokens: 15100,
          cost: 0.31,
          currency: 'USD',
          source: 'runtime_reported',
        },
        events: timeline('e1', 'a1', now - 40 * MIN, 'completed'),
      },
      {
        id: 'e2',
        workspaceId: 'w1',
        agentId: 'a2',
        status: 'failed',
        task: 'Rodar testes de regressão',
        startedAt: now - 25 * MIN,
        completedAt: now - 22 * MIN,
        runtimeId: 'claude',
        modelId: 'sonnet',
        failure: { kind: 'runtime_error', message: 'O runtime encerrou com erro.', details: null },
        metadata: {},
        usage: null,
        events: timeline('e2', 'a2', now - 25 * MIN, 'failed'),
      },
      {
        id: 'e3',
        workspaceId: 'w1',
        agentId: 'a3',
        status: 'waiting_for_input',
        task: 'Criar endpoint de recuperação de senha',
        startedAt: now - 8 * MIN,
        completedAt: null,
        runtimeId: 'codex',
        modelId: 'gpt-5',
        failure: null,
        metadata: {},
        usage: null,
        events: timeline('e3', 'a3', now - 8 * MIN, ''),
      },
    ]
  : [];
const messages = withRuns
  ? [
      {
        id: 'm1',
        workspaceId: 'w1',
        agentId: 'a1',
        executionId: 'e1',
        role: 'user',
        content: 'Revise a camada de faturamento e aponte acoplamentos indevidos.',
        timestamp: now - 40 * MIN,
        failed: false,
        failureKind: null,
      },
      {
        id: 'm2',
        workspaceId: 'w1',
        agentId: 'a1',
        executionId: 'e1',
        role: 'assistant',
        content:
          '## Revisão\n\nA camada de **faturamento** depende diretamente de `BillingRepository`.\n\n- Extrair a porta `InvoiceStore`\n- Mover regras de imposto para o domínio\n\n```ts\ninterface InvoiceStore { save(i: Invoice): Promise<void> }\n```',
        timestamp: now - 31 * MIN,
        failed: false,
        failureKind: null,
      },
    ]
  : [];

const analysis = {
  projectName: 'transport-erp',
  path: '/dev/transport-erp',
  findings: [
    ['repository', 'git', 'Git', 'true', 'high', 'fact'],
    ['language', 'typescript', 'TypeScript', 'true', 'high', 'fact'],
    ['framework', 'angular', 'Angular 21', '21', 'high', 'fact'],
    ['testing', 'vitest', 'Vitest', '3', 'high', 'fact'],
    ['infrastructure', 'docker', 'Docker', 'true', 'high', 'fact'],
    ['architecture', 'layered', 'Arquitetura em camadas', 'possible', 'medium', 'inference'],
  ].map(([category, key, label, value, confidence, origin]) => ({
    id: `${category}:${key}`,
    category,
    key,
    label,
    value,
    confidence,
    origin,
    verification:
      origin === 'fact'
        ? { status: 'verified', method: 'repository_file' }
        : { status: 'unverified' },
    evidence: [{ source: 'package.json', field: key }],
    byModel: false,
  })),
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
  user: { purpose: '', users: '', concepts: '', businessRules: '', constraints: '', decisions: '' },
  suggestedUser: {
    purpose: '',
    users: '',
    concepts: '',
    businessRules: '',
    constraints: '',
    decisions: '',
  },
  unmanagedUserFiles: [],
};

const perms = (workspaceId: string, agentId: string) => ({
  workspaceId,
  agentId,
  profile: 'developer',
  availableProfiles: ['read_only', 'developer'],
  policy: security,
  effective: security,
  runtimeAccess: { filesystemWrite: true, processExecution: true, network: true },
  unenforced: [],
});

const handlers: Record<string, (a: any) => unknown> = {
  get_settings: () => ({ language: 'pt-BR', selectedWorkspaceId: workspaces[0]?.id ?? null }),
  list_personalities: () => personalities,
  list_agents: () => agents,
  list_runtimes: () => runtimes,
  list_workspaces: () => workspaces,
  list_messages: () => messages,
  list_executions: () => executions,
  list_execution_worktrees: () => [],
  get_project_context: () => ({
    name: 'transport-erp',
    path: '/dev/transport-erp',
    technologies: ['Angular', 'Git', 'Docker'],
  }),
  get_project_harness: () => harnessSummary(),
  list_workspace_projects: () =>
    projects.map((p, i) => ({ ...p, exists: true, primary: i === 0, technologies: ['Angular'] })),
  get_workspace_harness: () => ({
    projects: projects.map((p) => ({
      projectId: p.id,
      state: projectState,
      summary: harnessSummary(),
    })),
    context: {
      exists: harnessMode !== 'none',
      generatedAt: now - 86_400_000,
      outdated: false,
      hasNotes: false,
    },
  }),
  get_workspace_notes: () => '',
  analyze_project: () => analysis,
  preview_task_context: () => ({ status: 'missing', context: null }),
  get_agent_permissions: (a) => perms(a.workspaceId, a.agentId),
  get_agent_usage: () => usage,
  get_workspace_usage: () => ({
    today: totals(52000, 4),
    week: totals(180000, 12),
    month: totals(640000, 41),
  }),
  get_workspace_security: () => security,
  get_execution_terminal: () => null,
  list_pending_approvals: () => [],
  list_pending_interactions: () => [],
  list_mcp_connections: () => [],
  list_mcp_catalog: () => [],
  list_workflows: () => [],
  list_workflow_templates: () => [],
  list_workflow_executions: () => [],
  list_specs: () => [],
  list_ides: () => [],
  list_memories: () => [],
  get_app_info: () => ({ name: 'Project Atlas', version: '0.1.5' }),
  get_remote_status: () => ({ enabled: false, devices: [] }),
  get_sandbox_status: () => ({}),
};

mockWindows('main');
mockIPC(
  (cmd, payload) => {
    const h = handlers[cmd];
    if (h) return h(payload);
    if (cmd.startsWith('plugin:')) return null;
    if (cmd.startsWith('list_')) return [];
    console.warn('[capture] unmocked command', cmd);
    return null;
  },
  { shouldMockEvents: true },
);

createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
