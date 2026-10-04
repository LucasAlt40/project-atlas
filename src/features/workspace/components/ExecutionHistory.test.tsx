import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {
  agent,
  chatMessage,
  emit,
  emitConversationMessage,
  emitSessionStatus,
  executionEvent,
  mockBackend,
  renderWithProviders,
  sessionStatus,
  storedExecution,
  workspace,
  worktreeOf,
} from '@/test/fixtures';
import { WorkspacePage } from '../pages/WorkspacePage';
import { listExecutions, mergeExecution } from '../services/workspaceService';
import { ActiveExecutions } from './ActiveExecutions';

vi.mock('@xterm/xterm', async () => (await import('@/test/xtermMock')).xtermModule);
vi.mock('@xterm/addon-fit', async () => (await import('@/test/xtermMock')).fitModule);
vi.mock('@xterm/addon-search', async () => (await import('@/test/xtermMock')).searchModule);
vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('../services/workspaceService');

const architect = agent('a1', 'Architect');
const developer = agent('a2', 'Developer');
const card = (name: string) => screen.getByRole('article', { name });

function show(options: Parameters<typeof mockBackend>[0] = {}) {
  mockBackend({
    agents: [architect, developer],
    workspaces: [
      workspace('w1', 'Atlas', '/dev/atlas', ['a1']),
      workspace('w2', 'Acme', '/dev/acme', ['a2']),
    ],
    selectedWorkspaceId: 'w1',
    ...options,
  });
  return renderWithProviders(
    <>
      <ActiveExecutions />
      <WorkspacePage />
    </>,
  );
}

describe('the execution history of an agent', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('brings back the conversation and the ended executions of this workspace only', async () => {
    const user = userEvent.setup();
    show({
      messages: [
        chatMessage('w1', 'a1', 'exec-1', 'user', 'Review the login flow'),
        chatMessage('w1', 'a1', 'exec-1', 'assistant', 'The login flow is fine.'),
        chatMessage('w2', 'a1', 'exec-9', 'user', 'Other workspace question'),
      ],
      executions: [
        storedExecution('w1', 'a1', 'exec-2', { task: 'Run the tests', status: 'failed' }),
        storedExecution('w1', 'a1', 'exec-1', { task: 'Review the login flow' }),
        storedExecution('w2', 'a1', 'exec-9', { task: 'Other workspace question' }),
      ],
    });

    // The conversation is there as soon as the app opens.
    expect(await screen.findByText('The login flow is fine.')).toBeInTheDocument();
    expect(screen.queryByText('Other workspace question')).not.toBeInTheDocument();

    await user.click(within(card('Architect')).getByRole('tab', { name: 'Executions' }));
    const list = within(card('Architect')).getByRole('tabpanel');
    expect(within(list).getAllByRole('button')).toHaveLength(2);
    expect(within(list).getByText('Run the tests')).toBeInTheDocument();
    expect(within(list).getByText('Review the login flow')).toBeInTheDocument();
    expect(within(list).queryByText('Other workspace question')).not.toBeInTheDocument();
  });

  it('says so when nothing has finished yet', async () => {
    const user = userEvent.setup();
    show();
    await user.click(await screen.findByRole('tab', { name: 'Executions' }));
    expect(screen.getByText('No execution has finished yet.')).toBeInTheDocument();
  });

  it('opens an execution in the inspector: details, activity, conversation and a terminal note', async () => {
    const user = userEvent.setup();
    show({
      messages: [
        chatMessage('w1', 'a1', 'exec-1', 'user', 'Review the login flow'),
        chatMessage('w1', 'a1', 'exec-1', 'assistant', 'The login flow is fine.'),
      ],
      executions: [
        storedExecution('w1', 'a1', 'exec-1', {
          task: 'Review the login flow',
          usage: {
            inputTokens: 120,
            outputTokens: 30,
            totalTokens: 150,
            cost: null,
            currency: null,
            source: 'runtime_reported',
          },
          events: [
            executionEvent('w1', 'a1', 'exec-1', 'started'),
            executionEvent('w1', 'a1', 'exec-1', 'user_interrupted'),
            executionEvent('w1', 'a1', 'exec-1', 'process_exited', { exitCode: '130' }),
            executionEvent('w1', 'a1', 'exec-1', 'cancelled'),
          ],
          status: 'cancelled',
        }),
      ],
    });
    await user.click(await screen.findByRole('tab', { name: 'Executions' }));
    await user.click(screen.getByRole('button', { name: 'Inspect execution #1' }));

    const inspector = screen.getByRole('dialog', { name: 'Execution #1' });
    expect(within(inspector).getByText('Review the login flow')).toBeInTheDocument();
    expect(within(inspector).getByText('Atlas')).toBeInTheDocument();
    expect(within(inspector).getByText('Claude CLI')).toBeInTheDocument();
    expect(within(inspector).getByText('sonnet')).toBeInTheDocument();
    expect(within(inspector).getByText('Interrupted by you (Ctrl+C)')).toBeInTheDocument();
    expect(within(inspector).getByText('Exited (code 130)')).toBeInTheDocument();
    // Tokens were reported; cost was not, and is not shown as zero.
    expect(within(inspector).getByText('150')).toBeInTheDocument();
    expect(within(inspector).getByText('Not reported')).toBeInTheDocument();

    await user.click(within(inspector).getByRole('tab', { name: 'Activity' }));
    expect(within(inspector).getByText(/Interrupted the execution/)).toBeInTheDocument();
    await user.click(within(inspector).getByRole('tab', { name: 'Chat' }));
    expect(within(inspector).getByText('The login flow is fine.')).toBeInTheDocument();
    await user.click(within(inspector).getByRole('tab', { name: 'Terminal' }));
    expect(within(inspector).getByText(/does not keep it/)).toBeInTheDocument();
  });

  it('offers the finished run for inspection as soon as its answer arrives', async () => {
    const user = userEvent.setup();
    show();
    await screen.findByRole('article', { name: 'Architect' });
    expect(screen.queryByRole('button', { name: 'View execution' })).not.toBeInTheDocument();

    const ended = storedExecution('w1', 'a1', 'exec-5', { task: 'Summarize' });
    vi.mocked(listExecutions).mockResolvedValue([ended]);
    act(() => {
      emit(executionEvent('w1', 'a1', 'exec-5', 'started'));
      emit(executionEvent('w1', 'a1', 'exec-5', 'completed'));
      emitConversationMessage(chatMessage('w1', 'a1', 'exec-5', 'assistant', 'Done.'));
    });
    await user.click(await screen.findByRole('button', { name: 'View execution' }));
    expect(screen.getByRole('dialog', { name: 'Execution #5' })).toBeInTheDocument();
  });
});

describe('the Git section of the inspector', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  async function inspect(options: Parameters<typeof mockBackend>[0]) {
    const user = userEvent.setup();
    show({
      executions: [storedExecution('w1', 'a1', 'exec-42', { task: 'Add login' })],
      ...options,
    });
    await user.click(await screen.findByRole('tab', { name: 'Executions' }));
    await user.click(screen.getByRole('button', { name: 'Inspect execution #42' }));
    return { user, inspector: screen.getByRole('dialog', { name: 'Execution #42' }) };
  }

  it('shows base, branch, worktree, changes, tests and merge state of an isolated execution', async () => {
    const { inspector } = await inspect({
      worktrees: [worktreeOf('w1', 'a1', 'exec-42', { branchName: 'atlas/exec-000042' })],
    });

    const git = await within(inspector).findByRole('region', { name: 'Git' });
    expect(within(git).getByText('main')).toBeInTheDocument();
    expect(within(git).getByText('atlas/exec-000042')).toBeInTheDocument();
    expect(within(git).getByText('Isolated · Completed')).toBeInTheDocument();
    expect(within(git).getByText('7 files · 2 commits')).toBeInTheDocument();
    // Atlas does not run tests yet: it says so instead of claiming they passed.
    expect(within(git).getByText('Not run')).toBeInTheDocument();
    expect(within(git).getByText('Waiting for you')).toBeInTheDocument();
    expect(within(git).getByText(/Merge into the base branch/)).toBeInTheDocument();
  });

  it('merges only when the user presses the button, and shows what became of it', async () => {
    const pending = worktreeOf('w1', 'a1', 'exec-42');
    vi.mocked(mergeExecution).mockResolvedValue({
      ...pending,
      status: 'cleaned',
      mergeStatus: 'merged',
      recommendation: null,
    });
    const { user, inspector } = await inspect({ worktrees: [pending] });

    const button = await within(inspector).findByRole('button', { name: 'Merge into main' });
    expect(mergeExecution).not.toHaveBeenCalled();
    await user.click(button);

    expect(mergeExecution).toHaveBeenCalledWith({
      workspaceId: 'w1',
      agentId: 'a1',
      executionId: 'exec-42',
    });
    expect(await within(inspector).findByText('Merged')).toBeInTheDocument();
    expect(
      within(inspector).queryByRole('button', { name: 'Merge into main' }),
    ).not.toBeInTheDocument();
  });

  it('offers no merge for a conflict, a denying policy or a failed execution, and says the work is kept', async () => {
    const { inspector } = await inspect({
      worktrees: [
        worktreeOf('w1', 'a1', 'exec-42', {
          mergeStatus: 'conflict',
          blockReason: 'conflict',
          recommendation: 'resolve_conflicts',
          changes: {
            filesChanged: 1,
            commitsAhead: 1,
            commitsBehind: 1,
            files: ['src/auth/service.ts'],
            conflicts: ['src/auth/service.ts'],
            mergeable: false,
          },
        }),
      ],
    });

    const git = await within(inspector).findByRole('region', { name: 'Git' });
    expect(within(git).getByText('Conflict')).toBeInTheDocument();
    expect(within(git).getByText('src/auth/service.ts')).toBeInTheDocument();
    expect(within(git).getByText(/Resolve the conflicts manually/)).toBeInTheDocument();
    expect(within(git).getByText(/Nothing was deleted/)).toBeInTheDocument();
    expect(within(git).queryByRole('button')).not.toBeInTheDocument();
  });

  it('says when a merge could not be done because of the policy, with no button', async () => {
    const { inspector } = await inspect({
      worktrees: [
        worktreeOf('w1', 'a1', 'exec-42', {
          mergeStatus: 'blocked',
          blockReason: 'policy_denied',
          recommendation: 'review',
        }),
      ],
    });

    const git = await within(inspector).findByRole('region', { name: 'Git' });
    expect(within(git).getByText(/does not allow Git writes/)).toBeInTheDocument();
    expect(within(git).queryByRole('button')).not.toBeInTheDocument();
  });

  it('words an execution of an agent that is not isolated as such', async () => {
    const user = userEvent.setup();
    show({
      agents: [{ ...architect, worktreeIsolation: false }, developer],
      executions: [storedExecution('w1', 'a1', 'exec-42', { task: 'Add login' })],
    });
    await user.click(await screen.findByRole('tab', { name: 'Executions' }));
    await user.click(screen.getByRole('button', { name: 'Inspect execution #42' }));

    const inspector = screen.getByRole('dialog', { name: 'Execution #42' });
    expect(within(inspector).getByText('Not isolated (project checkout)')).toBeInTheDocument();
  });

  it('shows the Git isolation setting on the agent card', async () => {
    show({ agents: [{ ...architect, worktreeIsolation: false }, developer] });

    const off = await screen.findByRole('article', { name: 'Architect' });
    expect(within(off).getByText('Git isolation: OFF')).toBeInTheDocument();
  });
});

describe('active executions in the header', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('counts runs in every workspace and takes the user to the one they pick', async () => {
    const user = userEvent.setup();
    show();
    await screen.findByRole('article', { name: 'Architect' });
    expect(screen.queryByText(/running/)).not.toBeInTheDocument();

    act(() => {
      emit(executionEvent('w1', 'a1', 'exec-1', 'started'));
      emitSessionStatus(sessionStatus('exec-1', 'running', { agentId: 'a1' }));
      emit(executionEvent('w2', 'a2', 'exec-2', 'started'));
      emitSessionStatus(sessionStatus('exec-2', 'running', { agentId: 'a2' }));
    });
    await user.click(await screen.findByRole('button', { name: '2 agents running' }));
    const list = screen.getByRole('list', { name: 'Active executions' });
    expect(within(list).getAllByRole('button')).toHaveLength(2);

    // The run in the other workspace is opened: its workspace is shown, nothing is stopped.
    await user.click(within(list).getByRole('button', { name: 'Open Developer in Acme' }));
    await waitFor(() => {
      expect(screen.getByRole('article', { name: 'Developer' })).toBeInTheDocument();
    });
    expect(screen.queryByRole('article', { name: 'Architect' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '2 agents running' })).toBeInTheDocument();
  });

  it('disappears when nothing runs any more', async () => {
    show();
    await screen.findByRole('article', { name: 'Architect' });
    act(() => {
      emit(executionEvent('w1', 'a1', 'exec-1', 'started'));
    });
    expect(await screen.findByRole('button', { name: '1 agent running' })).toBeInTheDocument();
    act(() => {
      emit(executionEvent('w1', 'a1', 'exec-1', 'completed'));
    });
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: /running/ })).not.toBeInTheDocument();
    });
  });
});
