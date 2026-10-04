import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  agent,
  mockBackend,
  renderWithProviders,
  storedExecution,
  taskContext,
  workspace,
} from '@/test/fixtures';
import { reductionPercent } from '@/features/harness/model/taskContext';
import { previewTaskContext, sendMessage } from '../services/workspaceService';
import { factsFromStored } from '../model/inspection';
import { ExecutionDetails } from './ExecutionDetails';
import { WorkspacePage } from '../pages/WorkspacePage';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('../services/workspaceService');

function show(taskContextAnswer = taskContext()) {
  mockBackend({
    agents: [agent('a1', 'Architect')],
    workspaces: [workspace('w1', 'ERP', '/dev/erp', ['a1'])],
    selectedWorkspaceId: 'w1',
    taskContext: { status: 'ready', context: taskContextAnswer },
  });
  return renderWithProviders(<WorkspacePage />);
}

async function type(task: string) {
  const user = userEvent.setup();
  await user.type(await screen.findByLabelText('Message Architect'), task);
  return user;
}

describe('Task context preview', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows, while the task is typed, how big the context is, what is in it and what is not', async () => {
    show();
    await type('Add password recovery endpoint');

    const preview = await screen.findByRole('region', { name: 'Task context' });
    expect(within(preview).getByText('1,984 / 6,000 characters')).toBeVisible();
    expect(within(preview).getByText(/Full Harness ~5,800 characters/)).toBeVisible();
    expect(within(preview).getByText(/Reduced by 66%/)).toBeVisible();
    const included = within(preview).getByRole('list', { name: 'Included' });
    expect(included).toHaveTextContent('Authentication module');
    expect(included).toHaveTextContent('Clean Architecture');
    expect(included).toHaveTextContent('Constraints and decisions');
    const excluded = within(preview).getByRole('list', { name: 'Not included' });
    expect(excluded).toHaveTextContent('Billing module');
    expect(excluded).toHaveTextContent('Maps module');
    expect(within(preview).getByText(/Areas included: 3/)).toBeVisible();
  });

  it('asks for the context of the task being typed, once the typing pauses', async () => {
    show();
    await type('Fix the build');

    await waitFor(() => {
      expect(previewTaskContext).toHaveBeenLastCalledWith('w1', 'a1', 'Fix the build');
    });
  });

  it('says why each item was chosen and shows the exact text the agent receives', async () => {
    show();
    const user = await type('Add password recovery endpoint');
    await user.click(await screen.findByRole('button', { name: 'View context' }));

    const dialog = await screen.findByRole('dialog', { name: 'Context for this task' });
    expect(within(dialog).getByText('Add')).toBeVisible();
    expect(within(dialog).getByText('authentication, password, api, backend')).toBeVisible();
    expect(within(dialog).getByText('✓ task tag: authentication')).toBeVisible();
    expect(within(dialog).getByText('✓ path relevance: src/auth/password-reset.ts')).toBeVisible();
    expect(within(dialog).getByText('✓ constraints are never left out')).toBeVisible();
    expect(
      within(dialog)
        .getByText(/Billing module/)
        .closest('li'),
    ).toHaveTextContent('not related to the task');
    expect(
      within(dialog)
        .getByText(/Maps module/)
        .closest('li'),
    ).toHaveTextContent('belongs to another layer than the task');
    expect(within(dialog).getByText(/^This context was selected from the project/)).toBeVisible();
  });

  it('says so when the whole Harness context is used instead, and why', async () => {
    show(
      taskContext({
        mode: 'fallback',
        fallbackReason: 'task_without_signals',
        signals: null,
        entries: [],
      }),
    );
    await type('???');

    const preview = await screen.findByRole('region', { name: 'Task context' });
    expect(preview).toHaveTextContent('the whole Harness context will be used');
    expect(preview).toHaveTextContent('the task says nothing the analysis can read');
  });

  it('shows nothing for a project without a Harness', async () => {
    mockBackend({
      agents: [agent('a1', 'Architect')],
      workspaces: [workspace('w1', 'ERP', '/dev/erp', ['a1'])],
      selectedWorkspaceId: 'w1',
    });
    renderWithProviders(<WorkspacePage />);
    await type('Add password recovery endpoint');

    await waitFor(() => {
      expect(previewTaskContext).toHaveBeenCalled();
    });
    expect(screen.queryByRole('region', { name: 'Task context' })).not.toBeInTheDocument();
  });

  it('is optional: the task can be sent without ever opening the context', async () => {
    show();
    vi.mocked(sendMessage).mockResolvedValue({
      userMessage: {
        id: 'm1',
        workspaceId: 'w1',
        agentId: 'a1',
        executionId: 'exec-1',
        role: 'user',
        content: 'Go',
        timestamp: 1,
        failed: false,
        failureKind: null,
      },
      executionId: 'exec-1',
    });
    const user = await type('Go');
    await screen.findByRole('region', { name: 'Task context' });

    await user.click(
      within(screen.getByRole('article', { name: 'Architect' })).getByRole('button', {
        name: 'Send',
      }),
    );

    expect(sendMessage).toHaveBeenCalledWith({ workspaceId: 'w1', agentId: 'a1', content: 'Go' });
  });

  it('does not break typing when the preview fails', async () => {
    show();
    vi.mocked(previewTaskContext).mockRejectedValue(new Error('boom'));
    await type('Go');

    await waitFor(() => {
      expect(previewTaskContext).toHaveBeenCalled();
    });
    expect(screen.queryByRole('region', { name: 'Task context' })).not.toBeInTheDocument();
    expect(screen.getByLabelText('Message Architect')).toHaveValue('Go');
  });
});

describe('Context of an execution', () => {
  it('computes how much smaller the task context is than the whole Harness', () => {
    expect(reductionPercent(taskContext())).toBe(66);
    expect(reductionPercent(taskContext({ selectedChars: 6000, fullHarnessChars: 5800 }))).toBe(0);
    expect(reductionPercent(taskContext({ fullHarnessChars: 0 }))).toBe(0);
  });

  it('is recorded in the details of the execution', async () => {
    const facts = factsFromStored(
      storedExecution('w1', 'a1', 'exec-1', {
        context: {
          mode: 'task_aware',
          totalHarnessCharacters: 5800,
          selectedContextCharacters: 1984,
          selectedItems: 5,
          omittedItems: 7,
        },
      }),
    );

    mockBackend();
    renderWithProviders(
      <ExecutionDetails
        facts={facts}
        context={{
          workspaceName: 'ERP',
          agentName: 'Architect',
          personalityName: 'Architect',
          runtimeName: 'Claude CLI',
          capabilities: undefined,
          worktreeIsolation: false,
        }}
      />,
    );

    expect(await screen.findByText('Selected for the task')).toBeVisible();
    expect(
      screen.getByText('1,984 of 5,800 characters · 5 items included · 7 left out'),
    ).toBeVisible();
  });

  it('says when the whole Harness was used as a fallback', async () => {
    const facts = factsFromStored(
      storedExecution('w1', 'a1', 'exec-1', {
        context: {
          mode: 'fallback',
          totalHarnessCharacters: 5800,
          selectedContextCharacters: 5800,
          selectedItems: 0,
          omittedItems: 0,
          fallbackReason: 'task_without_signals',
        },
      }),
    );

    mockBackend();
    renderWithProviders(
      <ExecutionDetails
        facts={facts}
        context={{
          workspaceName: 'ERP',
          agentName: 'Architect',
          personalityName: 'Architect',
          runtimeName: 'Claude CLI',
          capabilities: undefined,
          worktreeIsolation: false,
        }}
      />,
    );

    expect(await screen.findByText('Whole Harness (fallback: task_without_signals)')).toBeVisible();
  });
});
