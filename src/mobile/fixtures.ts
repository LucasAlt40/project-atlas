import type { InteractionView, Overview, RunDetail, RunSummary, StepView } from './types';

/** A run as the server lists it, with sensible defaults a test overrides. */
export function summary(overrides: Partial<RunSummary> = {}): RunSummary {
  return {
    id: 'run-1',
    workflowName: 'Password recovery',
    workspaceId: 'ws-1',
    workspaceName: 'Atlas',
    task: 'Implement password recovery',
    status: 'running',
    attention: 'none',
    pending: 0,
    current: [{ nodeId: 'dev', label: 'Developer', status: 'running', agent: 'Dev agent' }],
    stepsTotal: 3,
    stepsDone: 1,
    startedAt: 1_000_000,
    updatedAt: 1_060_000,
    completedAt: null,
    failure: null,
    ...overrides,
  };
}

export function question(overrides: Partial<InteractionView> = {}): InteractionView {
  return {
    id: 'q-1',
    nodeId: 'dev',
    step: 'Developer',
    agent: 'Dev agent',
    kind: 'plan',
    status: 'pending',
    question: 'Do you approve this plan?',
    context: '',
    document: '# The plan\n\n- add the route\n- write the tests',
    options: [
      { id: 'approve', label: 'Approve plan' },
      { id: 'revise', label: 'Request changes' },
      { id: 'redo', label: 'Redo the plan' },
    ],
    createdAt: 1_050_000,
    answeredAt: null,
    choice: null,
    answer: null,
    auto: false,
    remote: { canAnswer: true, choices: null, desktopOnly: null },
    ...overrides,
  };
}

const STEPS: StepView[] = [
  {
    nodeId: 'arch',
    kind: 'agent',
    label: 'Architect',
    status: 'completed',
    agent: 'Arch agent',
    iterations: 1,
    reason: null,
    startedAt: 1_000_000,
    completedAt: 1_030_000,
    attempts: [
      {
        attempt: 1,
        iteration: 1,
        status: 'completed',
        startedAt: 1_000_000,
        completedAt: 1_030_000,
        summary: 'Analysis done',
        outcome: null,
        failure: null,
      },
    ],
  },
  {
    nodeId: 'dev',
    kind: 'agent',
    label: 'Developer',
    status: 'waiting_for_input',
    agent: 'Dev agent',
    iterations: 1,
    reason: null,
    startedAt: 1_030_000,
    completedAt: null,
    attempts: [
      {
        attempt: 1,
        iteration: 1,
        status: 'waiting_for_input',
        startedAt: 1_030_000,
        completedAt: null,
        summary: null,
        outcome: null,
        failure: null,
      },
    ],
  },
  {
    nodeId: 'qa',
    kind: 'agent',
    label: 'QA',
    status: 'pending',
    agent: 'QA agent',
    iterations: 0,
    reason: null,
    startedAt: null,
    completedAt: null,
    attempts: [],
  },
];

/** A run being read in full. By default it waits for a plan to be approved. */
export function detail(overrides: Partial<RunDetail> = {}): RunDetail {
  return {
    ...summary({
      status: 'waiting_for_input',
      attention: 'needs_answer',
      pending: 1,
      current: [
        { nodeId: 'dev', label: 'Developer', status: 'waiting_for_input', agent: 'Dev agent' },
      ],
    }),
    steps: STEPS,
    edges: [
      { from: 'arch', to: 'dev', label: '', conditional: false },
      { from: 'dev', to: 'qa', label: '', conditional: false },
    ],
    interactions: [question()],
    events: [{ kind: 'started', step: null, message: 'Run started', at: 1_000_000 }],
    result: { summary: null, artifacts: [], decisions: [] },
    code: { status: 'in_progress', filesChanged: 0, additions: 0, deletions: 0 },
    autoApprovePlans: false,
    controls: { pause: false, resume: false, cancel: true },
    ...overrides,
  };
}

export function overview(runs: RunSummary[], overrides: Partial<Overview> = {}): Overview {
  return { serverTime: Date.now(), language: 'en-US', runs, ...overrides };
}
