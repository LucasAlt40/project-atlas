import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import type { SandboxStatusDto } from '@/lib/tauri/commands';
import { getSandboxStatus, setSandboxSettings } from '../services/settingsService';
import { SandboxSettings } from './SandboxSettings';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

function status(overrides: Partial<SandboxStatusDto> = {}): SandboxStatusDto {
  return {
    available: true,
    platform: 'macos',
    settings: { mode: 'off', devCaches: true },
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mockBackend({ language: 'en-US' });
});

describe('SandboxSettings', () => {
  it('starts off, says the system can confine, and saves the mode the person picks', async () => {
    const person = userEvent.setup();
    vi.mocked(getSandboxStatus).mockResolvedValue(status());
    vi.mocked(setSandboxSettings).mockImplementation((sandbox) =>
      Promise.resolve({ language: 'en-US', selectedWorkspaceId: null, sandbox }),
    );
    renderWithProviders(<SandboxSettings />);

    const mode = await screen.findByRole('group', { name: 'Sandbox' });
    expect(within(mode).getByRole('button', { name: 'Off' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(screen.getByText('Available on this system')).toBeInTheDocument();

    await person.click(within(mode).getByRole('button', { name: 'Required' }));

    expect(setSandboxSettings).toHaveBeenCalledWith({ mode: 'required', devCaches: true });
    expect(
      within(await screen.findByRole('group', { name: 'Sandbox' })).getByRole('button', {
        name: 'Required',
      }),
    ).toHaveAttribute('aria-pressed', 'true');
  });

  it('lets the person decide about the caches of developer tools and warns what each choice costs', async () => {
    const person = userEvent.setup();
    vi.mocked(getSandboxStatus).mockResolvedValue(
      status({ settings: { mode: 'auto', devCaches: true } }),
    );
    vi.mocked(setSandboxSettings).mockImplementation((sandbox) =>
      Promise.resolve({ language: 'en-US', selectedWorkspaceId: null, sandbox }),
    );
    renderWithProviders(<SandboxSettings />);

    const caches = await screen.findByRole('checkbox', { name: /caches of developer tools/ });
    expect(caches).toBeChecked();
    expect(screen.getByText(/could change a cache other projects use/)).toBeInTheDocument();
    await person.click(caches);

    expect(setSandboxSettings).toHaveBeenCalledWith({ mode: 'auto', devCaches: false });
  });

  it('says plainly when this system cannot confine anything and what the choices then mean', async () => {
    vi.mocked(getSandboxStatus).mockResolvedValue(
      status({ available: false, platform: 'windows' }),
    );
    renderWithProviders(<SandboxSettings />);

    expect(await screen.findByText(/cannot confine processes/)).toHaveTextContent(
      '“Required” refuses to start',
    );
  });

  it('is in Portuguese when the interface is', async () => {
    mockBackend({ language: 'pt-BR' });
    vi.mocked(getSandboxStatus).mockResolvedValue(status());
    renderWithProviders(<SandboxSettings />);

    expect(await screen.findByRole('group', { name: 'Sandbox' })).toBeInTheDocument();
    expect(screen.getByText('Disponível neste sistema')).toBeInTheDocument();
  });
});
