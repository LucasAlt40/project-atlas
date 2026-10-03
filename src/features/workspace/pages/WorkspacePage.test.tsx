import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { deleteAgent, updateAgent } from '@/features/agents/services/catalogService';
import {
  agent,
  chatMessage,
  claude,
  emit,
  emitConversationMessage,
  executionEvent,
  mockBackend,
  navigate,
  opencode,
  renderWithProviders,
  workspace,
} from '@/test/fixtures';
import {
  addAgentToWorkspace,
  listMessages,
  removeAgentFromWorkspace,
  sendMessage,
} from '../services/workspaceService';
import { WorkspacePage } from './WorkspacePage';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('../services/workspaceService');

const architect = agent('a1', 'Architect');
const developer = agent('a2', 'Developer', 'opencode', 'opencode/big-pickle');
const qa = agent('a3', 'QA');
const reviewer = agent('a4', 'Reviewer');
const extra = agent('a5', 'Extra');

const W1 = 'w1';
const card = (name: string) => screen.getByRole('article', { name });
const statusOf = (name: string) => within(card(name)).getByRole('status');

function show(placed: string[], agents = [architect, developer], options = {}) {
  mockBackend({
    agents,
    workspaces: [workspace(W1, 'Atlas', '/dev/atlas', placed)],
    selectedWorkspaceId: W1,
    ...options,
  });
  return renderWithProviders(<WorkspacePage />);
}

describe('WorkspacePage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows the workspace’s agents on the grid with identity, runtime, provider and model', async () => {
    show(['a1', 'a2']);

    const architectCard = await screen.findByRole('article', { name: 'Architect' });
    expect(
      within(architectCard).getByText(/Architect · Claude CLI · Anthropic/),
    ).toBeInTheDocument();
    expect(within(architectCard).getByText('sonnet')).toBeInTheDocument();
    expect(statusOf('Architect')).toHaveTextContent('Ready');
    expect(within(card('Developer')).getByText(/OpenCode CLI · OpenCode/)).toBeInTheDocument();
    expect(screen.getAllByText('Available')).toHaveLength(2);
    expect(screen.getByRole('heading', { name: 'Atlas' })).toBeInTheDocument();
  });

  it('shows the project context: folder, path and the technologies detected locally', async () => {
    show(['a1']);

    const context = await screen.findByRole('region', { name: 'Project context' });
    expect(await within(context).findByText('atlas')).toBeInTheDocument();
    expect(within(context).getByText(/Path: \/dev\/atlas/)).toBeInTheDocument();
    const technologies = within(context).getByRole('list', { name: 'Detected' });
    expect(
      within(technologies)
        .getAllByRole('listitem')
        .map((li) => li.textContent),
    ).toEqual(['Angular', 'Git']);
  });

  it('adds an existing agent into the first available slot', async () => {
    const user = userEvent.setup();
    show(['a1']);
    vi.mocked(addAgentToWorkspace).mockResolvedValue(
      workspace(W1, 'Atlas', '/dev/atlas', ['a1', 'a2']),
    );

    await user.click(await screen.findByRole('button', { name: '+ Add Agent' }));
    await user.click(screen.getByRole('button', { name: 'Add Developer' }));

    expect(addAgentToWorkspace).toHaveBeenCalledWith(W1, 'a2');
    expect(await screen.findByRole('article', { name: 'Developer' })).toBeInTheDocument();
    expect(screen.getAllByText('Available')).toHaveLength(2);
  });

  it('sends "Create new agent" to the agent creation flow', async () => {
    const user = userEvent.setup();
    show(['a1']);

    await user.click(await screen.findByRole('button', { name: '+ Add Agent' }));
    await user.click(screen.getByRole('button', { name: 'Create new agent' }));

    expect(navigate).toHaveBeenCalledWith('agents', { type: 'create-agent' });
  });

  it('removing an agent frees its slot but keeps the agent available to add again', async () => {
    const user = userEvent.setup();
    show(['a1', 'a2']);
    vi.mocked(removeAgentFromWorkspace).mockResolvedValue(
      workspace(W1, 'Atlas', '/dev/atlas', ['a2']),
    );

    await user.click(
      await screen.findByRole('button', { name: 'Remove Architect from workspace' }),
    );

    expect(removeAgentFromWorkspace).toHaveBeenCalledWith(W1, 'a1');
    await waitFor(() => {
      expect(screen.queryByRole('article', { name: 'Architect' })).not.toBeInTheDocument();
    });
    expect(screen.getAllByText('Available')).toHaveLength(3);
    await user.click(screen.getByRole('button', { name: '+ Add Agent' }));
    expect(screen.getByRole('button', { name: 'Add Architect' })).toBeInTheDocument();
  });

  it('allows a fourth agent and rejects a fifth in the UI without calling the core', async () => {
    const user = userEvent.setup();
    show(['a1', 'a2', 'a3'], [architect, developer, qa, reviewer, extra]);
    vi.mocked(addAgentToWorkspace).mockResolvedValue(
      workspace(W1, 'Atlas', '/dev/atlas', ['a1', 'a2', 'a3', 'a4']),
    );

    await user.click(await screen.findByRole('button', { name: '+ Add Agent' }));
    await user.click(screen.getByRole('button', { name: 'Add Reviewer' }));
    expect(await screen.findByRole('article', { name: 'Reviewer' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '+ Add Agent' }));

    expect(screen.getByRole('alert')).toHaveTextContent('up to 4 agents');
    expect(screen.queryByRole('region', { name: 'Add agent' })).not.toBeInTheDocument();
    expect(addAgentToWorkspace).toHaveBeenCalledTimes(1);
  });

  it('shows the user message at once, then running, live activity worded from codes, and the answer', async () => {
    const user = userEvent.setup();
    show(['a1', 'a2']);
    vi.mocked(sendMessage).mockResolvedValue({
      userMessage: chatMessage(W1, 'a1', 'exec-1', 'user', 'Analyze auth'),
      executionId: 'exec-1',
    });

    await user.type(await screen.findByLabelText('Message Architect'), 'Analyze auth');
    await user.click(within(card('Architect')).getByRole('button', { name: 'Send' }));

    expect(sendMessage).toHaveBeenCalledWith({
      workspaceId: W1,
      agentId: 'a1',
      content: 'Analyze auth',
    });
    const conversation = await screen.findByRole('list', { name: 'Conversation' });
    expect(within(conversation).getByText('Analyze auth')).toBeInTheDocument();
    expect(statusOf('Architect')).toHaveTextContent('Running');
    expect(within(card('Architect')).getByRole('button', { name: 'Send' })).toBeDisabled();

    act(() => {
      emit(executionEvent(W1, 'a1', 'exec-1', 'started'));
      emit(executionEvent(W1, 'a1', 'exec-1', 'starting_runtime', { runtime: 'Claude CLI' }));
      emit(executionEvent(W1, 'a1', 'exec-1', 'sending_prompt'));
      emit(executionEvent(W1, 'a1', 'exec-1', 'waiting_for_model', { model: 'sonnet' }));
    });
    const activity = within(card('Architect')).getByRole('list', { name: 'Activity' });
    expect(within(activity).getByText(/Starting Claude CLI/)).toHaveTextContent('✓');
    expect(within(activity).getByText(/Waiting for sonnet/)).toHaveTextContent('●');
    expect(statusOf('Architect')).toHaveTextContent('Waiting for model');

    act(() => {
      emit(executionEvent(W1, 'a1', 'exec-1', 'completed'));
      emitConversationMessage(chatMessage(W1, 'a1', 'exec-1', 'assistant', 'Three findings.'));
    });

    expect(within(card('Architect')).getByText('Three findings.')).toBeInTheDocument();
    expect(statusOf('Architect')).toHaveTextContent('Completed');
  });

  it('streams the answer live as Markdown, shows tool steps, then swaps in the final message', async () => {
    const user = userEvent.setup();
    show(['a1', 'a2']);
    vi.mocked(sendMessage).mockResolvedValue({
      userMessage: chatMessage(W1, 'a1', 'exec-1', 'user', 'Look around'),
      executionId: 'exec-1',
    });

    await user.type(await screen.findByLabelText('Message Architect'), 'Look around');
    await user.click(within(card('Architect')).getByRole('button', { name: 'Send' }));
    act(() => {
      emit(executionEvent(W1, 'a1', 'exec-1', 'started'));
      emit(executionEvent(W1, 'a1', 'exec-1', 'waiting_for_model', { model: 'sonnet' }));
    });
    expect(statusOf('Architect')).toHaveTextContent('Waiting for model');

    act(() => {
      emit(executionEvent(W1, 'a1', 'exec-1', 'output_chunk', {}, "I'll **list** the "));
      emit(executionEvent(W1, 'a1', 'exec-1', 'output_chunk', {}, 'files first.'));
    });
    expect(within(card('Architect')).getByText('list').tagName).toBe('STRONG');
    expect(within(card('Architect')).getByText(/files first\./)).toBeInTheDocument();
    expect(statusOf('Architect')).toHaveTextContent('Responding');

    act(() => {
      emit(executionEvent(W1, 'a1', 'exec-1', 'tool_started', { tool: 'Glob' }));
      emit(executionEvent(W1, 'a1', 'exec-1', 'tool_completed', { tool: 'Glob' }));
    });
    const activity = within(card('Architect')).getByRole('list', { name: 'Activity' });
    expect(within(activity).getByText(/Receiving response/)).toBeInTheDocument();
    expect(within(activity).getByText(/Finished Glob/)).toBeInTheDocument();
    expect(statusOf('Architect')).toHaveTextContent('Running');
    expect(within(card('Developer')).queryByText(/files first/)).not.toBeInTheDocument();

    act(() => {
      emit(executionEvent(W1, 'a1', 'exec-1', 'completed'));
      emitConversationMessage(chatMessage(W1, 'a1', 'exec-1', 'assistant', 'Final answer.'));
    });

    expect(within(card('Architect')).getByText('Final answer.')).toBeInTheDocument();
    expect(within(card('Architect')).queryByText(/files first/)).not.toBeInTheDocument();
    expect(statusOf('Architect')).toHaveTextContent('Completed');
  });

  it('renders a failure inside the right card, worded from its failure kind, never as a raw dump', async () => {
    const user = userEvent.setup();
    show(['a1', 'a2']);
    vi.mocked(sendMessage).mockResolvedValue({
      userMessage: chatMessage(W1, 'a1', 'exec-1', 'user', 'Analyze'),
      executionId: 'exec-1',
    });

    await user.type(await screen.findByLabelText('Message Architect'), 'Analyze');
    await user.click(within(card('Architect')).getByRole('button', { name: 'Send' }));
    act(() => {
      emit(executionEvent(W1, 'a1', 'exec-1', 'started'));
      emit(executionEvent(W1, 'a1', 'exec-1', 'sending_prompt'));
      emit(
        executionEvent(W1, 'a1', 'exec-1', 'failed', { failureKind: 'authentication_required' }),
      );
      emitConversationMessage(
        chatMessage(W1, 'a1', 'exec-1', 'assistant', 'Failed to execute request. RAW STDERR', {
          failed: true,
          failureKind: 'authentication_required',
        }),
      );
    });

    const architectCard = card('Architect');
    expect(statusOf('Architect')).toHaveTextContent('Authentication required');
    expect(
      within(architectCard).getByText(
        /Failed to execute request\. The runtime needs you to sign in/,
      ),
    ).toBeInTheDocument();
    expect(within(architectCard).queryByText(/RAW STDERR/)).not.toBeInTheDocument();
    const activity = within(architectCard).getByRole('list', { name: 'Activity' });
    expect(within(activity).getByText(/Sending prompt/)).toHaveTextContent('✓');
    expect(within(activity).getByText(/needs you to sign in/)).toHaveTextContent('✕');
    expect(statusOf('Developer')).toHaveTextContent('Ready');
  });

  it('runs two agents independently, each with its own conversation and state', async () => {
    const user = userEvent.setup();
    show(['a1', 'a2']);
    vi.mocked(sendMessage)
      .mockResolvedValueOnce({
        userMessage: chatMessage(W1, 'a1', 'exec-1', 'user', 'Question A'),
        executionId: 'exec-1',
      })
      .mockResolvedValueOnce({
        userMessage: chatMessage(W1, 'a2', 'exec-2', 'user', 'Question B'),
        executionId: 'exec-2',
      });

    await user.type(await screen.findByLabelText('Message Architect'), 'Question A');
    await user.click(within(card('Architect')).getByRole('button', { name: 'Send' }));
    await user.type(screen.getByLabelText('Message Developer'), 'Question B');
    await user.click(within(card('Developer')).getByRole('button', { name: 'Send' }));

    act(() => {
      emit(executionEvent(W1, 'a1', 'exec-1', 'waiting_for_model', { model: 'sonnet' }));
      emit(executionEvent(W1, 'a2', 'exec-2', 'sending_prompt'));
      emit(executionEvent(W1, 'a2', 'exec-2', 'completed'));
      emitConversationMessage(chatMessage(W1, 'a2', 'exec-2', 'assistant', 'Answer B'));
    });

    expect(statusOf('Architect')).toHaveTextContent('Waiting for model');
    expect(statusOf('Developer')).toHaveTextContent('Completed');
    expect(within(card('Architect')).getByText('Question A')).toBeInTheDocument();
    expect(within(card('Architect')).queryByText('Question B')).not.toBeInTheDocument();
    expect(within(card('Developer')).getByText('Answer B')).toBeInTheDocument();
    expect(within(card('Architect')).queryByText('Answer B')).not.toBeInTheDocument();
  });

  it('shows why a send was rejected inside the agent card, in words', async () => {
    const user = userEvent.setup();
    show(['a1']);
    vi.mocked(sendMessage).mockRejectedValue({ code: 'agent_busy', params: {}, detail: null });

    await user.type(await screen.findByLabelText('Message Architect'), 'again');
    await user.click(screen.getByRole('button', { name: 'Send' }));

    expect(await within(card('Architect')).findByRole('alert')).toHaveTextContent(
      'This agent is still working on a message.',
    );
  });

  it('shows only the conversations of this workspace', async () => {
    show(['a1'], [architect], {
      messages: [
        chatMessage(W1, 'a1', 'exec-0', 'user', 'Question here'),
        chatMessage('other-workspace', 'a1', 'exec-9', 'user', 'Question elsewhere'),
      ],
    });

    expect(await screen.findByText('Question here')).toBeInTheDocument();
    expect(screen.queryByText('Question elsewhere')).not.toBeInTheDocument();
    expect(vi.mocked(listMessages)).toHaveBeenCalledTimes(1);
  });

  it('renders the agent’s answers as Markdown but keeps the user’s text as typed', async () => {
    show(['a1'], [architect], {
      messages: [
        chatMessage(W1, 'a1', 'exec-1', 'user', '**not bold**'),
        chatMessage(
          W1,
          'a1',
          'exec-1',
          'assistant',
          '## Findings\n\n1. **Boundaries** are clear\n2. Use `cargo test`\n\n```rust\nfn main() {}\n```',
        ),
      ],
    });

    const conversation = await screen.findByRole('list', { name: 'Conversation' });
    expect(within(conversation).getByRole('heading', { name: 'Findings' })).toBeInTheDocument();
    expect(within(conversation).getByText('Boundaries').tagName).toBe('STRONG');
    expect(within(conversation).getByText('cargo test').tagName).toBe('CODE');
    expect(within(conversation).getByText('fn main() {}').closest('pre')).not.toBeNull();
    expect(within(conversation).getByText('**not bold**')).toBeInTheDocument();
  });

  it('shows runtime problems as the idle status of the agents that use them', async () => {
    show(['a1', 'a2'], [architect, developer], {
      runtimes: [
        { ...claude, availability: 'not_installed' },
        { ...opencode, availability: 'authentication_required' },
      ],
    });

    await screen.findByRole('article', { name: 'Architect' });
    await waitFor(() => {
      expect(statusOf('Architect')).toHaveTextContent('Unavailable');
    });
    expect(statusOf('Developer')).toHaveTextContent('Authentication required');
  });

  it('edits an agent from its card', async () => {
    const user = userEvent.setup();
    show(['a1', 'a2']);
    vi.mocked(updateAgent).mockResolvedValue({
      ...architect,
      name: 'Lead Architect',
      modelId: 'opus',
    });

    await user.click(await screen.findByRole('button', { name: 'Edit Architect' }));
    const panel = screen.getByRole('region', { name: 'Edit Architect' });
    expect(within(panel).getByLabelText('Model')).toHaveValue('sonnet');
    await user.clear(within(panel).getByLabelText('Name'));
    await user.type(within(panel).getByLabelText('Name'), 'Lead Architect');
    await user.clear(within(panel).getByLabelText('Model'));
    await user.type(within(panel).getByLabelText('Model'), 'opus');
    await user.click(within(panel).getByRole('button', { name: 'Save agent' }));

    expect(updateAgent).toHaveBeenCalledWith('a1', {
      name: 'Lead Architect',
      personalityId: 'architect',
      runtimeId: 'claude',
      modelId: 'opus',
      instructions: '',
    });
    expect(await screen.findByRole('article', { name: 'Lead Architect' })).toBeInTheDocument();
    expect(card('Developer')).toBeInTheDocument();
  });

  it('deletes an agent after confirmation and frees its slot', async () => {
    const user = userEvent.setup();
    show(['a1', 'a2']);
    vi.mocked(deleteAgent).mockResolvedValue();

    await user.click(await screen.findByRole('button', { name: 'Edit Architect' }));
    await user.click(screen.getByRole('button', { name: 'Delete agent' }));
    expect(deleteAgent).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Confirm delete' }));

    expect(deleteAgent).toHaveBeenCalledWith('a1');
    await waitFor(() => {
      expect(screen.queryByRole('article', { name: 'Architect' })).not.toBeInTheDocument();
    });
    expect(card('Developer')).toBeInTheDocument();
  });

  it('shows why deleting an agent failed and keeps it', async () => {
    const user = userEvent.setup();
    show(['a1']);
    vi.mocked(deleteAgent).mockRejectedValue({ code: 'agent_busy', params: {}, detail: null });

    await user.click(await screen.findByRole('button', { name: 'Edit Architect' }));
    await user.click(screen.getByRole('button', { name: 'Delete agent' }));
    await user.click(screen.getByRole('button', { name: 'Confirm delete' }));

    const panel = screen.getByRole('region', { name: 'Edit Architect' });
    expect(await within(panel).findByRole('alert')).toHaveTextContent('still working');
    expect(card('Architect')).toBeInTheDocument();
  });

  it('shows the welcome state when there are no workspaces', async () => {
    mockBackend({ agents: [architect], workspaces: [] });
    renderWithProviders(<WorkspacePage />);

    expect(await screen.findByRole('heading', { name: 'Welcome to Atlas' })).toBeInTheDocument();
    expect(
      screen.getByText('Create your first workspace to start working with agents.'),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create Workspace' })).toBeInTheDocument();
  });

  it('shows an error when the core is unreachable', async () => {
    mockBackend({ workspaces: [] });
    const { listPersonalities } = await import('@/features/agents/services/catalogService');
    vi.mocked(listPersonalities).mockRejectedValue({
      code: 'storage_failed',
      params: {},
      detail: null,
    });
    renderWithProviders(<WorkspacePage />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Could not save your changes.');
  });
});
