import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { agent, mockBackend, renderWithProviders } from '@/test/fixtures';
import { createAgent, deleteAgent, listRuntimes, updateAgent } from '../services/catalogService';
import { claude, opencode } from '@/test/fixtures';
import { AgentsPage } from './AgentsPage';

vi.mock('../services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

const architect = agent('agent-1', 'Architecture Expert', 'opencode', 'opencode/big-pickle');

function show(props: Parameters<typeof AgentsPage>[0] = {}, agents = [architect]) {
  mockBackend({ agents });
  return renderWithProviders(<AgentsPage {...props} />);
}

describe('AgentsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('lists every saved agent with its personality, runtime and model', async () => {
    show();

    const list = await screen.findByRole('list', { name: 'Agents' });
    expect(within(list).getByText('Architecture Expert')).toBeInTheDocument();
    expect(
      within(list).getByText(/Architect · OpenCode CLI · opencode\/big-pickle/),
    ).toBeInTheDocument();
  });

  it('creates an agent from a personality, provider and discovered model', async () => {
    const user = userEvent.setup();
    const onAgentCreated = vi.fn();
    show({ onAgentCreated }, []);
    vi.mocked(createAgent).mockResolvedValue(architect);

    await user.click(await screen.findByRole('button', { name: 'Create agent' }));
    const form = screen.getByRole('form', { name: 'Create agent' });
    await user.type(within(form).getByLabelText('Name'), 'Architecture Expert');
    await user.selectOptions(within(form).getByLabelText('Personality'), 'architect');
    await user.selectOptions(await within(form).findByLabelText('AI Provider'), 'opencode');
    expect(within(form).getByLabelText('Connection')).toHaveValue('opencode');
    expect(within(form).getByText(/✓ Ready · 1\.18\.34/)).toBeInTheDocument();
    await user.selectOptions(within(form).getByLabelText('Model'), 'opencode/big-pickle');
    await user.type(within(form).getByLabelText('Additional instructions'), 'Be brief.');
    await user.click(within(form).getByRole('button', { name: 'Create Agent' }));

    expect(createAgent).toHaveBeenCalledWith({
      name: 'Architecture Expert',
      personalityId: 'architect',
      runtimeId: 'opencode',
      modelId: 'opencode/big-pickle',
      instructions: 'Be brief.',
    });
    expect(onAgentCreated).toHaveBeenCalledWith(architect);
    expect(screen.queryByRole('form', { name: 'Create agent' })).not.toBeInTheDocument();
  });

  it('opens with the chosen personality already selected', async () => {
    show({ startCreating: { personalityId: 'qa' } }, []);

    const form = await screen.findByRole('form', { name: 'Create agent' });
    expect(within(form).getByLabelText('Personality')).toHaveValue('qa');
  });

  it('creates a Claude agent by typing a model id, since Claude cannot list models', async () => {
    const user = userEvent.setup();
    show({ startCreating: {} }, []);
    vi.mocked(createAgent).mockResolvedValue(agent('c', 'Claude Architect', 'claude', 'sonnet'));

    const form = await screen.findByRole('form', { name: 'Create agent' });
    await user.type(within(form).getByLabelText('Name'), 'Claude Architect');
    await user.selectOptions(within(form).getByLabelText('Personality'), 'architect');
    await user.selectOptions(await within(form).findByLabelText('AI Provider'), 'anthropic');

    expect(within(form).getByLabelText('Connection')).toHaveValue('claude');
    const model = within(form).getByLabelText('Model');
    expect(model).toHaveRole('textbox');
    expect(
      within(form).getByText(/Claude CLI does not expose model discovery/),
    ).toBeInTheDocument();
    expect(within(form).getByText(/Use an alias such as sonnet or opus/)).toBeInTheDocument();
    await user.type(model, 'sonnet');
    await user.click(within(form).getByRole('button', { name: 'Create Agent' }));

    expect(createAgent).toHaveBeenCalledWith(
      expect.objectContaining({ runtimeId: 'claude', modelId: 'sonnet', name: 'Claude Architect' }),
    );
  });

  it('shows structured runtime states: authentication required, not installed, not supported yet', async () => {
    const user = userEvent.setup();
    show({ startCreating: {} }, []);
    vi.mocked(listRuntimes).mockResolvedValue([
      { ...claude, availability: 'authentication_required', notice: 'sign_in_required' },
      { ...opencode, availability: 'not_installed' },
    ]);
    // Re-detect to load the new statuses.
    await user.click(await screen.findByRole('button', { name: 'Re-detect runtimes' }));

    const form = screen.getByRole('form', { name: 'Create agent' });
    expect(
      await within(form).findByRole('option', { name: 'OpenCode (not installed)' }),
    ).toBeDisabled();
    await user.selectOptions(within(form).getByLabelText('AI Provider'), 'anthropic');
    expect(within(form).getByText(/⚠ Authentication required/)).toHaveTextContent('Not signed in');
  });

  it('edits an agent and shows the saved version', async () => {
    const user = userEvent.setup();
    show();
    vi.mocked(updateAgent).mockResolvedValue({ ...architect, name: 'Renamed' });

    await user.click(await screen.findByRole('button', { name: 'Edit Architecture Expert' }));
    const form = await screen.findByRole('form', { name: 'Edit Architecture Expert' });
    expect(within(form).getByLabelText('Name')).toHaveValue('Architecture Expert');
    expect(within(form).getByLabelText('Personality')).toHaveValue('architect');
    expect(within(form).getByLabelText('Connection')).toHaveValue('opencode');
    expect(within(form).getByLabelText('Model')).toHaveValue('opencode/big-pickle');
    await user.clear(within(form).getByLabelText('Name'));
    await user.type(within(form).getByLabelText('Name'), 'Renamed');
    await user.click(within(form).getByRole('button', { name: 'Save agent' }));

    expect(updateAgent).toHaveBeenCalledWith(
      'agent-1',
      expect.objectContaining({ name: 'Renamed' }),
    );
    expect(await screen.findByText('Renamed')).toBeInTheDocument();
  });

  it('deletes an agent only after confirmation, and explains why a delete failed', async () => {
    const user = userEvent.setup();
    show({}, [architect, agent('agent-2', 'Second')]);
    vi.mocked(deleteAgent)
      .mockResolvedValueOnce()
      .mockRejectedValueOnce({ code: 'agent_busy', params: {}, detail: null });

    await user.click(await screen.findByRole('button', { name: 'Delete Second' }));
    expect(deleteAgent).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Confirm delete' }));
    expect(deleteAgent).toHaveBeenCalledWith('agent-2');
    await waitFor(() => {
      expect(screen.queryByText('Second')).not.toBeInTheDocument();
    });

    await user.click(screen.getByRole('button', { name: 'Delete Architecture Expert' }));
    await user.click(screen.getByRole('button', { name: 'Confirm delete' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('still working');
    expect(screen.getByText('Architecture Expert')).toBeInTheDocument();
  });
});
