import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { fetchAppInfo } from '@/features/home/services/appInfoService';
import { agent, mockBackend, renderWithProviders } from '@/test/fixtures';
import { exportConfigs } from '@/features/portability/services/portabilityService';
import { listTemplates, listWorkflows } from '@/features/workflow/services/workflowService';
import { setLanguage, setSpecsMaxTokens } from '../services/settingsService';
import { checkForUpdate, installUpdate } from '../services/updateService';
import { SettingsPage } from './SettingsPage';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/memory/services/memoryService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');
vi.mock('@/features/home/services/appInfoService');
vi.mock('../services/updateService');
vi.mock('@/features/portability/services/portabilityService');
vi.mock('@/features/workflow/services/workflowService');

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(fetchAppInfo).mockResolvedValue({
    name: 'Project Atlas',
    version: '0.1.0',
    platform: 'macos',
  });
});

describe('SettingsPage', () => {
  it('saves the token limit of a specification, and none when it is left empty', async () => {
    const user = userEvent.setup();
    mockBackend({ language: 'en-US' });
    vi.mocked(setSpecsMaxTokens).mockImplementation((max) =>
      Promise.resolve({
        language: 'en-US',
        selectedWorkspaceId: null,
        optimization: { metricsEnabled: true, specsMaxTokens: max },
      }),
    );
    renderWithProviders(<SettingsPage />);

    const field = await screen.findByLabelText('Token limit per specification');
    expect(field).toHaveValue('');
    await user.type(field, '300000');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    expect(setSpecsMaxTokens).toHaveBeenCalledWith(300000);

    await user.clear(field);
    await user.click(screen.getByRole('button', { name: 'Save' }));
    expect(setSpecsMaxTokens).toHaveBeenLastCalledWith(null);
  });

  it('is in Portuguese by default and offers both languages', async () => {
    mockBackend({ language: 'pt-BR' });
    renderWithProviders(<SettingsPage />);

    expect(await screen.findByRole('heading', { name: 'Configurações' })).toBeInTheDocument();
    const select = screen.getByLabelText('Idioma');
    expect(select).toHaveValue('pt-BR');
    expect(
      within(select)
        .getAllByRole('option')
        .map((o) => o.textContent),
    ).toEqual(['Português (Brasil)', 'English']);
    expect(await screen.findByText('macOS')).toBeInTheDocument();
  });

  it('switches to English at once and remembers the choice', async () => {
    const user = userEvent.setup();
    mockBackend({ language: 'pt-BR' });
    renderWithProviders(<SettingsPage />);

    await user.selectOptions(await screen.findByLabelText('Idioma'), 'en-US');

    expect(setLanguage).toHaveBeenCalledWith('en-US');
    expect(await screen.findByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByLabelText('Language')).toHaveValue('en-US');
    expect(screen.getByText('Version')).toBeInTheDocument();
  });

  it('opens in the language that was saved', async () => {
    mockBackend({ language: 'en-US' });
    renderWithProviders(<SettingsPage />);

    expect(await screen.findByRole('heading', { name: 'Settings' })).toBeInTheDocument();
  });

  it('says so when there is no newer version', async () => {
    const user = userEvent.setup();
    mockBackend({ language: 'pt-BR' });
    vi.mocked(checkForUpdate).mockResolvedValue(null);
    renderWithProviders(<SettingsPage />);

    await user.click(await screen.findByRole('button', { name: 'Buscar atualizações' }));

    expect(await screen.findByText(/versão mais recente/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Instalar e reiniciar' })).not.toBeInTheDocument();
  });

  it('offers a newer version and installs it only when asked', async () => {
    const user = userEvent.setup();
    mockBackend({ language: 'pt-BR' });
    vi.mocked(checkForUpdate).mockResolvedValue({ version: '0.2.0', notes: null });
    vi.mocked(installUpdate).mockResolvedValue();
    renderWithProviders(<SettingsPage />);

    await user.click(await screen.findByRole('button', { name: 'Buscar atualizações' }));
    expect(await screen.findByText('A versão 0.2.0 está disponível.')).toBeInTheDocument();
    expect(installUpdate).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Instalar e reiniciar' }));

    expect(installUpdate).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(/vai reiniciar/)).toBeInTheDocument();
  });

  it('reports a failed check', async () => {
    const user = userEvent.setup();
    mockBackend({ language: 'pt-BR' });
    vi.mocked(checkForUpdate).mockRejectedValue(new Error('offline'));
    renderWithProviders(<SettingsPage />);

    await user.click(await screen.findByRole('button', { name: 'Buscar atualizações' }));

    expect(await screen.findByRole('alert')).toBeInTheDocument();
  });

  it('has one place to import a file and to export several configurations at once', async () => {
    const user = userEvent.setup();
    mockBackend({ agents: [agent('agent-1', 'Architect', 'opencode', 'opencode/big-pickle')] });
    vi.mocked(listTemplates).mockResolvedValue([]);
    vi.mocked(listWorkflows).mockResolvedValue([]);
    vi.mocked(exportConfigs).mockResolvedValue({
      fileName: 'atlas-configs.atlas.json',
      text: '{}',
      items: [],
      external: [],
      warnings: [],
    });
    renderWithProviders(<SettingsPage />);

    expect(await screen.findByRole('heading', { name: 'Import and export' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Import' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Export several…' }));
    const agents = await screen.findByRole('checkbox', { name: 'Agents (1)' });
    expect(agents).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Saved workflow templates (0)' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Continue (1)' }));

    expect(exportConfigs).toHaveBeenCalledWith([{ kind: 'agent', id: 'agent-1' }], true);
  });
});
