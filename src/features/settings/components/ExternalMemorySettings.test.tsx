import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import {
  checkExternalMemory,
  getSettings,
  setExternalMemory,
  syncExternalMemory,
} from '../services/settingsService';
import { ExternalMemorySettings } from './ExternalMemorySettings';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

const OFF = {
  enabled: false,
  serverUrl: 'http://127.0.0.1:49374',
  dataDir: '',
  maxHits: 3,
  maxTokens: 400,
  workspace: 'default',
  share: false,
  precise: true,
};

beforeEach(() => {
  vi.clearAllMocks();
  mockBackend({ language: 'en-US' });
  vi.mocked(getSettings).mockResolvedValue({
    language: 'en-US',
    selectedWorkspaceId: null,
    externalMemory: OFF,
  });
  vi.mocked(setExternalMemory).mockImplementation((externalMemory) =>
    Promise.resolve({ language: 'en-US', selectedWorkspaceId: null, externalMemory }),
  );
});

describe('ExternalMemorySettings', () => {
  it('starts off, and switching it on saves the choice with the address and folder', async () => {
    const person = userEvent.setup();
    renderWithProviders(<ExternalMemorySettings />);

    const enabled = await screen.findByLabelText('Read notes from the memory server');
    expect(enabled).not.toBeChecked();

    await person.click(enabled);

    expect(setExternalMemory).toHaveBeenCalledWith({ ...OFF, enabled: true });
    expect(await screen.findByLabelText('Read notes from the memory server')).toBeChecked();
  });

  it('saves a new folder only after it is edited', async () => {
    const person = userEvent.setup();
    renderWithProviders(<ExternalMemorySettings />);

    const save = await screen.findByRole('button', { name: 'Save address, folder and workspace' });
    expect(save).toBeDisabled();

    await person.type(screen.getByLabelText("Server's data folder"), '/data/ai-memory');
    await person.click(save);

    expect(setExternalMemory).toHaveBeenCalledWith({ ...OFF, dataDir: '/data/ai-memory' });
  });

  it('shows why an address was refused', async () => {
    vi.mocked(setExternalMemory).mockRejectedValue({
      code: 'memory_invalid',
      details: { reason: 'the server must be on this machine' },
    });
    const person = userEvent.setup();
    renderWithProviders(<ExternalMemorySettings />);

    await person.click(await screen.findByLabelText('Read notes from the memory server'));

    expect(await screen.findByRole('alert')).toBeInTheDocument();
  });

  it('shares approved notes only when the server is on, and says what a sync did', async () => {
    vi.mocked(getSettings).mockResolvedValue({
      language: 'en-US',
      selectedWorkspaceId: null,
      externalMemory: { ...OFF, enabled: true, dataDir: '/data/ai-memory' },
    });
    vi.mocked(syncExternalMemory).mockResolvedValue({
      ok: true,
      reason: null,
      detail: null,
      published: 2,
      retired: 1,
      failed: 0,
    });
    const person = userEvent.setup();
    renderWithProviders(<ExternalMemorySettings />);

    const sync = await screen.findByRole('button', { name: 'Publish now' });
    expect(sync).toBeDisabled();

    await person.click(screen.getByLabelText('Share the notes I approve with the server'));
    expect(setExternalMemory).toHaveBeenCalledWith({
      ...OFF,
      enabled: true,
      dataDir: '/data/ai-memory',
      share: true,
    });

    await person.click(await screen.findByRole('button', { name: 'Publish now' }));
    expect(await screen.findByRole('status')).toHaveTextContent(
      'Published: 2 · retired: 1 · failed: 0',
    );
  });

  it('tells the person why the server did not answer', async () => {
    vi.mocked(getSettings).mockResolvedValue({
      language: 'en-US',
      selectedWorkspaceId: null,
      externalMemory: { ...OFF, enabled: true, dataDir: '/data/ai-memory' },
    });
    vi.mocked(checkExternalMemory).mockResolvedValue({
      reachable: false,
      reason: 'not_running',
      detail: null,
    });
    const person = userEvent.setup();
    renderWithProviders(<ExternalMemorySettings />);

    await person.click(await screen.findByRole('button', { name: 'Test connection' }));

    expect(await screen.findByRole('status')).toHaveTextContent(
      'No answer: the server is not running',
    );
  });

  it('a switch saves only itself: a half-typed address is neither saved nor lost', async () => {
    const person = userEvent.setup();
    renderWithProviders(<ExternalMemorySettings />);

    const url = await screen.findByLabelText('Server address');
    await person.clear(url);
    await person.type(url, 'http://127.0.0.1:4');
    await person.click(screen.getByLabelText('Read notes from the memory server'));

    expect(setExternalMemory).toHaveBeenCalledWith({ ...OFF, enabled: true });
    expect(screen.getByLabelText('Server address')).toHaveValue('http://127.0.0.1:4');
    expect(
      screen.getByRole('button', { name: 'Save address, folder and workspace' }),
    ).toBeEnabled();
  });

  it('nothing can be pressed while a save is on its way', async () => {
    let finish: (value: unknown) => void = () => undefined;
    vi.mocked(setExternalMemory).mockImplementation(
      (externalMemory) =>
        new Promise((resolve) => {
          finish = () => {
            resolve({ language: 'en-US', selectedWorkspaceId: null, externalMemory });
          };
        }),
    );
    const person = userEvent.setup();
    renderWithProviders(<ExternalMemorySettings />);

    await person.click(await screen.findByLabelText('Read notes from the memory server'));

    expect(screen.getByLabelText('Read notes from the memory server')).toBeDisabled();
    expect(screen.getByLabelText('Only bring notes whose title matches the task')).toBeDisabled();
    expect(screen.getByLabelText('Server address')).toBeDisabled();
    finish(undefined);
    expect(await screen.findByLabelText('Server address')).toBeEnabled();
  });

  it('an old result goes away when the fields are edited', async () => {
    vi.mocked(getSettings).mockResolvedValue({
      language: 'en-US',
      selectedWorkspaceId: null,
      externalMemory: { ...OFF, enabled: true, dataDir: '/data/ai-memory' },
    });
    vi.mocked(checkExternalMemory).mockResolvedValue({
      reachable: true,
      reason: null,
      detail: null,
    });
    const person = userEvent.setup();
    renderWithProviders(<ExternalMemorySettings />);

    await person.click(await screen.findByRole('button', { name: 'Test connection' }));
    expect(await screen.findByRole('status')).toHaveTextContent('The server answered.');
    await person.type(screen.getByLabelText("Server's workspace"), 'x');

    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });
});
