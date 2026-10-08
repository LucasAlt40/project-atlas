import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { fetchAppInfo } from '@/features/home/services/appInfoService';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import { setLanguage } from '../services/settingsService';
import { checkForUpdate, installUpdate } from '../services/updateService';
import { SettingsPage } from './SettingsPage';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');
vi.mock('@/features/home/services/appInfoService');
vi.mock('../services/updateService');

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(fetchAppInfo).mockResolvedValue({
    name: 'Project Atlas',
    version: '0.1.0',
    platform: 'macos',
  });
});

describe('SettingsPage', () => {
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
    expect(await screen.findByText('macos')).toBeInTheDocument();
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
});
