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

/** The element at `index`, or a failed test: the lists here are small and known. */
function at(list: HTMLElement[], index: number): HTMLElement {
  const element = list[index];
  if (!element) throw new Error(`no element at ${String(index)}`);
  return element;
}

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

  it('isolates executions in Git worktrees by default, says so, and lets the user opt out', async () => {
    const user = userEvent.setup();
    show({ onAgentCreated: vi.fn() }, []);
    vi.mocked(createAgent).mockResolvedValue(architect);

    await user.click(await screen.findByRole('button', { name: 'Create agent' }));
    const form = screen.getByRole('form', { name: 'Create agent' });
    const isolation = within(form).getByRole('checkbox', {
      name: 'Always work in an isolated Git worktree',
    });
    expect(isolation).toBeChecked();
    expect(
      within(form).getByText(
        'Each execution works in its own worktree and does not modify the project’s main checkout directly.',
      ),
    ).toBeInTheDocument();

    await user.click(isolation);
    expect(
      within(form).getByText('With this off, the agent may work directly in the project folder.'),
    ).toBeInTheDocument();

    await user.type(within(form).getByLabelText('Name'), 'Architecture Expert');
    await user.selectOptions(within(form).getByLabelText('Personality'), 'architect');
    await user.selectOptions(await within(form).findByLabelText('AI Provider'), 'opencode');
    await user.selectOptions(within(form).getByLabelText('Model'), 'opencode/big-pickle');
    await user.click(within(form).getByRole('button', { name: 'Create Agent' }));

    expect(createAgent).toHaveBeenCalledWith(expect.objectContaining({ worktreeIsolation: false }));
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
      // On unless the user turned it off.
      worktreeIsolation: true,
      // What the Architect personality suggests, since the user changed nothing.
      resultContract: { kind: 'review', outcomes: expect.any(Array) as unknown },
      // What the Architect personality suggests for permissions, since the user changed nothing.
      permissionProfileId: 'developer',
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

  describe('result contract', () => {
    async function fillAndOpen(user: ReturnType<typeof userEvent.setup>, personality: string) {
      show({ startCreating: {} }, []);
      vi.mocked(createAgent).mockResolvedValue(architect);
      const form = await screen.findByRole('form', { name: 'Create agent' });
      await user.type(within(form).getByLabelText('Name'), 'Validator');
      await user.selectOptions(within(form).getByLabelText('Personality'), personality);
      await user.selectOptions(await within(form).findByLabelText('AI Provider'), 'opencode');
      await user.selectOptions(within(form).getByLabelText('Model'), 'opencode/big-pickle');
      return form;
    }

    it('starts from what the personality suggests and sends exactly the outcomes chosen', async () => {
      const user = userEvent.setup();
      const form = await fillAndOpen(user, 'qa');

      expect(within(form).getByLabelText('Result contract')).toHaveValue('validation');
      const allowed = within(form).getByRole('group', { name: 'Allowed outcomes' });
      expect(within(allowed).getByRole('checkbox', { name: /pass/ })).toBeChecked();
      expect(within(allowed).getByRole('checkbox', { name: /fail/ })).toBeChecked();
      await user.click(within(form).getByRole('button', { name: 'Create Agent' }));

      const sent = vi.mocked(createAgent).mock.calls[0]?.[0];
      expect(sent?.resultContract?.kind).toBe('validation');
      expect(sent?.resultContract?.outcomes.map((o) => o.id)).toEqual(['pass', 'fail']);
    });

    it('suggests the permission profile from the personality and lets the user override it', async () => {
      const user = userEvent.setup();
      const form = await fillAndOpen(user, 'qa');

      // The QA personality suggests read only.
      expect(within(form).getByLabelText('Permissions')).toHaveValue('read_only');
      expect(within(form).getByText(/Suggested by the QA personality/)).toBeInTheDocument();

      await user.selectOptions(within(form).getByLabelText('Permissions'), 'developer');
      expect(within(form).queryByText(/Suggested by the QA personality/)).not.toBeInTheDocument();
      await user.click(within(form).getByRole('button', { name: 'Create Agent' }));

      expect(vi.mocked(createAgent).mock.calls[0]?.[0].permissionProfileId).toBe('developer');
    });

    it('sends the suggested profile when the user does not touch it', async () => {
      const user = userEvent.setup();
      const form = await fillAndOpen(user, 'qa');

      await user.click(within(form).getByRole('button', { name: 'Create Agent' }));

      expect(vi.mocked(createAgent).mock.calls[0]?.[0].permissionProfileId).toBe('read_only');
    });

    it('lets a preset be narrowed, but never emptied', async () => {
      const user = userEvent.setup();
      const form = await fillAndOpen(user, 'qa');
      const allowed = within(form).getByRole('group', { name: 'Allowed outcomes' });

      await user.click(within(allowed).getByRole('checkbox', { name: /fail/ }));
      await user.click(within(allowed).getByRole('checkbox', { name: /pass/ }));

      expect(within(form).getByRole('alert')).toHaveTextContent('Declare at least one outcome.');
      await user.click(within(form).getByRole('button', { name: 'Create Agent' }));
      expect(createAgent).not.toHaveBeenCalled();
    });

    it('defines custom outcomes with stable ids and refuses duplicates', async () => {
      const user = userEvent.setup();
      const form = await fillAndOpen(user, 'qa');
      await user.selectOptions(within(form).getByLabelText('Result contract'), 'custom');

      const custom = within(form).getByRole('group', { name: 'Custom outcomes' });
      // Turning a preset into a custom contract keeps what it had.
      expect(
        within(custom)
          .getAllByLabelText('ID')
          .map((i) => (i as HTMLInputElement).value),
      ).toEqual(['pass', 'fail']);
      await user.click(within(form).getByRole('button', { name: '+ Add outcome' }));
      const ids = within(custom).getAllByLabelText('ID');
      const labels = within(custom).getAllByLabelText('Label');
      await user.type(at(ids, 2), 'fail');
      await user.type(at(labels, 2), 'Failed again');
      expect(within(form).getByRole('alert')).toHaveTextContent('Two outcomes use the same ID.');
      await user.click(within(form).getByRole('button', { name: 'Create Agent' }));
      expect(createAgent).not.toHaveBeenCalled();

      await user.clear(at(ids, 2));
      await user.type(at(ids, 2), 'blocked');
      await user.click(within(form).getByRole('button', { name: 'Create Agent' }));
      const sent = vi.mocked(createAgent).mock.calls[0]?.[0];
      expect(sent?.resultContract).toMatchObject({ kind: 'custom' });
      expect(sent?.resultContract?.outcomes.map((o) => o.id)).toEqual(['pass', 'fail', 'blocked']);
    });

    it('rejects an id that is not stable-looking', async () => {
      const user = userEvent.setup();
      const form = await fillAndOpen(user, 'qa');
      await user.selectOptions(within(form).getByLabelText('Result contract'), 'custom');
      const custom = within(form).getByRole('group', { name: 'Custom outcomes' });
      await user.clear(at(within(custom).getAllByLabelText('ID'), 0));
      await user.type(at(within(custom).getAllByLabelText('ID'), 0), 'Not Valid');

      expect(within(form).getByRole('alert')).toHaveTextContent('Outcome IDs use lowercase');
    });

    it('says an agent is general when it declares nothing, and keeps an existing contract on edit', async () => {
      const user = userEvent.setup();
      const validator = {
        ...agent('agent-9', 'Validator', 'opencode', 'opencode/big-pickle'),
        personalityId: 'qa',
        resultContract: {
          kind: 'validation' as const,
          outcomes: [{ id: 'pass', label: 'Pass', description: '' }],
        },
      };
      show({}, [validator]);
      vi.mocked(updateAgent).mockResolvedValue(validator);

      await user.click(await screen.findByRole('button', { name: 'Edit Validator' }));
      const panel = screen.getByRole('form', { name: 'Edit Validator' });
      expect(within(panel).getByLabelText('Result contract')).toHaveValue('validation');
      // Changing the personality does not overwrite the agent's own contract.
      await user.selectOptions(within(panel).getByLabelText('Personality'), 'architect');
      expect(within(panel).getByLabelText('Result contract')).toHaveValue('validation');
      await user.click(within(panel).getByRole('button', { name: 'Save agent' }));

      expect(vi.mocked(updateAgent).mock.calls[0]?.[1].resultContract?.outcomes).toHaveLength(1);
    });

    it("keeps an existing agent's profile on edit unless the user changes it", async () => {
      const user = userEvent.setup();
      const validator = {
        ...agent('agent-9', 'Validator', 'opencode', 'opencode/big-pickle'),
        personalityId: 'qa',
        permissionProfileId: 'developer',
      };
      show({}, [validator]);
      vi.mocked(updateAgent).mockResolvedValue(validator);

      await user.click(await screen.findByRole('button', { name: 'Edit Validator' }));
      const panel = screen.getByRole('form', { name: 'Edit Validator' });
      // Its own profile, not what the QA personality suggests.
      expect(within(panel).getByLabelText('Permissions')).toHaveValue('developer');
      await user.click(within(panel).getByRole('button', { name: 'Save agent' }));
      expect(vi.mocked(updateAgent).mock.calls[0]?.[1]).not.toHaveProperty('permissionProfileId');

      await user.click(await screen.findByRole('button', { name: 'Edit Validator' }));
      const again = screen.getByRole('form', { name: 'Edit Validator' });
      await user.selectOptions(within(again).getByLabelText('Permissions'), 'read_only');
      await user.click(within(again).getByRole('button', { name: 'Save agent' }));
      expect(vi.mocked(updateAgent).mock.calls[1]?.[1].permissionProfileId).toBe('read_only');
    });

    it('tells a runtime that cannot edit files apart from a permission', async () => {
      const user = userEvent.setup();
      const form = await fillAndOpen(user, 'architect');

      expect(
        within(form).getByText('This runtime does not support file editing.'),
      ).toBeInTheDocument();
    });
  });
});
