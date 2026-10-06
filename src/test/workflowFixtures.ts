import type { WorkflowEvent } from '@/features/workflow/types';
import type {
  AgentNodeFieldsDto,
  AttemptStatusDto,
  FileChangeDto,
  LiveWorkspaceStateDto,
  LiveWorkspaceUpdateDto,
  NodeStateDto,
  NodeStatusDto,
  PendingInteractionDto,
  WorkflowDto,
  WorkflowEdgeDto,
  WorkflowExecutionDto,
  WorkflowExecutionStatusDto,
  WorkflowNodeDto,
  WorkflowTemplateDto,
} from '@/lib/tauri/commands';

/** A workflow node of an agent, with defaults a test rarely cares about. */
export function agentNode(
  id: string,
  agentId: string,
  label: string,
  extra: Partial<AgentNodeFieldsDto> & { loopMax?: number } = {},
): WorkflowNodeDto {
  const { loopMax, ...rest } = extra;
  return {
    id,
    type: 'agent',
    agentId,
    label,
    instructions: '',
    retryPolicy: { maxRetries: 0 },
    failurePolicy: { type: 'stop_workflow' },
    executionPolicy: { isolation: 'from_agent' },
    priority: 0,
    loopPolicy: loopMax ? { loopId: `loop-${id}`, maxIterations: loopMax } : null,
    position: null,
    ...rest,
  };
}

export function endNode(id: string, label = 'Done'): WorkflowNodeDto {
  return { id, type: 'end', label, outcome: 'done', priority: 0, loopPolicy: null, position: null };
}

export function edge(source: string, target: string, status?: 'pass' | 'fail'): WorkflowEdgeDto {
  return {
    id: status ? `${source}->${target}:${status}` : `${source}->${target}`,
    sourceNodeId: source,
    targetNodeId: target,
    condition: status ? { field: 'result.status', operator: 'equals', value: status } : null,
    label: status ?? '',
  };
}

/** Architect -> Developer -> QA (pass: done, fail: Bug Fixer -> QA again). */
export function passwordRecovery(overrides: Partial<WorkflowDto> = {}): WorkflowDto {
  return {
    id: 'wf-1',
    workspaceId: 'w1',
    name: 'Password recovery',
    description: '',
    mode: 'automatic',
    version: 1,
    status: 'ready',
    templateId: 'feature_with_bug_fix_loop',
    nodes: [
      agentNode('architect', 'a-architect', 'Architect'),
      agentNode('developer', 'a-developer', 'Developer', {
        instructions: 'Implement the endpoint.',
      }),
      agentNode('qa', 'a-qa', 'QA', { loopMax: 3 }),
      agentNode('bug-fixer', 'a-fixer', 'Bug Fixer'),
      endNode('done'),
    ],
    edges: [
      edge('architect', 'developer'),
      edge('developer', 'qa'),
      edge('qa', 'done', 'pass'),
      edge('qa', 'bug-fixer', 'fail'),
      edge('bug-fixer', 'qa'),
    ],
    viewport: null,
    routeRepairs: [],
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

/** A question the Developer step asked and nobody answered yet. */
export function pendingInteraction(
  overrides: Partial<PendingInteractionDto> = {},
): PendingInteractionDto {
  return {
    id: 'q1',
    executionId: 'exec-2',
    workflowId: 'wf-1',
    workflowExecutionId: 'wfx-1',
    workspaceId: 'w1',
    stepId: 'developer',
    stepLabel: 'Developer',
    agentId: 'a-developer',
    iteration: 1,
    kind: 'clarification',
    question: 'Which authentication strategy should I use?',
    context: 'Two modules expose users.',
    document: '## Plan\n\n1. Add the column\n2. Fix the webhook',
    options: [],
    source: 'structured',
    confidence: 100,
    status: 'pending',
    createdAt: 1_500,
    answeredAt: null,
    choice: null,
    answer: null,
    ...overrides,
  };
}

export function noIntegration(): WorkflowExecutionDto['integration'] {
  return {
    status: 'not_applicable',
    worktreeExecutionId: null,
    branch: null,
    baseBranch: null,
    baseRevision: null,
    currentRevision: null,
    blockReason: null,
    canApply: false,
    canUndo: false,
    conflicts: [],
    message: null,
    review: null,
    reviewPending: null,
    updatedAt: 0,
  };
}

export function nodeState(status: NodeStatusDto, extra: Partial<NodeStateDto> = {}): NodeStateDto {
  return {
    status,
    iterations: status === 'pending' ? 0 : 1,
    failedAttempts: 0,
    attempts: [],
    facts: {},
    reason: null,
    routed: false,
    unrouted: false,
    ...extra,
  };
}

export function attempt(executionId: string, n = 1, status: AttemptStatusDto = 'completed') {
  return {
    attempt: n,
    iteration: 1,
    executionId,
    status,
    startedAt: 1,
    completedAt: status === 'running' ? null : 2,
    summary: null,
    outcome: null,
    failure: null,
  };
}

/** A run of `workflow` with the given state per node (the rest are pending). */
export function run(
  workflow: WorkflowDto,
  status: WorkflowExecutionStatusDto,
  states: Record<string, NodeStateDto>,
  extra: Partial<WorkflowExecutionDto> = {},
): WorkflowExecutionDto {
  return {
    id: 'wfx-1',
    workflowId: workflow.id,
    workspaceId: workflow.workspaceId,
    workflowVersion: workflow.version,
    workflow,
    task: 'Implement password recovery',
    status,
    failure: null,
    cancelRequested: false,
    maxParallelSteps: 4,
    nodes: Object.fromEntries(
      workflow.nodes.map((n) => [n.id, states[n.id] ?? nodeState('pending')]),
    ),
    state: {
      task: 'Implement password recovery',
      workflow: workflow.name,
      currentNodes: Object.entries(states)
        .filter(
          ([, s]) =>
            s.status === 'running' ||
            s.status === 'waiting_approval' ||
            s.status === 'waiting_for_input',
        )
        .map(([id]) => id),
      completedNodes: Object.entries(states)
        .filter(([, s]) => s.status === 'completed')
        .map(([id]) => id),
      failedNodes: Object.entries(states)
        .filter(([, s]) => s.status === 'failed')
        .map(([id]) => id),
      artifacts: [],
      decisions: [],
      activeAgents: [],
      completedAgents: [],
      touchedAreas: {},
      touchedFiles: {},
      validationResults: [],
      warnings: [],
      iterationCount: {},
    },
    handoffs: [],
    changes: null,
    integration: noIntegration(),
    interactions: [],
    recoveries: [],
    events: [],
    startedAt: 1_000,
    updatedAt: 2_000,
    completedAt:
      status === 'running' || status === 'paused' || status === 'waiting_for_input' ? null : 3_000,
    ...extra,
  };
}

export const templates: WorkflowTemplateDto[] = [
  {
    id: 'software_feature',
    category: 'software_feature',
    recommended: true,
    name: 'Software Feature Development',
    description: '',
    roles: ['architect', 'developer', 'validator', 'bug_fixer', 'qa'],
  },
  {
    id: 'feature_basic',
    category: 'software_feature',
    recommended: false,
    name: 'Software Feature',
    description: '',
    roles: ['architect', 'developer', 'qa'],
  },
  {
    id: 'bug_fix',
    category: 'bug_fix',
    recommended: true,
    name: 'Bug Fix',
    description: '',
    roles: ['bug_fixer', 'qa'],
  },
];

export function workflowEvent(
  kind: WorkflowEvent['kind'],
  executionId = 'wfx-1',
  nodeId: string | null = null,
): WorkflowEvent {
  return {
    kind,
    workflowId: 'wf-1',
    executionId,
    workspaceId: 'w1',
    nodeId,
    message: '',
    timestamp: Date.now(),
    metadata: {},
  };
}

/** What one step handed to the next. */
export function handoff(
  from: string,
  to: string,
  overrides: Partial<WorkflowExecutionDto['handoffs'][number]> = {},
): WorkflowExecutionDto['handoffs'][number] {
  return {
    id: `handoff-${from}-${to}`,
    workflowExecutionId: 'wfx-1',
    linkId: `${from}->${to}`,
    fromNodeId: from,
    toNodeId: to,
    fromExecutionId: 'exec-42',
    iteration: 1,
    kind: 'result',
    createdAt: 1,
    status: 'success',
    outcome: null,
    summary: 'Implemented the endpoint.',
    instructions: null,
    decisions: [],
    artifacts: [],
    changedFiles: [],
    uncommittedFiles: [],
    reportedFiles: [],
    validation: null,
    failure: null,
    ...overrides,
  };
}

export function changeSet(
  overrides: Partial<NonNullable<WorkflowExecutionDto['changes']>> = {},
): NonNullable<WorkflowExecutionDto['changes']> {
  return {
    baseRevision: 'a'.repeat(40),
    currentRevision: 'b'.repeat(40),
    files: [
      {
        path: 'src/auth/password-reset.ts',
        oldPath: null,
        status: 'added',
        additions: 82,
        deletions: 0,
        binary: false,
      },
      {
        path: 'src/auth/auth.controller.ts',
        oldPath: null,
        status: 'modified',
        additions: 31,
        deletions: 4,
        binary: false,
      },
    ],
    filesChanged: 2,
    additions: 113,
    deletions: 4,
    uncommitted: [],
    capturedAt: 1,
    ...overrides,
  };
}

/** A run whose code is in this state. */
export function withCode(
  base: WorkflowExecutionDto,
  status: WorkflowExecutionDto['integration']['status'],
  integration: Partial<WorkflowExecutionDto['integration']> = {},
  changes: WorkflowExecutionDto['changes'] = changeSet(),
): WorkflowExecutionDto {
  return {
    ...base,
    changes,
    integration: {
      ...noIntegration(),
      status,
      worktreeExecutionId: 'exec-40',
      branch: 'atlas/exec-000040',
      baseBranch: 'main',
      canApply: status === 'changes_available',
      ...integration,
    },
  };
}

// ---- the live workspace -------------------------------------------------------------------------

type LiveStateDto = LiveWorkspaceStateDto;
type LiveUpdateDto = LiveWorkspaceUpdateDto;
type FileChangeFixture = FileChangeDto;

export function fileChange(
  path: string,
  status: FileChangeFixture['status'] = 'modified',
  extra: Partial<FileChangeFixture> = {},
): FileChangeFixture {
  return {
    path,
    oldPath: null,
    status,
    additions: status === 'deleted' ? 0 : 3,
    deletions: status === 'added' ? 0 : 1,
    binary: false,
    ...extra,
  };
}

/** The backend's snapshot of a run's worktree. */
export function liveState(extra: Partial<LiveStateDto> = {}): LiveStateDto {
  const files = extra.files ?? [];
  return {
    runId: 'wfx-1',
    worktreeExecutionId: 'exec-40',
    branch: 'atlas/exec-000040',
    baselineRevision: 'abc1234def5678',
    currentRevision: 'abc1234def5678',
    availability: 'available',
    phase: 'running',
    observation: 'events',
    files,
    filesChanged: files.length,
    additions: files.reduce((n, f) => n + (f.additions ?? 0), 0),
    deletions: files.reduce((n, f) => n + (f.deletions ?? 0), 0),
    revision: 1,
    updatedAt: Date.UTC(2026, 9, 5, 10, 32, 14),
    lastReconciledAt: null,
    ...extra,
  };
}

/** What the backend announces when the worktree changes. */
export function liveUpdate(
  revision: number,
  extra: Partial<LiveUpdateDto> = {},
  state: Partial<LiveStateDto> = {},
): LiveUpdateDto {
  const base = liveState(state);
  return {
    runId: base.runId,
    worktreeExecutionId: base.worktreeExecutionId,
    revision,
    full: false,
    changed: [],
    removed: [],
    currentRevision: base.currentRevision,
    availability: base.availability,
    phase: base.phase,
    observation: base.observation,
    filesChanged: base.filesChanged,
    additions: base.additions,
    deletions: base.deletions,
    updatedAt: Date.UTC(2026, 9, 5, 10, 32, 15),
    ...extra,
  };
}
