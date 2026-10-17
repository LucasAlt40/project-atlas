import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import { getSettings, setPresenceSettings } from '../services/settingsService';
import { PresenceSettings } from './PresenceSettings';

vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

beforeEach(() => {
  vi.clearAllMocks();
  mockBackend({ language: 'en-US' });
  vi.mocked(getSettings).mockResolvedValue({
    language: 'en-US',
    selectedWorkspaceId: null,
    presence: { enabled: false, showNames: true, clientId: null },
  });
  vi.mocked(setPresenceSettings).mockImplementation((presence) =>
    Promise.resolve({ language: 'en-US', selectedWorkspaceId: null, presence }),
  );
});

describe('PresenceSettings', () => {
  it('starts off, and switching it on saves the choice', async () => {
    const person = userEvent.setup();
    renderWithProviders(<PresenceSettings />);

    const enabled = await screen.findByLabelText('Show what Atlas is doing on Discord');
    expect(enabled).not.toBeChecked();

    await person.click(enabled);

    expect(setPresenceSettings).toHaveBeenCalledWith({
      enabled: true,
      showNames: true,
      clientId: null,
    });
    expect(await screen.findByLabelText('Show what Atlas is doing on Discord')).toBeChecked();
  });

  it('hides names on request and saves a trimmed application id', async () => {
    const person = userEvent.setup();
    renderWithProviders(<PresenceSettings />);

    await person.click(
      await screen.findByLabelText('Show the names of the workspace, workflow and step'),
    );
    expect(setPresenceSettings).toHaveBeenLastCalledWith({
      enabled: false,
      showNames: false,
      clientId: null,
    });

    await person.click(screen.getByRole('button', { name: 'Advanced' }));
    await person.type(screen.getByLabelText('Own Application ID (advanced)'), ' 123456 ');
    await person.click(screen.getByRole('button', { name: 'Save ID' }));
    expect(setPresenceSettings).toHaveBeenLastCalledWith({
      enabled: false,
      showNames: false,
      clientId: '123456',
    });
  });
});
