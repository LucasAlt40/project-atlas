import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { mockBackend, renderWithProviders } from '@/test/fixtures';
import { getSettings, setOptimizationSwitches } from '@/features/settings/services/settingsService';
import { OptimizationSettings } from './OptimizationSettings';

vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

beforeEach(() => {
  vi.clearAllMocks();
  mockBackend({ language: 'en-US' });
});

describe('OptimizationSettings', () => {
  it('shows every part on and saves only the one the person turns off', async () => {
    const person = userEvent.setup();
    const on = {
      language: 'en-US',
      selectedWorkspaceId: null,
      optimization: {
        metricsEnabled: true,
        contextEnabled: true,
        skillsEnabled: true,
        guardrailsEnabled: true,
      },
    };
    vi.mocked(getSettings).mockResolvedValue(on);
    vi.mocked(setOptimizationSwitches).mockResolvedValue(on);
    renderWithProviders(<OptimizationSettings />);

    const context = await screen.findByRole('checkbox', { name: /Context Engine/ });
    for (const box of screen.getAllByRole('checkbox')) expect(box).toBeChecked();
    await person.click(context);

    expect(setOptimizationSwitches).toHaveBeenCalledWith({
      metricsEnabled: true,
      contextEnabled: false,
      skillsEnabled: true,
      guardrailsEnabled: true,
    });
    await waitFor(() => {
      expect(context).not.toBeChecked();
    });
  });
});
