// Design-review capture harness: runs the real app in a browser with a scripted Tauri backend.
// Not part of the product; lives in the git-ignored tmp/ folder.
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { App } from '@/app/App';
import '@/styles/global.css';
import {
  agentNode,
  attempt as wfAttempt,
  edge,
  endNode,
  handoff as wfHandoff,
  nodeState,
  passwordRecovery,
  pendingInteraction,
  run as wfRun,
  templates as wfTemplates,
  withCode as wfWithCode,
} from '@/test/workflowFixtures';
import {
  allApproved,
  artifact,
  spec as specOf,
  summary as specSummary,
  view as specView,
} from '@/test/specFixtures';

const now = Date.now();
const MIN = 60_000;
const params = new URLSearchParams(location.search);
// ?harness=none|review|ready|stale  ?agents=0..3  ?runs=0|1
const harnessMode = params.get('harness') ?? 'ready';
const agentCount = Number(params.get('agents') ?? 3);
const withRuns = params.get('runs') !== '0';
const emptyWorkspace = params.get('workspaces') === '0';
// ?custom=1 adds a custom personality; ?runtimes=down makes Codex not installed (Agentes, Personalidades)
const withCustom = params.get('custom') === '1';
const runtimesDown = params.get('runtimes') === 'down';
// ?mcp=none|some (MCP Hub; default some)
const mcpMode = params.get('mcp') ?? 'some';
// ?memory=none|some (Configurações > Memória; default some)
const memoryMode = params.get('memory') ?? 'some';
// ?specs=none|some (Specs; default some)
const specsMode = params.get('specs') ?? 'some';
// ?remote=off|on|error (Celular dialog; default on, with one device paired)
const remoteMode = params.get('remote') ?? 'on';
// ?wf=none|idle|running|waiting|done|failed (Workflow; default idle: a workflow without a run)
const wfMode = params.get('wf') ?? 'idle';

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
if (withCustom) {
  personalities.push({
    id: 'dotnet-senior',
    name: 'Arquiteto .NET Sênior',
    description: 'Foca em Clean Architecture e DDD para serviços .NET.',
    systemInstructions:
      'Você é um arquiteto .NET sênior. Prefira Clean Architecture, DDD e design pragmático.',
    behavior: ['Aplica Clean Architecture', 'Modela o domínio com DDD'],
    tags: ['dotnet', 'ddd'],
    source: 'custom',
    suggestedContract: { kind: 'review', outcomes },
    suggestedPermissionProfile: 'developer',
  } as (typeof personalities)[number]);
}
if (runtimesDown) {
  Object.assign(runtimes[1], {
    availability: 'not_installed',
    version: null,
    authentication: { kind: 'cli_session', state: 'unknown' },
    availableModels: [],
  });
}
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

const mcpConnections =
  mcpMode === 'none'
    ? []
    : [
        {
          id: 'c1',
          workspaceId: 'w1',
          name: 'github',
          transport: {
            kind: 'stdio',
            executable: 'npx',
            args: ['-y', '@modelcontextprotocol/server-github@1.4.0'],
            env: [{ name: 'GITHUB_TOKEN', value: { kind: 'secret' } }],
          },
          enabled: true,
          required: false,
          secrets: { GITHUB_TOKEN: now - 86_400_000 },
          discovery: {
            discoveredAt: now - 12 * MIN,
            runtimeId: 'claude',
            status: 'connected',
            tools: [
              'create_issue',
              'list_pull_requests',
              'get_file_contents',
              'search_code',
              'create_branch',
            ],
          },
          createdAt: now - 5 * 86_400_000,
        },
        {
          id: 'c2',
          workspaceId: 'w1',
          name: 'playwright',
          transport: { kind: 'stdio', executable: 'npx', args: ['@playwright/mcp@0.0.30'] },
          enabled: true,
          required: true,
          secrets: {},
          discovery: {
            discoveredAt: now - 3 * 3_600_000,
            runtimeId: 'claude',
            status: 'failed',
            tools: ['browser_navigate', 'browser_click', 'browser_snapshot'],
          },
          createdAt: now - 4 * 86_400_000,
        },
        {
          id: 'c3',
          workspaceId: 'w1',
          name: 'postgres',
          transport: {
            kind: 'stdio',
            executable: 'uvx',
            args: ['mcp-server-postgres'],
            env: [{ name: 'DATABASE_URL', value: { kind: 'secret' } }],
          },
          enabled: false,
          required: false,
          secrets: {},
          createdAt: now - 2 * 86_400_000,
        },
      ];
const mcpGrants =
  mcpMode === 'none'
    ? []
    : [
        {
          id: 'g1',
          connectionId: 'c1',
          agentId: 'a1',
          workflowId: null,
          nodeId: null,
          tools: { kind: 'server' },
        },
        {
          id: 'g2',
          connectionId: 'c1',
          agentId: 'a3',
          workflowId: null,
          nodeId: null,
          tools: { kind: 'only', tools: ['create_issue', 'search_code'] },
        },
        {
          id: 'g3',
          connectionId: 'c2',
          agentId: 'a2',
          workflowId: null,
          nodeId: null,
          tools: { kind: 'server' },
        },
      ];
const mcpCatalog = [
  {
    id: 'chrome-devtools',
    connectionName: 'chrome-devtools',
    availability: 'ready',
    risk: 'high',
    sourceUrl: 'https://github.com/ChromeDevTools/chrome-devtools-mcp',
    pinnedVersion: '0.2.1',
    command: 'npx -y chrome-devtools-mcp@0.2.1',
    requirements: [
      { requirement: 'npx', found: true },
      { requirement: 'chrome', found: true },
    ],
  },
  {
    id: 'figma',
    connectionName: 'figma',
    availability: 'needs_http_and_oauth',
    risk: 'high',
    sourceUrl: 'https://mcp.figma.com',
    pinnedVersion: null,
    command: null,
    requirements: [],
  },
  {
    id: 'jira',
    connectionName: 'jira',
    availability: 'needs_http_and_oauth',
    risk: 'high',
    sourceUrl: 'https://mcp.atlassian.com',
    pinnedVersion: null,
    command: null,
    requirements: [],
  },
];
const mem = (
  id: string,
  scope: string,
  kind: string,
  title: string,
  content: string,
  status: string,
  automatic: boolean,
  uses: number,
) => ({
  id,
  scope,
  owner: scope === 'user' ? null : scope === 'workspace' ? 'w1' : 'a1',
  kind,
  title,
  content,
  topic: null,
  provenance: {
    origin: automatic ? 'agent' : 'user',
    executionId: automatic ? 'e1' : null,
    automatic,
    agentId: automatic ? 'a1' : null,
    validatedBy: null,
  },
  status,
  validity: { paths: [], scripts: [], expiresAfterDays: null },
  uses,
  createdAt: now - 3 * 86_400_000,
  updatedAt: now - 86_400_000,
  lastUsedAt: uses ? now - 3_600_000 : null,
});
const memories =
  memoryMode === 'none'
    ? []
    : [
        mem(
          'm1',
          'workspace',
          'pitfall',
          'Rodar os testes com --runInBand',
          'Os testes de integração do faturamento compartilham o banco; sem --runInBand eles falham de forma intermitente.',
          'candidate',
          true,
          0,
        ),
        mem(
          'm2',
          'agent',
          'how_to',
          'Gerar migração',
          'Use npm run migration:generate -- <nome>; nunca edite uma migração já aplicada.',
          'candidate',
          true,
          0,
        ),
        mem(
          'm3',
          'user',
          'preference',
          'Mensagens de commit em português',
          'Commits em português, no estilo convencional (feat, fix, refactor).',
          'active',
          false,
          7,
        ),
        mem(
          'm4',
          'workspace',
          'decision',
          'Camada de faturamento isolada',
          'A camada de faturamento só depende de portas do domínio.',
          'active',
          false,
          3,
        ),
      ];
const specList =
  specsMode === 'none'
    ? []
    : [
        specSummary({
          id: 's1',
          title: 'Modo escuro',
          changeId: 'dark-mode',
          status: 'awaiting_review',
          approved: 1,
          currentArtifact: 'specs',
          workspaceId: 'w1',
          projectId: 'w1-main',
        }),
        specSummary({
          id: 's2',
          title: 'Exportar faturas em CSV',
          changeId: 'export-csv',
          status: 'generating',
          approved: 0,
          currentArtifact: 'proposal',
          workspaceId: 'w1',
          projectId: 'w1-main',
        }),
        specSummary({
          id: 's3',
          title: 'Recuperação de senha',
          changeId: 'password-recovery',
          status: 'changes_requested',
          approved: 2,
          currentArtifact: 'design',
          pendingQuestions: 2,
          openFindings: 1,
          workspaceId: 'w1',
          projectId: 'w1-main',
        }),
        specSummary({
          id: 's4',
          title: 'Refatorar o faturamento',
          changeId: 'billing-refactor',
          status: 'ready_for_implementation',
          approved: 4,
          currentArtifact: null,
          workspaceId: 'w1',
          projectId: 'w1-main',
        }),
        specSummary({
          id: 's5',
          title: 'Painel de métricas',
          changeId: 'metrics-panel',
          status: 'draft',
          approved: 0,
          currentArtifact: null,
          workspaceId: 'w1',
          projectId: 'w1-main',
        }),
      ];
function specViewFor(id: string) {
  const base = {
    workspaceId: 'w1',
    projectId: 'w1-main',
    authorAgentId: 'a1',
    reviewerAgentId: 'a2',
  };
  if (id === 's2')
    return specView('generating', {
      ...base,
      id,
      title: 'Exportar faturas em CSV',
      changeId: 'export-csv',
      artifacts: [
        artifact('proposal', { generation: { state: 'running' } as never }),
        artifact('specs'),
        artifact('design'),
        artifact('tasks'),
      ],
    });
  if (id === 's3')
    return specView('changes_requested', {
      ...base,
      id,
      title: 'Recuperação de senha',
      changeId: 'password-recovery',
      artifacts: [
        artifact('proposal', { approved: true }),
        artifact('specs', { approved: true }),
        artifact('design', { review: 'changes_requested' }),
        artifact('tasks'),
      ],
    });
  if (id === 's4')
    return specView(
      'ready_for_implementation',
      {
        ...base,
        id,
        title: 'Refatorar o faturamento',
        changeId: 'billing-refactor',
        artifacts: allApproved(),
      },
      { readiness: { ready: true, blockers: [] } as never },
    );
  if (id === 's5')
    return specView('draft', {
      ...base,
      id,
      title: 'Painel de métricas',
      changeId: 'metrics-panel',
      started: false,
      artifacts: [artifact('proposal'), artifact('specs'), artifact('design'), artifact('tasks')],
    });
  return specView('awaiting_review', {
    ...base,
    id: 's1',
    title: 'Modo escuro',
    changeId: 'dark-mode',
    artifacts: [
      artifact('proposal', { approved: true }),
      artifact('specs', {}),
      artifact('design'),
      artifact('tasks'),
    ],
  });
}
void specOf;
const qr = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 29 29" shape-rendering="crispEdges"><rect width="29" height="29" fill="#fff"/><path fill="#000" d="M2 2h7v7H2zM20 2h7v7h-7zM2 20h7v7H2zM11 3h2v2h-2zM14 5h3v2h-3zM11 9h4v2h-4zM17 11h3v3h-3zM11 14h3v3h-3zM15 17h5v2h-5zM21 14h4v3h-4zM12 20h3v4h-3zM17 21h3v3h-3zM22 21h3v4h-3z"/></svg>`;
const remoteStatus = () => ({
  state: remoteMode === 'off' ? 'disabled' : remoteMode === 'error' ? 'error' : 'active',
  error: remoteMode === 'error' ? 'no_network' : null,
  url: remoteMode === 'on' ? 'http://192.168.1.20:47831' : null,
  address: remoteMode === 'on' ? '192.168.1.20' : null,
  port: remoteMode === 'on' ? 47831 : null,
  addressLost: false,
  candidates: [
    { address: '192.168.1.20', interface: 'en0', kind: 'lan', recommended: true },
    { address: '10.8.0.4', interface: 'utun3', kind: 'vpn', recommended: false },
  ],
  pairing:
    remoteMode === 'on'
      ? {
          code: 'K7QD-M2PX-9WTR-4HNB',
          url: 'http://192.168.1.20:47831/#K7QD-M2PX-9WTR-4HNB',
          qrSvg: qr,
          expiresAt: now + 4 * MIN,
        }
      : null,
  devices:
    remoteMode === 'on'
      ? [
          {
            id: 'd1',
            name: 'iPhone do Lucas',
            address: '192.168.1.31',
            pairedAt: now - 2 * 86_400_000,
            lastSeen: now - 90_000,
          },
        ]
      : [],
  waiting: remoteMode === 'on' ? 1 : 0,
  serverTime: now,
});
const wfDef = passwordRecovery({
  id: 'wf-1',
  workspaceId: 'w1',
  name: 'Recuperação de senha',
  nodes: [
    agentNode('architect', 'a1', 'Arquiteto'),
    agentNode('developer', 'a3', 'Desenvolvedor', {
      instructions: 'Implemente o endpoint de recuperação de senha.',
    }),
    agentNode('qa', 'a2', 'QA', { loopMax: 3 }),
    agentNode('bug-fixer', 'a3', 'Corretor de bugs'),
    endNode('done', 'Concluído'),
  ],
  edges: [
    edge('architect', 'developer'),
    edge('developer', 'qa'),
    edge('qa', 'done', 'pass'),
    edge('qa', 'bug-fixer', 'fail'),
    edge('bug-fixer', 'qa'),
  ],
});
const wfStates = (mode: string) =>
  mode === 'running'
    ? { architect: nodeState('completed'), developer: nodeState('running') }
    : mode === 'waiting'
      ? { architect: nodeState('completed'), developer: nodeState('waiting_for_input') }
      : mode === 'failed'
        ? {
            architect: nodeState('completed'),
            developer: nodeState('completed'),
            qa: nodeState('failed'),
          }
        : {
            architect: nodeState('completed', { attempts: [wfAttempt('exec-41', 1, 'completed')] }),
            developer: nodeState('completed', { attempts: [wfAttempt('exec-42', 1, 'completed')] }),
            qa: nodeState('completed', { attempts: [wfAttempt('exec-43', 1, 'completed')] }),
            done: nodeState('completed'),
          };
const wfBuild = (
  status: Parameters<typeof wfRun>[1],
  states: ReturnType<typeof wfStates>,
  extra: Parameters<typeof wfRun>[3],
) => {
  const base = wfRun(wfDef, status, states, extra);
  return wfMode === 'done'
    ? wfWithCode(
        {
          ...base,
          handoffs: [
            wfHandoff('architect', 'developer'),
            wfHandoff('developer', 'qa', {
              summary: 'Endpoint e testes escritos.',
              fromExecutionId: 'exec-42',
            }),
          ],
        },
        'changes_available',
      )
    : base;
};
const wfRuns =
  wfMode === 'none' || wfMode === 'idle'
    ? []
    : [
        wfBuild(
          wfMode === 'running'
            ? 'running'
            : wfMode === 'waiting'
              ? 'waiting_for_input'
              : wfMode === 'failed'
                ? 'failed'
                : 'completed',
          wfStates(wfMode),
          {
            task: 'Implementar a recuperação de senha por e-mail',
            startedAt: now - 20 * MIN,
            updatedAt: now - MIN,
            completedAt: wfMode === 'running' || wfMode === 'waiting' ? null : now - 2 * MIN,
            interactions:
              wfMode === 'waiting'
                ? [
                    pendingInteraction({
                      workflowId: 'wf-1',
                      workflowExecutionId: 'wfx-1',
                      workspaceId: 'w1',
                      stepId: 'developer',
                      stepLabel: 'Desenvolvedor',
                      agentId: 'a3',
                      question: 'Qual estratégia de autenticação devo usar?',
                      context: 'Dois módulos expõem usuários.',
                    }),
                  ]
                : [],
          },
        ),
      ];
const handlers: Record<string, (a: any) => unknown> = {
  get_settings: () => ({
    language: 'pt-BR',
    selectedWorkspaceId: workspaces[0]?.id ?? null,
    optimization: {
      metricsEnabled: true,
      contextEnabled: true,
      contextMaxTokens: null,
      skillsEnabled: false,
      guardrailsEnabled: true,
      specsMaxTokens: 200000,
      memory: { user: 'propose', workspace: 'propose', agent: 'auto', maxTokens: 800 },
    },
    sandbox: { mode: 'auto', devCaches: true },
  }),
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
  list_pending_interactions: () => (wfMode === 'waiting' ? (wfRuns[0]?.interactions ?? []) : []),
  list_mcp_connections: () => ({ connections: mcpConnections, grants: mcpGrants }),
  list_mcp_catalog: () => mcpCatalog,
  list_workspace_workflows: () => [],
  list_workflows: () => (wfMode === 'none' ? [] : [wfDef]),
  get_workflow: () => wfDef,
  list_workflow_templates: () => wfTemplates,
  select_workflow_template: () => 'software_feature',
  list_workflow_executions: () => wfRuns,
  get_workflow_execution: () => wfRuns[0] ?? null,
  list_specs: () => specList,
  get_spec: (a) => specViewFor(a.specId),
  list_ides: () => [],
  list_memories: () => memories,
  get_app_info: () => ({ name: 'Project Atlas', version: '0.1.5', platform: 'macos' }),
  check_for_update: () => null,
  get_remote_status: () => remoteStatus(),
  set_remote_enabled: () => remoteStatus(),
  new_remote_pairing_code: () => remoteStatus(),
  get_sandbox_status: () => ({
    available: true,
    platform: 'macos',
    settings: { mode: 'auto', devCaches: true },
  }),
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
