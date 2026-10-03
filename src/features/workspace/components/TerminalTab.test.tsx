import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { RuntimeStatus } from '@/features/agents/types';
import {
  agent,
  chatMessage,
  claude,
  emit,
  emitConversationMessage,
  emitSessionStatus,
  emitTerminalOutput,
  executionEvent,
  mockBackend,
  opencode,
  renderWithProviders,
  sessionStatus,
  terminalChunk,
  terminalSnapshot,
  workspace,
} from '@/test/fixtures';
import { FakeSearchAddon, FakeTerminal, lastTerminal } from '@/test/xtermMock';
import { WorkspacePage } from '../pages/WorkspacePage';
import {
  interruptExecution,
  resizeTerminal,
  sendMessage,
  sendTerminalInput,
  terminateExecution,
} from '../services/workspaceService';

vi.mock('@xterm/xterm', async () => (await import('@/test/xtermMock')).xtermModule);
vi.mock('@xterm/addon-fit', async () => (await import('@/test/xtermMock')).fitModule);
vi.mock('@xterm/addon-search', async () => (await import('@/test/xtermMock')).searchModule);
vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('../services/workspaceService');

const architect = agent('a1', 'Architect');
const developer = agent('a2', 'Developer');
const W1 = 'w1';
const card = (name: string) => screen.getByRole('article', { name });
const statusOf = (name: string) => within(card(name)).getAllByRole('status')[0];
const ref = (agentId: string, executionId: string) => ({ workspaceId: W1, agentId, executionId });

function withCapabilities(
  runtime: RuntimeStatus,
  overrides: Partial<RuntimeStatus['runtime']['capabilities']>,
): RuntimeStatus {
  return {
    ...runtime,
    runtime: {
      ...runtime.runtime,
      capabilities: { ...runtime.runtime.capabilities, ...overrides },
    },
  };
}

function show(options: Parameters<typeof mockBackend>[0] = {}, agents = [architect, developer]) {
  mockBackend({
    agents,
    workspaces: [
      workspace(
        W1,
        'Atlas',
        '/dev/atlas',
        agents.map((a) => a.id),
      ),
    ],
    selectedWorkspaceId: W1,
    terminal: terminalSnapshot('exec-1'),
    ...options,
  });
  return renderWithProviders(<WorkspacePage />);
}

type User = ReturnType<typeof userEvent.setup>;

/** Sends a message as `agentId`, and lets its execution and its process start. */
async function start(user: User, agentName: string, agentId: string, executionId: string) {
  vi.mocked(sendMessage).mockResolvedValueOnce({
    userMessage: chatMessage(W1, agentId, executionId, 'user', `Task of ${agentName}`),
    executionId,
  });
  await user.type(await screen.findByLabelText(`Message ${agentName}`), `Task of ${agentName}`);
  await user.click(within(card(agentName)).getByRole('button', { name: 'Send' }));
  act(() => {
    emit(executionEvent(W1, agentId, executionId, 'started'));
    emitSessionStatus(sessionStatus(executionId, 'running', { agentId }));
  });
}

async function openTerminal(user: User, agentName: string) {
  await user.click(within(card(agentName)).getByRole('tab', { name: 'Terminal' }));
}

/** Opens the terminal of a running agent and waits for xterm to be drawn. */
async function openedTerminal(user: User) {
  show();
  await start(user, 'Architect', 'a1', 'exec-1');
  await openTerminal(user, 'Architect');
  await waitFor(() => {
    expect(FakeTerminal.instances).toHaveLength(1);
  });
  return lastTerminal();
}

describe('the terminal of an agent', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    FakeTerminal.instances = [];
  });

  it('says there is no active process until a task runs', async () => {
    const user = userEvent.setup();
    show();

    await screen.findByRole('article', { name: 'Architect' });
    await openTerminal(user, 'Architect');

    expect(
      within(card('Architect')).getByText(
        'No active process. Run a task to open a terminal session.',
      ),
    ).toBeInTheDocument();
    expect(FakeTerminal.instances).toHaveLength(0);
  });

  it('offers the terminal and the interrupt while the process runs, from any tab', async () => {
    const user = userEvent.setup();
    show();

    await start(user, 'Architect', 'a1', 'exec-1');

    const architectCard = card('Architect');
    expect(within(architectCard).getByRole('button', { name: /Interrupt/ })).toBeEnabled();
    expect(within(architectCard).getByRole('button', { name: 'Open terminal' })).toBeVisible();
    // The other agent has no process, so it has no controls.
    expect(within(card('Developer')).queryByRole('button', { name: /Interrupt/ })).toBeNull();
    // Terminate is not offered next to the interrupt: it lives in the terminal.
    expect(within(architectCard).queryByRole('button', { name: 'Terminate' })).toBeNull();
  });

  it('shows the process output as it arrives, kept apart from the chat', async () => {
    const user = userEvent.setup();
    show();
    await start(user, 'Architect', 'a1', 'exec-1');

    act(() => {
      emitTerminalOutput(terminalChunk('exec-1', 0, 'Reading package.json\r\n'));
    });
    await user.click(within(card('Architect')).getByRole('button', { name: 'Open terminal' }));
    await waitFor(() => {
      expect(lastTerminal().text).toContain('Reading package.json');
    });
    act(() => {
      emitTerminalOutput(terminalChunk('exec-1', 1, 'Running tests...\r\n'));
    });

    expect(lastTerminal().text).toBe('Reading package.json\r\nRunning tests...\r\n');
    const architectCard = card('Architect');
    expect(within(architectCard).getByText(/Connected to execution exec-1/)).toBeInTheDocument();
    expect(within(architectCard).getByText(/\$ claude -p/)).toBeInTheDocument();
    // None of it is in the conversation.
    await user.click(within(architectCard).getByRole('tab', { name: 'Chat' }));
    expect(within(architectCard).queryByText(/Reading package.json/)).toBeNull();
  });

  it('redraws the output kept by the core when the terminal is opened late', async () => {
    const user = userEvent.setup();
    show({ terminal: terminalSnapshot('exec-1', { output: 'before\r\n', nextSeq: 2 }) });
    await start(user, 'Architect', 'a1', 'exec-1');
    act(() => {
      emitTerminalOutput(terminalChunk('exec-1', 2, 'after\r\n'));
    });

    await openTerminal(user, 'Architect');

    await waitFor(() => {
      expect(lastTerminal().text).toBe('before\r\nafter\r\n');
    });
  });

  it('interrupts the right execution, then shows what the user did and what the process did', async () => {
    const user = userEvent.setup();
    show();
    await start(user, 'Architect', 'a1', 'exec-1');
    await openTerminal(user, 'Architect');
    await screen.findByText(/Connected to execution exec-1/);

    await user.click(within(card('Architect')).getByRole('button', { name: /Interrupt/ }));

    expect(interruptExecution).toHaveBeenCalledWith(ref('a1', 'exec-1'));
    act(() => {
      emitSessionStatus(sessionStatus('exec-1', 'interrupting', { userAction: 'interrupted' }));
      emit(executionEvent(W1, 'a1', 'exec-1', 'user_interrupted', { signal: 'interrupt' }));
    });
    expect(statusOf('Architect')).toHaveTextContent('Stopping…');
    expect(within(card('Architect')).getByText('User-controlled')).toBeInTheDocument();
    expect(
      within(card('Architect')).getByText(/Interrupting… the process may take a moment/),
    ).toBeInTheDocument();
    // Terminate is the second step: available, but not the main action.
    expect(within(card('Architect')).getByRole('button', { name: 'Terminate' })).toBeEnabled();

    act(() => {
      emitSessionStatus(
        sessionStatus('exec-1', 'exited', { userAction: 'interrupted', exitCode: 130 }),
      );
      emit(
        executionEvent(W1, 'a1', 'exec-1', 'process_exited', {
          exitCode: '130',
          durationMs: '42000',
        }),
      );
      emit(executionEvent(W1, 'a1', 'exec-1', 'cancelled', { by: 'interrupted' }));
    });

    expect(statusOf('Architect')).toHaveTextContent('Cancelled');
    const architectCard = card('Architect');
    expect(within(architectCard).getByText(/Process exited · Exit code: 130/)).toBeInTheDocument();
    expect(within(architectCard).getByText('Execution stopped by you.')).toBeInTheDocument();
    expect(within(architectCard).queryByRole('button', { name: /Interrupt/ })).toBeNull();
    expect(within(architectCard).queryByRole('button', { name: 'Terminate' })).toBeNull();
    // The activity separates the user's action from the process's.
    await user.click(within(architectCard).getByRole('tab', { name: 'Activity' }));
    const activity = within(architectCard).getByRole('list', { name: 'Activity' });
    expect(
      within(activity)
        .getByText(/Interrupted the execution/)
        .closest('li'),
    ).toHaveAttribute('data-actor', 'user');
    expect(
      within(activity)
        .getByText(/Process exited \(code 130/)
        .closest('li'),
    ).toHaveAttribute('data-actor', 'agent');
    expect(within(activity).getByText(/Execution cancelled/)).toBeInTheDocument();
  });

  it('words a cancelled answer as the user’s own stop, not as a failure', async () => {
    const user = userEvent.setup();
    show();
    await start(user, 'Architect', 'a1', 'exec-1');

    act(() => {
      emit(executionEvent(W1, 'a1', 'exec-1', 'cancelled'));
      emitSessionStatus(sessionStatus('exec-1', 'exited'));
      emitConversationMessage(
        chatMessage(W1, 'a1', 'exec-1', 'assistant', 'Execution cancelled.', {
          failed: true,
          failureKind: 'cancelled',
        }),
      );
    });

    const architectCard = card('Architect');
    expect(within(architectCard).getByText('Execution cancelled by you.')).toBeInTheDocument();
    expect(within(architectCard).queryByText(/Failed to execute request/)).toBeNull();
    // The agent can be asked again.
    expect(within(architectCard).getByLabelText('Message Architect')).toBeEnabled();
  });

  it('terminates only when asked, as the second step', async () => {
    const user = userEvent.setup();
    show();
    await start(user, 'Architect', 'a1', 'exec-1');
    await openTerminal(user, 'Architect');
    await screen.findByText(/Connected to execution exec-1/);

    await user.click(within(card('Architect')).getByRole('button', { name: 'Terminate' }));

    expect(terminateExecution).toHaveBeenCalledWith(ref('a1', 'exec-1'));
    expect(interruptExecution).not.toHaveBeenCalled();
    act(() => {
      emitSessionStatus(sessionStatus('exec-1', 'terminating', { userAction: 'terminated' }));
    });
    expect(within(card('Architect')).getByText('Terminating the process…')).toBeInTheDocument();
    expect(within(card('Architect')).getByRole('button', { name: 'Terminate' })).toBeDisabled();
  });

  it('presses Ctrl+C in the terminal as an interrupt, and as a copy when text is selected', async () => {
    const user = userEvent.setup();
    const terminal = await openedTerminal(user);
    const ctrlC = () => new KeyboardEvent('keydown', { key: 'c', ctrlKey: true });
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });

    expect(terminal.keyHandler?.(ctrlC())).toBe(false);
    expect(interruptExecution).toHaveBeenCalledWith(ref('a1', 'exec-1'));

    vi.mocked(interruptExecution).mockClear();
    terminal.selection = 'some text';
    expect(terminal.keyHandler?.(ctrlC())).toBe(false);
    expect(interruptExecution).not.toHaveBeenCalled();
    expect(writeText).toHaveBeenCalledWith('some text');
  });

  it('never traps the keyboard in a read-only terminal', async () => {
    const terminal = await openedTerminal(userEvent.setup());

    // Tab is not swallowed by the terminal: focus can leave it.
    expect(terminal.keyHandler?.(new KeyboardEvent('keydown', { key: 'Tab' }))).toBe(false);
    // Other keys are left to xterm.
    expect(terminal.keyHandler?.(new KeyboardEvent('keydown', { key: 'a' }))).toBe(true);
  });

  it('is read-only for these runtimes: typing reaches nothing', async () => {
    const terminal = await openedTerminal(userEvent.setup());

    terminal.dataHandler?.('rm -rf /\r');

    expect(terminal.options.disableStdin).toBe(true);
    expect(sendTerminalInput).not.toHaveBeenCalled();
    expect(within(card('Architect')).getByText('Read-only')).toBeInTheDocument();
  });

  it('passes typing to the process, untouched, when the runtime reads input', async () => {
    const user = userEvent.setup();
    show({
      runtimes: [withCapabilities(claude, { terminalInput: true }), opencode],
      terminal: terminalSnapshot('exec-1', { inputEnabled: true }),
    });
    await start(user, 'Architect', 'a1', 'exec-1');
    await openTerminal(user, 'Architect');
    await waitFor(() => {
      expect(FakeTerminal.instances).toHaveLength(1);
    });

    lastTerminal().dataHandler?.('\u001b[A ls\r');

    expect(lastTerminal().options.disableStdin).toBe(false);
    expect(sendTerminalInput).toHaveBeenCalledWith(ref('a1', 'exec-1'), '\u001b[A ls\r');
    expect(within(card('Architect')).queryByText('Read-only')).toBeNull();
  });

  it('tells the process about a new size', async () => {
    const terminal = await openedTerminal(userEvent.setup());

    terminal.resizeHandler?.({ cols: 100, rows: 31 });

    await waitFor(() => {
      expect(resizeTerminal).toHaveBeenCalledWith(ref('a1', 'exec-1'), 100, 31);
    });
  });

  it('copies, clears and searches the terminal', async () => {
    const user = userEvent.setup();
    show();
    await start(user, 'Architect', 'a1', 'exec-1');
    act(() => {
      emitTerminalOutput(terminalChunk('exec-1', 0, 'hello world\r\n'));
    });
    await openTerminal(user, 'Architect');
    await waitFor(() => {
      expect(FakeTerminal.instances).toHaveLength(1);
    });
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    const architectCard = card('Architect');

    await user.click(within(architectCard).getByRole('button', { name: 'Copy' }));
    expect(writeText).toHaveBeenCalledWith('hello world\r\n');

    await user.click(within(architectCard).getByRole('button', { name: 'Search' }));
    await user.type(
      within(architectCard).getByRole('searchbox', { name: 'Search the terminal output' }),
      'world{Enter}',
    );
    expect(FakeSearchAddon.last?.findNext).toHaveBeenCalledWith('world');

    await user.click(within(architectCard).getByRole('button', { name: 'Clear' }));
    expect(lastTerminal().cleared).toBe(1);
  });

  it('follows the output unless auto-scroll is off', async () => {
    const user = userEvent.setup();
    const terminal = await openedTerminal(user);

    act(() => {
      emitTerminalOutput(terminalChunk('exec-1', 0, 'a\r\n'));
    });
    expect(terminal.scrollToBottom).toHaveBeenCalled();

    await user.click(within(card('Architect')).getByRole('checkbox', { name: 'Auto-scroll' }));
    terminal.scrollToBottom.mockClear();
    act(() => {
      emitTerminalOutput(terminalChunk('exec-1', 1, 'b\r\n'));
    });
    expect(terminal.scrollToBottom).not.toHaveBeenCalled();
    expect(terminal.scrollToLine).toHaveBeenCalled();
  });

  it('keeps agents apart: interrupting A leaves B running and with its own terminal', async () => {
    const user = userEvent.setup();
    show();
    await start(user, 'Architect', 'a1', 'exec-1');
    await start(user, 'Developer', 'a2', 'exec-2');
    act(() => {
      emitTerminalOutput(terminalChunk('exec-1', 0, 'from A\r\n', { agentId: 'a1' }));
      emitTerminalOutput(terminalChunk('exec-2', 0, 'from B\r\n', { agentId: 'a2' }));
    });

    await user.click(within(card('Architect')).getByRole('button', { name: /Interrupt/ }));
    expect(interruptExecution).toHaveBeenCalledTimes(1);
    expect(interruptExecution).toHaveBeenCalledWith(ref('a1', 'exec-1'));
    act(() => {
      emitSessionStatus(sessionStatus('exec-1', 'exited', { userAction: 'interrupted' }));
      emit(executionEvent(W1, 'a1', 'exec-1', 'cancelled'));
    });

    expect(statusOf('Architect')).toHaveTextContent('Cancelled');
    expect(statusOf('Developer')).toHaveTextContent('Running');
    expect(within(card('Developer')).getByRole('button', { name: /Interrupt/ })).toBeEnabled();
    await openTerminal(user, 'Developer');
    await waitFor(() => {
      expect(lastTerminal().text).toBe('from B\r\n');
    });
  });

  it('says so when a runtime has no terminal, and offers no controls', async () => {
    const user = userEvent.setup();
    show({
      runtimes: [
        withCapabilities(claude, { interactiveTerminal: false, interrupt: false }),
        opencode,
      ],
    });

    await screen.findByRole('article', { name: 'Architect' });
    await openTerminal(user, 'Architect');

    expect(
      within(card('Architect')).getByText('Claude CLI does not provide a terminal.'),
    ).toBeInTheDocument();
    expect(within(card('Architect')).queryByRole('button', { name: /Interrupt/ })).toBeNull();
  });

  it('hides the interrupt of a runtime that cannot be interrupted', async () => {
    const user = userEvent.setup();
    show({ runtimes: [withCapabilities(claude, { interrupt: false }), opencode] });

    await start(user, 'Architect', 'a1', 'exec-1');

    expect(within(card('Architect')).queryByRole('button', { name: /Interrupt/ })).toBeNull();
    expect(within(card('Architect')).getByRole('button', { name: 'Open terminal' })).toBeVisible();
  });

  it('explains a control the core refused, in words', async () => {
    const user = userEvent.setup();
    show();
    await start(user, 'Architect', 'a1', 'exec-1');
    vi.mocked(interruptExecution).mockRejectedValueOnce({
      code: 'execution_not_running',
      params: {},
      detail: null,
    });

    await user.click(within(card('Architect')).getByRole('button', { name: /Interrupt/ }));

    expect(await within(card('Architect')).findByRole('alert')).toHaveTextContent(
      'The process has already ended.',
    );
  });

  it('shows a finished process as history, with its exit code', async () => {
    const user = userEvent.setup();
    show({
      terminal: terminalSnapshot('exec-1', {
        status: 'exited',
        exitCode: 0,
        output: 'done\r\n',
        nextSeq: 1,
      }),
    });
    await start(user, 'Architect', 'a1', 'exec-1');
    act(() => {
      emitSessionStatus(sessionStatus('exec-1', 'exited', { exitCode: 0 }));
      emit(executionEvent(W1, 'a1', 'exec-1', 'completed'));
    });

    await openTerminal(user, 'Architect');

    await waitFor(() => {
      expect(lastTerminal().text).toBe('done\r\n');
    });
    expect(
      within(card('Architect')).getByText(/Process exited · Exit code: 0/),
    ).toBeInTheDocument();
    expect(within(card('Architect')).queryByRole('button', { name: /Interrupt/ })).toBeNull();
  });

  it('closes the xterm view when the tab is left, and nothing keeps listening', async () => {
    const user = userEvent.setup();
    const terminal = await openedTerminal(user);

    await user.click(within(card('Architect')).getByRole('tab', { name: 'Chat' }));

    expect(terminal.disposed).toBe(true);
    act(() => {
      emitTerminalOutput(terminalChunk('exec-1', 0, 'late\r\n'));
    });
    expect(terminal.text).not.toContain('late');
  });
});
