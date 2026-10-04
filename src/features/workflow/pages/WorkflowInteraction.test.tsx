import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { agent, mockBackend, navigate, renderWithProviders, workspace } from '@/test/fixtures';
import {
  attempt,
  nodeState,
  passwordRecovery,
  pendingInteraction,
  run,
  templates,
} from '@/test/workflowFixtures';
import type { PendingInteractionDto, WorkflowExecutionDto } from '@/lib/tauri/commands';
import { ActionRequired } from '../components/ActionRequired';
import {
  answerInteraction,
  cancelWorkflow,
  getRun,
  listIdes,
  listPendingInteractions,
  listRuns,
  listTemplates,
  listWorkflows,
  selectTemplate,
  subscribeToWorkflowEvents,
  validateWorkflow,
} from '../services/workflowService';
import type { WorkflowEvent } from '../types';
import { WorkflowPage } from './WorkflowPage';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');
vi.mock('../services/workflowService');

const AGENTS = [
  agent('a-architect', 'Architect agent'),
  agent('a-developer', 'Developer agent'),
  agent('a-qa', 'QA agent'),
  agent('a-fixer', 'Fixer agent'),
];

let emitWorkflow: (event: WorkflowEvent) => void = () => undefined;

/** A run whose Developer asked something: the run stands still, later steps are pending. */
function waitingRun(question: Partial<PendingInteractionDto> = {}): WorkflowExecutionDto {
  const workflow = passwordRecovery();
  return run(
    workflow,
    'waiting_for_input',
    {
      architect: nodeState('completed', { attempts: [attempt('exec-1')] }),
      developer: nodeState('waiting_for_input', {
        attempts: [attempt('exec-2', 1, 'waiting_for_input')],
      }),
    },
    { interactions: [pendingInteraction(question)] },
  );
}

function show(runs: WorkflowExecutionDto[]) {
  mockBackend({
    language: 'en-US',
    agents: AGENTS,
    workspaces: [workspace('w1', 'ERP', '/dev/erp', ['a-architect'])],
    selectedWorkspaceId: 'w1',
    executions: [],
  });
  vi.mocked(listWorkflows).mockResolvedValue([passwordRecovery()]);
  vi.mocked(listRuns).mockResolvedValue(runs);
  vi.mocked(getRun).mockImplementation((id) =>
    Promise.resolve(runs.find((r) => r.id === id) ?? null),
  );
  vi.mocked(listTemplates).mockResolvedValue(templates);
  vi.mocked(listIdes).mockResolvedValue([]);
  vi.mocked(selectTemplate).mockResolvedValue('software_feature');
  vi.mocked(validateWorkflow).mockResolvedValue({ valid: true, issues: [] });
  vi.mocked(listPendingInteractions).mockResolvedValue([]);
  vi.mocked(answerInteraction).mockResolvedValue(undefined);
  vi.mocked(cancelWorkflow).mockResolvedValue(undefined);
  vi.mocked(subscribeToWorkflowEvents).mockImplementation((handler) => {
    emitWorkflow = handler;
    return Promise.resolve(() => undefined);
  });
  return renderWithProviders(<WorkflowPage />);
}

const panel = () => screen.findByRole('alert', { name: /action required/i });

describe('An agent waiting for the person', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    navigate.mockReset();
  });

  it('is impossible to miss: the question, who asks, why, and the run says it is waiting', async () => {
    show([waitingRun()]);

    const ask = await panel();
    expect(within(ask).getByText('Developer is waiting for your response')).toBeInTheDocument();
    expect(
      within(ask).getByText('Which authentication strategy should I use?'),
    ).toBeInTheDocument();
    expect(within(ask).getByText('Waiting reason: Clarification')).toBeInTheDocument();
    expect(await screen.findByRole('status', { name: /waiting for you/i })).toBeInTheDocument();
    // The graph shows the node as waiting, and what comes after it as still pending.
    const developer = await screen.findByLabelText(/^Developer \(/);
    expect(within(developer).getByText('Waiting for input')).toBeInTheDocument();
    expect(within(await screen.findByLabelText(/^QA \(/)).getByText('Pending')).toBeInTheDocument();
    // A waiting run is neither pausable nor resumable: it waits for the answer.
    expect(screen.queryByRole('button', { name: 'Pause' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Resume' })).not.toBeInTheDocument();
  });

  it('shows the plan the agent wrote, in full and as Markdown, next to the question', async () => {
    show([waitingRun({ question: 'Would you like me to proceed?' })]);

    const ask = await panel();
    expect(within(ask).getByRole('heading', { name: 'Plan' })).toBeInTheDocument();
    expect(within(ask).getByText('Fix the webhook')).toBeInTheDocument();
  });

  it('takes a written answer for a clarification and sends exactly that', async () => {
    const user = userEvent.setup();
    show([waitingRun()]);

    const ask = await panel();
    const send = within(ask).getByRole('button', { name: 'Send answer' });
    expect(send).toBeDisabled();
    await user.type(within(ask).getByRole('textbox', { name: 'Your answer' }), '/api/users');
    await user.click(send);

    await waitFor(() => {
      expect(answerInteraction).toHaveBeenCalledWith('wfx-1', 'q1', { text: '/api/users' });
    });
  });

  it('offers the agent’s suggestions as quick picks that fill the answer', async () => {
    const user = userEvent.setup();
    show([
      waitingRun({
        options: [
          { id: '/api/users', label: '/api/users' },
          { id: '/api/customers', label: '/api/customers' },
        ],
      }),
    ]);

    const ask = await panel();
    await user.click(within(ask).getByRole('button', { name: '/api/customers' }));
    expect(within(ask).getByRole('textbox', { name: 'Your answer' })).toHaveValue('/api/customers');
  });

  it('asks an approval with two buttons and says an answer never applies anything', async () => {
    const user = userEvent.setup();
    show([
      waitingRun({
        kind: 'approval',
        question: 'May I modify UserService?',
        options: [
          { id: 'approve', label: 'Approve' },
          { id: 'reject', label: 'Reject' },
        ],
      }),
    ]);

    const ask = await panel();
    expect(within(ask).getByText('Waiting reason: Approval')).toBeInTheDocument();
    expect(within(ask).queryByRole('textbox')).not.toBeInTheDocument();
    expect(
      within(ask).getByText(/isolated worktree\. Your project checkout is not changed/),
    ).toBeInTheDocument();
    expect(within(ask).getByText(/never applies changes to the project/)).toBeInTheDocument();
    await user.click(within(ask).getByRole('button', { name: 'Approve' }));

    await waitFor(() => {
      expect(answerInteraction).toHaveBeenCalledWith('wfx-1', 'q1', { choice: 'approve' });
    });
  });

  it('a permission is answered with Allow or Deny', async () => {
    const user = userEvent.setup();
    show([
      waitingRun({
        kind: 'permission',
        options: [
          { id: 'allow', label: 'Allow' },
          { id: 'deny', label: 'Deny' },
        ],
      }),
    ]);

    await user.click(within(await panel()).getByRole('button', { name: 'Deny' }));

    await waitFor(() => {
      expect(answerInteraction).toHaveBeenCalledWith('wfx-1', 'q1', { choice: 'deny' });
    });
  });

  it('shows why the core refused an answer, next to the question', async () => {
    const user = userEvent.setup();
    show([waitingRun()]);
    vi.mocked(answerInteraction).mockRejectedValue({
      code: 'interaction_not_pending',
      params: {},
      detail: null,
    });

    const ask = await panel();
    await user.type(within(ask).getByRole('textbox', { name: 'Your answer' }), 'x');
    await user.click(within(ask).getByRole('button', { name: 'Send answer' }));

    expect(
      await within(ask).findByText('That question was already answered or cancelled.'),
    ).toBeInTheDocument();
  });

  it('lets the person cancel the whole execution instead of answering', async () => {
    const user = userEvent.setup();
    show([waitingRun()]);

    await user.click(within(await panel()).getByRole('button', { name: 'Cancel execution' }));

    await waitFor(() => {
      expect(cancelWorkflow).toHaveBeenCalledWith('wfx-1');
    });
  });

  it('goes away once the question is answered and the step runs again', async () => {
    const workflow = passwordRecovery();
    const answered = run(
      workflow,
      'running',
      {
        architect: nodeState('completed', { attempts: [attempt('exec-1')] }),
        developer: nodeState('running', {
          attempts: [attempt('exec-2', 1, 'waiting_for_input'), attempt('exec-3', 2, 'running')],
        }),
      },
      { interactions: [pendingInteraction({ status: 'answered', answer: '/api/users' })] },
    );
    show([answered]);

    await screen.findByLabelText(/^Developer \(/);
    expect(screen.queryByRole('alert', { name: /action required/i })).not.toBeInTheDocument();
  });
});

describe('The global "Action required" indicator', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    navigate.mockReset();
  });

  function showIndicator(pending: PendingInteractionDto[]) {
    mockBackend({
      language: 'en-US',
      agents: AGENTS,
      workspaces: [workspace('w1', 'ERP', '/dev/erp', ['a-architect'])],
      selectedWorkspaceId: 'w1',
      executions: [],
    });
    vi.mocked(listPendingInteractions).mockResolvedValue(pending);
    vi.mocked(subscribeToWorkflowEvents).mockImplementation((handler) => {
      emitWorkflow = handler;
      return Promise.resolve(() => undefined);
    });
    return renderWithProviders(<ActionRequired />);
  }

  it('shows nothing while no agent is waiting', async () => {
    showIndicator([]);
    await waitFor(() => {
      expect(listPendingInteractions).toHaveBeenCalledWith('w1');
    });
    expect(screen.queryByRole('button', { name: /waiting for you/ })).not.toBeInTheDocument();
  });

  it('counts the agents waiting and opens that run when it is the only one', async () => {
    const user = userEvent.setup();
    showIndicator([pendingInteraction()]);

    await user.click(await screen.findByRole('button', { name: /1 agent waiting for you/ }));

    expect(navigate).toHaveBeenCalledWith('workflow', {
      type: 'open-workflow',
      workflowId: 'wf-1',
      executionId: 'wfx-1',
    });
  });

  it('lists several and lets the person pick which to open', async () => {
    const user = userEvent.setup();
    showIndicator([
      pendingInteraction(),
      pendingInteraction({
        id: 'q2',
        workflowExecutionId: 'wfx-2',
        stepLabel: 'Bug Fixer',
        question: 'Which test should I fix first?',
      }),
    ]);

    await user.click(await screen.findByRole('button', { name: /2 agents waiting for you/ }));
    await user.click(await screen.findByRole('button', { name: /Bug Fixer is waiting/ }));

    expect(navigate).toHaveBeenCalledWith('workflow', {
      type: 'open-workflow',
      workflowId: 'wf-1',
      executionId: 'wfx-2',
    });
  });

  it('is read again when a workflow event says something changed', async () => {
    const user = userEvent.setup();
    const pending = pendingInteraction();
    showIndicator([]);
    await waitFor(() => {
      expect(listPendingInteractions).toHaveBeenCalledTimes(1);
    });

    vi.mocked(listPendingInteractions).mockResolvedValue([pending]);
    act(() => {
      emitWorkflow({
        kind: 'node_waiting_for_input',
        workflowId: 'wf-1',
        executionId: 'wfx-1',
        workspaceId: 'w1',
        nodeId: 'developer',
        message: 'Developer is waiting for your input',
        timestamp: 1,
        metadata: {},
      });
    });

    expect(await screen.findByRole('button', { name: /1 agent waiting for you/ })).toBeVisible();
    await user.click(screen.getByRole('button', { name: /1 agent waiting for you/ }));
    expect(navigate).toHaveBeenCalled();
  });
});
