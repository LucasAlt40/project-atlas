import { screen, waitFor, within } from '@testing-library/react';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { fetchAppInfo } from '@/features/home/services/appInfoService';
import { setLanguage } from '@/features/settings/services/settingsService';
import { createAgent } from '@/features/agents/services/catalogService';
import {
  addAgentToWorkspace,
  createWorkspace,
} from '@/features/workspace/services/workspaceService';
import { pickFolder } from '@/lib/tauri/dialog';
import { agent, mockBackend, workspace } from '@/test/fixtures';
import { App } from './App';

vi.mock('@/features/home/services/appInfoService');
vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');
vi.mock('@/lib/tauri/dialog', () => ({ pickFolder: vi.fn() }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(fetchAppInfo).mockResolvedValue({
    name: 'Project Atlas',
    version: '0.1.0',
    platform: 'linux',
  });
});

describe('App', () => {
  it('is in Portuguese by default, with the workspace as the first and main screen', async () => {
    mockBackend({
      language: 'pt-BR',
      workspaces: [workspace('w1', 'Acme ERP', '/dev/acme')],
      selectedWorkspaceId: 'w1',
    });
    render(<App />);

    const nav = await screen.findByRole('navigation', { name: 'Principal' });
    expect(
      within(nav)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['Workspace', 'Workflow', 'Agentes', 'Personalidades', 'Configurações']);
    expect(within(nav).getByRole('button', { name: 'Workspace' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(await screen.findByRole('button', { name: /^Workspace: Acme ERP/ })).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: '+ Adicionar agente' })).toBeInTheDocument();
    expect(screen.getAllByRole('banner')[0]).toHaveTextContent('Atlas');
  });

  it('switches the whole UI to English from the header without a restart, and remembers it', async () => {
    const user = userEvent.setup();
    mockBackend({
      language: 'pt-BR',
      workspaces: [workspace('w1', 'Acme ERP', '/dev/acme')],
      selectedWorkspaceId: 'w1',
    });
    render(<App />);
    await screen.findByRole('button', { name: '+ Adicionar agente' });

    await user.click(screen.getByRole('button', { name: 'EN' }));

    expect(setLanguage).toHaveBeenCalledWith('en-US');
    expect(await screen.findByRole('button', { name: '+ Add Agent' })).toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'Main' })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Project context' })).toBeInTheDocument();
    // Names the user chose are not translated.
    expect(screen.getAllByText('Acme ERP').length).toBeGreaterThan(0);
  });

  it('shows the translated Agents, Personalities and Settings screens', async () => {
    const user = userEvent.setup();
    mockBackend({
      language: 'pt-BR',
      agents: [agent('a1', 'Arquiteta')],
      workspaces: [workspace('w1', 'W', '/dev/w')],
      selectedWorkspaceId: 'w1',
    });
    render(<App />);

    await user.click(await screen.findByRole('button', { name: 'Agentes' }));
    expect(await screen.findByRole('heading', { name: 'Agentes' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Criar agente' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Personalidades' }));
    expect(await screen.findByRole('heading', { name: 'Personalidades' })).toBeInTheDocument();
    // Built-in descriptions are translated; the instructions are the prompt and stay as written.
    expect(
      screen.getByText(
        'Pergunta: o que deve ser construído e como? Decide a arquitetura, as fronteiras e os contratos antes de qualquer implementação.',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('Identifica fronteiras')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Configurações' }));
    expect(await screen.findByRole('combobox', { name: 'Idioma' })).toHaveValue('pt-BR');
  });

  it('guides a first-time user to create a workspace with the native folder picker', async () => {
    const user = userEvent.setup();
    mockBackend({ language: 'en-US', workspaces: [] });
    vi.mocked(pickFolder).mockResolvedValue('/Users/lucas/dev/acme');
    vi.mocked(createWorkspace).mockResolvedValue(workspace('w1', 'acme', '/Users/lucas/dev/acme'));
    render(<App />);

    expect(await screen.findByRole('heading', { name: 'Welcome to Atlas' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Create Workspace' }));
    const dialog = screen.getByRole('dialog', { name: 'Create Workspace' });
    await user.click(within(dialog).getByRole('button', { name: 'Select folder' }));
    await user.click(within(dialog).getByRole('button', { name: 'Create' }));

    expect(createWorkspace).toHaveBeenCalledWith({
      name: 'acme',
      projectPath: '/Users/lucas/dev/acme',
      description: '',
    });
    expect(await screen.findByRole('heading', { name: 'acme' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Welcome to Atlas' })).not.toBeInTheDocument();
  });

  it('an agent created from the workspace is placed in the active workspace and the workspace opens', async () => {
    const user = userEvent.setup();
    const created = agent('agent-1', 'Architecture Expert', 'claude', 'sonnet');
    mockBackend({
      language: 'en-US',
      workspaces: [workspace('w1', 'Atlas', '/dev/atlas')],
      selectedWorkspaceId: 'w1',
    });
    vi.mocked(createAgent).mockResolvedValue(created);
    vi.mocked(addAgentToWorkspace).mockResolvedValue(
      workspace('w1', 'Atlas', '/dev/atlas', ['agent-1']),
    );
    render(<App />);

    // "+ Add Agent" → "Create new agent" opens the creation flow directly.
    await user.click(await screen.findByRole('button', { name: '+ Add Agent' }));
    await user.click(screen.getByRole('button', { name: 'Create new agent' }));
    const form = await screen.findByRole('form', { name: 'Create agent' });
    await user.type(within(form).getByLabelText('Name'), 'Architecture Expert');
    await user.selectOptions(within(form).getByLabelText('Personality'), 'architect');
    await user.selectOptions(await within(form).findByLabelText('AI Provider'), 'anthropic');
    await user.type(within(form).getByLabelText('Model'), 'sonnet');
    await user.click(within(form).getByRole('button', { name: 'Create Agent' }));

    expect(createAgent).toHaveBeenCalledWith(
      expect.objectContaining({ runtimeId: 'claude', modelId: 'sonnet' }),
    );
    expect(addAgentToWorkspace).toHaveBeenCalledWith('w1', 'agent-1');
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Workspace' })).toHaveAttribute(
        'aria-current',
        'page',
      );
    });
  });

  it('opens the creation flow with a personality chosen on the Personalities screen', async () => {
    const user = userEvent.setup();
    mockBackend({
      language: 'en-US',
      workspaces: [workspace('w1', 'Atlas', '/dev/atlas')],
      selectedWorkspaceId: 'w1',
    });
    render(<App />);

    await user.click(await screen.findByRole('button', { name: 'Personalities' }));
    await user.click(
      within(await screen.findByRole('article', { name: 'QA' })).getByRole('button', {
        name: 'Use this personality',
      }),
    );

    const form = await screen.findByRole('form', { name: 'Create agent' });
    expect(within(form).getByLabelText('Personality')).toHaveValue('qa');
  });
});
