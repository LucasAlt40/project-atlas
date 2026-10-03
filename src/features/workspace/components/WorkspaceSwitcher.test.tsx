import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { selectWorkspace } from '@/features/settings/services/settingsService';
import {
  agent,
  chatMessage,
  emit,
  emitConversationMessage,
  executionEvent,
  mockBackend,
  renderWithProviders,
  workspace,
} from '@/test/fixtures';
import {
  createWorkspace,
  deleteWorkspace,
  sendMessage,
  updateWorkspace,
} from '../services/workspaceService';
import { WorkspacePage } from '../pages/WorkspacePage';
import { WorkspaceSwitcher } from './WorkspaceSwitcher';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('../services/workspaceService');

const architect = agent('a1', 'Architect');
const developer = agent('a2', 'Developer');
const erp = workspace('w1', 'Lontano ERP', '/dev/lontano', ['a1']);
const atlas = workspace('w2', 'Atlas', '/dev/atlas', ['a2']);

function mount(options: Parameters<typeof mockBackend>[0] = {}, pickFolder = vi.fn()) {
  mockBackend({
    agents: [architect, developer],
    workspaces: [erp, atlas],
    selectedWorkspaceId: 'w1',
    ...options,
  });
  renderWithProviders(
    <>
      <WorkspaceSwitcher pickFolder={pickFolder} />
      <WorkspacePage />
    </>,
  );
  return pickFolder;
}

const switcher = () => screen.getByRole('button', { name: /^Workspace:/ });
const card = (name: string) => screen.getByRole('article', { name });

describe('WorkspaceSwitcher', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('always shows the current workspace and lists every workspace with its folder', async () => {
    const user = userEvent.setup();
    mount();

    await waitFor(() => {
      expect(switcher()).toHaveTextContent('Lontano ERP');
    });
    await user.click(switcher());

    const menu = screen.getByRole('listbox', { name: 'Workspace' });
    const options = within(menu).getAllByRole('option');
    expect(options.map((o) => o.textContent)).toEqual([
      'Lontano ERP/dev/lontano',
      'Atlas/dev/atlas',
    ]);
    expect(options[0]).toHaveAttribute('aria-selected', 'true');
    expect(within(menu).getByRole('button', { name: '+ New Workspace' })).toBeInTheDocument();
    expect(within(menu).getByRole('button', { name: 'Manage Workspaces' })).toBeInTheDocument();
  });

  it('opens the workspace that was open last', async () => {
    mount({ selectedWorkspaceId: 'w2' });

    expect(await screen.findByRole('article', { name: 'Developer' })).toBeInTheDocument();
    expect(switcher()).toHaveTextContent('Atlas');
    expect(screen.queryByRole('article', { name: 'Architect' })).not.toBeInTheDocument();
  });

  it('switching shows that workspace’s own agents, project and conversations, and remembers the choice', async () => {
    const user = userEvent.setup();
    mount({
      messages: [
        chatMessage('w1', 'a1', 'e1', 'user', 'ERP question'),
        chatMessage('w2', 'a2', 'e2', 'user', 'Atlas question'),
      ],
    });
    expect(await screen.findByText('ERP question')).toBeInTheDocument();

    await user.click(switcher());
    await user.click(screen.getByRole('option', { name: /Atlas/ }));

    expect(selectWorkspace).toHaveBeenCalledWith('w2');
    expect(await screen.findByRole('article', { name: 'Developer' })).toBeInTheDocument();
    expect(screen.queryByRole('article', { name: 'Architect' })).not.toBeInTheDocument();
    expect(screen.getByText('Atlas question')).toBeInTheDocument();
    expect(screen.queryByText('ERP question')).not.toBeInTheDocument();
    expect(await screen.findByText(/Path: \/dev\/atlas/)).toBeInTheDocument();
    expect(switcher()).toHaveTextContent('Atlas');
  });

  it('creates a workspace with the folder picker and opens it', async () => {
    const user = userEvent.setup();
    const created = workspace('w3', 'lontano-new', '/Users/lucas/dev/lontano-new');
    const pick = mount({}, vi.fn().mockResolvedValue('/Users/lucas/dev/lontano-new'));
    vi.mocked(createWorkspace).mockResolvedValue(created);

    await user.click(await screen.findByRole('button', { name: /^Workspace:/ }));
    await user.click(screen.getByRole('button', { name: '+ New Workspace' }));
    const dialog = screen.getByRole('dialog', { name: 'Create Workspace' });
    expect(within(dialog).getByLabelText('Project folder')).toHaveAttribute('readonly');
    await user.click(within(dialog).getByRole('button', { name: 'Select folder' }));
    await user.type(within(dialog).getByLabelText('Description'), 'ERP system');
    await user.click(within(dialog).getByRole('button', { name: 'Create' }));

    expect(pick).toHaveBeenCalled();
    // The folder's name is suggested as the workspace name.
    expect(createWorkspace).toHaveBeenCalledWith({
      name: 'lontano-new',
      projectPath: '/Users/lucas/dev/lontano-new',
      description: 'ERP system',
    });
    await waitFor(() => {
      expect(selectWorkspace).toHaveBeenCalledWith('w3');
    });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(switcher()).toHaveTextContent('lontano-new');
  });

  it('explains an invalid project folder in words', async () => {
    const user = userEvent.setup();
    mount({}, vi.fn().mockResolvedValue('/nowhere'));
    vi.mocked(createWorkspace).mockRejectedValue({
      code: 'project_folder_not_found',
      params: { path: '/nowhere' },
      detail: null,
    });

    await user.click(await screen.findByRole('button', { name: /^Workspace:/ }));
    await user.click(screen.getByRole('button', { name: '+ New Workspace' }));
    const dialog = screen.getByRole('dialog', { name: 'Create Workspace' });
    await user.click(within(dialog).getByRole('button', { name: 'Select folder' }));
    await user.click(within(dialog).getByRole('button', { name: 'Create' }));

    expect(await within(dialog).findByRole('alert')).toHaveTextContent(
      'The folder “/nowhere” does not exist.',
    );
    expect(screen.getByRole('dialog', { name: 'Create Workspace' })).toBeInTheDocument();
  });

  it('keeps the form open and unchanged when the folder picker is cancelled', async () => {
    const user = userEvent.setup();
    mount({}, vi.fn().mockResolvedValue(null));

    await user.click(await screen.findByRole('button', { name: /^Workspace:/ }));
    await user.click(screen.getByRole('button', { name: '+ New Workspace' }));
    const dialog = screen.getByRole('dialog', { name: 'Create Workspace' });
    await user.click(within(dialog).getByRole('button', { name: 'Select folder' }));

    expect(within(dialog).getByLabelText('Project folder')).toHaveValue('');
    expect(within(dialog).getByLabelText('Name')).toHaveValue('');
  });

  it('renames a workspace and changes its folder', async () => {
    const user = userEvent.setup();
    mount({}, vi.fn().mockResolvedValue('/dev/new-home'));
    vi.mocked(updateWorkspace).mockResolvedValue({
      ...erp,
      name: 'ERP renamed',
      projectPath: '/dev/new-home',
    });

    await user.click(await screen.findByRole('button', { name: /^Workspace:/ }));
    await user.click(screen.getByRole('button', { name: 'Manage Workspaces' }));
    await user.click(screen.getByRole('button', { name: 'Edit Lontano ERP' }));
    const dialog = screen.getByRole('dialog', { name: 'Edit Workspace' });
    await user.clear(within(dialog).getByLabelText('Name'));
    await user.type(within(dialog).getByLabelText('Name'), 'ERP renamed');
    await user.click(within(dialog).getByRole('button', { name: 'Select folder' }));
    await user.click(within(dialog).getByRole('button', { name: 'Save' }));

    expect(updateWorkspace).toHaveBeenCalledWith('w1', {
      name: 'ERP renamed',
      projectPath: '/dev/new-home',
      description: '',
    });
    await waitFor(() => {
      expect(switcher()).toHaveTextContent('ERP renamed');
    });
  });

  it('deletes a workspace after confirmation and falls back to another', async () => {
    const user = userEvent.setup();
    mount();
    vi.mocked(deleteWorkspace).mockResolvedValue([atlas]);

    await user.click(await screen.findByRole('button', { name: /^Workspace:/ }));
    await user.click(screen.getByRole('button', { name: 'Manage Workspaces' }));
    await user.click(screen.getByRole('button', { name: 'Delete Lontano ERP' }));
    expect(deleteWorkspace).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Confirm delete' }));

    expect(deleteWorkspace).toHaveBeenCalledWith('w1');
    await waitFor(() => {
      expect(switcher()).toHaveTextContent('Atlas');
    });
    expect(await screen.findByRole('article', { name: 'Developer' })).toBeInTheDocument();
  });

  it('says why a workspace with a working agent cannot be deleted', async () => {
    const user = userEvent.setup();
    mount();
    vi.mocked(deleteWorkspace).mockRejectedValue({
      code: 'workspace_busy',
      params: {},
      detail: null,
    });

    await user.click(await screen.findByRole('button', { name: /^Workspace:/ }));
    await user.click(screen.getByRole('button', { name: 'Manage Workspaces' }));
    await user.click(screen.getByRole('button', { name: 'Delete Atlas' }));
    await user.click(screen.getByRole('button', { name: 'Confirm delete' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'An agent is still working in this workspace.',
    );
  });

  it('an agent keeps running while another workspace is shown, and its state is right when you return', async () => {
    const user = userEvent.setup();
    // The same agent is placed in both workspaces: conversations and runs must not mix.
    mount({
      workspaces: [
        workspace('w1', 'Lontano ERP', '/dev/lontano', ['a1']),
        workspace('w2', 'Atlas', '/dev/atlas', ['a1']),
      ],
    });
    vi.mocked(sendMessage).mockResolvedValue({
      userMessage: chatMessage('w1', 'a1', 'exec-1', 'user', 'Long task'),
      executionId: 'exec-1',
    });

    await user.type(await screen.findByLabelText('Message Architect'), 'Long task');
    await user.click(within(card('Architect')).getByRole('button', { name: 'Send' }));
    act(() => {
      emit(executionEvent('w1', 'a1', 'exec-1', 'started'));
      emit(executionEvent('w1', 'a1', 'exec-1', 'waiting_for_model', { model: 'sonnet' }));
    });
    expect(within(card('Architect')).getByRole('status')).toHaveTextContent('Waiting for model');

    // Switch away: the other workspace has the same agent, idle, with no conversation.
    await user.click(switcher());
    await user.click(screen.getByRole('option', { name: /Atlas/ }));
    await waitFor(() => {
      expect(switcher()).toHaveTextContent('Atlas');
    });
    expect(within(card('Architect')).getByRole('status')).toHaveTextContent('Ready');
    expect(screen.queryByText('Long task')).not.toBeInTheDocument();

    // The run goes on in the background and finishes while Atlas is on screen.
    act(() => {
      emit(executionEvent('w1', 'a1', 'exec-1', 'completed'));
      emitConversationMessage(
        chatMessage('w1', 'a1', 'exec-1', 'assistant', 'Done in the ERP workspace'),
      );
    });
    expect(within(card('Architect')).getByRole('status')).toHaveTextContent('Ready');
    expect(screen.queryByText('Done in the ERP workspace')).not.toBeInTheDocument();

    // Back in the first workspace the result is there.
    await user.click(switcher());
    await user.click(screen.getByRole('option', { name: /Lontano ERP/ }));
    expect(await screen.findByText('Done in the ERP workspace')).toBeInTheDocument();
    expect(within(card('Architect')).getByRole('status')).toHaveTextContent('Completed');
    expect(screen.getByText('Long task')).toBeInTheDocument();
  });
});
