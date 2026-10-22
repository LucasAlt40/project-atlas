import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import type { StorageUsageDto } from '@/lib/tauri/commands';
import { cleanStorage, getStorageUsage } from '../services/settingsService';
import { StorageSettings, formatBytes } from './StorageSettings';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

function usage(overrides: Partial<StorageUsageDto> = {}): StorageUsageDto {
  return {
    folder: '/Users/me/Library/Application Support/atlas',
    totalBytes: 86_000_000,
    parts: [
      { kind: 'worktrees', bytes: 42_000_000 },
      { kind: 'sandbox_temp', bytes: 40_000_000 },
      { kind: 'settings', bytes: 4_000_000 },
    ],
    leftovers: {
      worktrees: { count: 12, bytes: 42_000_000 },
      sandboxTemp: { count: 1, bytes: 40_000_000 },
      branches: 3,
    },
    running: false,
    ...overrides,
  };
}

const report = {
  freedBytes: 82_000_000,
  worktreesRemoved: 12,
  worktreesKept: 0,
  sandboxTempRemoved: 1,
  branchesRemoved: 0,
  branchesKept: 0,
};

beforeEach(() => {
  vi.clearAllMocks();
  mockBackend({ language: 'en-US' });
});

describe('formatBytes', () => {
  it('uses the largest unit that fits, in the interface language', () => {
    expect(formatBytes(0, 'en-US')).toBe('0 byte');
    expect(formatBytes(42_000_000, 'en-US')).toBe('42 MB');
    expect(formatBytes(1_500_000_000, 'pt-BR')).toBe('1,5 GB');
  });
});

describe('StorageSettings', () => {
  it('shows how much Atlas uses, by part, and what can be cleaned', async () => {
    vi.mocked(getStorageUsage).mockResolvedValue(usage());
    renderWithProviders(<StorageSettings />);

    expect(await screen.findByText('86 MB')).toBeInTheDocument();
    const parts = screen.getByRole('list', { name: 'Disk usage' });
    expect(within(parts).getByText('Worktrees (isolated copies of projects)')).toBeInTheDocument();
    expect(screen.getByText('12 folders · 42 MB')).toBeInTheDocument();
    expect(screen.getByText('1 item · 40 MB')).toBeInTheDocument();
    expect(screen.getByText('3 branches')).toBeInTheDocument();
    // Branches lose commits: off unless the person asks.
    expect(
      screen.getByRole('checkbox', { name: /Branches of removed worktrees/ }),
    ).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: /Worktrees of finished runs/ })).toBeChecked();
    expect(screen.getByText('Frees about 82 MB')).toBeInTheDocument();
  });

  it('asks first, cleans what was chosen and says what happened', async () => {
    const person = userEvent.setup();
    vi.mocked(getStorageUsage).mockResolvedValue(usage());
    vi.mocked(cleanStorage).mockResolvedValue(report);
    renderWithProviders(<StorageSettings />);

    await person.click(await screen.findByRole('button', { name: 'Clean up…' }));
    const dialog = screen.getByRole('dialog', { name: 'Clean up leftovers?' });
    expect(cleanStorage).not.toHaveBeenCalled();
    await person.click(within(dialog).getByRole('button', { name: 'Clean up' }));

    expect(cleanStorage).toHaveBeenCalledWith({
      worktrees: true,
      sandboxTemp: true,
      branches: false,
    });
    expect(await screen.findByText(/82 MB freed\. 12 worktrees removed\./)).toBeInTheDocument();
    expect(getStorageUsage).toHaveBeenCalledTimes(2);
  });

  it('warns before deleting branches', async () => {
    const person = userEvent.setup();
    vi.mocked(getStorageUsage).mockResolvedValue(usage());
    renderWithProviders(<StorageSettings />);

    await person.click(
      await screen.findByRole('checkbox', { name: /Branches of removed worktrees/ }),
    );
    await person.click(screen.getByRole('button', { name: 'Clean up…' }));

    expect(
      within(screen.getByRole('dialog')).getByText(/deletes the commits on them/),
    ).toBeInTheDocument();
  });

  it('waits while an agent is running', async () => {
    vi.mocked(getStorageUsage).mockResolvedValue(usage({ running: true }));
    renderWithProviders(<StorageSettings />);

    expect(await screen.findByText(/An agent is running/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Clean up…' })).toBeDisabled();
  });

  it('has nothing to offer when nothing was left behind', async () => {
    vi.mocked(getStorageUsage).mockResolvedValue(
      usage({
        leftovers: {
          worktrees: { count: 0, bytes: 0 },
          sandboxTemp: { count: 0, bytes: 0 },
          branches: 0,
        },
      }),
    );
    renderWithProviders(<StorageSettings />);

    expect(await screen.findAllByText('Nothing to clean')).toHaveLength(3);
    expect(screen.getByRole('button', { name: 'Clean up…' })).toBeDisabled();
  });

  it('is in Portuguese when the interface is', async () => {
    mockBackend({ language: 'pt-BR' });
    vi.mocked(getStorageUsage).mockResolvedValue(usage());
    renderWithProviders(<StorageSettings />);

    expect(await screen.findByText('Uso do disco')).toBeInTheDocument();
    expect(screen.getByText('12 pastas · 42 MB')).toBeInTheDocument();
  });
});
